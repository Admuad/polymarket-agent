use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use common::{AttributedTrade, OrderSide, PerformanceMetrics, Signal, StrategyPerformance, Trade};
use rust_decimal::prelude::*;
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

/// Attribution Engine - Maps trades to signals/agents and calculates P&L attribution
pub struct AttributionEngine {
    db_pool: Arc<PgPool>,
}

impl AttributionEngine {
    pub fn new(db_pool: Arc<PgPool>) -> Self {
        Self { db_pool }
    }

    /// Initialize attribution tables
    pub async fn initialize(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS signals (
                id UUID PRIMARY KEY,
                market_id UUID NOT NULL,
                outcome_id TEXT NOT NULL,
                predicted_probability NUMERIC(10, 6) NOT NULL,
                confidence NUMERIC(10, 6) NOT NULL,
                direction TEXT NOT NULL,
                agent_id UUID NOT NULL,
                strategy_id TEXT NOT NULL,
                generated_at TIMESTAMPTZ NOT NULL,
                metadata JSONB,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_signals_market ON signals(market_id);
            CREATE INDEX IF NOT EXISTS idx_signals_strategy ON signals(strategy_id);
            CREATE INDEX IF NOT EXISTS idx_signals_agent ON signals(agent_id);
            CREATE INDEX IF NOT EXISTS idx_signals_time ON signals(generated_at);

            CREATE TABLE IF NOT EXISTS attributed_trades (
                trade_id UUID PRIMARY KEY REFERENCES trades(id),
                signal_id UUID REFERENCES signals(id),
                agent_id UUID NOT NULL,
                strategy_id TEXT NOT NULL,
                pnl NUMERIC(15, 4),
                pnl_percent NUMERIC(10, 4),
                attributed_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_attributed_signal ON attributed_trades(signal_id);
            CREATE INDEX IF NOT EXISTS idx_attributed_strategy ON attributed_trades(strategy_id);
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to create attribution tables")?;

        info!("Attribution tables initialized");
        Ok(())
    }

    /// Store a generated signal
    pub async fn store_signal(&self, signal: &Signal) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO signals (
                id, market_id, outcome_id, predicted_probability, confidence,
                direction, agent_id, strategy_id, generated_at, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(signal.id)
        .bind(signal.market_id)
        .bind(&signal.outcome_id)
        .bind(signal.predicted_probability)
        .bind(signal.confidence)
        .bind(match signal.direction {
            OrderSide::Buy => "Buy",
            OrderSide::Sell => "Sell",
        })
        .bind(signal.agent_id)
        .bind(&signal.strategy_id)
        .bind(signal.generated_at)
        .bind(&signal.metadata)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to store signal")?;

        debug!("Stored signal {} for market {}", signal.id, signal.market_id);
        Ok(())
    }

    /// Attribute a trade to a signal
    pub async fn attribute_trade(
        &self,
        trade_id: Uuid,
        signal_id: Uuid,
        agent_id: Uuid,
        strategy_id: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO attributed_trades (trade_id, signal_id, agent_id, strategy_id)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(trade_id)
        .bind(signal_id)
        .bind(agent_id)
        .bind(strategy_id)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to attribute trade")?;

        debug!("Attributed trade {} to signal {}", trade_id, signal_id);
        Ok(())
    }

    /// Get all trades attributed to a strategy
    pub async fn get_strategy_trades(
        &self,
        strategy_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<AttributedTrade>> {
        let trades = sqlx::query_as::<_, AttributedTrade>(
            r#"
            SELECT
                t.id as trade_id,
                t.market_id,
                t.outcome_id,
                t.side,
                t.price as entry_price,
                t.size,
                t.timestamp as entry_time,
                NULL::TIMESTAMPTZ as exit_time,
                s.id as signal_id,
                s.agent_id,
                s.strategy_id,
                t.pnl,
                t.pnl_percent
            FROM trades t
            JOIN attributed_trades at ON t.id = at.trade_id
            JOIN signals s ON at.signal_id = s.id
            WHERE s.strategy_id = $1
            AND t.timestamp >= $2
            AND t.timestamp <= $3
            ORDER BY t.timestamp DESC
            "#,
        )
        .bind(strategy_id)
        .bind(from)
        .bind(to)
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to fetch strategy trades")?;

        Ok(trades)
    }

    /// Get all trades attributed to an agent
    pub async fn get_agent_trades(
        &self,
        agent_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<AttributedTrade>> {
        let trades = sqlx::query_as::<_, AttributedTrade>(
            r#"
            SELECT
                t.id as trade_id,
                t.market_id,
                t.outcome_id,
                t.side,
                t.price as entry_price,
                t.size,
                t.timestamp as entry_time,
                NULL::TIMESTAMPTZ as exit_time,
                s.id as signal_id,
                s.agent_id,
                s.strategy_id,
                t.pnl,
                t.pnl_percent
            FROM trades t
            JOIN attributed_trades at ON t.id = at.trade_id
            JOIN signals s ON at.signal_id = s.id
            WHERE s.agent_id = $1
            AND t.timestamp >= $2
            AND t.timestamp <= $3
            ORDER BY t.timestamp DESC
            "#,
        )
        .bind(agent_id)
        .bind(from)
        .bind(to)
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to fetch agent trades")?;

        Ok(trades)
    }

    /// Calculate P&L attribution by strategy
    pub async fn calculate_strategy_pnl(
        &self,
        strategy_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<PnlAttribution> {
        let row = sqlx::query_as::<_, (f64, i64, i64, i64, f64, f64, f64, f64)>(
            r#"
            SELECT
                COALESCE(SUM(t.pnl), 0.0) as total_pnl,
                COUNT(*) as total_trades,
                COUNT(*) FILTER (WHERE t.pnl > 0) as winning_trades,
                COUNT(*) FILTER (WHERE t.pnl < 0) as losing_trades,
                COALESCE(SUM(t.pnl) FILTER (WHERE t.pnl > 0), 0.0) as total_wins,
                COALESCE(SUM(ABS(t.pnl)) FILTER (WHERE t.pnl < 0), 0.0) as total_losses,
                COALESCE(AVG(t.pnl) FILTER (WHERE t.pnl > 0), 0.0) as avg_win,
                COALESCE(AVG(ABS(t.pnl)) FILTER (WHERE t.pnl < 0), 0.0) as avg_loss
            FROM trades t
            JOIN attributed_trades at ON t.id = at.trade_id
            WHERE at.strategy_id = $1
            AND t.timestamp >= $2
            AND t.timestamp <= $3
            "#,
        )
        .bind(strategy_id)
        .bind(from)
        .bind(to)
        .fetch_one(self.db_pool.as_ref())
        .await
        .context("Failed to calculate strategy P&L")?;

        let hit_rate = if row.1 > 0 {
            (row.2 as f64 / row.1 as f64) * 100.0
        } else {
            0.0
        };

        let profit_factor = if row.5 > 0.0 { row.4 / row.5 } else { 0.0 };

        let roi = if row.4 + row.5 > 0.0 {
            (row.0 / (row.4 + row.5)) * 100.0
        } else {
            0.0
        };

        Ok(PnlAttribution {
            strategy_id: strategy_id.to_string(),
            period: (from, to),
            total_pnl: row.0,
            total_trades: row.1,
            winning_trades: row.2,
            losing_trades: row.3,
            hit_rate,
            total_wins: row.4,
            total_losses: row.5,
            avg_win: row.6,
            avg_loss: row.7,
            profit_factor,
            roi,
        })
    }

    /// Get top performing strategies by P&L
    pub async fn get_top_strategies(
        &self,
        limit: usize,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<StrategyPerformance>> {
        let strategies = sqlx::query_as::<_, StrategyPerformance>(
            r#"
            SELECT
                at.strategy_id,
                COUNT(*) as total_trades,
                COALESCE(SUM(t.pnl), 0.0) as total_pnl,
                COALESCE(SUM(t.pnl) FILTER (WHERE t.pnl > 0), 0.0) as gross_profit,
                COALESCE(SUM(ABS(t.pnl)) FILTER (WHERE t.pnl < 0), 0.0) as gross_loss,
                COALESCE(AVG(t.pnl_percent), 0.0) as avg_return_pct
            FROM trades t
            JOIN attributed_trades at ON t.id = at.trade_id
            WHERE t.timestamp >= $1 AND t.timestamp <= $2
            GROUP BY at.strategy_id
            ORDER BY total_pnl DESC
            LIMIT $3
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(limit as i64)
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to get top strategies")?;

        Ok(strategies)
    }

    /// Get signals that led to winning vs losing trades
    pub async fn analyze_signal_outcomes(
        &self,
        strategy_id: &str,
    ) -> Result<SignalOutcomeAnalysis> {
        let row = sqlx::query_as::<_, (i64, f64, f64, i64, f64, f64)>(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE t.pnl > 0) as win_count,
                COALESCE(AVG(s.predicted_probability) FILTER (WHERE t.pnl > 0), 0.0) as win_avg_confidence,
                COALESCE(AVG(s.confidence) FILTER (WHERE t.pnl > 0), 0.0) as win_avg_model_conf,
                COUNT(*) FILTER (WHERE t.pnl <= 0) as loss_count,
                COALESCE(AVG(s.predicted_probability) FILTER (WHERE t.pnl <= 0), 0.0) as loss_avg_confidence,
                COALESCE(AVG(s.confidence) FILTER (WHERE t.pnl <= 0), 0.0) as loss_avg_model_conf
            FROM trades t
            JOIN attributed_trades at ON t.id = at.trade_id
            JOIN signals s ON at.signal_id = s.id
            WHERE at.strategy_id = $1
            "#,
        )
        .bind(strategy_id)
        .fetch_one(self.db_pool.as_ref())
        .await
        .context("Failed to analyze signal outcomes")?;

        Ok(SignalOutcomeAnalysis {
            win_count: row.0,
            win_avg_probability: row.1,
            win_avg_confidence: row.2,
            loss_count: row.3,
            loss_avg_probability: row.4,
            loss_avg_confidence: row.5,
        })
    }
}

/// P&L attribution for a strategy
#[derive(Debug, Clone)]
pub struct PnlAttribution {
    pub strategy_id: String,
    pub period: (DateTime<Utc>, DateTime<Utc>),
    pub total_pnl: f64,
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub hit_rate: f64,
    pub total_wins: f64,
    pub total_losses: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub profit_factor: f64,
    pub roi: f64,
}

/// Analysis of signal outcomes
#[derive(Debug, Clone)]
pub struct SignalOutcomeAnalysis {
    pub win_count: i64,
    pub win_avg_probability: f64,
    pub win_avg_confidence: f64,
    pub loss_count: i64,
    pub loss_avg_probability: f64,
    pub loss_avg_confidence: f64,
}
