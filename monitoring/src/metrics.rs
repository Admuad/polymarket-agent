use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use common::PerformanceMetrics;
use sqlx::postgres::PgPool;
use std::sync::Arc;
use tracing::info;

/// Metrics Calculator - Computes performance metrics for strategies/agents
pub struct MetricsCalculator {
    db_pool: Arc<PgPool>,
    risk_free_rate: f64, // Annualized risk-free rate for Sharpe calculation
}

impl MetricsCalculator {
    pub fn new(db_pool: Arc<PgPool>, risk_free_rate: f64) -> Self {
        Self {
            db_pool,
            risk_free_rate,
        }
    }

    /// Initialize metrics tables
    pub async fn initialize(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS performance_metrics (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                strategy_id TEXT NOT NULL,
                agent_id UUID,
                period_start TIMESTAMPTZ NOT NULL,
                period_end TIMESTAMPTZ NOT NULL,
                total_trades BIGINT NOT NULL,
                winning_trades BIGINT NOT NULL,
                losing_trades BIGINT NOT NULL,
                hit_rate NUMERIC(10, 4) NOT NULL,
                total_pnl NUMERIC(15, 4) NOT NULL,
                roi NUMERIC(10, 4) NOT NULL,
                sharpe_ratio NUMERIC(10, 4),
                max_drawdown NUMERIC(15, 4) NOT NULL,
                avg_win NUMERIC(10, 4) NOT NULL,
                avg_loss NUMERIC(10, 4) NOT NULL,
                profit_factor NUMERIC(10, 4) NOT NULL,
                calmar_ratio NUMERIC(10, 4),
                created_at TIMESTAMPTZ DEFAULT NOW(),
                UNIQUE(strategy_id, period_start, period_end)
            );

            CREATE INDEX IF NOT EXISTS idx_metrics_strategy ON performance_metrics(strategy_id);
            CREATE INDEX IF NOT EXISTS idx_metrics_agent ON performance_metrics(agent_id);
            CREATE INDEX IF NOT EXISTS idx_metrics_time ON performance_metrics(period_start, period_end);
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to create metrics tables")?;

        info!("Performance metrics tables initialized");
        Ok(())
    }

    /// Calculate comprehensive performance metrics for a strategy
    pub async fn calculate_strategy_metrics(
        &self,
        strategy_id: &str,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Result<PerformanceMetrics> {
        // Get trade-level data
        let trades = sqlx::query_as::<_, (DateTime<Utc>, f64, f64)>(
            r#"
            SELECT
                timestamp,
                pnl,
                pnl_percent
            FROM trades t
            JOIN attributed_trades at ON t.id = at.trade_id
            WHERE at.strategy_id = $1
            AND t.timestamp >= $2
            AND t.timestamp <= $3
            AND t.pnl IS NOT NULL
            ORDER BY timestamp
            "#,
        )
        .bind(strategy_id)
        .bind(period_start)
        .bind(period_end)
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to fetch trade data")?;

        if trades.is_empty() {
            return Ok(PerformanceMetrics {
                strategy_id: strategy_id.to_string(),
                period_start,
                period_end,
                total_trades: 0,
                winning_trades: 0,
                losing_trades: 0,
                hit_rate: 0.0,
                total_pnl: 0.0,
                roi: 0.0,
                sharpe_ratio: None,
                max_drawdown: 0.0,
                avg_win: 0.0,
                avg_loss: 0.0,
                profit_factor: 0.0,
                calmar_ratio: None,
            });
        }

        let total_trades = trades.len() as i64;
        let winning_trades = trades.iter().filter(|t| t.1 > 0.0).count() as i64;
        let losing_trades = trades.iter().filter(|t| t.1 < 0.0).count() as i64;
        let hit_rate = if total_trades > 0 {
            (winning_trades as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };

        let total_pnl: f64 = trades.iter().map(|t| t.1).sum();
        let roi = self.calculate_roi(&trades)?;

        let sharpe_ratio = self.calculate_sharpe_ratio(&trades)?;
        let max_drawdown = self.calculate_max_drawdown(&trades)?;

        let avg_win = if winning_trades > 0 {
            trades.iter().filter(|t| t.1 > 0.0).map(|t| t.1).sum::<f64>() / winning_trades as f64
        } else {
            0.0
        };

        let avg_loss = if losing_trades > 0 {
            trades.iter().filter(|t| t.1 < 0.0).map(|t| t.1.abs()).sum::<f64>() / losing_trades as f64
        } else {
            0.0
        };

        let total_wins = trades.iter().filter(|t| t.1 > 0.0).map(|t| t.1).sum::<f64>();
        let total_losses = trades.iter().filter(|t| t.1 < 0.0).map(|t| t.1.abs()).sum::<f64>();
        let profit_factor = if total_losses > 0.0 { total_wins / total_losses } else { 0.0 };

        let calmar_ratio = if max_drawdown != 0.0 {
            Some(total_pnl.abs() / max_drawdown)
        } else {
            None
        };

        let metrics = PerformanceMetrics {
            strategy_id: strategy_id.to_string(),
            period_start,
            period_end,
            total_trades,
            winning_trades,
            losing_trades,
            hit_rate,
            total_pnl,
            roi,
            sharpe_ratio,
            max_drawdown,
            avg_win,
            avg_loss,
            profit_factor,
            calmar_ratio,
        };

        // Store metrics
        self.store_metrics(&metrics).await?;

        Ok(metrics)
    }

    /// Calculate ROI (Return on Investment)
    fn calculate_roi(&self, trades: &[(DateTime<Utc>, f64, f64)]) -> Result<f64> {
        let total_invested: f64 = trades
            .iter()
            .map(|t| {
                // Rough estimate: entry price * size as cost
                // In production, you'd track actual invested amount
                t.1.abs() * 0.5 // Simplified assumption
            })
            .sum();

        let total_return: f64 = trades.iter().map(|t| t.1).sum();

        if total_invested > 0.0 {
            Ok((total_return / total_invested) * 100.0)
        } else {
            Ok(0.0)
        }
    }

    /// Calculate Sharpe Ratio
    fn calculate_sharpe_ratio(&self, trades: &[(DateTime<Utc>, f64, f64)]) -> Result<Option<f64>> {
        if trades.len() < 2 {
            return Ok(None);
        }

        let returns: Vec<f64> = trades.iter().map(|t| t.2).collect();
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;

        let variance = returns
            .iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / (returns.len() - 1) as f64;

        let std_dev = variance.sqrt();

        if std_dev > 0.0 {
            // Annualize: assume daily returns, multiply by sqrt(252)
            let annualized_std = std_dev * (252.0_f64).sqrt();
            let excess_return = (mean_return - self.risk_free_rate) * 252.0;
            Ok(Some(excess_return / annualized_std))
        } else {
            Ok(None)
        }
    }

    /// Calculate Maximum Drawdown
    fn calculate_max_drawdown(&self, trades: &[(DateTime<Utc>, f64, f64)]) -> Result<f64> {
        let mut cumulative_pnl = 0.0;
        let mut peak = 0.0;
        let mut max_drawdown = 0.0;

        for (_, pnl, _) in trades {
            cumulative_pnl += pnl;
            if cumulative_pnl > peak {
                peak = cumulative_pnl;
            }
            let drawdown = (peak - cumulative_pnl) / peak.abs().max(1.0);
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }

        Ok(max_drawdown * 100.0) // Return as percentage
    }

    /// Store calculated metrics
    async fn store_metrics(&self, metrics: &PerformanceMetrics) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO performance_metrics (
                strategy_id, period_start, period_end,
                total_trades, winning_trades, losing_trades,
                hit_rate, total_pnl, roi, sharpe_ratio,
                max_drawdown, avg_win, avg_loss, profit_factor, calmar_ratio
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            ON CONFLICT (strategy_id, period_start, period_end)
            DO UPDATE SET
                total_trades = EXCLUDED.total_trades,
                winning_trades = EXCLUDED.winning_trades,
                losing_trades = EXCLUDED.losing_trades,
                hit_rate = EXCLUDED.hit_rate,
                total_pnl = EXCLUDED.total_pnl,
                roi = EXCLUDED.roi,
                sharpe_ratio = EXCLUDED.sharpe_ratio,
                max_drawdown = EXCLUDED.max_drawdown,
                avg_win = EXCLUDED.avg_win,
                avg_loss = EXCLUDED.avg_loss,
                profit_factor = EXCLUDED.profit_factor,
                calmar_ratio = EXCLUDED.calmar_ratio
            "#,
        )
        .bind(&metrics.strategy_id)
        .bind(metrics.period_start)
        .bind(metrics.period_end)
        .bind(metrics.total_trades)
        .bind(metrics.winning_trades)
        .bind(metrics.losing_trades)
        .bind(metrics.hit_rate)
        .bind(metrics.total_pnl)
        .bind(metrics.roi)
        .bind(metrics.sharpe_ratio)
        .bind(metrics.max_drawdown)
        .bind(metrics.avg_win)
        .bind(metrics.avg_loss)
        .bind(metrics.profit_factor)
        .bind(metrics.calmar_ratio)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to store metrics")?;

        Ok(())
    }

    /// Get metrics for a strategy over time
    pub async fn get_metrics_history(
        &self,
        strategy_id: &str,
        days: i64,
    ) -> Result<Vec<PerformanceMetrics>> {
        let from = Utc::now() - Duration::days(days);

        let metrics = sqlx::query_as::<_, PerformanceMetrics>(
            "SELECT * FROM performance_metrics WHERE strategy_id = $1 AND period_start >= $2 ORDER BY period_start"
        )
        .bind(strategy_id)
        .bind(from)
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to fetch metrics history")?;

        Ok(metrics)
    }

    /// Compare performance between two strategies
    pub async fn compare_strategies(
        &self,
        strategy_a: &str,
        strategy_b: &str,
        days: i64,
    ) -> Result<StrategyComparison> {
        let metrics_a = self
            .calculate_strategy_metrics(
                strategy_a,
                Utc::now() - Duration::days(days),
                Utc::now(),
            )
            .await?;

        let metrics_b = self
            .calculate_strategy_metrics(
                strategy_b,
                Utc::now() - Duration::days(days),
                Utc::now(),
            )
            .await?;

        // Calculate winner before moving values
        let winner = if metrics_a.total_pnl > metrics_b.total_pnl {
            Some(strategy_a.to_string())
        } else {
            Some(strategy_b.to_string())
        };

        Ok(StrategyComparison {
            strategy_a: strategy_a.to_string(),
            strategy_b: strategy_b.to_string(),
            metrics_a,
            metrics_b,
            period_days: days,
            winner,
        })
    }
}

#[derive(Debug, Clone)]
pub struct StrategyComparison {
    pub strategy_a: String,
    pub strategy_b: String,
    pub metrics_a: PerformanceMetrics,
    pub metrics_b: PerformanceMetrics,
    pub period_days: i64,
    pub winner: Option<String>,
}
