// Signal Storage Interface
// Provides persistence for signals for backtesting and analysis

use super::signals::TradeSignal;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;

/// Trait for signal storage backends
#[async_trait::async_trait]
pub trait SignalStorage: Send + Sync {
    /// Store a signal
    async fn store(&self, signal: &TradeSignal) -> Result<()>;

    /// Retrieve a signal by ID
    async fn get(&self, signal_id: Uuid) -> Result<Option<TradeSignal>>;

    /// Retrieve all signals for a market
    async fn get_by_market(&self, market_id: Uuid) -> Result<Vec<TradeSignal>>;

    /// Retrieve signals within a time range
    async fn get_by_time_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<TradeSignal>>;

    /// Retrieve signals by type
    async fn get_by_type(&self, signal_type: &str) -> Result<Vec<TradeSignal>>;

    /// Get all signals (use with caution)
    async fn get_all(&self) -> Result<Vec<TradeSignal>>;

    /// Delete a signal by ID
    async fn delete(&self, signal_id: Uuid) -> Result<bool>;

    /// Get storage statistics
    async fn stats(&self) -> Result<StorageStats>;
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_signals: usize,
    pub signals_by_type: HashMap<String, usize>,
    pub oldest_signal: Option<DateTime<Utc>>,
    pub newest_signal: Option<DateTime<Utc>>,
    pub storage_size_bytes: Option<usize>,
}

/// In-memory signal storage (for testing and development)
pub struct InMemoryStorage {
    signals: tokio::sync::RwLock<HashMap<Uuid, TradeSignal>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            signals: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SignalStorage for InMemoryStorage {
    async fn store(&self, signal: &TradeSignal) -> Result<()> {
        let mut signals = self.signals.write().await;
        signals.insert(signal.id, signal.clone());
        Ok(())
    }

    async fn get(&self, signal_id: Uuid) -> Result<Option<TradeSignal>> {
        let signals = self.signals.read().await;
        Ok(signals.get(&signal_id).cloned())
    }

    async fn get_by_market(&self, market_id: Uuid) -> Result<Vec<TradeSignal>> {
        let signals = self.signals.read().await;
        let market_signals = signals
            .values()
            .filter(|s| s.market_id == market_id)
            .cloned()
            .collect();
        Ok(market_signals)
    }

    async fn get_by_time_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<TradeSignal>> {
        let signals = self.signals.read().await;
        let range_signals = signals
            .values()
            .filter(|s| s.created_at >= start && s.created_at <= end)
            .cloned()
            .collect();
        Ok(range_signals)
    }

    async fn get_by_type(&self, signal_type: &str) -> Result<Vec<TradeSignal>> {
        let signals = self.signals.read().await;
        let type_signals = signals
            .values()
            .filter(|s| format!("{:?}", s.signal_type) == signal_type)
            .cloned()
            .collect();
        Ok(type_signals)
    }

    async fn get_all(&self) -> Result<Vec<TradeSignal>> {
        let signals = self.signals.read().await;
        Ok(signals.values().cloned().collect())
    }

    async fn delete(&self, signal_id: Uuid) -> Result<bool> {
        let mut signals = self.signals.write().await;
        Ok(signals.remove(&signal_id).is_some())
    }

    async fn stats(&self) -> Result<StorageStats> {
        let signals = self.signals.read().await;

        let mut signals_by_type = HashMap::new();
        let mut oldest_signal = None;
        let mut newest_signal = None;

        for signal in signals.values() {
            let type_name = format!("{:?}", signal.signal_type);
            *signals_by_type.entry(type_name).or_insert(0) += 1;

            if oldest_signal.is_none() || signal.created_at < oldest_signal.unwrap() {
                oldest_signal = Some(signal.created_at);
            }

            if newest_signal.is_none() || signal.created_at > newest_signal.unwrap() {
                newest_signal = Some(signal.created_at);
            }
        }

        Ok(StorageStats {
            total_signals: signals.len(),
            signals_by_type,
            oldest_signal,
            newest_signal,
            storage_size_bytes: None,
        })
    }
}

/// Signal execution result (for backtesting)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalExecutionResult {
    pub signal_id: Uuid,
    pub market_id: Uuid,
    pub outcome_id: Option<String>,
    pub executed_at: DateTime<Utc>,
    pub entry_price: rust_decimal::Decimal,
    pub exit_price: Option<rust_decimal::Decimal>,
    pub position_size: rust_decimal::Decimal,
    pub pnl: Option<rust_decimal::Decimal>,
    pub pnl_percentage: Option<rust_decimal::Decimal>,
    pub holding_period_hours: Option<f64>,
    pub exit_reason: ExitReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExitReason {
    TargetHit,
    StopLoss,
    Manual,
    SignalExpired,
    MarketResolved,
    Timeout,
}

/// Trait for storing execution results
#[async_trait::async_trait]
pub trait ExecutionStorage: Send + Sync {
    /// Store an execution result
    async fn store(&self, result: &SignalExecutionResult) -> Result<()>;

    /// Get execution results for a signal
    async fn get_by_signal(&self, signal_id: Uuid) -> Result<Option<SignalExecutionResult>>;

    /// Get all execution results for a market
    async fn get_by_market(&self, market_id: Uuid) -> Result<Vec<SignalExecutionResult>>;

    /// Get backtesting statistics
    async fn get_backtest_stats(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<BacktestStats>;
}

/// Backtesting statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestStats {
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub total_pnl: rust_decimal::Decimal,
    pub average_pnl: rust_decimal::Decimal,
    pub average_win: rust_decimal::Decimal,
    pub average_loss: rust_decimal::Decimal,
    pub max_drawdown: rust_decimal::Decimal,
    pub sharpe_ratio: Option<f64>,
    pub by_signal_type: HashMap<String, SignalTypeStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalTypeStats {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub win_rate: f64,
    pub total_pnl: rust_decimal::Decimal,
    pub average_pnl: rust_decimal::Decimal,
}

/// In-memory execution storage (for backtesting)
pub struct InMemoryExecutionStorage {
    results: tokio::sync::RwLock<HashMap<Uuid, SignalExecutionResult>>,
}

impl InMemoryExecutionStorage {
    pub fn new() -> Self {
        Self {
            results: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryExecutionStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ExecutionStorage for InMemoryExecutionStorage {
    async fn store(&self, result: &SignalExecutionResult) -> Result<()> {
        let mut results = self.results.write().await;
        results.insert(result.signal_id, result.clone());
        Ok(())
    }

    async fn get_by_signal(&self, signal_id: Uuid) -> Result<Option<SignalExecutionResult>> {
        let results = self.results.read().await;
        Ok(results.get(&signal_id).cloned())
    }

    async fn get_by_market(&self, market_id: Uuid) -> Result<Vec<SignalExecutionResult>> {
        let results = self.results.read().await;
        let market_results = results
            .values()
            .filter(|r| r.market_id == market_id)
            .cloned()
            .collect();
        Ok(market_results)
    }

    async fn get_backtest_stats(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<BacktestStats> {
        let results = self.results.read().await;

        let filtered: Vec<_> = results
            .values()
            .filter(|r| r.executed_at >= start && r.executed_at <= end)
            .filter(|r| r.exit_price.is_some())
            .cloned()
            .collect();

        let total_trades = filtered.len();
        let winning_trades = filtered.iter().filter(|r| r.pnl.map_or(false, |p| p > rust_decimal::Decimal::ZERO)).count();
        let losing_trades = filtered.iter().filter(|r| r.pnl.map_or(false, |p| p < rust_decimal::Decimal::ZERO)).count();
        let win_rate = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64
        } else {
            0.0
        };

        let total_pnl = filtered
            .iter()
            .filter_map(|r| r.pnl)
            .fold(rust_decimal::Decimal::ZERO, |acc, p| acc + p);

        let average_pnl = if total_trades > 0 {
            total_pnl / rust_decimal::Decimal::from(total_trades as i64)
        } else {
            rust_decimal::Decimal::ZERO
        };

        let winning_pnl: rust_decimal::Decimal = filtered
            .iter()
            .filter_map(|r| r.pnl)
            .filter(|p| *p > rust_decimal::Decimal::ZERO)
            .fold(rust_decimal::Decimal::ZERO, |acc, p| acc + p);

        let losing_pnl: rust_decimal::Decimal = filtered
            .iter()
            .filter_map(|r| r.pnl)
            .filter(|p| *p < rust_decimal::Decimal::ZERO)
            .fold(rust_decimal::Decimal::ZERO, |acc, p| acc + p);

        let average_win = if winning_trades > 0 {
            winning_pnl / rust_decimal::Decimal::from(winning_trades as i64)
        } else {
            rust_decimal::Decimal::ZERO
        };

        let average_loss = if losing_trades > 0 {
            losing_pnl / rust_decimal::Decimal::from(losing_trades as i64)
        } else {
            rust_decimal::Decimal::ZERO
        };

        // Calculate max drawdown (simplified)
        let max_drawdown = rust_decimal::Decimal::ZERO; // TODO: Implement proper drawdown calculation

        Ok(BacktestStats {
            period_start: start,
            period_end: end,
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            total_pnl,
            average_pnl,
            average_win,
            average_loss,
            max_drawdown,
            sharpe_ratio: None, // TODO: Implement Sharpe ratio calculation
            by_signal_type: HashMap::new(), // TODO: Implement per-signal-type stats
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::signals::{SignalMetadata, SignalType, SignalDirection};
    use rust_decimal::prelude::*;

    #[tokio::test]
    async fn test_in_memory_storage() {
        let storage = InMemoryStorage::new();

        let signal = TradeSignal {
            id: Uuid::new_v4(),
            market_id: Uuid::new_v4(),
            signal_type: SignalType::SpreadArbitrage,
            direction: SignalDirection::Long,
            outcome_id: Some("test".to_string()),
            entry_price: rust_decimal::Decimal::from_f64(0.5).unwrap(),
            target_price: rust_decimal::Decimal::from_f64(0.6).unwrap(),
            stop_loss: rust_decimal::Decimal::from_f64(0.4).unwrap(),
            position_size: rust_decimal::Decimal::from_f64(100.0).unwrap(),
            confidence: 0.8,
            expected_value: rust_decimal::Decimal::from_f64(10.0).unwrap(),
            edge: rust_decimal::Decimal::from_f64(0.05).unwrap(),
            kelly_fraction: 0.1,
            reasoning: "test".to_string(),
            metadata: SignalMetadata {
                research_sources: vec![],
                data_points: 10,
                liquidity_score: 0.5,
                volatility_score: 0.5,
                custom_fields: serde_json::json!({}),
            },
            created_at: Utc::now(),
            expires_at: None,
        };

        storage.store(&signal).await.unwrap();
        let retrieved = storage.get(signal.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, signal.id);

        let stats = storage.stats().await.unwrap();
        assert_eq!(stats.total_signals, 1);
    }
}
