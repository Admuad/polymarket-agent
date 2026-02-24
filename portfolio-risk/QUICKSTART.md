# Quick Start Guide - Portfolio & Risk Management

## 5-Minute Quick Start

### 1. Add to Workspace

Already added to `/home/opc/.openclaw/workspace/projects/polymarket-agent/Cargo.toml`:

```toml
[workspace]
members = [
    "data-ingestion",
    "common",
    "portfolio-risk",
]
```

### 2. Use in Your Code

```rust
use portfolio_risk::{
    PortfolioRiskManager, RiskConfig
};
use common::{OrderSide, Uuid};

fn main() -> anyhow::Result<()> {
    // Create risk manager
    let mut manager = PortfolioRiskManager::new()?;

    // Evaluate a trade
    let market_id = Uuid::new_v4();
    let evaluation = manager.evaluate_trade(
        market_id,
        "YES",
        OrderSide::Buy,
        0.5,   // Price: 50 cents
        25.0,  // Size: $25
    )?;

    if evaluation.approved {
        println!("Trade approved! Kelly limit: ${:.2}", evaluation.kelly_limit);
    }

    // Process trade event
    manager.process_event(&trade_event)?;

    // Get portfolio summary
    let summary = manager.get_summary();
    println!("Total: ${:.2}, Positions: {}, Risk: {:?}",
        summary.total_value,
        summary.num_positions,
        summary.risk_level
    );

    Ok(())
}
```

### 3. Customize Configuration

Create `risk_config.toml`:

```toml
[risk_limits]
max_position_size = 100.0
max_total_exposure = 1000.0
max_theme_exposure = 500.0
max_positions = 20
max_theme_percentage = 0.30
daily_loss_limit = 100.0
stop_loss_percentage = 0.20

[risk_limits.theme_limits.politics]
max_exposure = 500.0
max_positions = 5
max_percentage = 0.30

[circuit_breakers]
enabled = true
daily_loss_limit = 100.0
max_drawdown_percentage = 0.15
var_95_limit = 200.0
cooldown_minutes = 30
max_violations_per_day = 3

kelly_multiplier = 0.25
correlation_threshold = 0.7
```

Load it:

```rust
use portfolio_risk::config::load_config;

let config = load_config("risk_config.toml")?;
let manager = PortfolioRiskManager::with_config(config)?;
```

### 4. Run Examples

```bash
# Build and run basic usage example
cargo run -p portfolio-risk --example basic_usage

# Run tests
cargo test -p portfolio-risk
```

## Key API Functions

### PortfolioRiskManager

```rust
// Creation
PortfolioRiskManager::new()
PortfolioRiskManager::with_config(RiskConfig)

// Trade Evaluation
evaluate_trade(market_id, outcome_id, side, price, size) -> Result<TradeEvaluation, RiskViolation>

// Event Processing
process_event(&MarketEvent) -> Result<()>

// Query
get_summary() -> PortfolioSummary
get_metrics() -> RiskMetrics
```

### Portfolio

```rust
// Position Management
add_position(market_id, outcome_id, value, price) -> Result<()>
remove_position(market_id, outcome_id, value, price) -> Result<()>
update_price(market_id, outcome_id, price)
resolve_market(market_id, winning_outcome_id) -> Result<f64>

// Query
total_value() -> f64
num_positions() -> usize
total_pnl() -> f64
unrealized_pnl() -> f64
exposure_by_category() -> Vec<(String, f64)>
```

### RiskChecker

```rust
// Trade Validation
check_trade(market_id, outcome_id, side, value, portfolio) -> Result<(), RiskViolation>

// Circuit Breakers
check_circuit_breakers(&portfolio) -> Vec<RiskViolation>

// Risk Assessment
calculate_risk_level(&portfolio) -> RiskLevel
```

### KellyCriterion

```rust
// Creation
Kelly::new(multiplier, edge)  // multiplier: 0.25 = quarter-Kelly

// Position Sizing
calculate_position(price, bankroll) -> f64

// Edge Estimation
estimate_edge_from_history(win_rate, avg_win, avg_loss) -> f64
```

## Common Patterns

### Pattern 1: Trade Approval Workflow

```rust
// 1. Check if trade is allowed
let evaluation = manager.evaluate_trade(
    market_id, "YES", OrderSide::Buy, 0.5, 25.0
)?;

// 2. Execute trade if approved
if evaluation.approved {
    // Execute trade
    execute_trade(market_id, "YES", OrderSide::Buy, 0.5, 25.0)?;

    // 3. Record in portfolio
    let trade_event = create_trade_event(...);
    manager.process_event(&trade_event)?;
}
```

### Pattern 2: Risk Monitoring

```rust
// Get current risk level
let summary = manager.get_summary();
match summary.risk_level {
    RiskLevel::Low => println!("All clear"),
    RiskLevel::Medium => println!("Caution advised"),
    RiskLevel::High => println!("High risk - reduce exposure"),
    RiskLevel::Critical => println!("CRITICAL - halt trading"),
}
```

### Pattern 3: Theme Exposure Management

```rust
// Check exposure by category
let exposure = manager.get_summary().exposure_by_category;

for (theme, value) in &exposure {
    let percentage = value / manager.get_summary().total_value;
    println!("{}: ${:.2} ({:.1}%)", theme, value, percentage * 100.0);
}
```

### Pattern 4: Position Sizing with Kelly

```rust
// Use Kelly to determine optimal position size
let kelly = Kelly::new(0.25, Some(0.05));  // quarter-Kelly with 5% edge
let bankroll = manager.get_summary().total_value;
let price = get_current_price(market_id);

let optimal_size = kelly.calculate_position(price, bankroll);

// Don't exceed Kelly limit
let proposed_size = 25.0;
if proposed_size > optimal_size {
    println!("Trade exceeds Kelly limit: ${:.2} > ${:.2}",
        proposed_size, optimal_size);
}
```

## File Structure

```
portfolio-risk/
├── Cargo.toml                      # Dependencies
├── README.md                       # Full documentation
├── IMPLEMENTATION_SUMMARY.md       # Implementation details
├── QUICKSTART.md                   # This file
├── examples/
│   ├── basic_usage.rs             # Comprehensive examples
│   └── risk_config.toml           # Configuration template
└── src/
    ├── lib.rs                     # Main API
    ├── config.rs                  # Configuration management
    ├── portfolio.rs               # Portfolio tracking
    ├── risk.rs                    # Risk checking
    └── metrics.rs                 # Risk metrics
```

## Next Steps

1. **Read the full documentation**: `README.md`
2. **Review examples**: `examples/basic_usage.rs`
3. **Customize configuration**: Copy and modify `examples/risk_config.toml`
4. **Integrate with your trading system**: Use `PortfolioRiskManager` as a gatekeeper before executing trades
5. **Monitor risk metrics**: Periodically check `get_metrics()` to track portfolio health

## Support

- Full API documentation in `src/*.rs` files
- Usage examples in `examples/basic_usage.rs`
- Configuration template in `examples/risk_config.toml`
- Implementation details in `IMPLEMENTATION_SUMMARY.md`
