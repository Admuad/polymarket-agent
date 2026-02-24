//! Risk checking and circuit breaker implementation

use crate::config::{RiskLimits, CircuitBreakerConfig};
use crate::portfolio::Portfolio;

/// Current risk level assessment
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s >= 0.9 => RiskLevel::Critical,
            s if s >= 0.7 => RiskLevel::High,
            s if s >= 0.4 => RiskLevel::Medium,
            _ => RiskLevel::Low,
        }
    }
}
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Risk checker for evaluating trades and monitoring portfolio risk
#[derive(Debug, Clone)]
pub struct RiskChecker {
    risk_limits: RiskLimits,
    circuit_breaker_config: CircuitBreakerConfig,
    circuit_breaker: CircuitBreaker,
    pub kelly_criterion: KellyCriterion,
    violation_count: usize,
    last_violation_time: Option<DateTime<Utc>>,
}

impl RiskChecker {
    pub fn new(risk_limits: RiskLimits) -> Self {
        Self {
            risk_limits,
            circuit_breaker_config: CircuitBreakerConfig::default(),
            circuit_breaker: CircuitBreaker::new(),
            kelly_criterion: KellyCriterion::new(0.25, None),
            violation_count: 0,
            last_violation_time: None,
        }
    }

    /// Check if a trade violates any risk limits
    pub fn check_trade(
        &self,
        market_id: Uuid,
        outcome_id: &str,
        side: common::OrderSide,
        value: f64,
        portfolio: &Portfolio,
    ) -> Result<(), RiskViolation> {
        let total_value = portfolio.total_value();
        let new_total_value = total_value + value;

        // Check maximum total exposure
        if new_total_value > self.risk_limits.max_total_exposure {
            return Err(RiskViolation::MaxTotalExposureExceeded {
                current: total_value,
                proposed: value,
                limit: self.risk_limits.max_total_exposure,
            });
        }

        // Check maximum position size
        if value > self.risk_limits.max_position_size {
            return Err(RiskViolation::MaxPositionSizeExceeded {
                proposed: value,
                limit: self.risk_limits.max_position_size,
            });
        }

        // Check maximum number of positions
        if side == common::OrderSide::Buy && portfolio.num_positions() >= self.risk_limits.max_positions {
            return Err(RiskViolation::MaxPositionsExceeded {
                current: portfolio.num_positions(),
                limit: self.risk_limits.max_positions,
            });
        }

        // Check theme exposure (if category is known)
        let category = portfolio
            .positions()
            .iter()
            .find(|((id, _), _)| *id == market_id)
            .and_then(|_| {
                // This would need category info - for now just check general theme limit
                Some("default")
            });

        if let Some(_cat) = category {
            let theme_exposure = self.calculate_theme_exposure(portfolio, "default");
            let new_theme_exposure = theme_exposure + value;

            if new_theme_exposure > self.risk_limits.max_theme_exposure {
                return Err(RiskViolation::MaxThemeExposureExceeded {
                    theme: "default".to_string(),
                    current: theme_exposure,
                    proposed: value,
                    limit: self.risk_limits.max_theme_exposure,
                });
            }
        }

        Ok(())
    }

    /// Check all circuit breakers
    pub fn check_circuit_breakers(&self, portfolio: &Portfolio) -> Vec<RiskViolation> {
        let mut violations = Vec::new();

        // Check daily loss limit
        if let Some(daily_pnl) = self.calculate_daily_pnl(portfolio) {
            if daily_pnl < -self.risk_limits.daily_loss_limit {
                violations.push(RiskViolation::DailyLossLimitExceeded {
                    daily_pnl,
                    limit: self.risk_limits.daily_loss_limit,
                });
            }
        }

        // Check max drawdown
        let metrics = portfolio.calculate_metrics();
        if metrics.max_drawdown > self.circuit_breaker_config.max_drawdown_percentage {
            violations.push(RiskViolation::MaxDrawdownExceeded {
                current: metrics.max_drawdown,
                limit: self.circuit_breaker_config.max_drawdown_percentage,
            });
        }

        // Check VaR limit
        if let Some(var) = metrics.var_95 {
            if var.abs() > self.circuit_breaker_config.var_95_limit {
                violations.push(RiskViolation::VaRLimitExceeded {
                    var_95: var.abs(),
                    limit: self.circuit_breaker_config.var_95_limit,
                });
            }
        }

        violations
    }

    /// Calculate current risk level (0.0 to 1.0)
    pub fn calculate_risk_level(&self, portfolio: &Portfolio) -> RiskLevel {
        let metrics = portfolio.calculate_metrics();

        // Normalize and combine risk factors
        let exposure_ratio = portfolio.total_value() / self.risk_limits.max_total_exposure;
        let drawdown_ratio = metrics.max_drawdown / self.circuit_breaker_config.max_drawdown_percentage;
        let position_ratio = portfolio.num_positions() as f64 / self.risk_limits.max_positions as f64;

        // Weighted score
        let score = (exposure_ratio * 0.4 + drawdown_ratio * 0.4 + position_ratio * 0.2).min(1.0);

        RiskLevel::from_score(score)
    }

    /// Calculate exposure for a specific theme
    fn calculate_theme_exposure(&self, portfolio: &Portfolio, theme: &str) -> f64 {
        portfolio
            .exposure_by_category()
            .iter()
            .filter(|(cat, _)| cat == theme)
            .map(|(_, value)| *value)
            .sum()
    }

    /// Calculate daily PnL
    fn calculate_daily_pnl(&self, _portfolio: &Portfolio) -> Option<f64> {
        // This would need access to PnL history
        // For now, return None
        None
    }
}

/// Kelly criterion for optimal position sizing
#[derive(Debug, Clone)]
pub struct KellyCriterion {
    /// Estimated edge (advantage) as a percentage
    edge: Option<f64>,

    /// Kelly multiplier (0.25 = quarter-Kelly for safety)
    multiplier: f64,
}

impl KellyCriterion {
    /// Create a new Kelly criterion calculator
    ///
    /// # Arguments
    /// * `multiplier` - Fraction of full Kelly to use (e.g., 0.25 for quarter-Kelly)
    /// * `edge` - Optional fixed edge estimation. If None, will estimate from market data.
    pub fn new(multiplier: f64, edge: Option<f64>) -> Self {
        Self { edge, multiplier }
    }

    /// Calculate optimal position size
    ///
    /// # Arguments
    /// * `price` - Current market price (0.0 to 1.0)
    /// * `bankroll` - Total available capital
    ///
    /// # Returns
    /// Optimal position size in USD
    pub fn calculate_position(&self, price: f64, bankroll: f64) -> f64 {
        if price <= 0.0 || price >= 1.0 || bankroll <= 0.0 {
            return 0.0;
        }

        // Kelly formula: f* = (bp - q) / b
        // where:
        //   b = odds - 1 = (1/price) - 1
        //   p = probability of winning (our estimated probability)
        //   q = 1 - p (probability of losing)

        let b = (1.0 / price) - 1.0;

        // Use provided edge or estimate from price difference
        let p = if let Some(edge) = self.edge {
            // Edge is our advantage over the market
            // If market price is 0.5 and we think it's 0.55, edge = 0.05
            (price + edge).min(0.99)
        } else {
            // Conservative: use market price (no edge assumption)
            price
        };

        let q = 1.0 - p;

        // Full Kelly fraction
        let full_kelly = (b * p - q) / b;

        // Apply multiplier (usually 0.25 for quarter-Kelly)
        let kelly_fraction = (full_kelly * self.multiplier).max(0.0);

        // Cap at 25% of bankroll for safety
        let capped_fraction = kelly_fraction.min(0.25);

        capped_fraction * bankroll
    }

    /// Estimate edge from historical performance
    pub fn estimate_edge_from_history(
        &mut self,
        win_rate: f64,
        avg_win: f64,
        avg_loss: f64,
    ) -> f64 {
        // Simplified edge estimation
        if win_rate <= 0.0 || avg_loss <= 0.0 {
            self.edge = None;
            return 0.0;
        }

        // Edge = (win_rate * avg_win) - ((1 - win_rate) * avg_loss)
        let edge = (win_rate * avg_win) - ((1.0 - win_rate) * avg_loss);

        self.edge = Some(edge.max(0.0));
        self.edge.unwrap()
    }
}

/// Circuit breaker for automatic risk limits
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    is_triggered: bool,
    trigger_time: Option<DateTime<Utc>>,
    violations_today: Vec<DateTime<Utc>>,
    cooldown_duration: Duration,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            is_triggered: false,
            trigger_time: None,
            violations_today: Vec::new(),
            cooldown_duration: Duration::minutes(30),
        }
    }

    /// Trigger the circuit breaker
    pub fn trigger(&mut self) {
        self.is_triggered = true;
        self.trigger_time = Some(Utc::now());
        self.violations_today.push(Utc::now());

        tracing::error!(
            time = ?Utc::now(),
            "Circuit breaker TRIGGERED - Trading halted"
        );
    }

    /// Check if circuit breaker is active
    pub fn is_active(&self) -> bool {
        if !self.is_triggered {
            return false;
        }

        // Check if cooldown period has passed
        if let Some(trigger_time) = self.trigger_time {
            if Utc::now() - trigger_time > self.cooldown_duration {
                return false;
            }
        }

        true
    }

    /// Reset the circuit breaker after cooldown
    pub fn reset(&mut self) {
        self.is_triggered = false;
        self.trigger_time = None;

        // Clear old violations (older than 24 hours)
        let cutoff = Utc::now() - Duration::hours(24);
        self.violations_today
            .retain(|t| *t > cutoff);

        tracing::info!("Circuit breaker reset - Trading resumed");
    }

    /// Get number of violations today
    pub fn violations_today(&self) -> usize {
        self.violations_today.len()
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Risk violation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskViolation {
    MaxPositionSizeExceeded {
        proposed: f64,
        limit: f64,
    },
    MaxTotalExposureExceeded {
        current: f64,
        proposed: f64,
        limit: f64,
    },
    MaxPositionsExceeded {
        current: usize,
        limit: usize,
    },
    MaxThemeExposureExceeded {
        theme: String,
        current: f64,
        proposed: f64,
        limit: f64,
    },
    DailyLossLimitExceeded {
        daily_pnl: f64,
        limit: f64,
    },
    MaxDrawdownExceeded {
        current: f64,
        limit: f64,
    },
    VaRLimitExceeded {
        var_95: f64,
        limit: f64,
    },
    KellyLimitExceeded {
        proposed: f64,
        kelly_limit: f64,
    },
    CorrelationDetected {
        market_1: String,
        market_2: String,
        correlation: f64,
    },
}

impl std::fmt::Display for RiskViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskViolation::MaxPositionSizeExceeded { proposed, limit } => {
                write!(f, "Position size ${:.2} exceeds limit ${:.2}", proposed, limit)
            }
            RiskViolation::MaxTotalExposureExceeded { current, proposed, limit } => {
                write!(
                    f,
                    "Total exposure ${:.2} + ${:.2} exceeds limit ${:.2}",
                    current, proposed, limit
                )
            }
            RiskViolation::MaxPositionsExceeded { current, limit } => {
                write!(f, "Number of positions {} exceeds limit {}", current, limit)
            }
            RiskViolation::MaxThemeExposureExceeded { theme, current, proposed, limit } => {
                write!(
                    f,
                    "Theme '{}' exposure ${:.2} + ${:.2} exceeds limit ${:.2}",
                    theme, current, proposed, limit
                )
            }
            RiskViolation::DailyLossLimitExceeded { daily_pnl, limit } => {
                write!(
                    f,
                    "Daily loss ${:.2} exceeds limit ${:.2}",
                    daily_pnl.abs(),
                    limit
                )
            }
            RiskViolation::MaxDrawdownExceeded { current, limit } => {
                write!(
                    f,
                    "Drawdown {:.2}% exceeds limit {:.2}%",
                    current * 100.0,
                    limit * 100.0
                )
            }
            RiskViolation::VaRLimitExceeded { var_95, limit } => {
                write!(f, "VaR (95%) ${:.2} exceeds limit ${:.2}", var_95, limit)
            }
            RiskViolation::KellyLimitExceeded { proposed, kelly_limit } => {
                write!(
                    f,
                    "Position ${:.2} exceeds Kelly limit ${:.2}",
                    proposed, kelly_limit
                )
            }
            RiskViolation::CorrelationDetected { market_1, market_2, correlation } => {
                write!(
                    f,
                    "High correlation {:.2} detected between {} and {}",
                    correlation, market_1, market_2
                )
            }
        }
    }
}

impl std::error::Error for RiskViolation {}

/// Correlation monitoring for detecting correlated positions
#[derive(Debug, Clone)]
pub struct CorrelationMonitor {
    /// Correlation threshold for flagging
    threshold: f64,
    /// Price history for correlation calculation
    price_history: HashMap<String, Vec<f64>>,
}

impl CorrelationMonitor {
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold,
            price_history: HashMap::new(),
        }
    }

    /// Update price history for a market
    pub fn update_price(&mut self, market_id: &str, price: f64) {
        let history = self.price_history.entry(market_id.to_string()).or_insert_with(Vec::new);
        history.push(price);

        // Keep last 100 prices
        if history.len() > 100 {
            history.remove(0);
        }
    }

    /// Check for correlated positions
    pub fn check_correlations(&self) -> Vec<RiskViolation> {
        let mut violations = Vec::new();
        let market_ids: Vec<_> = self.price_history.keys().cloned().collect();

        for i in 0..market_ids.len() {
            for j in (i + 1)..market_ids.len() {
                let market_1 = &market_ids[i];
                let market_2 = &market_ids[j];

                if let Some(correlation) = self.calculate_correlation(market_1, market_2) {
                    if correlation.abs() >= self.threshold {
                        violations.push(RiskViolation::CorrelationDetected {
                            market_1: market_1.clone(),
                            market_2: market_2.clone(),
                            correlation,
                        });
                    }
                }
            }
        }

        violations
    }

    /// Calculate Pearson correlation between two markets
    fn calculate_correlation(&self, market_1: &str, market_2: &str) -> Option<f64> {
        let prices_1 = self.price_history.get(market_1)?;
        let prices_2 = self.price_history.get(market_2)?;

        // Need at least 2 data points
        if prices_1.len() < 2 || prices_2.len() < 2 {
            return None;
        }

        // Use minimum length
        let n = prices_1.len().min(prices_2.len());

        // Calculate means
        let mean_1: f64 = prices_1.iter().take(n).sum::<f64>() / n as f64;
        let mean_2: f64 = prices_2.iter().take(n).sum::<f64>() / n as f64;

        // Calculate covariance and variances
        let mut covariance = 0.0;
        let mut variance_1 = 0.0;
        let mut variance_2 = 0.0;

        for i in 0..n {
            let diff_1 = prices_1[i] - mean_1;
            let diff_2 = prices_2[i] - mean_2;

            covariance += diff_1 * diff_2;
            variance_1 += diff_1 * diff_1;
            variance_2 += diff_2 * diff_2;
        }

        covariance /= n as f64;
        variance_1 /= n as f64;
        variance_2 /= n as f64;

        // Calculate correlation
        let denominator = (variance_1 * variance_2).sqrt();
        if denominator == 0.0 {
            return None;
        }

        Some(covariance / denominator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kelly_criterion() {
        // Create Kelly with 5% edge (we think probability is 5% higher than market price)
        let kelly = KellyCriterion::new(0.25, Some(0.05));

        // At 50% price with 1000 bankroll, should return a reasonable position
        let position = kelly.calculate_position(0.5, 1000.0);
        assert!(position > 0.0);
        assert!(position < 250.0); // Quarter-Kelly should be conservative
    }

    #[test]
    fn test_circuit_breaker() {
        let mut cb = CircuitBreaker::new();
        assert!(!cb.is_active());

        cb.trigger();
        assert!(cb.is_active());
        assert_eq!(cb.violations_today(), 1);
    }

    #[test]
    fn test_correlation_monitor() {
        let mut monitor = CorrelationMonitor::new(0.7);

        // Add perfectly correlated prices
        for i in 0..10 {
            monitor.update_price("market1", 0.5 + i as f64 * 0.01);
            monitor.update_price("market2", 0.5 + i as f64 * 0.01);
        }

        let violations = monitor.check_correlations();
        assert!(!violations.is_empty());
    }
}
