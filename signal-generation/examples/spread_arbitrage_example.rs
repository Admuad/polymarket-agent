// Example: Spread Arbitrage Signal Generation
// Demonstrates how to use the signal generation framework to generate trade signals

use chrono::Utc;
use common::Market;
use rust_decimal::prelude::*;
use signal_generation::{
    PipelineConfig, SignalPipeline, SpreadArbitrageGenerator,
    EdgeThresholdValidator, ConfidenceValidator, LiquidityValidator,
    ExpectedValueValidator, InMemoryStorage, SignalStorage,
    SignalInput, ResearchOutput, SentimentScore, SentimentSource,
    PriceSnapshot,
};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Signal Generation Framework - Spread Arbitrage Example ===\n");

    // Create sample market data
    let market = create_sample_market();

    println!("Market: {}", market.question);
    println!("Outcomes:");
    for outcome in &market.outcomes {
        println!("  - {}: ${:.2} (liquidity: ${:.0})",
            outcome.name, outcome.price, outcome.liquidity);
    }
    println!();

    // Create research output (from research agents)
    let research_output = ResearchOutput {
        market_id: market.id,
        analysis: "Based on recent polling data and sentiment analysis, YES outcome has increased probability.".to_string(),
        sentiment: SentimentScore {
            overall: 0.4,
            sources: vec![
                SentimentSource { name: "Twitter".to_string(), score: 0.5, weight: 0.3 },
                SentimentSource { name: "Reddit".to_string(), score: 0.3, weight: 0.2 },
                SentimentSource { name: "News".to_string(), score: 0.4, weight: 0.5 },
            ],
        },
        confidence: 0.75,
        probability_estimate: Some(0.52),
        key_factors: vec![
            "Recent poll shows 52% support".to_string(),
            "Influencer endorsements trending positive".to_string(),
        ],
        timestamp: Utc::now(),
    };

    println!("Research Analysis:");
    println!("  Sentiment: {:.2}", research_output.sentiment.overall);
    println!("  Confidence: {:.0}%", research_output.confidence * 100.0);
    println!("  Probability Estimate: {:.0}%",
        research_output.probability_estimate.unwrap_or(0.0) * 100.0);
    println!();

    // Create price history
    let price_history = vec![
        PriceSnapshot {
            outcome_id: "yes".to_string(),
            price: Decimal::from_f64(0.48).unwrap(),
            volume: Decimal::from_f64(5000.0).unwrap(),
            liquidity: Decimal::from_f64(8000.0).unwrap(),
            timestamp: Utc::now() - chrono::Duration::hours(24),
        },
        PriceSnapshot {
            outcome_id: "yes".to_string(),
            price: Decimal::from_f64(0.49).unwrap(),
            volume: Decimal::from_f64(6000.0).unwrap(),
            liquidity: Decimal::from_f64(9000.0).unwrap(),
            timestamp: Utc::now() - chrono::Duration::hours(12),
        },
        PriceSnapshot {
            outcome_id: "yes".to_string(),
            price: Decimal::from_f64(0.50).unwrap(),
            volume: Decimal::from_f64(7000.0).unwrap(),
            liquidity: Decimal::from_f64(10000.0).unwrap(),
            timestamp: Utc::now(),
        },
    ];

    // Create signal input
    let input = SignalInput {
        market: market.clone(),
        research_output: research_output.clone(),
        order_book: None, // Optional order book data
        price_history,
    };

    // Create pipeline configuration
    let config = PipelineConfig {
        enabled: true,
        max_signals_per_cycle: 5,
        min_confidence: 0.6,
        min_edge: Decimal::from_str_exact("0.03").unwrap(), // 3%
    };

    println!("=== Creating Signal Pipeline ===\n");

    // Build the pipeline with generators and validators
    let pipeline = SignalPipeline::new(config)
        .add_generator(Box::new(SpreadArbitrageGenerator::default()))
        .add_validator(Box::new(EdgeThresholdValidator::default()))
        .add_validator(Box::new(ConfidenceValidator::default()))
        .add_validator(Box::new(LiquidityValidator::default()))
        .add_validator(Box::new(ExpectedValueValidator::default()))
        .with_storage(Box::new(InMemoryStorage::new()));

    println!("Pipeline configured with:");
    println!("  - Signal Generators: {}", pipeline.generator_count());
    println!("  - Signal Validators: {}", pipeline.validator_count());
    println!();

    println!("=== Generating Signals ===\n");

    // Process the input and generate signals
    let signals = pipeline.process(&input).await?;

    if signals.is_empty() {
        println!("No signals generated (filters may have rejected all signals)");
    } else {
        println!("Generated {} signal(s):\n", signals.len());

        for (i, signal) in signals.iter().enumerate() {
            print_signal(signal, i + 1);
        }
    }

    // Demonstrate signal storage
    println!("=== Signal Storage ===\n");

    let storage = InMemoryStorage::new();
    if let Some(signal) = signals.first() {
        storage.store(signal).await?;

        let stats = storage.stats().await?;
        println!("Storage Statistics:");
        println!("  Total Signals: {}", stats.total_signals);
        println!("  Oldest Signal: {:?}", stats.oldest_signal);
        println!("  Newest Signal: {:?}", stats.newest_signal);
        println!();
    }

    // Demonstrate signal validator logic
    println!("=== Signal Validator Logic ===\n");
    explain_validators();

    Ok(())
}

fn create_sample_market() -> Market {
    Market {
        id: Uuid::new_v4(),
        condition_id: "0x123...".to_string(),
        question: "Will Bitcoin exceed $100,000 by end of 2026?".to_string(),
        description: "This market resolves YES if Bitcoin trades above $100,000 on any major exchange before December 31, 2026.".to_string(),
        category: "Cryptocurrency".to_string(),
        outcomes: vec![
            common::Outcome {
                id: "yes".to_string(),
                name: "Yes".to_string(),
                price: 0.50,
                liquidity: 10000.0,
            },
            common::Outcome {
                id: "no".to_string(),
                name: "No".to_string(),
                price: 0.45,
                liquidity: 8000.0,
            },
        ],
        created_at: Utc::now() - chrono::Duration::days(30),
        updated_at: Utc::now(),
    }
}

fn print_signal(signal: &signal_generation::TradeSignal, index: usize) {
    println!("--- Signal #{} ---", index);
    println!("ID: {}", signal.id);
    println!("Type: {:?}", signal.signal_type);
    println!("Direction: {:?}", signal.direction);
    println!("Outcome: {:?}", signal.outcome_id);
    println!("Entry Price: ${:.4}", signal.entry_price);
    println!("Target Price: ${:.4}", signal.target_price);
    println!("Stop Loss: ${:.4}", signal.stop_loss);
    println!("Position Size: ${:.2}", signal.position_size);
    println!("Confidence: {:.1}%", signal.confidence * 100.0);
    println!("Expected Value: ${:.2}", signal.expected_value);
    println!("Edge: {:.2}%", signal.edge * Decimal::from(100));
    println!("Kelly Fraction: {:.1}%", signal.kelly_fraction * 100.0);
    println!();
    println!("Reasoning:");
    for line in signal.reasoning.lines() {
        println!("  {}", line);
    }
    println!();
    println!("Metadata:");
    println!("  Data Points: {}", signal.metadata.data_points);
    println!("  Liquidity Score: {:.2}", signal.metadata.liquidity_score);
    println!("  Volatility Score: {:.2}", signal.metadata.volatility_score);
    println!("  Created At: {}", signal.created_at);
    if let Some(expires_at) = signal.expires_at {
        println!("  Expires At: {}", expires_at);
    }
    println!();
}

fn explain_validators() {
    println!("1. Edge Threshold Validator");
    println!("   - Checks if the signal has sufficient edge (profit margin)");
    println!("   - Default: Minimum 5% edge required");
    println!("   - Formula: edge = market_implied_probability - true_probability");
    println!();

    println!("2. Confidence Validator");
    println!("   - Checks if the signal has sufficient confidence level");
    println!("   - Default: Minimum 70% confidence required");
    println!("   - Combines research confidence with data quality metrics");
    println!();

    println!("3. Liquidity Validator");
    println!("   - Checks if there is sufficient liquidity to execute the trade");
    println!("   - Default: Minimum liquidity score of 0.3");
    println!("   - Also ensures position size doesn't exceed 10% of available liquidity");
    println!();

    println!("4. Expected Value Validator");
    println!("   - Checks if the trade has positive expected value");
    println!("   - Default: Minimum EV of $5.00");
    println!("   - EV = (win_probability × win_amount) - (lose_probability × loss_amount)");
    println!();

    println!("All validators must pass for a signal to be approved for execution.");
}
