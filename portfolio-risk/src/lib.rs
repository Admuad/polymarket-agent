//! Portfolio & Risk Management System (Layer 3)
//!
//! This crate provides comprehensive portfolio tracking, risk monitoring,
//! and position management for Polymarket trading operations.

mod config;
mod portfolio;
mod risk;
mod metrics;

pub use config::{RiskConfig, RiskLimits, CircuitBreakerConfig};
pub use portfolio::{Portfolio, Position, PositionState, Exposure};
pub use risk::{RiskChecker, RiskViolation, CircuitBreaker, KellyCriterion, KellyCriterion as Kelly, RiskLevel};
pub use metrics::{RiskMetrics, VaRResult};

use common::{MarketEvent, Uuid};
use tracing::{info, error};

/// Main entry point for portfolio and risk management
#[derive(Debug, Clone)]
pub struct PortfolioRiskManager {
    portfolio: Portfolio,
    risk_checker: RiskChecker,
    config: RiskConfig,
}

impl PortfolioRiskManager {
    /// Create a new portfolio risk manager with default configuration
    pub fn new() -> anyhow::Result<Self> {
        Self::with_config(RiskConfig::default())
    }

    /// Create a new portfolio risk manager with custom configuration
    pub fn with_config(config: RiskConfig) -> anyhow::Result<Self> {
        Ok(Self {
            portfolio: Portfolio::new(),
            risk_checker: RiskChecker::new(config.risk_limits.clone()),
            config,
        })
    }

    /// Process a market event (e.g., trade, price update)
    pub fn process_event(&mut self, event: &MarketEvent) -> anyhow::Result<()> {
        match event {
            MarketEvent::Trade(trade) => {
                self.update_position_from_trade(trade)?;
                self.check_circuit_breakers(trade)?;
            }
            MarketEvent::PriceTick(tick) => {
                self.update_position_prices(tick)?;
            }
            MarketEvent::MarketResolved { market_id, outcome_id } => {
                self.resolve_market(*market_id, outcome_id)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Evaluate a potential trade before execution
    pub fn evaluate_trade(
        &self,
        market_id: Uuid,
        outcome_id: &str,
        side: common::OrderSide,
        price: f64,
        size: f64,
    ) -> Result<TradeEvaluation, RiskViolation> {
        let position_value = price * size;

        // Check all risk limits
        self.risk_checker.check_trade(
            market_id,
            outcome_id,
            side,
            position_value,
            &self.portfolio,
        )?;

        // Calculate Kelly-optimal position size
        let kelly_limit = self.risk_checker.kelly_criterion.calculate_position(
            price,
            self.config.kelly_multiplier,
        );

        // Check if position exceeds Kelly criterion
        if position_value > kelly_limit {
            return Err(RiskViolation::KellyLimitExceeded {
                proposed: position_value,
                kelly_limit,
            });
        }

        Ok(TradeEvaluation {
            approved: true,
            kelly_limit,
            risk_level: self.risk_checker.calculate_risk_level(&self.portfolio),
        })
    }

    /// Update position after a trade is executed
    fn update_position_from_trade(&mut self, trade: &common::Trade) -> anyhow::Result<()> {
        let position_value = trade.price * trade.size;

        match trade.side {
            common::OrderSide::Buy => {
                self.portfolio.add_position(
                    trade.market_id,
                    &trade.outcome_id,
                    position_value,
                    trade.price,
                )?;
            }
            common::OrderSide::Sell => {
                self.portfolio.remove_position(
                    trade.market_id,
                    &trade.outcome_id,
                    position_value,
                    trade.price,
                )?;
            }
        }

        info!(
            market_id = %trade.market_id,
            outcome_id = %trade.outcome_id,
            side = ?trade.side,
            value = position_value,
            "Trade executed"
        );

        Ok(())
    }

    /// Update position prices from market data
    fn update_position_prices(&mut self, tick: &common::PriceTick) -> anyhow::Result<()> {
        self.portfolio.update_price(tick.market_id, &tick.outcome_id, tick.price);
        Ok(())
    }

    /// Check if any circuit breakers are triggered
    fn check_circuit_breakers(&self, trade: &common::Trade) -> anyhow::Result<()> {
        let violations = self.risk_checker.check_circuit_breakers(&self.portfolio);

        if !violations.is_empty() {
            let violation_summary: Vec<String> = violations
                .iter()
                .map(|v| format!("{:?}", v))
                .collect();

            error!(
                market_id = %trade.market_id,
                violations = ?violation_summary,
                "Circuit breaker triggered - trading halted"
            );

            return Err(anyhow::anyhow!(
                "Circuit breaker triggered: {}",
                violation_summary.join(", ")
            ));
        }

        Ok(())
    }

    /// Resolve a market and update portfolio accordingly
    fn resolve_market(
        &mut self,
        market_id: Uuid,
        winning_outcome_id: &str,
    ) -> anyhow::Result<()> {
        let pnl = self.portfolio.resolve_market(market_id, winning_outcome_id)?;

        info!(
            market_id = %market_id,
            winning_outcome = %winning_outcome_id,
            pnl = pnl,
            "Market resolved"
        );

        Ok(())
    }

    /// Get current portfolio metrics
    pub fn get_metrics(&self) -> RiskMetrics {
        self.portfolio.calculate_metrics()
    }

    /// Get portfolio summary
    pub fn get_summary(&self) -> PortfolioSummary {
        PortfolioSummary {
            total_value: self.portfolio.total_value(),
            num_positions: self.portfolio.num_positions(),
            total_pnl: self.portfolio.total_pnl(),
            exposure_by_category: self.portfolio.exposure_by_category(),
            risk_level: self.risk_checker.calculate_risk_level(&self.portfolio),
        }
    }
}

impl Default for PortfolioRiskManager {
    fn default() -> Self {
        Self::new().expect("Failed to create PortfolioRiskManager")
    }
}

/// Result of evaluating a potential trade
#[derive(Debug, Clone)]
pub struct TradeEvaluation {
    pub approved: bool,
    pub kelly_limit: f64,
    pub risk_level: RiskLevel,
}

/// Portfolio summary for reporting
#[derive(Debug, Clone)]
pub struct PortfolioSummary {
    pub total_value: f64,
    pub num_positions: usize,
    pub total_pnl: f64,
    pub exposure_by_category: Vec<(String, f64)>,
    pub risk_level: RiskLevel,
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{Market, Outcome, OrderBook, Order, PriceTick};

    #[test]
    fn test_portfolio_creation() {
        let manager = PortfolioRiskManager::new().unwrap();
        assert_eq!(manager.get_summary().total_value, 0.0);
    }

    #[test]
    fn test_position_tracking() {
        let mut portfolio = Portfolio::new();
        let market_id = Uuid::new_v4();

        portfolio.add_position(market_id, "YES", 100.0, 0.5).unwrap();
        assert_eq!(portfolio.total_value(), 100.0);
        assert_eq!(portfolio.num_positions(), 1);
    }

    #[test]
    fn test_risk_limits() {
        let config = RiskConfig {
            risk_limits: RiskLimits {
                max_position_size: 50.0,
                max_total_exposure: 200.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let manager = PortfolioRiskManager::with_config(config).unwrap();
        let market_id = Uuid::new_v4();

        // This should fail due to max position size
        let result = manager.evaluate_trade(
            market_id,
            "YES",
            common::OrderSide::Buy,
            0.5,
            150.0, // exceeds max_position_size of 50.0
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_kelly_criterion() {
        // Create Kelly with 5% edge
        let kelly = KellyCriterion::new(0.25, Some(0.05));

        // At 50% price with bankroll of 1000, Kelly suggests a small position
        let position = kelly.calculate_position(0.5, 1000.0);
        assert!(position > 0.0);
        assert!(position < 100.0); // Should be reasonable
    }

    #[test]
    fn test_var_calculation() {
        let mut portfolio = Portfolio::new();
        let market_id = Uuid::new_v4();

        // Add some positions
        portfolio.add_position(market_id, "YES", 100.0, 0.5).unwrap();

        // Update prices to generate PnL history
        for i in 0..20 {
            let price = 0.5 + (i as f64 * 0.01);
            portfolio.update_price(market_id, "YES", price);

            // Sell some to create PnL records
            if i % 5 == 0 && i > 0 {
                portfolio.remove_position(market_id, "YES", 10.0, price).unwrap();
            }
        }

        let metrics = portfolio.calculate_metrics();
        // VaR should be calculated if we have enough PnL history
        // The test just checks it doesn't crash
        println!("VaR (95%): {:?}", metrics.var_95);
    }
}
