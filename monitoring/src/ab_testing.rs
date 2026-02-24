use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use common::{AbTest, AbTestResult, AbTestStatus, PerformanceMetrics};
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn, debug};
use uuid::Uuid;

/// A/B Test Manager - Manages strategy comparison tests
pub struct AbTestManager {
    db_pool: Arc<PgPool>,
}

impl AbTestManager {
    pub fn new(db_pool: Arc<PgPool>) -> Self {
        Self { db_pool }
    }

    /// Initialize A/B test tables
    pub async fn initialize(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ab_tests (
                id UUID PRIMARY KEY,
                name TEXT NOT NULL,
                strategy_a TEXT NOT NULL,
                strategy_b TEXT NOT NULL,
                start_time TIMESTAMPTZ NOT NULL,
                end_time TIMESTAMPTZ,
                status TEXT NOT NULL,
                allocation_ratio NUMERIC(4, 2) NOT NULL,
                min_sample_size BIGINT NOT NULL,
                statistical_significance NUMERIC(4, 2) NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS ab_test_assignments (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                test_id UUID NOT NULL REFERENCES ab_tests(id),
                market_id UUID NOT NULL,
                assigned_strategy TEXT NOT NULL,
                assigned_at TIMESTAMPTZ NOT NULL,
                UNIQUE(test_id, market_id)
            );

            CREATE TABLE IF NOT EXISTS ab_test_results (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                test_id UUID NOT NULL REFERENCES ab_tests(id),
                strategy_metrics_a JSONB NOT NULL,
                strategy_metrics_b JSONB NOT NULL,
                winner TEXT,
                confidence NUMERIC(4, 2),
                p_value NUMERIC(10, 6),
                recommendation TEXT NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                UNIQUE(test_id)
            );

            CREATE INDEX IF NOT EXISTS idx_abtest_status ON ab_tests(status);
            CREATE INDEX IF NOT EXISTS idx_abtest_assignment ON ab_test_assignments(test_id);
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to create A/B test tables")?;

        info!("A/B test tables initialized");
        Ok(())
    }

    /// Create a new A/B test
    pub async fn create_test(
        &self,
        test: AbTest,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO ab_tests (
                id, name, strategy_a, strategy_b, start_time,
                end_time, status, allocation_ratio, min_sample_size, statistical_significance
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(test.id)
        .bind(&test.name)
        .bind(&test.strategy_a)
        .bind(&test.strategy_b)
        .bind(test.start_time)
        .bind(test.end_time)
        .bind(match test.status {
            AbTestStatus::Running => "Running",
            AbTestStatus::Paused => "Paused",
            AbTestStatus::Completed => "Completed",
            AbTestStatus::Inconclusive => "Inconclusive",
        })
        .bind(test.allocation_ratio)
        .bind(test.min_sample_size)
        .bind(test.statistical_significance)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to create A/B test")?;

        info!("Created A/B test: {} ({} vs {})", test.name, test.strategy_a, test.strategy_b);
        Ok(())
    }

    /// Assign a market to a strategy in an A/B test
    pub async fn assign_market(
        &self,
        test_id: Uuid,
        market_id: Uuid,
        strategy: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO ab_test_assignments (test_id, market_id, assigned_strategy, assigned_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (test_id, market_id) DO NOTHING
            "#,
        )
        .bind(test_id)
        .bind(market_id)
        .bind(strategy)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to assign market")?;

        debug!("Assigned market {} to strategy {} in test {}", market_id, strategy, test_id);
        Ok(())
    }

    /// Get which strategy was assigned to a market
    pub async fn get_assignment(
        &self,
        test_id: Uuid,
        market_id: Uuid,
    ) -> Result<Option<String>> {
        let strategy = sqlx::query_scalar(
            "SELECT assigned_strategy FROM ab_test_assignments WHERE test_id = $1 AND market_id = $2"
        )
        .bind(test_id)
        .bind(market_id)
        .fetch_optional(self.db_pool.as_ref())
        .await
        .context("Failed to get assignment")?;

        Ok(strategy)
    }

    /// Pause a running test
    pub async fn pause_test(&self, test_id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE ab_tests SET status = 'Paused', end_time = NOW() WHERE id = $1"
        )
        .bind(test_id)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to pause test")?;

        info!("Paused A/B test {}", test_id);
        Ok(())
    }

    /// Resume a paused test
    pub async fn resume_test(&self, test_id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE ab_tests SET status = 'Running', end_time = NULL WHERE id = $1"
        )
        .bind(test_id)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to resume test")?;

        info!("Resumed A/B test {}", test_id);
        Ok(())
    }

    /// Get all running tests
    pub async fn get_running_tests(&self) -> Result<Vec<AbTest>> {
        let tests = sqlx::query_as::<_, AbTest>(
            "SELECT * FROM ab_tests WHERE status = 'Running'"
        )
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to fetch running tests")?;

        Ok(tests)
    }

    /// Get assignment counts for a test
    pub async fn get_assignment_counts(&self, test_id: Uuid) -> Result<AssignmentCounts> {
        let counts = sqlx::query_as::<_, (i64, i64)>(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE assigned_strategy = $2) as count_a,
                COUNT(*) FILTER (WHERE assigned_strategy = $3) as count_b
            FROM ab_test_assignments
            WHERE test_id = $1
            "#,
        )
        .bind(test_id)
        .bind(self.get_strategy_a(test_id).await?.unwrap_or_default())
        .bind(self.get_strategy_b(test_id).await?.unwrap_or_default())
        .fetch_one(self.db_pool.as_ref())
        .await
        .context("Failed to get assignment counts")?;

        Ok(AssignmentCounts {
            test_id,
            count_a: counts.0,
            count_b: counts.1,
        })
    }

    async fn get_strategy_a(&self, test_id: Uuid) -> Result<Option<String>> {
        sqlx::query_scalar("SELECT strategy_a FROM ab_tests WHERE id = $1")
            .bind(test_id)
            .fetch_optional(self.db_pool.as_ref())
            .await
            .context("Failed to get strategy A")
    }

    async fn get_strategy_b(&self, test_id: Uuid) -> Result<Option<String>> {
        sqlx::query_scalar("SELECT strategy_b FROM ab_tests WHERE id = $1")
            .bind(test_id)
            .fetch_optional(self.db_pool.as_ref())
            .await
            .context("Failed to get strategy B")
    }

    /// Complete a test and store results
    pub async fn complete_test(&self, test_id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE ab_tests SET status = 'Completed', end_time = NOW() WHERE id = $1"
        )
        .bind(test_id)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to complete test")?;

        info!("Completed A/B test {}", test_id);
        Ok(())
    }
}

/// A/B Test Engine - Statistical analysis for comparing strategies
pub struct AbTestEngine {
    db_pool: Arc<PgPool>,
    manager: AbTestManager,
}

impl AbTestEngine {
    pub fn new(db_pool: Arc<PgPool>) -> Self {
        let manager = AbTestManager::new(db_pool.clone());
        Self { db_pool, manager }
    }

    /// Analyze an A/B test and generate results
    pub async fn analyze_test(&self, test_id: Uuid) -> Result<AbTestResult> {
        let test = sqlx::query_as::<_, AbTest>(
            "SELECT * FROM ab_tests WHERE id = $1"
        )
        .bind(test_id)
        .fetch_one(self.db_pool.as_ref())
        .await
        .context("Test not found")?;

        // Get metrics for both strategies
        let metrics_a = self
            .calculate_test_metrics(&test.strategy_a, test.start_time, test.end_time.unwrap_or(Utc::now()))
            .await?;

        let metrics_b = self
            .calculate_test_metrics(&test.strategy_b, test.start_time, test.end_time.unwrap_or(Utc::now()))
            .await?;

        // Perform statistical tests
        let (winner, confidence, p_value) = self
            .perform_t_test(&metrics_a, &metrics_b)
            .await?;

        let recommendation = self.generate_recommendation(&metrics_a, &metrics_b, winner.as_deref());

        let result = AbTestResult {
            test_id,
            strategy_a_metrics: metrics_a,
            strategy_b_metrics: metrics_b,
            winner,
            confidence,
            p_value,
            recommendation,
        };

        // Store result
        self.store_result(&result).await?;

        Ok(result)
    }

    /// Calculate metrics for a strategy within a test period
    async fn calculate_test_metrics(
        &self,
        strategy_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<PerformanceMetrics> {
        let row = sqlx::query_as::<_, (i64, i64, i64, f64, f64, f64)>(
            r#"
            SELECT
                COUNT(*) as total_trades,
                COUNT(*) FILTER (WHERE t.pnl > 0) as winning_trades,
                COUNT(*) FILTER (WHERE t.pnl < 0) as losing_trades,
                COALESCE(SUM(t.pnl), 0.0) as total_pnl,
                COALESCE(AVG(t.pnl_percent), 0.0) as avg_return,
                COALESCE(STDDEV(t.pnl_percent), 0.0) as std_return
            FROM trades t
            JOIN attributed_trades at ON t.id = at.trade_id
            WHERE at.strategy_id = $1
            AND t.timestamp >= $2 AND t.timestamp <= $3
            "#,
        )
        .bind(strategy_id)
        .bind(from)
        .bind(to)
        .fetch_one(self.db_pool.as_ref())
        .await
        .context("Failed to calculate test metrics")?;

        let hit_rate = if row.0 > 0 { (row.1 as f64 / row.0 as f64) * 100.0 } else { 0.0 };
        let roi = if row.0 > 0 { (row.3 / (row.0 as f64 * 100.0)) * 100.0 } else { 0.0 };

        Ok(PerformanceMetrics {
            strategy_id: strategy_id.to_string(),
            period_start: from,
            period_end: to,
            total_trades: row.0,
            winning_trades: row.1,
            losing_trades: row.2,
            hit_rate,
            total_pnl: row.3,
            roi,
            sharpe_ratio: None,
            max_drawdown: 0.0,
            avg_win: 0.0,
            avg_loss: 0.0,
            profit_factor: 0.0,
            calmar_ratio: None,
        })
    }

    /// Perform t-test to compare strategies
    async fn perform_t_test(
        &self,
        metrics_a: &PerformanceMetrics,
        metrics_b: &PerformanceMetrics,
    ) -> Result<(Option<String>, Option<f64>, Option<f64>)> {
        if metrics_a.total_trades < 10 || metrics_b.total_trades < 10 {
            return Ok((None, None, None)); // Not enough data
        }

        // Simple t-test on P&L per trade
        let mean_a = metrics_a.total_pnl / metrics_a.total_trades as f64;
        let mean_b = metrics_b.total_pnl / metrics_b.total_trades as f64;

        let var_a = (metrics_a.total_pnl / metrics_a.total_trades as f64 - mean_a).powi(2);
        let var_b = (metrics_b.total_pnl / metrics_b.total_trades as f64 - mean_b).powi(2);

        let n_a = metrics_a.total_trades as f64;
        let n_b = metrics_b.total_trades as f64;

        // Pooled variance
        let pooled_var = ((n_a - 1.0) * var_a + (n_b - 1.0) * var_b) / (n_a + n_b - 2.0);
        let std_err = (pooled_var / n_a + pooled_var / n_b).sqrt();

        if std_err == 0.0 {
            return Ok((None, None, None));
        }

        let t_stat = (mean_a - mean_b) / std_err;
        let df = (n_a + n_b - 2.0) as i32;

        // Simplified p-value calculation (would use proper t-distribution in production)
        let p_value = 2.0 * (1.0 - self.approx_cdf(t_stat.abs()));

        let confidence = 1.0 - p_value;

        let winner = if confidence > 0.95 {
            if mean_a > mean_b {
                Some("A".to_string())
            } else {
                Some("B".to_string())
            }
        } else {
            None
        };

        Ok((winner, Some(confidence), Some(p_value)))
    }

    /// Approximate CDF for t-distribution (simplified)
    fn approx_cdf(&self, x: f64) -> f64 {
        // Approximation of standard normal CDF
        let a1 = 0.254829592;
        let a2 = -0.284496736;
        let a3 = 1.421413741;
        let a4 = -1.453152027;
        let a5 = 1.061405429;
        let p = 0.3275911;

        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();

        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-0.5 * x * x).exp();

        0.5 * (1.0 + sign * y)
    }

    /// Generate a recommendation based on test results
    fn generate_recommendation(
        &self,
        metrics_a: &PerformanceMetrics,
        metrics_b: &PerformanceMetrics,
        winner: Option<&str>,
    ) -> String {
        match winner.as_deref() {
            Some("A") => format!(
                "Strategy {} significantly outperforms {} (ΔP&L: ${:.2}). Consider promoting {}.",
                metrics_a.strategy_id,
                metrics_b.strategy_id,
                metrics_a.total_pnl - metrics_b.total_pnl,
                metrics_a.strategy_id
            ),
            Some("B") => format!(
                "Strategy {} significantly outperforms {} (ΔP&L: ${:.2}). Consider promoting {}.",
                metrics_b.strategy_id,
                metrics_a.strategy_id,
                metrics_b.total_pnl - metrics_a.total_pnl,
                metrics_b.strategy_id
            ),
            _ => {
                if (metrics_a.total_pnl - metrics_b.total_pnl).abs() < 10.0 {
                    format!(
                        "No significant difference between {} and {}. Both strategies perform similarly. Either can be kept or deprecated.",
                        metrics_a.strategy_id, metrics_b.strategy_id
                    )
                } else {
                    format!(
                        "Inconclusive result. {} has higher P&L but not statistically significant. Continue test or increase sample size.",
                        if metrics_a.total_pnl > metrics_b.total_pnl { &metrics_a.strategy_id } else { &metrics_b.strategy_id }
                    )
                }
            }
        }
    }

    /// Store test result
    async fn store_result(&self, result: &AbTestResult) -> Result<()> {
        let metrics_a_json = serde_json::to_string(&result.strategy_a_metrics)?;
        let metrics_b_json = serde_json::to_string(&result.strategy_b_metrics)?;

        sqlx::query(
            r#"
            INSERT INTO ab_test_results (
                test_id, strategy_metrics_a, strategy_metrics_b,
                winner, confidence, p_value, recommendation
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (test_id) DO UPDATE SET
                strategy_metrics_a = EXCLUDED.strategy_metrics_a,
                strategy_metrics_b = EXCLUDED.strategy_metrics_b,
                winner = EXCLUDED.winner,
                confidence = EXCLUDED.confidence,
                p_value = EXCLUDED.p_value,
                recommendation = EXCLUDED.recommendation
            "#,
        )
        .bind(result.test_id)
        .bind(metrics_a_json)
        .bind(metrics_b_json)
        .bind(&result.winner)
        .bind(result.confidence)
        .bind(result.p_value)
        .bind(&result.recommendation)
        .execute(self.db_pool.as_ref())
        .await
        .context("Failed to store test result")?;

        info!("Stored result for A/B test {}", result.test_id);
        Ok(())
    }

    /// Check if a test has enough samples to be meaningful
    pub async fn check_sample_size(&self, test_id: Uuid) -> Result<bool> {
        let test = sqlx::query_as::<_, AbTest>("SELECT * FROM ab_tests WHERE id = $1")
            .bind(test_id)
            .fetch_optional(self.db_pool.as_ref())
            .await?
            .context("Test not found")?;

        let counts = self.manager.get_assignment_counts(test_id).await?;

        Ok(counts.count_a >= test.min_sample_size && counts.count_b >= test.min_sample_size)
    }
}

#[derive(Debug, Clone)]
pub struct AssignmentCounts {
    pub test_id: Uuid,
    pub count_a: i64,
    pub count_b: i64,
}
