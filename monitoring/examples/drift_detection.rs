use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use common::{DriftDetection, DriftSeverity, DriftType, PerformanceMetrics};
use monitoring::{DriftDetector, DriftDetectionConfig};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing::{error, info, warn};
use tracing_subscriber;
use uuid::Uuid;

/// Comprehensive Drift Detection Example

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/polymarket".to_string());

    let pool = Arc::new(
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?,
    );

    info!("=== Drift Detection Example ===\n");

    // Initialize drift detector with custom config
    let config = DriftDetectionConfig {
        window_hours: 24,
        pnl_decline_threshold: 20.0,   // Alert if P&L drops 20%
        hit_rate_decline_threshold: 10.0,
        brier_score_increase_threshold: 0.05,  // Alert if Brier score increases by 0.05
        volume_decline_threshold: 30.0,
    };

    let drift_detector = DriftDetector::new(pool.clone(), config);
    drift_detector.initialize().await?;

    // Example 1: Check all strategies
    info!("Example 1: Checking all strategies for drift");
    info!("-----------------------------------------------");
    match drift_detector.check_all_strategies().await {
        Ok(drifts) => {
            if drifts.is_empty() {
                info!("✓ No drift detected across all strategies\n");
            } else {
                warn!("⚠️ Drift detected in {} strategies:", drifts.len());
                for drift in &drifts {
                    print_drift_alert(drift);
                }
                println!();
            }
        }
        Err(e) => {
            error!("Error checking strategies: {}", e);
        }
    }

    // Example 2: Check specific strategy
    info!("Example 2: Checking specific strategy for drift");
    info!("-----------------------------------------------");
    let strategy_id = "sentiment-v1";
    match drift_detector.check_strategy_drift(strategy_id).await {
        Ok(drifts) => {
            if drifts.is_empty() {
                info!("✓ Strategy '{}' has no drift detected\n", strategy_id);
            } else {
                warn!("⚠️ Strategy '{}' has {} drift alerts:", strategy_id, drifts.len());
                for drift in &drifts {
                    print_drift_alert(drift);
                }
                println!();
            }
        }
        Err(e) => {
            error!("Error checking strategy '{}': {}", strategy_id, e);
        }
    }

    // Example 3: Get unacknowledged alerts
    info!("Example 3: Fetching unacknowledged alerts");
    info!("-----------------------------------------------");
    match drift_detector.get_unacknowledged_alerts().await {
        Ok(alerts) => {
            if alerts.is_empty() {
                info!("✓ No unacknowledged alerts\n");
            } else {
                info!("Found {} unacknowledged alerts:", alerts.len());
                for (i, alert) in alerts.iter().enumerate() {
                    println!("  [{}] {} - {}", i + 1, alert.strategy_id, alert.description);
                }
                println!();
            }
        }
        Err(e) => {
            error!("Error fetching alerts: {}", e);
        }
    }

    // Example 4: Simulate drift scenarios
    info!("Example 4: Simulated drift scenarios");
    info!("-----------------------------------------------");

    // Scenario 1: Performance drift (severe P&L decline)
    simulate_performance_drift();

    // Scenario 2: Prediction drift (Brier score degradation)
    simulate_prediction_drift();

    // Scenario 3: Volume drift (trading activity drop)
    simulate_volume_drift();

    // Example 5: Acknowledging alerts
    info!("\nExample 5: Acknowledging alerts");
    info!("-----------------------------------------------");
    // In a real scenario, you would acknowledge alerts after investigation
    // drift_detector.acknowledge_alert("sentiment-v1", detection_time).await?;

    info!("✓ Drift detection example complete!");
    Ok(())
}

/// Print a formatted drift alert
fn print_drift_alert(drift: &DriftDetection) {
    let severity_icon = match drift.severity {
        DriftSeverity::Critical => "🔴",
        DriftSeverity::High => "🟠",
        DriftSeverity::Medium => "🟡",
        DriftSeverity::Low => "🟢",
    };

    let drift_type = match drift.drift_type {
        DriftType::PerformanceDrift => "Performance",
        DriftType::PredictionDrift => "Prediction",
        DriftType::MarketStructureDrift => "Market Structure",
        DriftType::VolumeDrift => "Volume",
        DriftType::CalibrationDrift => "Calibration",
    };

    println!("  {} [{}] {}", severity_icon, drift_type, drift.strategy_id);
    println!("     ├─ Severity: {:?}", drift.severity);
    println!("     ├─ Metric Value: {:.4}", drift.metric_value);
    println!("     ├─ Threshold: {:.4}", drift.threshold);
    println!("     ├─ Detected At: {}", drift.detected_at.format("%Y-%m-%d %H:%M:%S UTC"));
    println!("     └─ {}", drift.description);
    println!();
}

/// Simulate a performance drift scenario
fn simulate_performance_drift() {
    info!("Scenario 1: Performance Drift");
    info!("  Historical (last 72h): P&L = $1250 over 100 trades = $12.50/trade");
    info!("  Recent (last 24h):   P&L = $400 over 50 trades = $8.00/trade");
    info!("  Decline: 36.0% (threshold: 20.0)");
    info!("  Result: 🔴 HIGH SEVERITY - P&L per trade declined 36.0%");

    println!("  Recommendation:");
    println!("    1. Investigate recent market conditions");
    println!("    2. Check if prediction quality degraded");
    println!("    3. Consider reducing position sizes temporarily");
    println!("    4. Review recent signal performance");
}

/// Simulate a prediction drift scenario
fn simulate_prediction_drift() {
    info!("\nScenario 2: Prediction Drift");
    info!("  Historical (last 72h): Brier Score = 0.08 (excellent predictions)");
    info!("  Recent (last 24h):   Brier Score = 0.22 (moderate predictions)");
    info!("  Increase: 0.14 (threshold: 0.05)");
    info!("  Result: 🔴 CRITICAL SEVERITY - Brier score increased by 0.14");

    println!("  Analysis:");
    println!("    • Model is becoming uncalibrated");
    println!("    • Confidence levels may not match actual accuracy");
    println!("    • Possible causes: market regime change, data drift");

    println!("  Recommendations:");
    println!("    1. Re-train or fine-tune the model");
    println!("    2. Review feature importance for recent changes");
    println!("    3. Consider switching to backup strategy");
    println!("    4. Increase shadow mode testing");
}

/// Simulate a volume drift scenario
fn simulate_volume_drift() {
    info!("\nScenario 3: Volume Drift");
    info!("  Historical (last 72h): 120 trades over 3 days = 40 trades/day");
    info!("  Recent (last 24h):   22 trades = 22 trades/day");
    info!("  Decline: 45.0% (threshold: 30.0%)");
    info!("  Result: 🟠 HIGH SEVERITY - Trading volume declined 45.0%");

    println!("  Possible Causes:");
    println!("    • Market liquidity dropped");
    println!("    • Risk filters became more restrictive");
    println!("    • Strategy signal generation slowed");

    println!("  Recommendations:");
    println!("    1. Check market liquidity conditions");
    println!("    2. Review risk filter logs");
    println!("    3. Verify signal generation pipeline");
    println!("    4. Monitor for infrastructure issues");
}

/// Example alert action workflow
async fn example_alert_workflow(drift_detector: &DriftDetector) -> Result<()> {
    // Step 1: Get unacknowledged alerts
    let alerts = drift_detector.get_unacknowledged_alerts().await?;

    for alert in alerts {
        info!("Processing alert for strategy: {}", alert.strategy_id);

        // Step 2: Determine action based on severity
        match alert.severity {
            DriftSeverity::Critical => {
                // Immediately stop strategy execution
                warn!("⛔ Stopping strategy {} due to critical drift", alert.strategy_id);
                // await execution_manager.stop_strategy(&alert.strategy_id).await?;

                // Notify team
                // await notification_manager.send_critical_alert(&alert).await?;
            }
            DriftSeverity::High => {
                // Reduce position sizes
                warn!("⚠️ Reducing position sizes for strategy {}", alert.strategy_id);
                // await risk_manager.reduce_position_sizes(&alert.strategy_id, 0.5).await?;
            }
            DriftSeverity::Medium => {
                // Log warning, continue monitoring
                info!("📝 Medium drift logged, will continue monitoring");
            }
            DriftSeverity::Low => {
                // Informational only
                info!("ℹ️ Low severity drift noted");
            }
        }

        // Step 3: Acknowledge alert after taking action
        drift_detector
            .acknowledge_alert(&alert.strategy_id, alert.detected_at)
            .await?;
    }

    Ok(())
}
