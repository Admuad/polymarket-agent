# Monitoring & Learning System - Implementation Summary

## Overview
The Monitoring & Learning system (Layer 5) has been designed and implemented for the polymarket-agent project. This system provides comprehensive tracking, analysis, and learning capabilities for automated trading.

## Components Implemented

### 1. Resolution Monitor (`src/resolution.rs`)
**Purpose:** Track market resolutions and update trade outcomes

**Features:**
- Market resolution event processing
- P&L calculation for resolved trades
- Resolution status tracking (Pending, Resolved, Disputed, Cancelled)
- Stale resolution detection
- Resolution statistics tracking

**Key Functions:**
- `process_event()` - Handle market events
- `handle_market_resolution()` - Process resolution events
- `update_trades_for_resolution()` - Calculate P&L for affected trades
- `get_resolution()` - Query resolution status
- `check_stale_resolutions()` - Find markets past end time without resolution

### 2. Attribution Engine (`src/attribution.rs`)
**Purpose:** Map trades to signals/agents for P&L attribution

**Features:**
- Signal generation tracking
- Trade-to-signal mapping
- Strategy-level P&L breakdown
- Agent-level performance tracking
- Signal outcome analysis (winning vs losing trade characteristics)

**Key Functions:**
- `store_signal()` - Record generated signals
- `attribute_trade()` - Link trades to signals
- `calculate_strategy_pnl()` - Calculate strategy P&L with detailed breakdown
- `get_top_strategies()` - Rank strategies by performance
- `analyze_signal_outcomes()` - Compare win/loss signal characteristics

**P&L Attribution Output:**
```rust
PnlAttribution {
    total_pnl: $1250.50,
    total_trades: 85,
    hit_rate: 62.35%,
    profit_factor: 2.15,
    roi: 12.5%
}
```

### 3. Metrics Calculator (`src/metrics.rs`)
**Purpose:** Compute comprehensive performance metrics

**Features:**
- Hit rate and win/loss analysis
- ROI calculation
- Sharpe ratio (risk-adjusted returns)
- Maximum drawdown tracking
- Profit factor
- Calmar ratio (return/drawdown)
- Historical performance tracking

**Metrics Calculated:**
- **Hit Rate:** Percentage of winning trades
- **ROI:** Return on Investment percentage
- **Sharpe Ratio:** Risk-adjusted return (annualized, 252 trading days)
- **Max Drawdown:** Largest peak-to-trough decline (percentage)
- **Profit Factor:** Gross profit / Gross loss
- **Calmar Ratio:** Annualized return / Max drawdown

### 4. Calibration Engine (`src/calibration.rs`)
**Purpose:** Track prediction accuracy using Brier scores

**Features:**
- Brier score calculation (measure of probabilistic prediction accuracy)
- Brier score decomposition (reliability, resolution, uncertainty)
- Log loss calculation
- Expected Calibration Error (ECE)
- Confidence bucket analysis
- Calibration drift detection

**Brier Score Interpretation:**
- **0.0:** Perfect predictions
- **0.25:** Random guessing (binary outcomes)
- **< 0.1:** Excellent predictions
- **0.1-0.2:** Good predictions
- **0.2-0.3:** Moderate predictions
- **> 0.3:** Poor predictions

**Confidence Bucket Example:**
```
[0.5-0.6): 42 predictions, predicted: 55%, actual: 58%, error: 0.03
[0.6-0.7): 35 predictions, predicted: 65%, actual: 61%, error: 0.04
[0.7-0.8): 28 predictions, predicted: 75%, actual: 73%, error: 0.02
```

### 5. Drift Detector (`src/drift_detection.rs`)
**Purpose:** Monitor for performance and prediction degradation

**Features:**
- Performance drift detection (P&L decline)
- Prediction drift detection (Brier score increase)
- Volume drift detection (trading activity drop)
- Configurable thresholds
- Severity levels (Low, Medium, High, Critical)
- Alert management system

**Configuration:**
```rust
DriftDetectionConfig {
    window_hours: 24,              // Compare recent 24h vs historical 72h
    pnl_decline_threshold: 20.0,   // Alert if P&L drops 20%
    hit_rate_decline_threshold: 10.0,
    brier_score_increase_threshold: 0.05,  // Alert if Brier score +0.05
    volume_decline_threshold: 30.0,  // Alert if volume drops 30%
}
```

**Example Drift Detection Output:**
```
[High] sentiment-v1: P&L per trade declined 36.0% (historical: $12.50/trade -> recent: $8.00/trade)
[Critical] momentum-v2: Brier score increased by 0.14 (historical: 0.08 -> recent: 0.22)
[Medium] trend-following: Trading volume declined 45.0% (historical: 120 trades -> recent: 66 trades)
```

### 6. A/B Testing Framework (`src/ab_testing.rs`)
**Purpose:** Statistical testing for comparing strategy performance

**Features:**
- Randomized market assignment (50/50 or custom split)
- T-test statistical analysis
- P-value and confidence calculation
- Winner recommendation generation
- Sample size validation
- Test lifecycle management (create, pause, resume, complete)

**A/B Test Configuration:**
```rust
AbTest {
    strategy_a: "sentiment-v1",
    strategy_b: "sentiment-v2",
    allocation_ratio: 0.5,      // 50/50 split
    min_sample_size: 50,         // Minimum trades per strategy
    statistical_significance: 0.95  // 95% confidence
}
```

**Statistical Analysis:**
- Computes t-statistic for P&L comparison
- Calculates p-value using normal distribution approximation
- Determines winner only if confidence > 95%
- Provides actionable recommendation

**Recommendation Examples:**
- `"Strategy sentiment-v2 significantly outperforms sentiment-v1 (ΔP&L: $150.00). Consider promoting sentiment-v2."`
- `"Inconclusive result. sentiment-v2 has higher P&L but not statistically significant. Continue test or increase sample size."`

### 7. Shadow Mode (`src/shadow_mode.rs`)
**Purpose:** Paper trading for testing strategies without real money

**Features:**
- Hypothetical trade tracking
- Outcome simulation based on resolutions
- Shadow vs real performance comparison
- Strategy promotion/demotion decisions

**Shadow Mode Workflow:**
1. Execute paper trades alongside real trades
2. Track hypothetical P&L
3. Compare shadow vs real performance
4. Use correlation metrics to validate model
5. Promote to live trading if validated

**Comparison Metrics:**
```rust
ShadowRealComparison {
    shadow_performance: ShadowPerformance {
        total_trades: 75,
        hit_rate: 65.3%,
        hypothetical_pnl: $950.00
    },
    real_trades: 80,
    real_hit_rate: 62.5%,
    real_pnl: $1020.00,
    hit_rate_diff: 2.8%,   // Shadow slightly higher
    pnl_diff: -$70.00       // Real performed better
}
```

## Database Schema

### Tables Created

1. **resolutions** - Market resolution tracking
   - market_id, outcome_id, status, resolved_at, resolution_price

2. **signals** - Generated prediction signals
   - id, market_id, outcome_id, predicted_probability, confidence, direction, agent_id, strategy_id, metadata

3. **attributed_trades** - Trade-to-signal mapping
   - trade_id, signal_id, agent_id, strategy_id, pnl, pnl_percent

4. **performance_metrics** - Historical performance data
   - strategy_id, period_start, period_end, total_trades, hit_rate, total_pnl, roi, sharpe_ratio, max_drawdown, profit_factor, calmar_ratio

5. **predictions** - Calibration data
   - id, signal_id, strategy_id, market_id, outcome_id, predicted_probability, actual_outcome, timestamp

6. **calibration_metrics** - Calibration analysis results
   - strategy_id, period_start, period_end, brier_score, log_loss, calibration_error, confidence_buckets

7. **drift_alerts** - Drift detection alerts
   - strategy_id, detected_at, drift_type, severity, metric_value, threshold, description, acknowledged

8. **ab_tests** - A/B test configurations
   - id, name, strategy_a, strategy_b, start_time, end_time, status, allocation_ratio, min_sample_size, statistical_significance

9. **ab_test_assignments** - Market to strategy assignments
   - id, test_id, market_id, assigned_strategy, assigned_at

10. **ab_test_results** - Test analysis results
    - test_id, strategy_metrics_a, strategy_metrics_b, winner, confidence, p_value, recommendation

11. **shadow_trades** - Paper trading data
    - id, trade_id, market_id, outcome_id, side, price, size, timestamp, strategy_id, hypothetical_pnl, would_have_won

## Integration Points

### Event Bus Integration
```rust
bus.subscribe(|event| {
    match event {
        MarketEvent::MarketResolved { market_id, outcome_id } => {
            resolution_monitor.handle_market_resolution(market_id, outcome_id).await?;
            calibration.update_prediction_outcome(market_id, outcome_id).await?;
            shadow_mode.update_shadow_outcomes(market_id, outcome_id).await?;
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
        // Send alerts for critical/high severity
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

## Examples

### Drift Detection Example
See `examples/drift_detection.rs` for comprehensive drift detection scenarios including:
- Performance drift (severe P&L decline)
- Prediction drift (Brier score degradation)
- Volume drift (trading activity drop)
- Alert action workflow

### A/B Testing Example
See `examples/ab_testing.rs` for A/B testing workflow including:
- Test creation and configuration
- Market assignment
- Statistical analysis
- Decision framework
- Safe rollout strategy

## Key Design Decisions

1. **Separation of Concerns:** Each component has a single responsibility (resolution, attribution, metrics, calibration, drift, A/B testing, shadow mode)

2. **Database-First:** All tracking data stored in PostgreSQL for persistence and historical analysis

3. **Async-First:** All operations are async to avoid blocking

4. **Configurable Thresholds:** Drift detection and calibration thresholds are configurable

5. **Statistical Rigor:** A/B testing uses proper t-tests with confidence intervals

6. **Safe Rollout:** Multi-stage rollout (shadow mode → small A/B test → full A/B test → gradual rollout)

## Known Limitations

1. **Statistical Tests:** Current implementation uses normal distribution approximation for t-tests. Production should use proper t-distribution.

2. **Database Types:** Some types need sqlx derives for FromRow. Current implementation uses tuples for queries.

3. **Time Zones:** All times stored in UTC. Need proper timezone handling for reporting.

## Future Enhancements

1. **Machine Learning Drift Detection:** Use ML models to detect subtle patterns of degradation

2. **Portfolio-Level Calibration:** Track calibration across all strategies together

3. **Real-Time Strategy Recommendation:** Suggest optimal strategy mixes based on current conditions

4. **Automated Strategy Evolution:** Promote/deprecate strategies based on performance

5. **Risk-Adjusted Performance Ranking:** Rank strategies by Sharpe ratio, not just P&L

## Files Structure

```
monitoring/
├── Cargo.toml                      # Dependencies
├── README.md                       # Documentation
├── IMPLEMENTATION_SUMMARY.md          # This file
├── src/
│   ├── lib.rs                      # Public API exports
│   ├── main.rs                     # Example code
│   ├── resolution.rs                # Resolution tracking
│   ├── attribution.rs               # P&L attribution
│   ├── metrics.rs                  # Performance metrics
│   ├── calibration.rs              # Brier score analysis
│   ├── drift_detection.rs          # Drift monitoring
│   ├── ab_testing.rs              # A/B testing framework
│   └── shadow_mode.rs            # Paper trading
└── examples/
    ├── drift_detection.rs          # Drift detection examples
    └── ab_testing.rs            # A/B testing examples
```

## Dependencies

- **sqlx:** Database operations with PostgreSQL
- **tokio:** Async runtime
- **chrono:** Date/time handling
- **uuid:** Unique identifiers
- **serde:** Serialization
- **anyhow:** Error handling
- **tracing:** Logging
- **statrs:** Statistical calculations
- **fastrand:** Random number generation

## Testing

Run examples:
```bash
# Drift detection example
cargo run --example drift_detection

# A/B testing example
cargo run --example ab_testing
```

Tests included in `main.rs`:
- Brier score calculation validation
- Drift severity configuration tests
