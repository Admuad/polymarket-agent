# Signal Generation Framework (Layer 2)

The Signal Generation framework processes research outputs and market data to generate trade signals for the Polymarket trading system.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Signal Generation Framework                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐   │
│  │   Research   │     │   Market     │     │   Order      │   │
│  │   Agents     │────▶│   Data       │────▶│   Book       │   │
│  └──────────────┘     └──────────────┘     └──────────────┘   │
│         │                    │                    │            │
│         └────────────────────┼────────────────────┘            │
│                              ▼                                 │
│                    ┌──────────────┐                            │
│                    │ Signal Input │                            │
│                    └──────────────┘                            │
│                              │                                 │
│                              ▼                                 │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │              Signal Generators                          │  │
│  │  • SpreadArbitrage  • Momentum  • MeanReversion        │  │
│  │  • Value  • Sentiment                                    │  │
│  └─────────────────────────────────────────────────────────┘  │
│                              │                                 │
│                              ▼                                 │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │              Signal Validators                         │  │
│  │  • Edge Threshold  • Confidence  • Liquidity           │  │
│  │  • Expected Value                                       │  │
│  └─────────────────────────────────────────────────────────┘  │
│                              │                                 │
│                              ▼                                 │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │           Trade Signal with Metadata                     │  │
│  │  - Entry/Target/Stop Loss                               │  │
│  │  - Position Size (Kelly Criterion)                       │  │
│  │  - Confidence, EV, Edge                                 │  │
│  └─────────────────────────────────────────────────────────┘  │
│                              │                                 │
│                              ▼                                 │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐   │
│  │   Storage    │────▶│   Execution  │────▶│ Backtesting  │   │
│  └──────────────┘     └──────────────┘     └──────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Components

### 1. Signal Pipeline (`pipeline.rs`)

The `SignalPipeline` orchestrates the entire signal generation process:

- **Input**: Research agent outputs + market data
- **Processing**: Multiple signal generators + validators
- **Output**: Validated trade signals with metadata

```rust
use signal_generation::{
    PipelineConfig, SignalPipeline, SpreadArbitrageGenerator,
    EdgeThresholdValidator, ConfidenceValidator, InMemoryStorage,
};

let config = PipelineConfig {
    enabled: true,
    max_signals_per_cycle: 10,
    min_confidence: 0.6,
    min_edge: Decimal::from_str_exact("0.03").unwrap(),
};

let pipeline = SignalPipeline::new(config)
    .add_generator(Box::new(SpreadArbitrageGenerator::default()))
    .add_validator(Box::new(EdgeThresholdValidator::default()))
    .add_validator(Box::new(ConfidenceValidator::default()))
    .with_storage(Box::new(InMemoryStorage::new()));

let signals = pipeline.process(&input).await?;
```

### 2. Signal Generators (`signals/spread_arbitrage.rs`)

Signal generators implement the `SignalGenerator` trait:

```rust
pub trait SignalGenerator {
    fn generate(&self, input: &SignalInput) -> anyhow::Result<Option<TradeSignal>>;
    fn signal_type(&self) -> SignalType;
}
```

#### Spread Arbitrage Generator

Detects price discrepancies across outcomes and calculates expected value:

**Detection Logic:**
- Calculates total market probability: `Σ(outcome_prices)`
- Edge exists when: `total_probability < 1.0`
- Edge = `1.0 - total_probability`

**Position Sizing (Kelly Criterion):**
```
f* = (bp - q) / b

where:
  b = (target_price - entry_price) / (entry_price - stop_loss)  (odds)
  p = probability of winning
  q = 1 - p  (probability of losing)
```

**Expected Value:**
```
EV = (p × win_amount) - (q × loss_amount)
```

**Configuration:**
- `min_edge`: Minimum edge percentage (default: 5%)
- `min_liquidity`: Minimum liquidity score (default: 0.3)
- `max_kelly_fraction`: Maximum Kelly position size (default: 10%)
- `stop_loss_pct`: Stop loss as % of entry (default: 10%)
- `target_pct`: Target as % of entry (default: 15%)

### 3. Signal Validators (`validators.rs`)

Validators filter signals based on quality criteria:

#### Edge Threshold Validator
Ensures sufficient profit margin:
```rust
EdgeThresholdValidator::new(EdgeThresholdConfig {
    min_edge: Decimal::from_str_exact("0.05").unwrap(), // 5%
})
```

#### Confidence Validator
Ensures sufficient confidence in the signal:
```rust
ConfidenceValidator::new(ConfidenceValidatorConfig {
    min_confidence: 0.7, // 70%
})
```

#### Liquidity Validator
Ensures sufficient liquidity to execute:
```rust
LiquidityValidator::new(LiquidityValidatorConfig {
    min_liquidity_score: 0.3,
    max_position_liquidity_ratio: 0.1, // Max 10% of liquidity
})
```

#### Expected Value Validator
Ensures positive expected value:
```rust
ExpectedValueValidator::new(ExpectedValueValidatorConfig {
    min_expected_value: Decimal::from_str_exact("5.0").unwrap(), // $5
})
```

### 4. Signal Storage (`storage.rs`)

Provides persistence for backtesting and analysis:

**Traits:**
- `SignalStorage`: Store and retrieve signals
- `ExecutionStorage`: Store execution results
- `InMemoryStorage`: Default in-memory implementation
- `InMemoryExecutionStorage`: Execution result storage

**Usage:**
```rust
let storage = InMemoryStorage::new();

// Store a signal
storage.store(&signal).await?;

// Retrieve signals
let signals = storage.get_by_market(market_id).await?;

// Get statistics
let stats = storage.stats().await?;
```

## Trade Signal Structure

```rust
pub struct TradeSignal {
    pub id: Uuid,
    pub market_id: Uuid,
    pub signal_type: SignalType,
    pub direction: SignalDirection,
    pub outcome_id: Option<String>,
    pub entry_price: Decimal,
    pub target_price: Decimal,
    pub stop_loss: Decimal,
    pub position_size: Decimal,      // Kelly-sized
    pub confidence: f64,              // 0.0 to 1.0
    pub expected_value: Decimal,      // $EV
    pub edge: Decimal,                 // Edge percentage
    pub kelly_fraction: f64,          // Recommended fraction of bankroll
    pub reasoning: String,
    pub metadata: SignalMetadata,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}
```

## Example Output

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "market_id": "...",
  "signal_type": "SpreadArbitrage",
  "direction": "Long",
  "outcome_id": "yes",
  "entry_price": "0.50",
  "target_price": "0.575",
  "stop_loss": "0.45",
  "position_size": "10.00",
  "confidence": 0.78,
  "expected_value": "1.25",
  "edge": "0.05",
  "kelly_fraction": 0.08,
  "reasoning": "Spread arbitrage opportunity detected. Market total probability: 95% (edge: 5%). Research confidence: 75%. Estimated win probability: 52%. Liquidity score: 0.80. Volatility: 0.15.",
  "metadata": {
    "research_sources": ["Recent poll data", "Twitter sentiment"],
    "data_points": 24,
    "liquidity_score": 0.8,
    "volatility_score": 0.15,
    "custom_fields": {
      "win_probability": 0.52,
      "total_market_probability": "0.95"
    }
  },
  "created_at": "2026-02-23T01:00:00Z",
  "expires_at": "2026-02-24T01:00:00Z"
}
```

## Running the Example

```bash
cd signal-generation
cargo run --example spread_arbitrage_example
```

## Running Tests

```bash
cargo test --package signal-generation
```

## Signal Generation Flow

1. **Input Preparation**
   - Research agents provide analysis, sentiment, and probability estimates
   - Market data provides current prices, order book, and price history

2. **Signal Generation**
   - Each generator processes the input independently
   - May produce 0 or more signals per input

3. **Validation**
   - Signals pass through all validators
   - All validators must pass for signal approval
   - Validators check: edge, confidence, liquidity, EV

4. **Ranking & Selection**
   - Signals ranked by (expected_value × confidence)
   - Top N signals selected (configurable)

5. **Storage**
   - Approved signals stored for backtesting
   - Metadata preserved for analysis

## Future Extensions

Additional signal types planned:
- **Momentum**: Trend-following signals based on price momentum
- **Mean Reversion**: Contrarian signals for overextended prices
- **Value**: Fundamental analysis-based value signals
- **Sentiment**: Social media and news sentiment signals

Each will implement the `SignalGenerator` trait with custom logic.

## Mathematical Foundation

### Kelly Criterion
The Kelly Criterion provides optimal position sizing based on edge and odds:

```
f* = (bp - q) / b

where:
  f* = fraction of bankroll to wager
  b = odds received on the wager (win/loss ratio)
  p = probability of winning
  q = probability of losing (1 - p)
```

**Key Properties:**
- Maximizes long-term growth rate
- Prevents ruin (never bets more than edge justifies)
- Can be aggressive with small edges
- Must be capped to manage risk

### Expected Value
The expected value of a trade:

```
EV = (p × win_amount) - (q × loss_amount)

where:
  win_amount = target_price - entry_price
  loss_amount = entry_price - stop_loss
  p = win_probability
  q = 1 - p
```

**Interpretation:**
- Positive EV: Profitable over many trades
- Negative EV: Losing proposition
- Zero EV: Break-even proposition

## Configuration

### Pipeline Configuration
```rust
pub struct PipelineConfig {
    pub enabled: bool,                    // Enable/disable pipeline
    pub max_signals_per_cycle: usize,     // Max signals per run
    pub min_confidence: f64,              // Min confidence 0-1
    pub min_edge: Decimal,                // Min edge (e.g., 0.05 = 5%)
}
```

### Spread Arbitrage Configuration
```rust
pub struct SpreadArbitrageConfig {
    pub min_edge: Decimal,               // Minimum edge required
    pub min_liquidity: f64,               // Minimum liquidity score
    pub max_kelly_fraction: f64,          // Max Kelly position size
    pub default_position_size: Decimal,   // Default if no Kelly
    pub stop_loss_pct: Decimal,           // Stop loss % of entry
    pub target_pct: Decimal,              // Target % of entry
    pub signal_expiration_hours: i64,     // Signal lifetime
}
```
