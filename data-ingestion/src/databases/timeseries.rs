use anyhow::Result;
use common::{OrderBook, PriceTick, Trade};
use sqlx::postgres::PgPool;
use tracing::info;

/// Time-series database for prices, volumes, and order book data
/// Uses TimescaleDB (PostgreSQL extension)
pub struct TimeSeriesDB {
    pool: PgPool,
}

impl TimeSeriesDB {
    pub async fn new(url: &str) -> Result<Self> {
        let pool = PgPool::connect(url).await?;

        info!("✅ Connected to TimescaleDB");

        Ok(Self { pool })
    }

    pub async fn init_tables(&self) -> Result<()> {
        // Create hypertable for price ticks
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS price_ticks (
                time TIMESTAMPTZ NOT NULL,
                market_id UUID NOT NULL,
                outcome_id TEXT NOT NULL,
                price DOUBLE PRECISION NOT NULL,
                volume_24h DOUBLE PRECISION NOT NULL,
                liquidity DOUBLE PRECISION NOT NULL
            );

            SELECT create_hypertable('price_ticks', 'time', if_not_exists => TRUE);
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create hypertable for trades
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS trades (
                time TIMESTAMPTZ NOT NULL,
                trade_id UUID NOT NULL,
                market_id UUID NOT NULL,
                outcome_id TEXT NOT NULL,
                price DOUBLE PRECISION NOT NULL,
                size DOUBLE PRECISION NOT NULL,
                side TEXT NOT NULL
            );

            SELECT create_hypertable('trades', 'time', if_not_exists => TRUE);
            "#,
        )
        .execute(&self.pool)
        .await?;

        info!("✅ Initialized time-series tables");

        Ok(())
    }

    pub async fn insert_price_tick(&self, tick: &PriceTick) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO price_ticks (time, market_id, outcome_id, price, volume_24h, liquidity)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(tick.timestamp)
        .bind(tick.market_id)
        .bind(&tick.outcome_id)
        .bind(tick.price)
        .bind(tick.volume_24h)
        .bind(tick.liquidity)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_trade(&self, trade: &Trade) -> Result<()> {
        let side = match trade.side {
            common::OrderSide::Buy => "buy",
            common::OrderSide::Sell => "sell",
        };

        sqlx::query(
            r#"
            INSERT INTO trades (time, trade_id, market_id, outcome_id, price, size, side)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(trade.timestamp)
        .bind(trade.id)
        .bind(trade.market_id)
        .bind(&trade.outcome_id)
        .bind(trade.price)
        .bind(trade.size)
        .bind(side)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
