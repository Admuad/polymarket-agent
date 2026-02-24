# Research Agents Framework - Layer 1

A modular, extensible agent architecture for analyzing Polymarket prediction markets.

## Overview

The Research Agents framework provides a coordinated system for running multiple specialist agents across thousands of markets. Each agent can analyze different aspects of market data (sentiment, technical indicators, volatility, etc.) and generate signals that can be aggregated for trading decisions.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Orchestrator                            │
│  - Coordinates agents across ~10k markets                       │
│  - Load balancing and scheduling                                │
│  - Signal aggregation                                           │
└─────────────────────────────────────────────────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │                   │
              ┌─────▼─────┐      ┌─────▼─────┐
              │Sentiment  │      │ Technical │  ← Specialist Agents
              │  Agent    │      │  Agent    │
              └───────────┘      └───────────┘
                    │                   │
                    └─────────┬─────────┘
                              │
                    ┌─────────▼─────────┐
                    │  Agent Bus        │
                    │  (Tokio/Redis)    │
                    └───────────────────┘
```

## Components

### 1. Base Agent Trait (`agent.rs`)

All specialist agents implement the `Agent` trait for consistency:

```rust
#[async_trait]
pub trait Agent: Send + Sync {
    fn config(&self) -> &AgentConfig;
    fn status(&self) -> AgentStatus;
    async fn process_market(&self, input: AgentInput) -> anyhow::Result<Option<AgentOutput>>;
    async fn handle_control(&self, msg: ControlMessage) -> anyhow::Result<ControlResponse>;
    async fn on_start(&self) -> anyhow::Result<()>;
    async fn on_stop(&self) -> anyhow::Result<()>;
}
```

### 2. Orchestrator (`orchestrator.rs`)

Coordinates multiple agents across markets:
- Manages agent lifecycle (start, stop, pause, resume)
- Distributes markets to agents in batches
- Aggregates signals from multiple agents
- Supports concurrent processing with configurable limits

### 3. Agent Bus (`bus.rs`)

Communication layer for agent-to-agent messaging:
- **Unicast**: Point-to-point messaging between specific agents
- **Broadcast**: Pub/sub to topic-based channels
- **Priority system**: Messages can be prioritized (Low/Normal/High/Critical)
- **Extensible**: Currently uses tokio channels, can be upgraded to Redis Streams

### 4. Sentiment Agent (`sentiment.rs`)

First specialist agent implementation:
- Processes news data from Layer 0 (GDELT)
- Calculates sentiment scores using:
  - GDELT's tone scores
  - Keyword-based sentiment analysis (basic NLP)
- Matches news themes to market categories
- Outputs sentiment signals with confidence scores

## Usage Example

```rust
use research_agents::{Agent, AgentBus, Orchestrator, SentimentAgent};

// Create agent bus
let bus = Arc::new(AgentBus::new(AgentBusConfig::default()).await?);

// Create orchestrator
let mut orchestrator = Orchestrator::new(
    OrchestratorConfig::default(),
    Arc::clone(&bus)
).await?;

// Register sentiment agent
let sentiment_agent = SentimentAgent::new(SentimentAgentConfig::default());
orchestrator.register_agent(Box::new(sentiment_agent)).await?;

// Add markets
orchestrator.add_markets(markets).await?;

// Start processing
orchestrator.start().await?;
```

## Signal Output Format

Agents output structured signals with the following format:

```json
{
  "agent_id": "sentiment-agent",
  "market_id": "uuid-here",
  "signal_type": "sentiment",
  "confidence": 0.78,
  "timestamp": "2024-02-23T01:00:00Z",
  "processing_time_ms": 45,
  "data": {
    "market_id": "uuid-here",
    "market_category": "Economics",
    "sentiment": {
      "score": 0.65,
      "magnitude": 0.65,
      "confidence": 0.78,
      "article_count": 12
    },
    "top_themes": ["bitcoin", "growth", "finance", "crypto", "economy"],
    "timestamp": "2024-02-23T01:00:00Z",
    "sources": ["article1", "article2", "article3"]
  }
}
```

### Signal Interpretation

- **score**: -1.0 (very negative) to 1.0 (very positive)
- **magnitude**: 0.0 (weak/neutral) to 1.0 (strong signal)
- **confidence**: 0.0 to 1.0 (based on data quality and consistency)
- **article_count**: Number of data points analyzed

## Design Decisions

### 1. Agent Trait vs. Actor Model

**Decision**: Use trait-based agents instead of actor model (like actix)

**Rationale**:
- Simpler to understand and debug
- Less boilerplate for simple analysis agents
- Easy to test (no async message handling required)
- Still supports concurrent processing via orchestrator

### 2. Tokio Channels vs. Redis Streams

**Decision**: Start with tokio channels, design for Redis migration

**Rationale**:
- Tokio channels: Fast, in-memory, no external dependencies
- Redis Streams: Distributed, persistent, scalable across nodes
- Architecture designed to support both with minimal changes

**Migration path**:
- Replace `broadcast::Sender` with Redis producer
- Replace `broadcast::Receiver` with Redis consumer
- Keep same message format and API

### 3. Simple NLP vs. Full ML Models

**Decision**: Start with simple keyword/tone analysis, upgrade to rust-bert later

**Rationale**:
- Faster to implement and test the framework
- Lower resource requirements (no heavy ML models)
- Good enough for initial signal generation
- Can incrementally add ML capabilities

**Upgrade path**:
- Add `full-nlp` feature flag
- Integrate rust-bert for BERT-based sentiment
- Compare simple vs. ML outputs
- Gradually transition based on performance

### 4. Batch Processing vs. Event-Driven

**Decision**: Batch processing with configurable intervals

**Rationale**:
- Better for analyzing ~10k markets efficiently
- Reduces noise and false signals
- Allows time for data aggregation
- Simpler resource management

**Future enhancement**: Add event-driven mode for real-time critical markets

## Performance Considerations

### Scalability (~10k markets)

- **Concurrent processing**: Configurable limit (default: 100 concurrent markets)
- **Batch size**: Markets processed in batches (default: 50 per batch)
- **Agent parallelism**: Multiple agents can process different markets simultaneously
- **Memory**: DashMap for thread-safe data structures without global locks

### Resource Usage

- **CPU**: ~0.1ms per market for simple sentiment analysis
- **Memory**: ~10MB per agent (mostly data cache)
- **Network**: Depends on bus implementation (tokio = 0, Redis = minimal)

## Future Enhancements

### Planned Specialist Agents

1. **Technical Agent**: Price action, volume, volatility patterns
2. **Correlation Agent**: Cross-market correlation analysis
3. **Momentum Agent**: Trend detection and momentum indicators
4. **Event Agent**: Scheduled event analysis (elections, earnings, etc.)
5. **Liquidity Agent**: Market depth and slippage analysis
6. **Whale Agent**: Large holder and smart money tracking

### Infrastructure Improvements

1. **Redis Integration**: For distributed deployments
2. **Signal Aggregation**: Combine multiple agent signals with weights
3. **Backtesting**: Historical signal performance tracking
4. **Signal Persistence**: Store signals for analysis
5. **Metrics Dashboard**: Real-time monitoring of agent performance

## Testing

Run the example:

```bash
cargo run --package research-agents --example demo
```

Run tests:

```bash
cargo test -p research-agents
```

## Integration with Layer 0

The research agents consume data from Layer 0's event bus:

1. **News data**: From GDELT connector (sentiment agent)
2. **Market data**: From Polymarket WebSocket (technical agent - future)
3. **Social data**: From Twitter/Reddit (social agent - future)

Agents subscribe to relevant topics and process incoming data in real-time.

## License

Part of the Polymarket Agentic Trading System
