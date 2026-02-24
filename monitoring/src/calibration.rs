use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use common::{CalibrationMetrics, ConfidenceBucket, Resolution};
use sqlx::postgres::PgPool;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Calibration Engine - Tracks prediction accuracy using Brier scores and calibration analysis
pub struct CalibrationEngine {
    db_pool: Arc<PgPool>,
}

impl CalibrationEngine {
    pub fn new(db_pool: Arc<PgPool>) -> Self {
        Self { db_pool }
    }

    /// Initialize calibration tables
    pub async fn initialize(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS predictions (
                id UUID PRIMARY KEY,
                signal_id UUID NOT NULL,
                strategy_id TEXT NOT NULL,
                market_id UUID NOT NULL,
                outcome_id TEXT NOT NULL,
                predicted_probability NUMERIC(10, 6) NOT NULL,
                actual_outcome NUMERIC(1, 0), -- 0 or 1
                timestamp TIMESTAMPTZ NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_predictions_strategy ON predictions(strategy_id);
            CREATE INDEX IF NOT EXISTS idx_predictions_market ON predictions(market_id);
            CREATE INDEX IF NOT EXISTS idx_predictions_timestamp ON predictions(timestamp);
            CREATE INDEX IF NOT EXISTS idx_predictions_outcome ON predictions(actual_outcome) WHERE actual_outcome IS NOT NULL;

            CREATE TABLE IF NOT EXISTS calibration_metrics (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                strategy_id TEXT NOT NULL,
                period_start TIMESTAMPTZ NOT NULL,
                period_end TIMESTAMPTZ NOT NULL,
                brier_score NUMERIC(10, 6) NOT NULL,
                log_loss NUMERIC(10, 6) NOT NULL,
                calibration_error NUMERIC(10, 6) NOT NULL,
                total_predictions BIGINT NOT NULL,
                confidence_buckets JSONB NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                UNIQUE(strategy_id, period_start, period_end)
            );

            CREATE INDEX IF NOT EXISTS idx_calibration_strategy ON calibration_metrics(strategy_id);
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to create calibration tables")?;

        info!("Calibration tables initialized");
        Ok(())
    }

    /// Record a prediction
    pub async fn record_prediction(
        &self,
        prediction_id: Uuid,
        signal_id: Uuid,
        strategy_id: &str,
        market_id: Uuid,
        outcome_id: &str,
        predicted_probability: f64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO predictions (id, signal_id, strategy_id, market_id, outcome_id, predicted_probability, timestamp)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            "#,
        )
        .bind(prediction_id)
        .bind(signal_id)
        .bind(strategy_id)
        .bind(market_id)
        .bind(outcome_id)
        .bind(predicted_probability)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to record prediction")?;

        debug!("Recorded prediction {} for strategy {}", prediction_id, strategy_id);
        Ok(())
    }

    /// Update prediction with actual outcome after resolution
    pub async fn update_prediction_outcome(
        &self,
        market_id: Uuid,
        winning_outcome: &str,
    ) -> Result<usize> {
        let result = sqlx::query(
            r#"
            UPDATE predictions
            SET actual_outcome = CASE WHEN outcome_id = $1 THEN 1 ELSE 0 END
            WHERE market_id = $2 AND actual_outcome IS NULL
            "#,
        )
        .bind(winning_outcome)
        .bind(market_id)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to update prediction outcomes")?;

        info!("Updated {} predictions for resolved market {}", result.rows_affected(), market_id);
        Ok(result.rows_affected() as usize)
    }

    /// Calculate calibration metrics for a strategy
    pub async fn calculate_calibration(
        &self,
        strategy_id: &str,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Result<CalibrationMetrics> {
        let predictions = sqlx::query_as::<_, (f64, i32)>(
            r#"
            SELECT predicted_probability, actual_outcome
            FROM predictions
            WHERE strategy_id = $1
            AND timestamp >= $2
            AND timestamp <= $3
            AND actual_outcome IS NOT NULL
            "#,
        )
        .bind(strategy_id)
        .bind(period_start)
        .bind(period_end)
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to fetch predictions")?;

        if predictions.is_empty() {
            return Ok(CalibrationMetrics {
                strategy_id: strategy_id.to_string(),
                period_start,
                period_end,
                brier_score: 0.0,
                log_loss: 0.0,
                calibration_error: 0.0,
                total_predictions: 0,
                confidence_buckets: vec![],
            });
        }

        let brier_score = BrierScoreCalculator::calculate(&predictions)?;
        let log_loss = self.calculate_log_loss(&predictions)?;
        let calibration_error = self.calculate_calibration_error(&predictions)?;
        let confidence_buckets = self.create_confidence_buckets(&predictions)?;

        let metrics = CalibrationMetrics {
            strategy_id: strategy_id.to_string(),
            period_start,
            period_end,
            brier_score,
            log_loss,
            calibration_error,
            total_predictions: predictions.len() as i64,
            confidence_buckets,
        };

        self.store_calibration_metrics(&metrics).await?;

        Ok(metrics)
    }

    /// Calculate log loss
    fn calculate_log_loss(&self, predictions: &[(f64, i32)]) -> Result<f64> {
        let mut log_loss_sum = 0.0;

        for (pred, actual) in predictions {
            let actual_f = *actual as f64;
            // Add epsilon to avoid log(0)
            let eps = 1e-10;
            let prob = pred.clamp(eps, 1.0 - eps);
            log_loss_sum -= actual_f * prob.ln() + (1.0 - actual_f) * (1.0 - prob).ln();
        }

        Ok(log_loss_sum / predictions.len() as f64)
    }

    /// Calculate calibration error (Expected Calibration Error)
    fn calculate_calibration_error(&self, predictions: &[(f64, i32)]) -> Result<f64> {
        let buckets = self.create_confidence_buckets(predictions)?;
        let mut total_error = 0.0;
        let mut total_weight = 0.0;

        for bucket in &buckets {
            if bucket.count > 0 {
                let weight = bucket.count as f64;
                total_error += bucket.calibration_error.abs() * weight;
                total_weight += weight;
            }
        }

        Ok(if total_weight > 0.0 { total_error / total_weight } else { 0.0 })
    }

    /// Create confidence buckets for calibration analysis
    fn create_confidence_buckets(&self, predictions: &[(f64, i32)]) -> Result<Vec<ConfidenceBucket>> {
        let mut buckets = Vec::with_capacity(10);

        for i in 0..10 {
            let min_conf = i as f64 / 10.0;
            let max_conf = (i + 1) as f64 / 10.0;

            let bucket_preds: Vec<&(f64, i32)> = predictions
                .iter()
                .filter(|(p, _)| *p >= min_conf && *p < max_conf)
                .collect();

            if bucket_preds.is_empty() {
                continue;
            }

            let count = bucket_preds.len() as i64;
            let avg_predicted_prob = bucket_preds.iter().map(|(p, _)| p).sum::<f64>() / count as f64;
            let actual_outcome_rate = bucket_preds.iter().map(|(_, a)| *a as f64).sum::<f64>() / count as f64;
            let calibration_error = (avg_predicted_prob - actual_outcome_rate).abs();

            buckets.push(ConfidenceBucket {
                min_confidence: min_conf,
                max_confidence: max_conf,
                count,
                avg_predicted_prob,
                actual_outcome_rate,
                calibration_error,
            });
        }

        Ok(buckets)
    }

    /// Store calibration metrics
    async fn store_calibration_metrics(&self, metrics: &CalibrationMetrics) -> Result<()> {
        let buckets_json = serde_json::to_string(&metrics.confidence_buckets)?;

        sqlx::query(
            r#"
            INSERT INTO calibration_metrics (
                strategy_id, period_start, period_end,
                brier_score, log_loss, calibration_error,
                total_predictions, confidence_buckets
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (strategy_id, period_start, period_end)
            DO UPDATE SET
                brier_score = EXCLUDED.brier_score,
                log_loss = EXCLUDED.log_loss,
                calibration_error = EXCLUDED.calibration_error,
                total_predictions = EXCLUDED.total_predictions,
                confidence_buckets = EXCLUDED.confidence_buckets
            "#,
        )
        .bind(&metrics.strategy_id)
        .bind(metrics.period_start)
        .bind(metrics.period_end)
        .bind(metrics.brier_score)
        .bind(metrics.log_loss)
        .bind(metrics.calibration_error)
        .bind(metrics.total_predictions)
        .bind(buckets_json)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to store calibration metrics")?;

        Ok(())
    }

    /// Check if a strategy is becoming uncalibrated
    pub async fn detect_calibration_drift(
        &self,
        strategy_id: &str,
        recent_days: i64,
        threshold: f64,
    ) -> Result<bool> {
        let recent = Utc::now() - chrono::Duration::days(recent_days);
        let historical = Utc::now() - chrono::Duration::days(recent_days * 3);

        let recent_metrics = self
            .calculate_calibration(strategy_id, recent, Utc::now())
            .await?;

        let historical_metrics = self
            .calculate_calibration(strategy_id, historical, recent)
            .await?;

        if historical_metrics.total_predictions < 10 {
            warn!("Not enough historical data for calibration drift detection");
            return Ok(false);
        }

        let drift = recent_metrics.brier_score - historical_metrics.brier_score;
        let is_drifted = drift > threshold;

        if is_drifted {
            warn!(
                "Calibration drift detected for {}: {:.4} -> {:.4}",
                strategy_id,
                historical_metrics.brier_score,
                recent_metrics.brier_score
            );
        }

        Ok(is_drifted)
    }
}

/// Brier Score Calculator
pub struct BrierScoreCalculator;

impl BrierScoreCalculator {
    /// Calculate Brier score for a set of predictions
    pub fn calculate(predictions: &[(f64, i32)]) -> Result<f64> {
        if predictions.is_empty() {
            return Ok(0.0);
        }

        let sum: f64 = predictions
            .iter()
            .map(|(prob, actual)| {
                let actual_f = *actual as f64;
                (prob - actual_f).powi(2)
            })
            .sum();

        Ok(sum / predictions.len() as f64)
    }

    /// Decompose Brier score into reliability, resolution, and uncertainty
    pub fn decompose(predictions: &[(f64, i32)]) -> Result<BrierDecomposition> {
        if predictions.is_empty() {
            return Ok(BrierDecomposition {
                brier_score: 0.0,
                reliability: 0.0,
                resolution: 0.0,
                uncertainty: 0.0,
            });
        }

        let brier_score = Self::calculate(predictions)?;

        // Calculate base rate (average outcome)
        let base_rate: f64 = predictions.iter().map(|(_, a)| *a as f64).sum::<f64>() / predictions.len() as f64;
        let uncertainty = base_rate * (1.0 - base_rate);

        // Group by predicted probability
        let mut groups: std::collections::HashMap<i32, Vec<f64>> = std::collections::HashMap::new();
        for (prob, actual) in predictions {
            let rounded_prob = (prob * 10.0) as i32; // Round to nearest 0.1
            groups
                .entry(rounded_prob)
                .or_insert_with(Vec::new)
                .push(*actual as f64);
        }

        // Calculate reliability
        let mut reliability = 0.0;
        let mut total_count = 0.0;

        for (rounded_prob, outcomes) in &groups {
            let count = outcomes.len() as f64;
            let observed_freq = outcomes.iter().sum::<f64>() / count;
            let predicted_prob = *rounded_prob as f64 / 10.0;
            reliability += count * (observed_freq - predicted_prob).powi(2);
            total_count += count;
        }
        reliability /= total_count;

        // Resolution
        let resolution = uncertainty - reliability;

        Ok(BrierDecomposition {
            brier_score,
            reliability,
            resolution,
            uncertainty,
        })
    }
}

#[derive(Debug, Clone)]
pub struct BrierDecomposition {
    pub brier_score: f64,
    pub reliability: f64,
    pub resolution: f64,
    pub uncertainty: f64,
}
