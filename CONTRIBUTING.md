# Contributing to Polymarket Agent

Thanks for your interest in contributing! This is a complex system, so please follow this guide.

## Getting Started

1. **Fork the repository**
   ```bash
   gh repo fork Admuad/polymarket-agent
   git clone https://github.com/YOUR_USERNAME/polymarket-agent.git
   cd polymarket-agent
   ```

2. **Set up the environment**
   ```bash
   # Start databases
   docker-compose up -d

   # Build the project
   cargo build --release
   ```

3. **Run tests**
   ```bash
   cargo test --all
   ```

## Development Guidelines

### Code Style

- Use **Rust 2021 edition**
- Follow **RAII** and ownership patterns
- Use **async/await** for I/O operations
- Add **documentation comments** (`///`) for public APIs
- Write **tests** for new functionality

### Module Organization

```
├── common/           # Shared types & utilities
├── data-ingestion/   # Layer 0: Data sources
├── research-agents/  # Layer 1: AI agents
├── signal-generation/ # Layer 2: Alpha signals
├── portfolio-risk/   # Layer 3: Risk management
├── execution/        # Layer 4: Order execution
└── monitoring/       # Layer 5: Analytics
```

### Adding New Features

1. **Determine the layer** - Where does your feature fit?
2. **Update relevant module** - Add code to the appropriate crate
3. **Add tests** - Ensure your code is tested
4. **Update docs** - Modify README.md or ARCHITECTURE.md
5. **Run clippy** - `cargo clippy --all-targets`
6. **Format code** - `cargo fmt --all`

## Areas for Contribution

### Data Sources
- [ ] Twitter API connector
- [ ] Reddit API connector
- [ ] AP/Reuters news feeds
- [ ] Metaculus predictions API
- [ ] Manifold markets API

### Trading Strategies
- [ ] Additional arbitrage types
- [ ] Market making strategies
- [ ] Mean reversion
- [ ] Momentum strategies
- [ ] Cross-exchange arbitrage

### Risk Management
- [ ] VaR (Value at Risk) calculations
- [ ] CVaR (Conditional VaR)
- [ ] Greeks calculation for options
- [ ] Dynamic position sizing

### Machine Learning
- [ ] Real-time model training
- [ ] Ensemble methods
- [ ] Feature engineering pipeline
- [ ] Model versioning & rollback

### Infrastructure
- [ ] Prometheus metrics
- [ ] Grafana dashboards
- [ ] Alerting (PagerDuty/Slack)
- [ ] Rate limiting
- [ ] Circuit breakers

## Testing

### Unit Tests
```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific module
cargo test -p data-ingestion
```

### Integration Tests
```bash
# Test database connections
docker-compose up -d
cargo run --bin test_polymarket
```

## Pull Request Process

1. **Create a branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**
   - Write clear commit messages
   - Squash related commits
   - Update documentation

3. **Push and open PR**
   ```bash
   git push origin feature/your-feature-name
   gh pr create --title "Add your feature" --body "Description..."
   ```

4. **Address review feedback**
   - Respond to comments
   - Make requested changes
   - Keep the PR focused

## Code Review Criteria

- [ ] Tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Documentation updated
- [ ] No breaking changes (or clearly marked)
- [ ] Performance impact assessed

## Questions?

- Open an issue for bugs or feature requests
- Join discussions for architecture questions
- Check ARCHITECTURE.md for design decisions

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
