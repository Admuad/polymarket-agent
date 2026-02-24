# Polymarket Agentic Trading System

## Architecture Overview

### Layer 0 - Data Ingestion
**Sources:**
- Polymarket CLOB (WebSocket / REST)
- News APIs (Reuters, AP, GDELT)
- Social (Twitter, Reddit, Telegram)
- Prediction Markets (Metaculus, Manifold)
- Alt Data (FRED, BLS)

**Components:**
- Event Bus (Kafka or alternative)
- Vector Store (news, claims, resolutions)
- Time-Series DB (prices, volumes, order book)
- Graph DB (people, organizations, event dependencies)

### Layer 1 - Research Agents
- Orchestrator Agent (monitors ~10k markets, routes to specialists)
- Specialist Agents: Sentiment, Forecasting, Resolution, Calibration, Liquidity
- Research Synthesis Agent (Bayesian reconciliation, confidence intervals)

### Layer 2 - Signal Generation
- Alpha Signal (edge calculation, Kelly sizing, EV)
- Devil's Advocate (stress-testing assumptions)
- Backtester (historical analogues, hit rate)

### Layer 3 - Portfolio & Risk
- Portfolio Manager (risk limits, exposure management)
- Correlation Monitor (co-movement tracking)
- Tail Risk Agent (black swan scenarios)
- Platform Risk (smart contract, gas, withdrawal)

### Layer 4 - Execution
- Execution Agent (order placement, timing optimization)
- Order Book Sniper (spread detection, sub-tick timing)
- Fill Monitor (partial fills, stale orders)
- Hedge Agent (correlated market hedges)

### Layer 5 - Monitoring & Learning
- Resolution Monitor (validation, disputes)
- Attribution (P&L by signal)
- Model Calibration (Brier scores, priors)
- Drift Detection (microstructure, concept drift)
- Strategy Evolution (A/B tests, shadow mode)

## Human-in-the-Loop Checkpoints
- Strategy Evolution review
- Drawdown circuit breaker override
- Novel market category approval
- Platform risk threshold changes
