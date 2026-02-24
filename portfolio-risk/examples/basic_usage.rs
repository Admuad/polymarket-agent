//! Example usage of the Portfolio & Risk Management system

use portfolio_risk::{
    PortfolioRiskManager, RiskConfig, RiskLimits, Portfolio
};
use common::{OrderSide, Uuid};

fn main() -> anyhow::Result<()> {
    println!("=== Portfolio & Risk Management Example ===\n");

    // Example 1: Create a portfolio risk manager
    println!("Example 1: Creating Portfolio Risk Manager");
    let manager = PortfolioRiskManager::new()?;
    println!("✓ Manager created with default configuration\n");

    // Example 2: Check portfolio summary
    println!("Example 2: Portfolio Summary");
    let summary = manager.get_summary();
    println!("  Total Value: ${:.2}", summary.total_value);
    println!("  Positions: {}", summary.num_positions);
    println!("  Total PnL: ${:.2}", summary.total_pnl);
    println!("  Risk Level: {:?}\n", summary.risk_level);

    // Example 3: Create custom risk configuration
    println!("Example 3: Custom Risk Configuration");
    let custom_config = RiskConfig {
        risk_limits: RiskLimits {
            max_position_size: 50.0,      // $50 per position
            max_total_exposure: 500.0,     // $500 total
            max_theme_exposure: 250.0,     // $250 per theme
            max_positions: 10,              // Max 10 positions
            max_theme_percentage: 0.30,     // 30% max per theme
            daily_loss_limit: 50.0,        // $50 daily loss limit
            stop_loss_percentage: 0.20,    // 20% stop loss
            ..Default::default()
        },
        ..Default::default()
    };
    println!("✓ Custom configuration created\n");

    // Example 4: Evaluate a trade
    println!("Example 4: Evaluating a Potential Trade");
    let market_id = Uuid::new_v4();
    let evaluation = manager.evaluate_trade(
        market_id,
        "YES",
        OrderSide::Buy,
        0.5,   // Price: 50 cents
        25.0,  // Size: $25
    );

    match evaluation {
        Ok(eval) => {
            println!("  Trade: {:?}", eval.approved);
            println!("  Kelly Limit: ${:.2}", eval.kelly_limit);
            println!("  Risk Level: {:?}\n", eval.risk_level);
        }
        Err(violation) => {
            println!("  ✗ Trade rejected: {}\n", violation);
        }
    }

    // Example 5: Risk limit violation
    println!("Example 5: Testing Risk Limit Violation");
    let large_trade = manager.evaluate_trade(
        Uuid::new_v4(),
        "YES",
        OrderSide::Buy,
        0.5,
        150.0,  // Exceeds default max_position_size of $100
    );

    match large_trade {
        Ok(_) => println!("  Unexpected: Trade approved\n"),
        Err(violation) => println!("  ✗ Expected violation: {}\n", violation),
    }

    // Example 6: Using Kelly Criterion directly
    println!("Example 6: Kelly Criterion Position Sizing");
    use portfolio_risk::Kelly;

    // Kelly with no edge = 0 (conservative, no advantage over market)
    let kelly_conservative = Kelly::new(0.25, None);
    let position_conservative = kelly_conservative.calculate_position(0.5, 1000.0);

    println!("  Without edge (no advantage over market):");
    println!("  - Price: ${:.2}, Bankroll: $1000.00", 0.5);
    println!("  - Kelly position: ${:.2}", position_conservative);

    // Kelly with edge = positive position
    let kelly_with_edge = Kelly::new(0.25, Some(0.05)); // 5% edge
    let position_with_edge = kelly_with_edge.calculate_position(0.5, 1000.0);

    println!("\n  With 5% edge (better than market):");
    println!("  - Price: ${:.2}, Bankroll: $1000.00", 0.5);
    println!("  - Kelly position: ${:.2}", position_with_edge);
    println!("  - Position size: {:.2}% of bankroll\n",
        (position_with_edge / 1000.0) * 100.0);

    // Example 7: Portfolio position tracking
    println!("Example 7: Position Tracking");
    let mut portfolio = Portfolio::new();
    let market_id_1 = Uuid::new_v4();

    portfolio.add_position(market_id_1, "YES", 50.0, 0.5)?;
    portfolio.update_price(market_id_1, "YES", 0.55);

    println!("  Position 1 added: $50.00 at 0.50");
    println!("  Price updated to: 0.55");
    println!("  Unrealized PnL: ${:.2}", portfolio.unrealized_pnl());
    println!("  Total Value: ${:.2}", portfolio.total_value());
    println!("  Positions: {}\n", portfolio.num_positions());

    // Example 8: Risk metrics calculation
    println!("Example 8: Risk Metrics");
    let metrics = portfolio.calculate_metrics();
    println!("  Total Value: ${:.2}", metrics.total_value);
    println!("  Unrealized PnL: ${:.2}", metrics.unrealized_pnl);
    println!("  Max Drawdown: {:.2}%", metrics.max_drawdown * 100.0);

    if let Some(var_95) = metrics.var_95 {
        println!("  VaR (95%): ${:.2}", var_95);
    }

    if let Some(sharpe) = metrics.sharpe_ratio {
        println!("  Sharpe Ratio: {:.2}", sharpe);
    }
    println!();

    // Example 9: Simulating market resolution
    println!("Example 9: Simulating Market Resolution");
    let market_id_2 = Uuid::new_v4();

    portfolio.add_position(market_id_2, "YES", 30.0, 0.6)?;
    portfolio.add_position(market_id_2, "NO", 20.0, 0.4)?;

    println!("  Before resolution: ${:.2}", portfolio.total_value());

    let pnl = portfolio.resolve_market(market_id_2, "YES")?;
    println!("  Market resolved: YES wins");
    println!("  Realized PnL: ${:.2}", pnl);
    println!("  Total Realized PnL: ${:.2}", portfolio.total_pnl());
    println!("  Total Value: ${:.2}", portfolio.total_value());
    println!();

    println!("=== Example Complete ===");
    Ok(())
}

// Example configuration template
fn example_config_template() -> String {
    r#"
# Example Risk Configuration

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
"#.to_string()
}
