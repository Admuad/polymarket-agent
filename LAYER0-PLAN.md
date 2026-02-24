# Layer 0 - Data Ingestion Implementation Plan

## Tech Stack

**Language:** Rust (performance, memory safety, async/await)

**Event Bus:**
- Primary: Apache Kafka (production-grade)
- Alternative for MVP: Redis Streams / NATS (lighter weight)

**Databases:**
- Vector Store: Qdrant (open-source, Rust SDK available)
- Time-Series: TimescaleDB (PostgreSQL extension) or InfluxDB
- Graph DB: Neo4j or Memgraph (Cypher query language)

## Implementation Order

### Phase 1: Core Infrastructure
1. Project setup (Cargo workspace structure)
2. Database setup (Docker Compose for dev)
3. Event bus setup
4. Basic data models

### Phase 2: Polymarket Connector (Priority 1)
1. WebSocket client for real-time data
2. REST client for historical data
3. Order book data ingestion
4. Market data normalization

### Phase 3: News APIs (Priority 2)
1. GDELT connector (free, comprehensive)
2. AP/Reuters connectors (if API access)
3. News entity extraction
4. Sentiment tagging

### Phase 4: Social Data (Priority 3)
1. Twitter API integration
2. Reddit API integration
3. Telegram monitoring
4. Social signal aggregation

### Phase 5: Alt Data (Priority 4)
1. FRED economic data connector
2. BLS data connector
3. Custom alternative data sources

### Phase 6: Cross-Platform (Priority 5)
1. Metaculus API connector
2. Manifold API connector
3. Data normalization across platforms

## Directory Structure

```
polymarket-agent/
├── Cargo.toml (workspace)
├── data-ingestion/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── event_bus.rs
│       ├── connectors/
│       │   ├── polymarket.rs
│       │   ├── gdelt.rs
│       │   └── mod.rs
│       └── models/
├── databases/
│   ├── vector/
│   ├── timeseries/
│   └── graph/
└── docker-compose.yml
```

## Next Steps

1. Set up Cargo workspace
2. Create Docker Compose for databases
3. Implement Polymarket WebSocket connector
4. Get first data flowing into Time-Series DB
