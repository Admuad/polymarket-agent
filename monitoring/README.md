# Monitoring & Learning System (Layer 5)

The monitoring system provides comprehensive tracking, analysis, and learning capabilities for the polymarket-agent trading system.

## Architecture

```
monitoring/
├── src/
│   ├── lib.rs              # Main module exports
│   ├── resolution.rs       # Resolution tracking and trade updates
│   ├── attribution.rs      # P&L attribution to signals/agents
│   ├── metrics.rs          # Performance metrics calculation
│   ├── calibration.rs     # Brier scores and calibration analysis
│   ├── drift_detection.rs # Performance/prediction drift detection
│   ├── ab_testing.rs       # Strategy A/B testing framework
│   ├── shadow_mode.rs      # Paper trading for strategy testing
│   └── main.rs             # Examples and tests
└── Cargo.toml
```

## Components

### 1. Resolution Monitor (`resolution.rs`)

Tracks market resolutions and updates trade outcomes when markets resolve.

**Features:**
- Automatic detection of market resolution events
- P&L calculation for all affected trades
- Resolution status tracking (Pending, Resolved, Disputed, Cancelled)
- Stale resolution detection (markets past end time)

**Usage:**
```rust
let monitor = ResolutionMonitor::new(db_pool).await?;
monitor.initialize().await?;

// Process events from the event bus
monitor.process_event(&market_event).await?;

// Get resolution statistics
let stats = monitor.get_resolution_stats().await?;
```

### 2. Attribution Engine (`attribution.rs`)

Maps trades to signals and agents, enabling P&L attribution.

**Features:**
- Signal generation tracking
- Trade-to-signal attribution
- Strategy-level P&L breakdown
- Agent-level performance tracking
- Signal outcome analysis

**Usage:**
```rust
let attribution = AttributionEngine::new(db_pool);
attribution.initialize().await?;

// Store a generated signal
attribution.store_signal(&signal).await?;

// Attribute a trade to a signal
attribution.attribute_trade(trade_id, signal_id, agent_id, strategy_id).await?;

// Calculate strategy P&L
let pnl = attribution.calculate_strategy_pnl("sentiment-v1", from, to).await?;
```

### 3. Metrics Calculator (`metrics.rs`)

Computes comprehensive performance metrics.

**Features:**
- Hit rate, ROI, Sharpe ratio
- Maximum drawdown, Calmar ratio
- Profit factor, win/loss analysis
- Historical performance tracking

**Usage:**
```rust
let calc = MetricsCalculator::new(db_pool, 0.02); // 2% risk-free rate
calc.initialize().await?;

let metrics = calc.calculate_strategy_metrics("sentiment-v1", from, to).await?;
println!("Hit Rate: {:.2}%", metrics.hit_rate);
println!("Sharpe Ratio: {:?}", metrics.sharpe_ratio);
```

### 4. Calibration Engine (`calibration.rs`)

Tracks prediction accuracy using Brier scores and calibration analysis.

**Features:**
- Brier score calculation and decomposition
- Log loss calculation
- Calibration error analysis (Expected Calibration Error)
- Confidence bucket analysis
- Calibration drift detection

**Usage:**
```rust
let calibration = CalibrationEngine::new(db_pool);
calibration.initialize().await?;

// Record a prediction
calibration.record_prediction(pred_id, signal_id, "sentiment-v1", market_id, "YES", 0.75).await?;

// Calculate calibration metrics
let metrics = calibration.calculate_calibration("sentiment-v1", from, to).await?;
println!("Brier Score: {:.4}", metrics.brier_score);
println!("Calibration Error: {:.4}", metrics.calibration_error);
```

**Brier Score Interpretation:**
- **0.0**: Perfect predictions
- **0.25**: Random guessing (binary outcomes)
- **< 0.1**: Excellent predictions
- **0.1-0.2**: Good predictions
- **0.2-0.3**: Moderate predictions
- **> 0.3**: Poor predictions

### 5. Drift Detector (`drift_detection.rs`)

Monitors for performance and prediction degradation.

**Features:**
- Performance drift (P&L decline)
- Prediction drift (Brier score increase)
- Volume drift (trading activity)
- Configurable thresholds and severity levels
- Alert management

**Configuration:**
```rust
let config = DriftDetectionConfig {
    window_hours: 24,              // Compare recent 24h vs historical
    pnl_decline_threshold: 20.0,   // Alert on 20% P&L decline
    hit_rate_decline_threshold: 10.0,
    brier_score_increase_threshold: 0.05,
    volume_decline_threshold: 30.0,
};
```

**Usage:**
```rust
let detector = DriftDetector::new(db_pool, config);
detector.initialize().await?;

// Check all strategies
let drifts = detector.check_all_strategies().await?;

for drift in drifts {
    println!("[{}] {}: {}", drift.severity, drift.strategy_id, drift.description);
}
```

**Example Drift Detection Output:**
```
[High] sentiment-v1: P&L per trade declined 35.2% (historical: $12.50/trade -> recent: $8.10/trade)
[Critical] momentum-v2: Brier score increased by 0.15 (historical: 0.08 -> recent: 0.23)
[Medium] trend-following: Trading volume declined 45.0% (historical: 120 trades -> recent: 66 trades)
```

### 6. A/B Testing Framework (`ab_testing.rs`)

Statistical testing for comparing strategy performance.

**Features:**
- Randomized market assignment
- T-test statistical analysis
- P-value and confidence calculation
- Winner recommendation generation
- Sample size validation

**Usage:**
```rust
let manager = AbTestManager::new(db_pool);
manager.initialize().await?;

// Create a test
let test = AbTest {
    id: Uuid::new_v4(),
    name: "Sentiment-v1 vs v2".to_string(),
    strategy_a: "sentiment-v1".to_string(),
    strategy_b: "sentiment-v2".to_string(),
    start_time: Utc::now(),
    end_time: None,
    status: AbTestStatus::Running,
    allocation_ratio: 0.5,  // 50/50 split
    min_sample_size: 50,
    statistical_significance: 0.95,
};

manager.create_test(test).await?;

// Assign market to strategy A or B
let assigned = manager.get_assignment(test_id, market_id).await?;

// Analyze test results
let engine = AbTestEngine::new(db_pool);
let result = engine.analyze_test(test_id).await?;
println!("Winner: {:?} (confidence: {:.1}%)", result.winner, result.confidence.unwrap() * 100.0);
```

### 7. Shadow Mode (`shadow_mode.rs`)

Paper trading for testing strategies without real money.

**Features:**
- Hypothetical trade tracking
- Outcome simulation based on resolutions
- Shadow vs real performance comparison
- Strategy promotion/demotion decisions

**Usage:**
```rust
let shadow_mode = ShadowMode::new(db_pool);
shadow_mode.initialize().await?;

let paper_trader = PaperTrader::new(db_pool);

// Execute a paper trade
let trade = paper_trader.execute_paper_trade(
    market_id, "YES", OrderSide::Buy, 0.65, 100.0, "sentiment-v2"
).await?;

// Update outcomes when market resolves
shadow_mode.update_shadow_outcomes(market_id, "YES").await?;

// Compare shadow vs real performance
let comparison = paper_trader.compare_shadow_real("sentiment-v2").await?;
println!("Shadow Hit Rate: {:.2}% vs Real: {:.2}%",
    comparison.shadow_performance.hit_rate,
    comparison.real_hit_rate
);
```

## Database Schema

### Tables

1. **resolutions** - Market resolution tracking
2. **signals** - Generated prediction signals
3. **attributed_trades** - Trade-to-signal mapping
4. **performance_metrics** - Historical performance data
5. **predictions** - Calibration data
6. **calibration_metrics** - Calibration analysis results
7. **drift_alerts** - Drift detection alerts
8. **ab_tests** - A/B test configurations
9. **ab_test_assignments** - Market to strategy assignments
10. **ab_test_results** - Test analysis results
11. **shadow_trades** - Paper trading data

## Integration

### Event Bus Integration

```rust
// Subscribe to market events
bus.subscribe(|event| {
    match event {
        MarketEvent::MarketResolved { market_id, outcome_id } => {
            resolution_monitor.handle_market_resolution(market_id, outcome_id).await?;
        }
        _ => {}
    }
    Ok(())
}).await?;
```

### Periodic Jobs

```rust
// Check for drift every hour
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
        let drifts = drift_detector.check_all_strategies().await?;
        // Send alerts if needed
    }
});

// Update calibration metrics daily
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(86400)).await;
        for strategy in strategies {
            let metrics = calibration.calculate_calibration(&strategy, from, now).await?;
        }
    }
});
```

## Example Workflow

### Full Monitoring Pipeline

1. **Track Signals:**
   ```rust
   attribution.store_signal(&signal).await?;
   ```

2. **Execute Trade:**
   ```rust
   execution.execute(&order).await?;
   attribution.attribute_trade(trade_id, signal_id, agent_id, strategy_id).await?;
   ```

3. **Market Resolves:**
   ```rust
   resolution_monitor.handle_market_resolution(market_id, winning_outcome).await?;
   calibration.update_prediction_outcome(market_id, winning_outcome).await?;
   shadow_mode.update_shadow_outcomes(market_id, winning_outcome).await?;
   ```

4. **Calculate Metrics:**
   ```rust
   let metrics = calc.calculate_strategy_metrics(strategy_id, from, to).await?;
   ```

5. **Check Drift:**
   ```rust
   if let Ok(drifts) = drift_detector.check_strategy_drift(strategy_id).await {
       for drift in drifts {
           alert_team(&drift).await?;
       }
   }
   ```

6. **A/B Test New Strategy:**
   ```rust
   // Run in shadow mode first
   paper_trader.execute_paper_trade(...).await?;

   // If results good, promote to A/B test
   let test = create_ab_test("old", "new");
   manager.create_test(test).await?;

   // Analyze results
   let result = engine.analyze_test(test_id).await?;
   if result.winner == Some("B".to_string()) {
       promote_strategy("new").await?;
   }
   ```

## Performance Considerations

- **Indexes:** All tables have appropriate indexes for common queries
- **Caching:** Resolution data is cached in memory
- **Batching:** Bulk updates for resolution processing
- **Async:** All database operations are async

## Monitoring the Monitoring System

### Key Metrics to Track

1. **Resolution Lag:** Time between market end and resolution
2. **P&L Attribution Accuracy:** Trade coverage percentage
3. **Drift Detection Rate:** False positives/negatives
4. **Calibration Stability:** Brier score over time
5. **Shadow Mode Accuracy:** Correlation with real trades

### Alerts

- Stale resolutions (> 24h past end time)
- High drift severity (Critical/High)
- Calibration degradation (> 0.05 Brier score increase)
- A/B test completion
- Shadow mode performance divergence

## Future Enhancements

- Machine learning-based drift detection
- Portfolio-level calibration metrics
- Real-time strategy recommendation
- Automated strategy evolution
- Risk-adjusted performance ranking
