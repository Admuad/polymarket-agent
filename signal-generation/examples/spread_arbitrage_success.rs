// Example: Successful Spread Arbitrage Signal Generation
// Demonstrates a case where a signal is successfully generated

use chrono::Utc;
use common::Market;
use rust_decimal::prelude::*;
use signal_generation::{
    PipelineConfig, SignalPipeline, SpreadArbitrageGenerator,
    EdgeThresholdValidator, EdgeThresholdConfig,
    ConfidenceValidator, ConfidenceValidatorConfig,
    LiquidityValidator, LiquidityValidatorConfig,
    ExpectedValueValidator, ExpectedValueValidatorConfig,
    InMemoryStorage, SignalStorage,
    SignalInput, ResearchOutput, SentimentScore, SentimentSource,
    PriceSnapshot,
};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Signal Generation Framework - Successful Spread Arbitrage Example ===\n");

    // Create market with arbitrage opportunity
    let market = create_arbitrage_market();

    println!("Market: {}", market.question);
    println!("Outcomes:");
    for outcome in &market.outcomes {
        println!("  - {}: ${:.2} (liquidity: ${:.0})",
            outcome.name, outcome.price, outcome.liquidity);
    }
    println!();

    // Calculate total market probability
    let total_prob: f64 = market.outcomes.iter().map(|o| o.price).sum();
    let edge = Decimal::ONE - Decimal::from_f64(total_prob).unwrap_or(Decimal::ONE);
    println!("Market Analysis:");
    println!("  Total Probability: {:.1}%", total_prob * 100.0);
    println!("  Edge: {:.1}%\n", edge * Decimal::from(100));

    // Create research output (from research agents)
    let research_output = ResearchOutput {
        market_id: market.id,
        analysis: "Based on comprehensive analysis, YES outcome has significantly higher probability than market pricing suggests.".to_string(),
        sentiment: SentimentScore {
            overall: 0.65,
            sources: vec![
                SentimentSource { name: "Twitter".to_string(), score: 0.70, weight: 0.3 },
                SentimentSource { name: "Reddit".to_string(), score: 0.60, weight: 0.2 },
                SentimentSource { name: "News".to_string(), score: 0.65, weight: 0.5 },
            ],
        },
        confidence: 0.80,
        probability_estimate: Some(0.58),
        key_factors: vec![
            "Strong technical indicators".to_string(),
            "Institutional interest increasing".to_string(),
            "Recent positive news flow".to_string(),
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
            price: Decimal::from_f64(0.42).unwrap(),
            volume: Decimal::from_f64(8000.0).unwrap(),
            liquidity: Decimal::from_f64(12000.0).unwrap(),
            timestamp: Utc::now() - chrono::Duration::hours(24),
        },
        PriceSnapshot {
            outcome_id: "yes".to_string(),
            price: Decimal::from_f64(0.44).unwrap(),
            volume: Decimal::from_f64(9000.0).unwrap(),
            liquidity: Decimal::from_f64(13000.0).unwrap(),
            timestamp: Utc::now() - chrono::Duration::hours(12),
        },
        PriceSnapshot {
            outcome_id: "yes".to_string(),
            price: Decimal::from_f64(0.45).unwrap(),
            volume: Decimal::from_f64(10000.0).unwrap(),
            liquidity: Decimal::from_f64(15000.0).unwrap(),
            timestamp: Utc::now(),
        },
    ];

    // Create signal input
    let input = SignalInput {
        market: market.clone(),
        research_output: research_output.clone(),
        order_book: None,
        price_history,
    };

    // Create pipeline configuration
    let config = PipelineConfig {
        enabled: true,
        max_signals_per_cycle: 5,
        min_confidence: 0.5,
        min_edge: Decimal::from_str_exact("0.02").unwrap(), // 2%
    };

    println!("=== Creating Signal Pipeline ===\n");

    // Build the pipeline with generators and custom validators (lower thresholds for demo)
    let pipeline = SignalPipeline::new(config)
        .add_generator(Box::new(SpreadArbitrageGenerator::default()))
        .add_validator(Box::new(EdgeThresholdValidator::new(EdgeThresholdConfig {
            min_edge: Decimal::from_str_exact("0.02").unwrap(), // 2%
        })))
        .add_validator(Box::new(ConfidenceValidator::new(ConfidenceValidatorConfig {
            min_confidence: 0.5, // 50%
        })))
        .add_validator(Box::new(LiquidityValidator::new(LiquidityValidatorConfig {
            min_liquidity_score: 0.2,
            max_position_liquidity_ratio: 0.2, // More lenient
        })))
        .add_validator(Box::new(ExpectedValueValidator::new(ExpectedValueValidatorConfig {
            min_expected_value: Decimal::from_str_exact("0.1").unwrap(), // $0.10 for demo
        })))
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

        // Demonstrate signal storage
        println!("=== Signal Storage ===\n");

        let storage = InMemoryStorage::new();
        for signal in &signals {
            storage.store(signal).await?;
        }

        let stats = storage.stats().await?;
        println!("Storage Statistics:");
        println!("  Total Signals: {}", stats.total_signals);
        println!("  Signals by Type:");
        for (signal_type, count) in stats.signals_by_type.iter() {
            println!("    - {}: {}", signal_type, count);
        }
        println!("  Oldest Signal: {:?}", stats.oldest_signal);
        println!("  Newest Signal: {:?}", stats.newest_signal);
        println!();
    }

    Ok(())
}

fn create_arbitrage_market() -> Market {
    // Create market with clear arbitrage opportunity
    // Total probability: 0.45 + 0.42 = 0.87 (13% edge)
    Market {
        id: Uuid::new_v4(),
        condition_id: "0x456...".to_string(),
        question: "Will Ethereum exceed $5,000 by end of Q2 2026?".to_string(),
        description: "This market resolves YES if Ethereum trades above $5,000 on any major exchange before June 30, 2026.".to_string(),
        category: "Cryptocurrency".to_string(),
        outcomes: vec![
            common::Outcome {
                id: "yes".to_string(),
                name: "Yes".to_string(),
                price: 0.45,  // Undervalued
                liquidity: 15000.0,
            },
            common::Outcome {
                id: "no".to_string(),
                name: "No".to_string(),
                price: 0.42,  // Also undervalued
                liquidity: 12000.0,
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
    println!("  Research Sources: {}", signal.metadata.research_sources.join(", "));
    println!("  Data Points: {}", signal.metadata.data_points);
    println!("  Liquidity Score: {:.2}", signal.metadata.liquidity_score);
    println!("  Volatility Score: {:.2}", signal.metadata.volatility_score);
    println!("  Created At: {}", signal.created_at);
    if let Some(expires_at) = signal.expires_at {
        println!("  Expires At: {}", expires_at);
    }
    println!();
}
