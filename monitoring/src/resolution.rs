use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use common::{Market, MarketEvent, Resolution, ResolutionStatus, Trade};
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Resolution Monitor - Tracks market resolutions and updates trade outcomes
pub struct ResolutionMonitor {
    db_pool: Arc<PgPool>,
    // In-memory cache of active trades awaiting resolution
    pending_trades: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>, // market_id -> trade_ids
    // Cache of resolutions
    resolutions: Arc<RwLock<HashMap<Uuid, Resolution>>>, // market_id -> Resolution
}

impl ResolutionMonitor {
    pub async fn new(db_pool: Arc<PgPool>) -> Result<Self> {
        Ok(Self {
            db_pool,
            pending_trades: Arc::new(RwLock::new(HashMap::new())),
            resolutions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Initialize the resolution tracking tables
    pub async fn initialize(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS resolutions (
                market_id UUID PRIMARY KEY,
                outcome_id TEXT NOT NULL,
                status TEXT NOT NULL,
                resolved_at TIMESTAMPTZ,
                resolution_price NUMERIC(10, 4),
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_resolutions_status ON resolutions(status);
            CREATE INDEX IF NOT EXISTS idx_resolutions_resolved_at ON resolutions(resolved_at);
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to create resolutions table")?;

        info!("Resolution tracking tables initialized");
        Ok(())
    }

    /// Process a market event and update resolutions if needed
    pub async fn process_event(&self, event: &MarketEvent) -> Result<()> {
        match event {
            MarketEvent::MarketResolved { market_id, outcome_id } => {
                self.handle_market_resolution(*market_id, outcome_id.clone()).await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle a market resolution event
    async fn handle_market_resolution(&self, market_id: Uuid, outcome_id: String) -> Result<()> {
        debug!("Market resolved: {} -> {}", market_id, outcome_id);

        // Create resolution record
        let resolution = Resolution {
            market_id,
            outcome_id: outcome_id.clone(),
            status: ResolutionStatus::Resolved,
            resolved_at: Some(Utc::now()),
            resolution_price: None, // Will be set based on outcome
        };

        // Store in database
        sqlx::query(
            r#"
            INSERT INTO resolutions (market_id, outcome_id, status, resolved_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (market_id) DO UPDATE
            SET outcome_id = EXCLUDED.outcome_id,
                status = EXCLUDED.status,
                resolved_at = EXCLUDED.resolved_at,
                updated_at = NOW()
            "#,
        )
        .bind(resolution.market_id)
        .bind(&resolution.outcome_id)
        .bind(&resolution.status)
        .bind(resolution.resolved_at)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to store resolution")?;

        // Update in-memory cache
        self.resolutions.write().await.insert(market_id, resolution.clone());

        // Update all pending trades for this market
        self.update_trades_for_resolution(market_id, &outcome_id).await?;

        info!("Successfully processed resolution for market {}", market_id);
        Ok(())
    }

    /// Update all trades for a resolved market
    async fn update_trades_for_resolution(&self, market_id: Uuid, winning_outcome: &str) -> Result<()> {
        // Fetch all trades for this market that haven't been resolved yet
        let trades = sqlx::query_as::<_, (Uuid, Uuid, String, bool, f64, DateTime<Utc>)>(
            r#"
            SELECT id, market_id, outcome_id, side, price, timestamp
            FROM trades
            WHERE market_id = $1 AND pnl IS NULL
            "#,
        )
        .bind(market_id)
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to fetch trades for resolution")?;

        let trades_count = trades.len();
        let mut total_pnl = 0.0;
        let mut winning_count = 0;
        let mut losing_count = 0;

        for (trade_id, _, outcome_id, side, price, timestamp) in trades {
            let did_win = outcome_id == winning_outcome;
            let (pnl, pnl_percent) = self.calculate_pnl(did_win, side, price, 1.0)?;

            // Update the trade
            sqlx::query(
                r#"
                UPDATE trades
                SET pnl = $1, pnl_percent = $2, updated_at = NOW()
                WHERE id = $3
                "#,
            )
            .bind(pnl)
            .bind(pnl_percent)
            .bind(trade_id)
            .execute(self.db_pool.as_ref())
            .await
            .context("Failed to update trade P&L")?;

            total_pnl += pnl;
            if did_win {
                winning_count += 1;
            } else {
                losing_count += 1;
            }
        }

        info!(
            "Resolved {} trades for market {}: {} wins, {} losses, total P&L: ${:.2}",
            trades_count,
            market_id,
            winning_count,
            losing_count,
            total_pnl
        );

        Ok(())
    }

    /// Calculate P&L for a trade
    fn calculate_pnl(
        &self,
        did_win: bool,
        side: bool, // true = Buy, false = Sell
        price: f64,
        size: f64,
    ) -> Result<(f64, f64)> {
        let pnl = if did_win {
            if side {
                // Bought YES, won: payout = (1.0 / price) * size * 1.0
                // P&L = payout - cost
                let cost = price * size;
                let payout = size * 1.0; // Full payout
                payout - cost
            } else {
                // Sold YES, lost: payout = 0, keep premium
                let premium = (1.0 - price) * size;
                premium
            }
        } else {
            if side {
                // Bought YES, lost: payout = 0
                -(price * size)
            } else {
                // Sold YES, won: keep full premium
                (1.0 - price) * size
            }
        };

        let cost = if side { price * size } else { (1.0 - price) * size };
        let pnl_percent = if cost > 0.0 { (pnl / cost) * 100.0 } else { 0.0 };

        Ok((pnl, pnl_percent))
    }

    /// Get resolution for a market
    pub async fn get_resolution(&self, market_id: Uuid) -> Option<Resolution> {
        // Check cache first
        if let Some(resolution) = self.resolutions.read().await.get(&market_id) {
            return Some(resolution.clone());
        }

        // Query database
        match sqlx::query_as::<_, Resolution>(
            "SELECT market_id, outcome_id, status, resolved_at, resolution_price FROM resolutions WHERE market_id = $1"
        )
        .bind(market_id)
        .fetch_optional(self.db_pool.as_ref())
        .await
        {
            Ok(Some(resolution)) => {
                self.resolutions.write().await.insert(market_id, resolution.clone());
                Some(resolution)
            }
            _ => None,
        }
    }

    /// Get all pending resolutions
    pub async fn get_pending_resolutions(&self) -> Result<Vec<Resolution>> {
        let resolutions = sqlx::query_as::<_, Resolution>(
            "SELECT market_id, outcome_id, status, resolved_at, resolution_price FROM resolutions WHERE status = 'Pending'"
        )
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to fetch pending resolutions")?;

        Ok(resolutions)
    }

    /// Check for markets that should be resolved but aren't
    pub async fn check_stale_resolutions(&self, stale_threshold_hours: i64) -> Result<Vec<Uuid>> {
        let threshold = Utc::now() - chrono::Duration::hours(stale_threshold_hours);

        let stale_markets = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT DISTINCT m.id
            FROM markets m
            LEFT JOIN resolutions r ON m.id = r.market_id
            WHERE m.end_time < $1
            AND (r.status = 'Pending' OR r.status IS NULL)
            "#,
        )
        .bind(threshold)
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to check stale resolutions")?;

        if !stale_markets.is_empty() {
            warn!("Found {} markets past end time without resolution", stale_markets.len());
        }

        Ok(stale_markets)
    }
}

/// Resolution Tracker - Maintains the lifecycle of market resolutions
pub struct ResolutionTracker {
    db_pool: Arc<PgPool>,
}

impl ResolutionTracker {
    pub fn new(db_pool: Arc<PgPool>) -> Self {
        Self { db_pool }
    }

    /// Track a market as pending resolution
    pub async fn track_market(&self, market_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO resolutions (market_id, outcome_id, status, resolved_at)
            VALUES ($1, '', 'Pending', NULL)
            ON CONFLICT (market_id) DO NOTHING
            "#,
        )
        .bind(market_id)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to track market")?;

        Ok(())
    }

    /// Get resolution statistics
    pub async fn get_resolution_stats(&self) -> Result<ResolutionStats> {
        let row = sqlx::query_as::<_, (i64, i64, i64, f64)>(
            r#"
            SELECT
                COUNT(*) as total,
                COUNT(*) FILTER (WHERE status = 'Resolved') as resolved,
                COUNT(*) FILTER (WHERE status = 'Pending') as pending,
                COALESCE(AVG(EXTRACT(EPOCH FROM (resolved_at - created_at))/3600.0), 0.0) as avg_hours_to_resolve
            FROM resolutions
            "#,
        )
        .fetch_one(self.db_pool.as_ref())
        .await
        .context("Failed to get resolution stats")?;

        Ok(ResolutionStats {
            total_markets: row.0,
            resolved_markets: row.1,
            pending_markets: row.2,
            avg_hours_to_resolve: row.3,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ResolutionStats {
    pub total_markets: i64,
    pub resolved_markets: i64,
    pub pending_markets: i64,
    pub avg_hours_to_resolve: f64,
}
