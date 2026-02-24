# Portfolio & Risk Management (Layer 3)

This crate provides comprehensive portfolio tracking, risk monitoring, and position management for Polymarket trading operations.

## Features

### Portfolio Management
- **Position Tracking**: Track all open positions by market and outcome
- **Category Organization**: Group positions by theme/category (politics, sports, crypto, etc.)
- **PnL Calculation**: Real-time unrealized and realized PnL tracking
- **Market Resolution**: Automatic PnL settlement when markets resolve

### Risk Management
- **Risk Limits**:
  - Maximum position size per market
  - Maximum total portfolio exposure
  - Maximum exposure per theme/category
  - Maximum number of positions
  - Stop loss triggers

### Risk Checks
- **Circuit Breakers**:
  - Daily loss limit monitoring
  - Maximum drawdown enforcement
  - VaR (Value at Risk) limits
  - Automatic trading halt when limits exceeded

- **Correlation Monitoring**:
  - Detect highly correlated positions
  - Alert on exposure concentration

- **Kelly Criterion**:
  - Optimal position sizing
  - Edge estimation from historical performance
  - Configurable fraction (quarter-Kelly recommended)

### Risk Metrics
- **Value at Risk (VaR)**:
  - 95% and 99% confidence levels
  - Historical simulation method
  - Expected Shortfall (Conditional VaR)

- **Drawdown Tracking**:
  - Maximum drawdown calculation
  - Real-time drawdown monitoring

- **Sharpe Ratio**:
  - Risk-adjusted return calculation
  - Annualized performance metrics
  - Configurable risk-free rate

## Usage Example

```rust
use portfolio_risk::{
    PortfolioRiskManager, RiskConfig, RiskLevel,
    TradeEvaluation, PortfolioSummary
};
use common::{MarketEvent, OrderSide, Uuid};
use chrono::Utc;

// Create a portfolio risk manager with default configuration
let mut manager = PortfolioRiskManager::new()?;

// Or use custom configuration
let config = RiskConfig::default();
let mut manager = PortfolioRiskManager::with_config(config)?;

// Evaluate a potential trade before executing
let market_id = Uuid::new_v4();
let evaluation = manager.evaluate_trade(
    market_id,
    "YES",
    OrderSide::Buy,
    0.5,  // price
    50.0, // size ($50)
)?;

if evaluation.approved {
    println!("Trade approved! Kelly limit: ${:.2}", evaluation.kelly_limit);
}

// Process market events (trades, price updates, resolutions)
let trade_event = /* create trade event */;
manager.process_event(&trade_event)?;

// Get portfolio summary
let summary = manager.get_summary();
println!("Total value: ${:.2}", summary.total_value);
println!("Open positions: {}", summary.num_positions);
println!("Risk level: {:?}", summary.risk_level);

// Get detailed risk metrics
let metrics = manager.get_metrics();
if let Some(var_95) = metrics.var_95 {
    println!("VaR (95%): ${:.2}", var_95);
}
if let Some(sharpe) = metrics.sharpe_ratio {
    println!("Sharpe ratio: {:.2}", sharpe);
}
```

## Configuration

Create a configuration file (e.g., `risk_config.toml`):

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

[risk_limits.theme_limits.sports]
max_exposure = 300.0
max_positions = 3
max_percentage = 0.20

[circuit_breakers]
enabled = true
daily_loss_limit = 100.0
max_drawdown_percentage = 0.15
var_95_limit = 200.0
cooldown_minutes = 30
max_violations_per_day = 3

kelly_multiplier = 0.25
correlation_threshold = 0.7

[metrics]
var_samples = 100
var_confidence = 0.95
sharpe_lookback_days = 30
risk_free_rate = 0.05
```

Load configuration:

```rust
use portfolio_risk::config::load_config;

let config = load_config("risk_config.toml")?;
let manager = PortfolioRiskManager::with_config(config)?;
```

## Risk Limit Types

### Position-Level Limits
- `max_position_size`: Maximum USD value for any single position
- `stop_loss_percentage`: Stop loss trigger per position (e.g., 20% = stop if down 20%)

### Portfolio-Level Limits
- `max_total_exposure`: Maximum total USD invested across all positions
- `max_positions`: Maximum number of open positions

### Theme-Level Limits
- `max_theme_exposure`: Maximum USD in any single theme
- `max_theme_percentage`: Maximum percentage of portfolio in any theme

### Circuit Breakers
- `daily_loss_limit`: Halt trading if daily PnL drops below this amount
- `max_drawdown_percentage`: Halt trading if portfolio drawdown exceeds this
- `var_95_limit`: Halt trading if VaR (95%) exceeds this amount
- `cooldown_minutes`: Wait this many minutes before resuming after trigger

## Kelly Criterion

The Kelly Criterion helps determine optimal bet sizing based on your edge:

```rust
use portfolio_risk::risk::KellyCriterion;

// Create with 25% Kelly (conservative quarter-Kelly)
let kelly = KellyCriterion::new(0.25, None);

// Calculate optimal position size
let price = 0.5; // Market price
let bankroll = 1000.0; // Available capital
let optimal_size = kelly.calculate_position(price, bankroll);

println!("Optimal position: ${:.2}", optimal_size);
```

**Note**: Always use a fraction of full Kelly (e.g., 0.25 for quarter-Kelly) to account for:
- Estimation errors
- Changing market conditions
- Risk of ruin

## Risk Metrics

### Value at Risk (VaR)
VaR estimates the maximum loss at a given confidence level:

- **VaR (95%)**: 95% confidence we won't lose more than this
- **VaR (99%)**: 99% confidence we won't lose more than this
- **Expected Shortfall**: Average loss in the worst 5% of cases

### Maximum Drawdown
The largest peak-to-trough decline in portfolio value.

### Sharpe Ratio
Risk-adjusted return: (Return - RiskFreeRate) / Volatility

- **> 2.0**: Excellent
- **1.0 - 2.0**: Good
- **0.5 - 1.0**: Adequate
- **< 0.5**: Poor

## Architecture

### Core Components

1. **PortfolioRiskManager**: Main entry point for portfolio and risk management
2. **Portfolio**: Position tracking and PnL calculation
3. **RiskChecker**: Risk limit enforcement and circuit breakers
4. **KellyCriterion**: Optimal position sizing
5. **RiskMetrics**: VaR, drawdown, Sharpe ratio calculations

### Data Flow

```
Market Events → PortfolioRiskManager → Portfolio Update
                                      → Risk Check
                                      → Circuit Breaker Check
                                      → Metrics Update
```

## Testing

Run tests:

```bash
cargo test -p portfolio-risk
```

## Best Practices

1. **Start Conservative**: Use quarter-Kelly or less
2. **Diversify**: Limit exposure per theme (e.g., max 30%)
3. **Monitor Drawdown**: Set circuit breakers to halt trading on significant losses
4. **Use VaR**: Understand potential downside at 95% confidence
5. **Track Sharpe**: Aim for > 1.0 risk-adjusted return
6. **Set Stop Losses**: Protect against large losses on individual positions

## License

MIT
