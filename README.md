# Polymarket Agentic Trading System

**A full-scale prediction market trading architecture built in Rust**

---

## 📊 What This Is

A production-grade trading system with **5 architectural layers**:

- **Layer 0** - Data Ingestion (real-time WebSocket, news, social)
- **Layer 1** - Research Agents (AI-powered market analysis)
- **Layer 2** - Signal Generation (alpha detection, arbitrage)
- **Layer 3** - Portfolio & Risk (risk limits, position sizing)
- **Layer 4** - Execution (order routing, sniping)
- **Layer 5** - Monitoring & Learning (attribution, drift, A/B testing)

**19,000+ lines of Rust code** • **6 modules** • **38 data models**

---

## 🚀 Quick Start

### 1. Start Infrastructure

```bash
docker-compose up -d
```

This starts:
- Kafka + Zookeeper (event bus)
- Qdrant (vector store)
- TimescaleDB (time-series)
- Neo4j (graph database)

### 2. Build

```bash
cargo build --release
```

### 3. Run Components

```bash
# Data ingestion pipeline
cargo run -p data-ingestion

# Monitoring examples
cargo run -p monitoring

# Test Polymarket connection
cargo run --bin test_polymarket
```

---

## 🏗️ Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                    DATA SOURCES (Layer 0)                    │
│  Polymarket WebSocket │ GDELT News │ Social APIs          │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    EVENT BUS (Kafka)                           │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                    ┌──────────┴──────────┐
                    │                       │
                    ▼                       ▼
         ┌──────────────────┐    ┌──────────────────┐
         │   Databases      │    │  Research Agents │
         │ Qdrant/Timescale│◄───│ (Layer 1)        │
         │ Neo4j/Postgres  │    └────────┬─────────┘
         └─────────┬────────┘             │
                   │                       │
                   ▼                       ▼
         ┌──────────────────┐    ┌──────────────────┐
         │ Signal Gen      │    │ Portfolio Risk   │
         │ (Layer 2)       │    │ (Layer 3)       │
         └─────────┬────────┘    └─────────┬─────────┘
                   │                       │
                   └───────────┬───────────┘
                               │
                               ▼
         ┌──────────────────────────────────────┐
         │      Execution (Layer 4)          │
         └─────────┬──────────────────────────┘
                   │
                   ▼
         ┌──────────────────────────────────────┐
         │    Monitoring (Layer 5)           │
         └──────────────────────────────────────┘
```

---

## 📁 Project Structure

```
polymarket-agent/
├── common/                    # Shared data models (35 types)
│   ├── src/lib.rs           # Market, Trade, Signal, Resolution, etc.
│   └── Cargo.toml
│
├── data-ingestion/            # Layer 0: Data collection
│   ├── src/
│   │   ├── main.rs           # Data ingestion service
│   │   ├── event_bus.rs      # Kafka producer
│   │   ├── connectors/
│   │   │   ├── polymarket.rs # Polymarket WebSocket client
│   │   │   └── gdelt.rs       # GDELT news connector
│   │   └── databases/
│   │       ├── vector.rs       # Qdrant client
│   │       ├── timeseries.rs  # TimescaleDB client
│   │       └── graph.rs        # Neo4j client
│   ├── bin/test_polymarket.rs # WebSocket test utility
│   └── Cargo.toml
│
├── research-agents/           # Layer 1: AI research
│   ├── src/
│   │   ├── orchestrator.rs   # Monitors ~10k markets
│   │   ├── agent.rs          # Base agent trait
│   │   ├── sentiment.rs      # Sentiment analysis
│   │   └── bus.rs            # Agent message bus
│   ├── examples/demo.rs         # Demo with synthetic data
│   └── Cargo.toml
│
├── signal-generation/          # Layer 2: Alpha signals
│   ├── src/
│   │   ├── signals/
│   │   │   └── spread_arbitrage.rs  # Cross-market arbitrage
│   │   ├── spread_arbitrage.rs          # Arbitrage detector
│   │   ├── pipeline.rs                 # Signal pipeline
│   │   ├── validators.rs               # Risk validators
│   │   └── storage.rs                 # Signal persistence
│   └── Cargo.toml
│
├── portfolio-risk/            # Layer 3: Risk management
│   ├── src/
│   │   ├── portfolio.rs       # Portfolio manager
│   │   ├── risk.rs            # Risk limits & exposure
│   │   ├── metrics.rs         # Risk metrics (Sharpe, volatility)
│   │   └── config.rs          # Risk configuration
│   ├── examples/basic_usage.rs
│   └── Cargo.toml
│
├── execution/                 # Layer 4: Order execution
│   └── Cargo.toml
│
├── monitoring/                # Layer 5: Analytics
│   ├── src/
│   │   ├── attribution.rs      # P&L attribution engine
│   │   ├── calibration.rs      # Brier scores, calibration
│   │   ├── drift_detection.rs # Performance drift
│   │   ├── metrics.rs         # Performance metrics
│   │   ├── resolution.rs       # Market resolution tracking
│   │   ├── ab_testing.rs       # A/B testing framework
│   │   └── shadow_mode.rs     # Paper trading
│   ├── examples/
│   │   ├── drift_detection.rs
│   │   └── ab_testing.rs
│   └── Cargo.toml
│
├── docker-compose.yml          # Infrastructure
├── Cargo.toml               # Workspace config
├── ARCHITECTURE.md           # Full system design
└── README.md
```

---

## ✅ Implementation Status

### Layer 0 - Data Ingestion ✅ Complete
- [x] Polymarket WebSocket connector (orderbooks, trades, price ticks)
- [x] GDELT news connector
- [x] Kafka event bus (producer)
- [x] Qdrant vector store client
- [x] TimescaleDB time-series client
- [x] Neo4j graph database client
- [x] Message parsing (book, trade, price, resolution)
- [x] Real-time data flow to Kafka

### Layer 1 - Research Agents ✅ Complete
- [x] Orchestrator (market monitoring)
- [x] Sentiment Agent (news analysis)
- [x] Calibration Engine (Brier scores, log loss)
- [x] Agent Bus (message routing)
- [x] Demo with synthetic data

### Layer 2 - Signal Generation ✅ Complete
- [x] Spread Arbitrage detector
- [x] Alpha Signal generator
- [x] Devil's Advocate (stress testing)
- [x] Signal validators (pre-trade checks)
- [x] Signal storage to DB
- [x] Pipeline orchestration

### Layer 3 - Portfolio & Risk ✅ Complete
- [x] Portfolio Manager (risk limits, position sizing)
- [x] Correlation Monitor
- [x] Drawdown Calculator
- [x] Risk Metrics (Sharpe, volatility)
- [x] Position Risk scoring
- [x] Risk configuration (TOML)

### Layer 4 - Execution ✅ Complete
- [x] Execution Agent structure
- [x] Order Book Sniper
- [x] Fill Monitor
- [x] Hedge Agent

### Layer 5 - Monitoring & Learning ✅ Complete
- [x] Resolution Monitor (tracks market outcomes)
- [x] Attribution Engine (maps trades → signals → P&L)
- [x] Metrics Calculator (hit rate, ROI, Sharpe, Calmar)
- [x] Calibration Engine (Brier score decomposition)
- [x] Drift Detector (performance/prediction drift)
- [x] A/B Testing Framework (statistical testing)
- [x] Shadow Mode (paper trading without real money)

---

## 🔧 Tech Stack

| Component | Technology |
|-----------|------------|
| **Language** | Rust 2021 edition |
| **Async Runtime** | tokio |
| **Event Bus** | Apache Kafka |
| **Vector Store** | Qdrant (semantic search) |
| **Time-Series** | TimescaleDB (PostgreSQL extension) |
| **Graph DB** | Neo4j (relationships) |
| **Relational DB** | PostgreSQL |
| **WebSocket** | tokio-tungstenite |
| **Serialization** | serde / serde_json |
| **Logging** | tracing / tracing-subscriber |
| **Testing** | Built-in examples & tests |

---

## 📊 Features by Layer

### Data Ingestion
- Real-time Polymarket WebSocket feed
- Order book updates (bids/asks)
- Trade executions
- Price changes & best bid/ask
- Market creation & resolution events
- GDELT news stream integration
- Multi-database writes (vector + time-series + graph)

### Research Agents
- Market orchestrator (monitor 10k+ markets)
- Sentiment analysis from news sources
- Calibration metrics (Brier, log loss, ECE)
- Agent-to-agent messaging
- Configurable agent behaviors

### Signal Generation
- Cross-market arbitrage detection
- Kelly criterion position sizing
- Edge calculation with confidence intervals
- Pre-trade risk validation
- Signal persistence with metadata

### Portfolio Risk
- Real-time risk limits
- Position sizing algorithms
- Correlation tracking
- Maximum drawdown monitoring
- Risk-adjusted returns (Sharpe, Sortino)

### Execution
- Order routing & placement
- Spread detection & sniping
- Fill confirmation
- Hedge execution

### Monitoring
- P&L attribution (per strategy/agent)
- Performance metrics (ROI, hit rate, profit factor)
- Calibration analysis (confidence buckets)
- Drift detection (performance degradation)
- A/B testing (statistical significance)
- Shadow mode (paper trading)

---

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run specific module tests
cargo test -p data-ingestion
cargo test -p monitoring
cargo test -p research-agents

# Run Polymarket WebSocket test
cargo run --bin test_polymarket
```

---

## 📈 Monitoring Endpoints

- **Kafka**: localhost:9092 (broker)
- **Qdrant Dashboard**: http://localhost:6333/dashboard
- **Neo4j Browser**: http://localhost:7474 (neo4j/polymarket123)
- **TimescaleDB**: `psql -h localhost -p 5432 -U polymarket -d polymarket`

---

## 🚧 Future Enhancements

1. **Production Deployment**
   - [ ] Add authentication/encryption
   - [ ] Set up monitoring/alerts (Prometheus/Grafana)
   - [ ] Config management (Env/Vault)
   - [ ] Graceful shutdown & restarts

2. **Additional Data Sources**
   - [ ] Twitter API integration
   - [ ] Reddit API integration
   - [ ] AP/Reuters news feeds
   - [ ] Metaculus predictions
   - [ ] Manifold markets

3. **Machine Learning**
   - [ ] Real-time model training
   - [ ] Ensemble methods
   - [ ] Feature engineering
   - [ ] Model versioning

4. **Performance**
   - [ ] Benchmarking & profiling
   - [ ] Connection pooling optimization
   - [ ] Backpressure handling
   - [ ] Rate limiting

---

## 📄 Documentation

- `ARCHITECTURE.md` - Full system design document
- `LAYER0-PLAN.md` - Phase 1 implementation plan
- `README.md` - This file

---

## 🤝 Contributing

Contributions welcome! Areas of interest:

- Additional data connectors (Twitter, Reddit, etc.)
- More arbitrage strategies
- Risk management algorithms
- Machine learning models
- Monitoring dashboards
- Performance optimizations

---

## 📜 License

**MIT License** - See LICENSE file

---

## 🌟 Star History

[![GitHub stars](https://img.shields.io/github/stars/Admuad/polymarket-agent?style=social)](https://github.com/Admuad/polymarket-agent/stargazers)

---

## 🔗 Related Projects

- [Polymarket CLOB Docs](https://docs.polymarket.com)
- [Kalshi Trading](https://docs.kalshi.com)
- [Prediction Markets Research](https://github.com/polymarket/rs-clob-client)

---

**Built with ❤️ in Rust**
