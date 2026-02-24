use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use common::{OrderSide, ShadowTrade};
use sqlx::postgres::PgPool;
use std::sync::Arc;
use tracing::{info, debug};
use uuid::Uuid;

/// Shadow Mode - Paper trading for testing new strategies without real money
pub struct ShadowMode {
    db_pool: Arc<PgPool>,
}

impl ShadowMode {
    pub fn new(db_pool: Arc<PgPool>) -> Self {
        Self { db_pool }
    }

    /// Initialize shadow mode tables
    pub async fn initialize(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS shadow_trades (
                id UUID PRIMARY KEY,
                trade_id UUID REFERENCES trades(id),
                market_id UUID NOT NULL,
                outcome_id TEXT NOT NULL,
                side TEXT NOT NULL,
                price NUMERIC(10, 4) NOT NULL,
                size NUMERIC(15, 4) NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL,
                strategy_id TEXT NOT NULL,
                hypothetical_pnl NUMERIC(15, 4),
                would_have_won BOOLEAN,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_shadow_strategy ON shadow_trades(strategy_id);
            CREATE INDEX IF NOT EXISTS idx_shadow_market ON shadow_trades(market_id);
            CREATE INDEX IF NOT EXISTS idx_shadow_time ON shadow_trades(timestamp);
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to create shadow mode tables")?;

        info!("Shadow mode tables initialized");
        Ok(())
    }

    /// Record a shadow trade (paper trade)
    pub async fn record_shadow_trade(&self, trade: &ShadowTrade) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO shadow_trades (
                id, trade_id, market_id, outcome_id, side, price, size,
                timestamp, strategy_id, hypothetical_pnl, would_have_won
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(trade.id)
        .bind(trade.trade_id)
        .bind(trade.market_id)
        .bind(&trade.outcome_id)
        .bind(match trade.side {
            OrderSide::Buy => "Buy",
            OrderSide::Sell => "Sell",
        })
        .bind(trade.price)
        .bind(trade.size)
        .bind(trade.timestamp)
        .bind(&trade.strategy_id)
        .bind(trade.hypothetical_pnl)
        .bind(trade.would_have_won)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to record shadow trade")?;

        debug!("Recorded shadow trade {} for strategy {}", trade.id, trade.strategy_id);
        Ok(())
    }

    /// Get all shadow trades for a strategy
    pub async fn get_strategy_shadow_trades(
        &self,
        strategy_id: &str,
        limit: usize,
    ) -> Result<Vec<ShadowTrade>> {
        let trades = sqlx::query_as::<_, ShadowTrade>(
            r#"
            SELECT
                id, trade_id, market_id, outcome_id, side,
                price, size, timestamp, strategy_id,
                hypothetical_pnl, would_have_won
            FROM shadow_trades
            WHERE strategy_id = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
        )
        .bind(strategy_id)
        .bind(limit as i64)
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to fetch shadow trades")?;

        Ok(trades)
    }

    /// Update shadow trade outcomes after market resolution
    pub async fn update_shadow_outcomes(
        &self,
        market_id: Uuid,
        winning_outcome: &str,
    ) -> Result<usize> {
        let rows = sqlx::query(
            r#"
            UPDATE shadow_trades
            SET would_have_won = CASE WHEN outcome_id = $2 THEN TRUE ELSE FALSE END,
                hypothetical_pnl = CASE
                    WHEN outcome_id = $2 THEN
                        CASE
                            WHEN side = 'Buy' THEN (1.0 - price) * size
                            ELSE price * size
                        END
                    ELSE
                        CASE
                            WHEN side = 'Buy' THEN -price * size
                            ELSE (1.0 - price) * size
                        END
                END
            WHERE market_id = $1 AND hypothetical_pnl IS NULL
            "#,
        )
        .bind(market_id)
        .bind(winning_outcome)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to update shadow outcomes")?;

        info!("Updated {} shadow trades for market {}", rows.rows_affected(), market_id);
        Ok(rows.rows_affected() as usize)
    }

    /// Calculate shadow mode performance
    pub async fn calculate_shadow_performance(
        &self,
        strategy_id: &str,
    ) -> Result<ShadowPerformance> {
        let row = sqlx::query_as::<_, (i64, i64, i64, f64, f64)>(
            r#"
            SELECT
                COUNT(*) as total_trades,
                COUNT(*) FILTER (WHERE would_have_won = TRUE) as winning_trades,
                COUNT(*) FILTER (WHERE would_have_won = FALSE) as losing_trades,
                COALESCE(SUM(hypothetical_pnl), 0.0) as total_pnl,
                COALESCE(AVG(hypothetical_pnl) FILTER (WHERE hypothetical_pnl > 0), 0.0) as avg_win
            FROM shadow_trades
            WHERE strategy_id = $1
            AND hypothetical_pnl IS NOT NULL
            "#,
        )
        .bind(strategy_id)
        .fetch_one(self.db_pool.as_ref())
        .await
        .context("Failed to calculate shadow performance")?;

        let hit_rate = if row.0 > 0 {
            (row.1 as f64 / row.0 as f64) * 100.0
        } else {
            0.0
        };

        let avg_loss = sqlx::query_scalar(
            r#"
            SELECT COALESCE(AVG(ABS(hypothetical_pnl)), 0.0)
            FROM shadow_trades
            WHERE strategy_id = $1 AND hypothetical_pnl < 0
            "#,
        )
        .bind(strategy_id)
        .fetch_one(self.db_pool.as_ref())
        .await?;

        Ok(ShadowPerformance {
            strategy_id: strategy_id.to_string(),
            total_trades: row.0,
            winning_trades: row.1,
            losing_trades: row.2,
            hit_rate,
            total_pnl: row.3,
            avg_win: row.4,
            avg_loss,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ShadowPerformance {
    pub strategy_id: String,
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub hit_rate: f64,
    pub total_pnl: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
}

/// Paper Trader - Executes trades in shadow mode
pub struct PaperTrader {
    db_pool: Arc<PgPool>,
    shadow_mode: ShadowMode,
}

impl PaperTrader {
    pub fn new(db_pool: Arc<PgPool>) -> Self {
        let shadow_mode = ShadowMode::new(db_pool.clone());
        Self { db_pool, shadow_mode }
    }

    /// Execute a paper trade
    pub async fn execute_paper_trade(
        &self,
        market_id: Uuid,
        outcome_id: &str,
        side: OrderSide,
        price: f64,
        size: f64,
        strategy_id: &str,
    ) -> Result<ShadowTrade> {
        let shadow_trade = ShadowTrade {
            id: Uuid::new_v4(),
            trade_id: None, // No real trade executed
            market_id,
            outcome_id: outcome_id.to_string(),
            side,
            price,
            size,
            timestamp: Utc::now(),
            strategy_id: strategy_id.to_string(),
            hypothetical_pnl: None, // Will be calculated on resolution
            would_have_won: None,
        };

        self.shadow_mode.record_shadow_trade(&shadow_trade).await?;

        info!("Executed paper trade {} for strategy {}", shadow_trade.id, strategy_id);
        Ok(shadow_trade)
    }

    /// Compare shadow vs real performance
    pub async fn compare_shadow_real(&self, strategy_id: &str) -> Result<ShadowRealComparison> {
        let shadow_perf = self.shadow_mode.calculate_shadow_performance(strategy_id).await?;

        // Get real performance
        let real_row = sqlx::query_as::<_, (i64, i64, i64, f64)>(
            r#"
            SELECT
                COUNT(*) as total_trades,
                COUNT(*) FILTER (WHERE pnl > 0) as winning_trades,
                COUNT(*) FILTER (WHERE pnl < 0) as losing_trades,
                COALESCE(SUM(pnl), 0.0) as total_pnl
            FROM trades t
            JOIN attributed_trades at ON t.id = at.trade_id
            WHERE at.strategy_id = $1
            "#,
        )
        .bind(strategy_id)
        .fetch_one(self.db_pool.as_ref())
        .await
        .context("Failed to get real performance")?;

        let real_hit_rate = if real_row.0 > 0 {
            (real_row.1 as f64 / real_row.0 as f64) * 100.0
        } else {
            0.0
        };

        // Calculate differences before moving shadow_perf
        let hit_rate_diff = shadow_perf.hit_rate - real_hit_rate;
        let pnl_diff = shadow_perf.total_pnl - real_row.3;

        Ok(ShadowRealComparison {
            strategy_id: strategy_id.to_string(),
            shadow_performance: shadow_perf,
            real_trades: real_row.0,
            real_hit_rate,
            real_pnl: real_row.3,
            hit_rate_diff,
            pnl_diff,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ShadowRealComparison {
    pub strategy_id: String,
    pub shadow_performance: ShadowPerformance,
    pub real_trades: i64,
    pub real_hit_rate: f64,
    pub real_pnl: f64,
    pub hit_rate_diff: f64,
    pub pnl_diff: f64,
}
