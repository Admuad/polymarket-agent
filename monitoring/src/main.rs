use anyhow::Result;
use chrono::{Duration, Utc};
use common::{AbTest, AbTestStatus, OrderSide, ResolutionStatus};
use monitoring::{
    AbTestEngine, AbTestManager, AttributionEngine, CalibrationEngine, DriftDetector,
    DriftDetectionConfig, MetricsCalculator, PaperTrader, ResolutionMonitor, ShadowMode,
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::fmt;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    fmt().with_max_level(Level::INFO).init();

    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/polymarket".to_string());

    let pool = Arc::new(
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?,
    );

    info!("Monitoring System Example");
    info!("==========================");

    // Example 1: Resolution Monitoring
    info!("\n=== Example 1: Resolution Monitoring ===");
    let resolution_monitor = ResolutionMonitor::new(pool.clone()).await?;
    resolution_monitor.initialize().await?;

    // Simulate a market resolution
    let market_id = Uuid::new_v4();
    info!("Simulating resolution for market {}", market_id);

    // Example 2: Attribution
    info!("\n=== Example 2: Attribution Engine ===");
    let attribution = AttributionEngine::new(pool.clone());
    attribution.initialize().await?;

    // Calculate P&L attribution for a strategy
    let strategy_id = "sentiment-v1";
    let from = Utc::now() - Duration::days(7);
    let to = Utc::now();

    if let Ok(attribution_result) = attribution.calculate_strategy_pnl(strategy_id, from, to).await {
        info!("Strategy {} P&L Attribution:", strategy_id);
        info!("  Total P&L: ${:.2}", attribution_result.total_pnl);
        info!("  Hit Rate: {:.2}%", attribution_result.hit_rate);
        info!("  Win/Loss: {}/{}", attribution_result.winning_trades, attribution_result.losing_trades);
        info!("  Profit Factor: {:.2}", attribution_result.profit_factor);
    }

    // Example 3: Metrics Calculation
    info!("\n=== Example 3: Performance Metrics ===");
    let metrics_calc = MetricsCalculator::new(pool.clone(), 0.02);
    metrics_calc.initialize().await?;

    if let Ok(metrics) = metrics_calc.calculate_strategy_metrics(strategy_id, from, to).await {
        info!("Strategy {} Performance Metrics:", strategy_id);
        info!("  Total Trades: {}", metrics.total_trades);
        info!("  Hit Rate: {:.2}%", metrics.hit_rate);
        info!("  Total P&L: ${:.2}", metrics.total_pnl);
        info!("  ROI: {:.2}%", metrics.roi);
        info!("  Sharpe Ratio: {:?}", metrics.sharpe_ratio);
        info!("  Max Drawdown: {:.2}%", metrics.max_drawdown);
        info!("  Profit Factor: {:.2}", metrics.profit_factor);
    }

    // Example 4: Calibration (Brier Score)
    info!("\n=== Example 4: Calibration Analysis ===");
    let calibration = CalibrationEngine::new(pool.clone());
    calibration.initialize().await?;

    if let Ok(calib_metrics) = calibration.calculate_calibration(strategy_id, from, to).await {
        info!("Strategy {} Calibration:", strategy_id);
        info!("  Brier Score: {:.4}", calib_metrics.brier_score);
        info!("  Log Loss: {:.4}", calib_metrics.log_loss);
        info!("  Calibration Error: {:.4}", calib_metrics.calibration_error);
        info!("  Total Predictions: {}", calib_metrics.total_predictions);

        for bucket in &calib_metrics.confidence_buckets {
            info!(
                "  [{:.1}-{:.1}): {} predictions, predicted: {:.2}%, actual: {:.2}%, error: {:.4}",
                bucket.min_confidence,
                bucket.max_confidence,
                bucket.count,
                bucket.avg_predicted_prob * 100.0,
                bucket.actual_outcome_rate * 100.0,
                bucket.calibration_error
            );
        }
    }

    // Example 5: Drift Detection
    info!("\n=== Example 5: Drift Detection ===");
    let drift_config = DriftDetectionConfig {
        window_hours: 24,
        pnl_decline_threshold: 15.0,
        hit_rate_decline_threshold: 10.0,
        brier_score_increase_threshold: 0.05,
        volume_decline_threshold: 25.0,
    };

    let drift_detector = DriftDetector::new(pool.clone(), drift_config);
    drift_detector.initialize().await?;

    // Check for drift in all strategies
    let drifts = drift_detector.check_all_strategies().await?;
    if drifts.is_empty() {
        info!("No drift detected across all strategies");
    } else {
        info!("Drift detected in {} strategies:", drifts.len());
        for drift in &drifts {
            info!(
                "  [{}] {}: {}",
                drift.severity, drift.strategy_id, drift.description
            );
        }
    }

    // Example 6: A/B Testing
    info!("\n=== Example 6: A/B Testing ===");
    let ab_test_manager = AbTestManager::new(pool.clone());
    ab_test_manager.initialize().await?;

    let ab_engine = AbTestEngine::new(pool.clone());

    // Create a test
    let test_id = Uuid::new_v4();
    let test = AbTest {
        id: test_id,
        name: "Sentiment-v1 vs Sentiment-v2".to_string(),
        strategy_a: "sentiment-v1".to_string(),
        strategy_b: "sentiment-v2".to_string(),
        start_time: Utc::now() - Duration::days(7),
        end_time: None,
        status: AbTestStatus::Running,
        allocation_ratio: 0.5,
        min_sample_size: 50,
        statistical_significance: 0.95,
    };

    ab_test_manager.create_test(test).await?;
    info!("Created A/B test: sentiment-v1 vs sentiment-v2");

    // Analyze test (if we have data)
    if let Ok(result) = ab_engine.analyze_test(test_id).await {
        info!("A/B Test Results:");
        info!("  Strategy A P&L: ${:.2}", result.strategy_a_metrics.total_pnl);
        info!("  Strategy B P&L: ${:.2}", result.strategy_b_metrics.total_pnl);
        info!("  Winner: {:?}", result.winner);
        info!("  Confidence: {:?}%", result.confidence.map(|c| c * 100.0));
        info!("  Recommendation: {}", result.recommendation);
    }

    // Example 7: Shadow Mode (Paper Trading)
    info!("\n=== Example 7: Shadow Mode ===");
    let shadow_mode = ShadowMode::new(pool.clone());
    shadow_mode.initialize().await?;

    let paper_trader = PaperTrader::new(pool.clone());

    // Execute a paper trade
    let shadow_market_id = Uuid::new_v4();
    let shadow_trade = paper_trader
        .execute_paper_trade(
            shadow_market_id,
            "YES",
            OrderSide::Buy,
            0.65,
            100.0,
            "sentiment-v2",
        )
        .await?;

    info!("Executed paper trade {}", shadow_trade.id);

    // Calculate shadow performance
    if let Ok(perf) = shadow_mode.calculate_shadow_performance("sentiment-v2").await {
        info!("Shadow Mode Performance (sentiment-v2):");
        info!("  Total Trades: {}", perf.total_trades);
        info!("  Hit Rate: {:.2}%", perf.hit_rate);
        info!("  Hypothetical P&L: ${:.2}", perf.total_pnl);
    }

    // Compare shadow vs real
    if let Ok(comparison) = paper_trader.compare_shadow_real("sentiment-v2").await {
        info!("Shadow vs Real Comparison:");
        info!("  Shadow Hit Rate: {:.2}%", comparison.shadow_performance.hit_rate);
        info!("  Real Hit Rate: {:.2}%", comparison.real_hit_rate);
        info!("  Hit Rate Diff: {:.2}%", comparison.hit_rate_diff);
        info!("  Shadow P&L: ${:.2}", comparison.shadow_performance.total_pnl);
        info!("  Real P&L: ${:.2}", comparison.real_pnl);
        info!("  P&L Diff: ${:.2}", comparison.pnl_diff);
    }

    info!("\n=== Monitoring System Examples Complete ===");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_brier_score_calculation() {
        // Test Brier score calculation
        // Perfect predictions should have Brier score of 0
        let predictions = vec![
            (1.0, 1), // Predicted 1.0, actual 1
            (0.0, 0), // Predicted 0.0, actual 0
            (1.0, 1),
            (0.0, 0),
        ];

        let brier = monitoring::BrierScoreCalculator::calculate(&predictions).unwrap();
        assert!(brier.abs() < 0.001, "Perfect predictions should have Brier score near 0");

        // Random predictions should have higher Brier score
        let random_predictions = vec![
            (0.5, 1),
            (0.7, 0),
            (0.3, 1),
            (0.6, 0),
        ];

        let random_brier = monitoring::BrierScoreCalculator::calculate(&random_predictions).unwrap();
        assert!(random_brier > brier, "Random predictions should have higher Brier score");
    }

    #[test]
    fn test_drift_severity() {
        let config = DriftDetectionConfig::default();

        assert!(config.pnl_decline_threshold > 0.0);
        assert!(config.brier_score_increase_threshold > 0.0);
        assert!(config.volume_decline_threshold > 0.0);
    }
}
