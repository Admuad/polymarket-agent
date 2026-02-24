//! Portfolio management and position tracking

use crate::metrics::{RiskMetrics, VaRResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Current portfolio with all positions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    /// All open positions by (market_id, outcome_id)
    positions: HashMap<(Uuid, String), Position>,

    /// Historical PnL data for metrics calculation
    pnl_history: Vec<PnLRecord>,

    /// Total realized PnL
    total_realized_pnl: f64,

    /// Portfolio creation timestamp
    created_at: DateTime<Utc>,

    /// Category mapping for positions
    categories: HashMap<Uuid, String>,
}

impl Portfolio {
    /// Create a new empty portfolio
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            pnl_history: Vec::new(),
            total_realized_pnl: 0.0,
            created_at: Utc::now(),
            categories: HashMap::new(),
        }
    }

    /// Add or update a position (buy)
    pub fn add_position(
        &mut self,
        market_id: Uuid,
        outcome_id: &str,
        value: f64,
        price: f64,
    ) -> anyhow::Result<()> {
        let key = (market_id, outcome_id.to_string());

        match self.positions.get_mut(&key) {
            Some(position) => {
                // Update existing position
                position.update_on_buy(value, price)?;
            }
            None => {
                // Create new position
                let position = Position::new(market_id, outcome_id, value, price);
                self.positions.insert(key, position);
            }
        }

        Ok(())
    }

    /// Remove from position (sell)
    pub fn remove_position(
        &mut self,
        market_id: Uuid,
        outcome_id: &str,
        value: f64,
        price: f64,
    ) -> anyhow::Result<()> {
        let key = (market_id, outcome_id.to_string());

        let pnl = match self.positions.get_mut(&key) {
            Some(position) => {
                position.update_on_sell(value, price)?
            }
            None => {
                return Err(anyhow::anyhow!(
                    "Position not found for market {} outcome {}",
                    market_id,
                    outcome_id
                ));
            }
        };

        // Record PnL
        self.record_pnl(pnl);

        // Remove position if fully closed
        if let Some(position) = self.positions.get(&key) {
            if position.is_closed() {
                self.positions.remove(&key);
            }
        }

        Ok(())
    }

    /// Update current price for a position
    pub fn update_price(&mut self, market_id: Uuid, outcome_id: &str, price: f64) {
        let key = (market_id, outcome_id.to_string());

        if let Some(position) = self.positions.get_mut(&key) {
            position.current_price = price;
            position.updated_at = Utc::now();
        }
    }

    /// Resolve a market and calculate final PnL
    pub fn resolve_market(
        &mut self,
        market_id: Uuid,
        winning_outcome_id: &str,
    ) -> anyhow::Result<f64> {
        let mut total_pnl = 0.0;

        // Collect all positions for this market
        let market_positions: Vec<_> = self
            .positions
            .iter()
            .filter(|((id, _), _)| *id == market_id)
            .map(|(key, _)| key.clone())
            .collect();

        for key in market_positions {
            let outcome_id = key.1.clone();

            if let Some(position) = self.positions.remove(&key) {
                let pnl = if outcome_id == winning_outcome_id {
                    // Winning position
                    position.unrealized_pnl + position.investment
                } else {
                    // Losing position
                    -position.investment
                };

                total_pnl += pnl;
                self.record_pnl(pnl);

                tracing::info!(
                    market_id = %market_id,
                    outcome_id = %outcome_id,
                    pnl = pnl,
                    "Position resolved"
                );
            }
        }

        self.total_realized_pnl += total_pnl;
        Ok(total_pnl)
    }

    /// Set category for a market
    pub fn set_category(&mut self, market_id: Uuid, category: String) {
        self.categories.insert(market_id, category);
    }

    /// Get total portfolio value
    pub fn total_value(&self) -> f64 {
        self.positions.values().map(|p| p.current_value()).sum()
    }

    /// Get number of open positions
    pub fn num_positions(&self) -> usize {
        self.positions.len()
    }

    /// Get total realized PnL
    pub fn total_pnl(&self) -> f64 {
        self.total_realized_pnl
    }

    /// Get current unrealized PnL
    pub fn unrealized_pnl(&self) -> f64 {
        self.positions.values().map(|p| p.unrealized_pnl()).sum()
    }

    /// Get exposure by category
    pub fn exposure_by_category(&self) -> Vec<(String, f64)> {
        let mut exposure: HashMap<String, f64> = HashMap::new();

        for position in self.positions.values() {
            let category = self
                .categories
                .get(&position.market_id)
                .cloned()
                .unwrap_or_else(|| "uncategorized".to_string());

            *exposure.entry(category).or_insert(0.0) += position.current_value();
        }

        let mut result: Vec<_> = exposure.into_iter().collect();
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        result
    }

    /// Get all positions
    pub fn positions(&self) -> &HashMap<(Uuid, String), Position> {
        &self.positions
    }

    /// Record PnL for metrics calculation
    fn record_pnl(&mut self, pnl: f64) {
        self.pnl_history.push(PnLRecord {
            timestamp: Utc::now(),
            pnl,
            portfolio_value: self.total_value(),
        });

        // Keep last 1000 records
        if self.pnl_history.len() > 1000 {
            self.pnl_history.remove(0);
        }
    }

    /// Calculate risk metrics
    pub fn calculate_metrics(&self) -> RiskMetrics {
        let total_value = self.total_value();
        let positions_count = self.positions.len();

        // Calculate VaR
        let var_result = if self.pnl_history.len() >= 10 {
            self.calculate_var(0.95, 100)
        } else {
            VaRResult {
                var_95: None,
                var_99: None,
                expected_shortfall: None,
            }
        };

        // Calculate max drawdown
        let max_drawdown = self.calculate_max_drawdown();

        // Calculate Sharpe ratio
        let sharpe_ratio = self.calculate_sharpe_ratio();

        RiskMetrics {
            total_value,
            positions_count,
            unrealized_pnl: self.unrealized_pnl(),
            realized_pnl: self.total_realized_pnl,
            max_drawdown,
            sharpe_ratio,
            var_95: var_result.var_95,
            var_99: var_result.var_99,
            expected_shortfall: var_result.expected_shortfall,
        }
    }

    /// Calculate VaR using historical method
    fn calculate_var(&self, confidence: f64, samples: usize) -> VaRResult {
        if self.pnl_history.is_empty() {
            return VaRResult {
                var_95: None,
                var_99: None,
                expected_shortfall: None,
            };
        }

        let len = self.pnl_history.len().min(samples);
        let returns: Vec<f64> = self.pnl_history
            .iter()
            .rev()
            .take(len)
            .map(|r| r.pnl)
            .collect();

        let mut sorted_returns = returns.clone();
        sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let var_index = ((1.0 - confidence) * sorted_returns.len() as f64) as usize;
        let var_95 = sorted_returns.get(var_index).copied();

        // VaR 99%
        let var_99_index = ((1.0 - 0.99) * sorted_returns.len() as f64) as usize;
        let var_99 = sorted_returns.get(var_99_index).copied();

        // Expected Shortfall (average of worst 5%)
        let tail_size = (sorted_returns.len() as f64 * 0.05).ceil() as usize;
        let expected_shortfall = if tail_size > 0 {
            let sum: f64 = sorted_returns.iter().take(tail_size).sum();
            Some(sum / tail_size as f64)
        } else {
            None
        };

        VaRResult {
            var_95,
            var_99,
            expected_shortfall,
        }
    }

    /// Calculate maximum drawdown
    fn calculate_max_drawdown(&self) -> f64 {
        if self.pnl_history.is_empty() {
            return 0.0;
        }

        let mut peak = f64::MIN;
        let mut max_drawdown = 0.0;

        for record in &self.pnl_history {
            if record.portfolio_value > peak {
                peak = record.portfolio_value;
            }

            let drawdown = (peak - record.portfolio_value) / peak.max(1.0);
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }

        max_drawdown
    }

    /// Calculate Sharpe ratio
    fn calculate_sharpe_ratio(&self) -> Option<f64> {
        if self.pnl_history.len() < 2 {
            return None;
        }

        let returns: Vec<f64> = self
            .pnl_history
            .windows(2)
            .map(|w| {
                if w[0].portfolio_value > 0.0 {
                    (w[1].portfolio_value - w[0].portfolio_value) / w[0].portfolio_value
                } else {
                    0.0
                }
            })
            .collect();

        if returns.is_empty() {
            return None;
        }

        let mean: f64 = returns.iter().sum::<f64>() / returns.len() as f64;

        let variance: f64 = returns
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;

        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return Some(0.0);
        }

        // Annualize (assuming daily returns)
        let annualized_mean = mean * 365.0;
        let annualized_std = std_dev * (365.0_f64).sqrt();

        // Use 5% as risk-free rate
        let risk_free = 0.05;

        Some((annualized_mean - risk_free) / annualized_std)
    }
}

impl Default for Portfolio {
    fn default() -> Self {
        Self::new()
    }
}

/// Single position in a market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub market_id: Uuid,
    pub outcome_id: String,
    pub investment: f64,
    pub avg_entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub state: PositionState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Position {
    pub fn new(market_id: Uuid, outcome_id: &str, value: f64, price: f64) -> Self {
        Self {
            market_id,
            outcome_id: outcome_id.to_string(),
            investment: value,
            avg_entry_price: price,
            current_price: price,
            unrealized_pnl: 0.0,
            state: PositionState::Open,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Update position on buy (add to position)
    pub fn update_on_buy(&mut self, value: f64, price: f64) -> anyhow::Result<()> {
        if value <= 0.0 {
            return Err(anyhow::anyhow!("Buy value must be positive"));
        }

        // Calculate new average entry price
        let total_value = self.investment + value;
        let total_shares = self.investment / self.avg_entry_price + value / price;
        self.avg_entry_price = total_value / total_shares;
        self.investment = total_value;
        self.current_price = price;
        self.updated_at = Utc::now();

        Ok(())
    }

    /// Update position on sell (remove from position)
    pub fn update_on_sell(&mut self, value: f64, price: f64) -> anyhow::Result<f64> {
        if value <= 0.0 {
            return Err(anyhow::anyhow!("Sell value must be positive"));
        }

        if value > self.investment {
            return Err(anyhow::anyhow!("Cannot sell more than invested amount"));
        }

        // Calculate PnL for this trade
        let shares_sold = value / price;
        let cost_basis = shares_sold * self.avg_entry_price;
        let pnl = value - cost_basis;

        self.investment -= cost_basis;
        self.current_price = price;
        self.updated_at = Utc::now();

        Ok(pnl)
    }

    /// Check if position is closed
    pub fn is_closed(&self) -> bool {
        self.investment <= 0.01 // Near zero
    }

    /// Calculate current position value
    pub fn current_value(&self) -> f64 {
        if self.avg_entry_price > 0.0 {
            self.investment * (self.current_price / self.avg_entry_price)
        } else {
            0.0
        }
    }

    /// Calculate unrealized PnL
    pub fn unrealized_pnl(&self) -> f64 {
        self.current_value() - self.investment
    }
}

/// Position state
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PositionState {
    Open,
    PartiallyClosed,
    Closed,
}

/// Exposure information for a category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exposure {
    pub category: String,
    pub total_value: f64,
    pub position_count: usize,
    pub percentage_of_portfolio: f64,
}

/// PnL record for metrics calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PnLRecord {
    timestamp: DateTime<Utc>,
    pnl: f64,
    portfolio_value: f64,
}
