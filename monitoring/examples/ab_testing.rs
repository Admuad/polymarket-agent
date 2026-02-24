use anyhow::Result;
use chrono::{Duration, Utc};
use common::{AbTest, AbTestStatus, OrderSide, Signal, Trade};
use monitoring::{AbTestEngine, AbTestManager};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber;
use uuid::Uuid;

/// A/B Testing Framework Example

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

    info!("=== A/B Testing Framework Example ===\n");

    let manager = AbTestManager::new(pool.clone());
    let engine = AbTestEngine::new(pool.clone());

    manager.initialize().await?;

    // Example 1: Create a new A/B test
    info!("Example 1: Creating a new A/B test");
    info!("------------------------------------");

    let test_id = Uuid::new_v4();
    let test = AbTest {
        id: test_id,
        name: "Sentiment-v1 vs Sentiment-v2".to_string(),
        strategy_a: "sentiment-v1".to_string(),
        strategy_b: "sentiment-v2".to_string(),
        start_time: Utc::now() - Duration::days(7),
        end_time: None,
        status: AbTestStatus::Running,
        allocation_ratio: 0.5,  // 50/50 split between strategies
        min_sample_size: 50,    // Need at least 50 trades per strategy
        statistical_significance: 0.95,  // 95% confidence level
    };

    manager.create_test(test).await?;
    info!("✓ Created A/B test '{}'", test_id);
    info!("  Strategy A: sentiment-v1 (base model)");
    info!("  Strategy B: sentiment-v2 (enhanced model)");
    info!("  Allocation: 50/50");
    info!("  Minimum samples: 50 trades per strategy");

    // Example 2: Assign markets to strategies
    info!("Example 2: Assigning markets to strategies");
    info!("------------------------------------");

    // In a real scenario, you'd iterate through incoming markets
    let market_1 = Uuid::new_v4();
    let market_2 = Uuid::new_v4();

    // Random assignment based on allocation ratio
    let rng = fastrand::f32();
    let strategy_a = if rng < 0.5 {
        "sentiment-v1"
    } else {
        "sentiment-v2"
    };

    manager.assign_market(test_id, market_1, strategy_a).await?;
    info!("✓ Assigned market {} to {}", market_1, strategy_a);

    let rng = fastrand::f32();
    let strategy_b = if rng < 0.5 {
        "sentiment-v1"
    } else {
        "sentiment-v2"
    };

    manager.assign_market(test_id, market_2, strategy_b).await?;
    info!("✓ Assigned market {} to {}", market_2, strategy_b);

    // Example 3: Check assignment counts
    info!("Example 3: Monitoring assignment counts");
    info!("------------------------------------");

    let counts = manager.get_assignment_counts(test_id).await?;
    info!("Strategy A assignments: {}", counts.count_a);
    info!("Strategy B assignments: {}", counts.count_b);

    // Check if we have enough samples
    let has_enough = engine.check_sample_size(test_id).await?;
    info!("Minimum sample size met: {}", has_enough);

    // Example 4: Analyze test results
    info!("Example 4: Analyzing test results");
    info!("------------------------------------");

    match engine.analyze_test(test_id).await {
        Ok(result) => {
            info!("Test Analysis Results:");
            info!("  Strategy A ({}):", result.strategy_a_metrics.strategy_id);
            info!("    Total Trades: {}", result.strategy_a_metrics.total_trades);
            info!("    Total P&L: ${:.2}", result.strategy_a_metrics.total_pnl);
            info!("    Hit Rate: {:.2}%", result.strategy_a_metrics.hit_rate);
            info!("    ROI: {:.2}%", result.strategy_a_metrics.roi);

            info!("  Strategy B ({}):", result.strategy_b_metrics.strategy_id);
            info!("    Total Trades: {}", result.strategy_b_metrics.total_trades);
            info!("    Total P&L: ${:.2}", result.strategy_b_metrics.total_pnl);
            info!("    Hit Rate: {:.2}%", result.strategy_b_metrics.hit_rate);
            info!("    ROI: {:.2}%", result.strategy_b_metrics.roi);

            info!("  Statistical Analysis:");
            info!("    Winner: {:?}", result.winner);
            info!("    Confidence: {:?}%", result.confidence.map(|c| c * 100.0));
            info!("    P-value: {:?}", result.p_value);

            info!("  Recommendation:");
            info!("    {}", result.recommendation);
        }
        Err(e) => {
            info!("Could not analyze test yet: {}", e);
            info!("This is expected if not enough trades have been executed.");
        }
    }

    // Example 5: Decision framework
    info!("Example 5: A/B Test Decision Framework");
    info!("------------------------------------");

    let test_complete = engine.check_sample_size(test_id).await?;

    if test_complete {
        info!("✓ Test has sufficient samples for analysis");

        let result = engine.analyze_test(test_id).await?;

        match result.winner.as_deref() {
            Some("A") => {
                info!("Decision: Keep Strategy A (sentiment-v1), deprecate Strategy B");
                info!("Action: Roll back to production, remove sentiment-v2 from rotation");
            }
            Some("B") => {
                info!("Decision: Promote Strategy B (sentiment-v2) to production");
                info!("Action: Replace sentiment-v1 with sentiment-v2 in live trading");
            }
            None => {
                if result.confidence.unwrap_or(0.0) < 0.5 {
                    info!("Decision: Test inconclusive");
                    info!("Action: Continue test or increase sample size");
                } else {
                    info!("Decision: No significant difference");
                    info!("Action: Keep either strategy, consider cost/performance tradeoffs");
                }
            }
            Some(_) => {
                info!("Decision: Unknown winner");
                info!("Action: Review test configuration");
            }
        }
    } else {
        info!("⏳ Test needs more samples before conclusion");
        info!("Current sample size: {} vs minimum: {}",
            manager.get_assignment_counts(test_id).await?.count_a +
            manager.get_assignment_counts(test_id).await?.count_b,
            100  // min_sample_size * 2
        );
    }

    // Example 6: Pause/Resume test
    info!("Example 6: Pausing/Resuming a test");
    info!("------------------------------------");

    manager.pause_test(test_id).await?;
    info!("✓ Test paused (useful during market volatility)");

    // ... time passes, markets stabilize ...

    manager.resume_test(test_id).await?;
    info!("✓ Test resumed");

    // Example 7: Complete test
    info!("Example 7: Completing a test");
    info!("------------------------------------");

    manager.complete_test(test_id).await?;
    info!("✓ Test marked as complete");

    // Example 8: Multi-variant testing framework
    info!("Example 8: Multi-variant Testing Framework");
    info!("------------------------------------");

    info!("For comparing >2 strategies, use a tournament bracket:");
    info!("Round 1:");
    info!("  Test 1: sentiment-v1 vs sentiment-v2");
    info!("  Test 2: momentum-v1 vs momentum-v2");
    info!("  Test 3: trend-v1 vs trend-v2");
    info!("Round 2 (Winners):");
    info!("  Test 4: sentiment-winner vs momentum-winner");
    info!("  Test 5: sentiment-winner vs trend-winner");
    info!("Final Round:");
    info!("  Test 6: All winners compete");

    // Example 9: Safe rollout strategy
    info!("Example 9: Safe Rollout Strategy");
    info!("------------------------------------");

    info!("Phase 1: Shadow Mode");
    info!("  - Run new strategy in paper trading only");
    info!("  - Compare hypothetical vs real performance");
    info!("  - Duration: 1-2 weeks");

    info!("Phase 2: Small A/B Test");
    info!("  - Allocate 10% of trades to new strategy");
    info!("  - Monitor for issues");
    info!("  - Duration: 1 week");

    info!("Phase 3: Full A/B Test");
    info!("  - 50/50 split");
    info!("  - Statistical validation");
    info!("  - Duration: 2-4 weeks");

    info!("Phase 4: Gradual Rollout");
    info!("  - If new strategy wins: 25% → 50% → 75% → 100%");
    info!("  - Monitor at each step");
    info!("  - Rollback if issues detected");

    info!("✓ A/B testing framework example complete!");
    Ok(())
}
