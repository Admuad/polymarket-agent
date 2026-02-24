use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use common::{DriftDetection, DriftSeverity, DriftType, PerformanceMetrics};
use sqlx::postgres::PgPool;
use std::sync::Arc;
use tracing::{warn, info};
use uuid::Uuid;

/// Configuration for drift detection
#[derive(Debug, Clone)]
pub struct DriftDetectionConfig {
    /// Window size for comparing recent vs historical performance
    pub window_hours: i64,
    /// Threshold for P&L decline (%)
    pub pnl_decline_threshold: f64,
    /// Threshold for hit rate decline (%)
    pub hit_rate_decline_threshold: f64,
    /// Threshold for Brier score increase
    pub brier_score_increase_threshold: f64,
    /// Threshold for volume decline (%)
    pub volume_decline_threshold: f64,
}

impl Default for DriftDetectionConfig {
    fn default() -> Self {
        Self {
            window_hours: 24,
            pnl_decline_threshold: 20.0,
            hit_rate_decline_threshold: 10.0,
            brier_score_increase_threshold: 0.05,
            volume_decline_threshold: 30.0,
        }
    }
}

/// Drift Detector - Monitors for performance and prediction drift
pub struct DriftDetector {
    db_pool: Arc<PgPool>,
    config: DriftDetectionConfig,
}

impl DriftDetector {
    pub fn new(db_pool: Arc<PgPool>, config: DriftDetectionConfig) -> Self {
        Self { db_pool, config }
    }

    pub fn new_with_defaults(db_pool: Arc<PgPool>) -> Self {
        Self::new(db_pool, DriftDetectionConfig::default())
    }

    /// Initialize drift detection tables
    pub async fn initialize(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS drift_alerts (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                strategy_id TEXT NOT NULL,
                detected_at TIMESTAMPTZ NOT NULL,
                drift_type TEXT NOT NULL,
                severity TEXT NOT NULL,
                metric_value NUMERIC(10, 6) NOT NULL,
                threshold NUMERIC(10, 6) NOT NULL,
                description TEXT NOT NULL,
                acknowledged BOOLEAN DEFAULT FALSE,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_drift_strategy ON drift_alerts(strategy_id);
            CREATE INDEX IF NOT EXISTS idx_drift_time ON drift_alerts(detected_at);
            CREATE INDEX IF NOT EXISTS idx_drift_severity ON drift_alerts(severity);
            CREATE INDEX IF NOT EXISTS idx_drift_acknowledged ON drift_alerts(acknowledged);
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to create drift alert tables")?;

        info!("Drift detection tables initialized");
        Ok(())
    }

    /// Check all strategies for drift
    pub async fn check_all_strategies(&self) -> Result<Vec<DriftDetection>> {
        let strategies: Vec<String> = sqlx::query_scalar("SELECT DISTINCT strategy_id FROM attributed_trades")
            .fetch_all(self.db_pool.as_ref())
            .await
            .context("Failed to fetch strategies")?;

        let mut all_drifts = Vec::new();

        for strategy_id in strategies {
            if let Ok(drifts) = self.check_strategy_drift(&strategy_id).await {
                all_drifts.extend(drifts);
            }
        }

        Ok(all_drifts)
    }

    /// Check a specific strategy for drift
    pub async fn check_strategy_drift(&self, strategy_id: &str) -> Result<Vec<DriftDetection>> {
        let mut drifts = Vec::new();

        // Check performance drift
        if let Some(drift) = self
            .check_performance_drift(strategy_id)
            .await?
        {
            drifts.push(drift);
        }

        // Check prediction drift (Brier score)
        if let Some(drift) = self
            .check_prediction_drift(strategy_id)
            .await?
        {
            drifts.push(drift);
        }

        // Check volume drift
        if let Some(drift) = self
            .check_volume_drift(strategy_id)
            .await?
        {
            drifts.push(drift);
        }

        // Store alerts
        for drift in &drifts {
            self.store_drift_alert(drift).await?;
        }

        Ok(drifts)
    }

    /// Check for performance drift (P&L and hit rate decline)
    async fn check_performance_drift(&self, strategy_id: &str) -> Result<Option<DriftDetection>> {
        let now = Utc::now();
        let recent_start = now - Duration::hours(self.config.window_hours);
        let historical_start = now - Duration::hours(self.config.window_hours * 3);
        let historical_end = recent_start;

        // Get recent metrics
        let recent = sqlx::query_as::<_, (f64, i64)>(
            r#"
            SELECT COALESCE(SUM(t.pnl), 0.0),
                   COUNT(*)
            FROM trades t
            JOIN attributed_trades at ON t.id = at.trade_id
            WHERE at.strategy_id = $1
            AND t.timestamp >= $2 AND t.timestamp <= $3
            "#,
        )
        .bind(strategy_id)
        .bind(recent_start)
        .bind(now)
        .fetch_optional(self.db_pool.as_ref())
        .await?
        .unwrap_or((0.0, 0));

        // Get historical metrics
        let historical = sqlx::query_as::<_, (f64, i64)>(
            r#"
            SELECT COALESCE(SUM(t.pnl), 0.0),
                   COUNT(*)
            FROM trades t
            JOIN attributed_trades at ON t.id = at.trade_id
            WHERE at.strategy_id = $1
            AND t.timestamp >= $2 AND t.timestamp <= $3
            "#,
        )
        .bind(strategy_id)
        .bind(historical_start)
        .bind(historical_end)
        .fetch_optional(self.db_pool.as_ref())
        .await?
        .unwrap_or((0.0, 0));

        if historical.1 == 0 {
            return Ok(None); // Not enough historical data
        }

        // Calculate P&L per trade
        let recent_pnl_per_trade = if recent.1 > 0 { recent.0 / recent.1 as f64 } else { 0.0 };
        let historical_pnl_per_trade = historical.0 / historical.1 as f64;

        // Check for significant decline
        if historical_pnl_per_trade > 0.0 {
            let decline_pct =
                ((historical_pnl_per_trade - recent_pnl_per_trade) / historical_pnl_per_trade.abs()) * 100.0;

            if decline_pct > self.config.pnl_decline_threshold {
                let severity = if decline_pct > 50.0 {
                    DriftSeverity::Critical
                } else if decline_pct > 30.0 {
                    DriftSeverity::High
                } else {
                    DriftSeverity::Medium
                };

                return Ok(Some(DriftDetection {
                    strategy_id: strategy_id.to_string(),
                    detected_at: now,
                    drift_type: DriftType::PerformanceDrift,
                    severity,
                    metric_value: decline_pct,
                    threshold: self.config.pnl_decline_threshold,
                    description: format!(
                        "P&L per trade declined {:.1}% (historical: ${:.2}/trade -> recent: ${:.2}/trade)",
                        decline_pct, historical_pnl_per_trade, recent_pnl_per_trade
                    ),
                }));
            }
        }

        Ok(None)
    }

    /// Check for prediction drift (Brier score increase)
    async fn check_prediction_drift(&self, strategy_id: &str) -> Result<Option<DriftDetection>> {
        let now = Utc::now();
        let recent_start = now - Duration::hours(self.config.window_hours);
        let historical_start = now - Duration::hours(self.config.window_hours * 3);

        let recent_brier = sqlx::query_scalar::<_, f64>(
            r#"
            SELECT AVG((predicted_probability - actual_outcome)^2)
            FROM predictions
            WHERE strategy_id = $1
            AND timestamp >= $2 AND timestamp <= $3
            AND actual_outcome IS NOT NULL
            "#,
        )
        .bind(strategy_id)
        .bind(recent_start)
        .bind(now)
        .fetch_optional(self.db_pool.as_ref())
        .await?
        .unwrap_or(0.0);

        let historical_brier = sqlx::query_scalar::<_, f64>(
            r#"
            SELECT AVG((predicted_probability - actual_outcome)^2)
            FROM predictions
            WHERE strategy_id = $1
            AND timestamp >= $2 AND timestamp < $3
            AND actual_outcome IS NOT NULL
            "#,
        )
        .bind(strategy_id)
        .bind(historical_start)
        .bind(recent_start)
        .fetch_optional(self.db_pool.as_ref())
        .await?
        .unwrap_or(0.0);

        if historical_brier > 0.0 {
            let increase = recent_brier - historical_brier;

            if increase > self.config.brier_score_increase_threshold {
                let severity = if increase > 0.2 {
                    DriftSeverity::Critical
                } else if increase > 0.1 {
                    DriftSeverity::High
                } else {
                    DriftSeverity::Medium
                };

                return Ok(Some(DriftDetection {
                    strategy_id: strategy_id.to_string(),
                    detected_at: now,
                    drift_type: DriftType::PredictionDrift,
                    severity,
                    metric_value: increase,
                    threshold: self.config.brier_score_increase_threshold,
                    description: format!(
                        "Brier score increased by {:.4} (historical: {:.4} -> recent: {:.4})",
                        increase, historical_brier, recent_brier
                    ),
                }));
            }
        }

        Ok(None)
    }

    /// Check for volume drift (trading activity decline)
    async fn check_volume_drift(&self, strategy_id: &str) -> Result<Option<DriftDetection>> {
        let now = Utc::now();
        let recent_start = now - Duration::hours(self.config.window_hours);
        let historical_start = now - Duration::hours(self.config.window_hours * 3);
        let historical_end = recent_start;

        let recent_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM trades t
            JOIN attributed_trades at ON t.id = at.trade_id
            WHERE at.strategy_id = $1
            AND t.timestamp >= $2 AND t.timestamp <= $3
            "#,
        )
        .bind(strategy_id)
        .bind(recent_start)
        .bind(now)
        .fetch_one(self.db_pool.as_ref())
        .await?;

        let historical_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM trades t
            JOIN attributed_trades at ON t.id = at.trade_id
            WHERE at.strategy_id = $1
            AND t.timestamp >= $2 AND t.timestamp <= $3
            "#,
        )
        .bind(strategy_id)
        .bind(historical_start)
        .bind(historical_end)
        .fetch_one(self.db_pool.as_ref())
        .await?;

        if historical_count > 0 {
            let decline_pct =
                ((historical_count - recent_count) as f64 / historical_count as f64) * 100.0;

            if decline_pct > self.config.volume_decline_threshold {
                let severity = if decline_pct > 50.0 {
                    DriftSeverity::High
                } else {
                    DriftSeverity::Medium
                };

                return Ok(Some(DriftDetection {
                    strategy_id: strategy_id.to_string(),
                    detected_at: now,
                    drift_type: DriftType::VolumeDrift,
                    severity,
                    metric_value: decline_pct,
                    threshold: self.config.volume_decline_threshold,
                    description: format!(
                        "Trading volume declined {:.1}% (historical: {} trades -> recent: {} trades)",
                        decline_pct, historical_count, recent_count
                    ),
                }));
            }
        }

        Ok(None)
    }

    /// Store drift alert
    async fn store_drift_alert(&self, drift: &DriftDetection) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO drift_alerts (
                strategy_id, detected_at, drift_type, severity,
                metric_value, threshold, description
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&drift.strategy_id)
        .bind(drift.detected_at)
        .bind(match drift.drift_type {
            DriftType::PerformanceDrift => "PerformanceDrift",
            DriftType::PredictionDrift => "PredictionDrift",
            DriftType::MarketStructureDrift => "MarketStructureDrift",
            DriftType::VolumeDrift => "VolumeDrift",
            DriftType::CalibrationDrift => "CalibrationDrift",
        })
        .bind(match drift.severity {
            DriftSeverity::Low => "Low",
            DriftSeverity::Medium => "Medium",
            DriftSeverity::High => "High",
            DriftSeverity::Critical => "Critical",
        })
        .bind(drift.metric_value)
        .bind(drift.threshold)
        .bind(&drift.description)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to store drift alert")?;

        warn!("Drift detected: {}", drift.description);
        Ok(())
    }

    /// Get unacknowledged drift alerts
    pub async fn get_unacknowledged_alerts(&self) -> Result<Vec<DriftDetection>> {
        let alerts = sqlx::query_as::<_, DriftDetection>(
            r#"
            SELECT
                strategy_id, detected_at, drift_type, severity,
                metric_value, threshold, description
            FROM drift_alerts
            WHERE acknowledged = FALSE
            ORDER BY detected_at DESC
            "#,
        )
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to fetch unacknowledged alerts")?;

        Ok(alerts)
    }

    /// Acknowledge a drift alert
    pub async fn acknowledge_alert(&self, strategy_id: &str, detected_at: DateTime<Utc>) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE drift_alerts
            SET acknowledged = TRUE
            WHERE strategy_id = $1 AND detected_at = $2
            "#,
        )
        .bind(strategy_id)
        .bind(detected_at)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to acknowledge alert")?;

        info!("Acknowledged drift alert for strategy {}", strategy_id);
        Ok(())
    }
}
