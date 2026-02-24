//! Example usage of the Research Agents Framework
//!
//! This example demonstrates:
//! 1. Setting up the orchestrator and agent bus
//! 2. Creating a sentiment agent
//! 3. Adding markets and news articles
//! 4. Running the agents and collecting signals
//! 5. Example output format

use anyhow::Result;
use chrono::Utc;
use common::Market;
use research_agents::{
    Agent, AgentBus, AgentBusConfig, AgentBusHandle, AgentInput, Orchestrator, OrchestratorConfig,
    SentimentAgent, SentimentAgentConfig, SentimentSignal,
};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, Level};
use tracing_subscriber;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🤖 Research Agents Framework - Example");
    info!("=====================================");

    // Step 1: Create the agent bus
    let bus = Arc::new(AgentBus::new(AgentBusConfig::default()).await?);
    info!("✅ Agent bus created");

    // Step 2: Create the orchestrator
    let mut orchestrator = Orchestrator::new(
        OrchestratorConfig {
            max_concurrent_markets: 10,
            max_concurrent_agents: 5,
            scan_interval_secs: 5,
            ..Default::default()
        },
        Arc::clone(&bus),
    ).await?;
    info!("✅ Orchestrator created");

    // Step 3: Create and register the sentiment agent
    let sentiment_config = SentimentAgentConfig::default();
    let sentiment_agent_for_registration = SentimentAgent::new(sentiment_config.clone());

    // Create a boxed version for registration
    let sentiment_agent_boxed: Box<dyn Agent> = Box::new(sentiment_agent_for_registration);

    let handle = orchestrator.control_handle();
    handle.register_agent(sentiment_agent_boxed).await?;
    info!("✅ Sentiment agent registered");

    // Also create a separate agent for manual processing in the demo
    let sentiment_agent = SentimentAgent::new(sentiment_config);

    // Step 4: Create example markets
    let markets = create_example_markets();
    let market_count = markets.len();
    info!("📊 Created {} example markets", market_count);

    // Add markets to orchestrator
    handle.add_markets(markets.clone()).await?;
    info!("✅ Markets added to orchestrator");

    // Step 5: Add news articles to sentiment agent
    let articles = create_example_articles();
    let article_count = articles.len();
    sentiment_agent.add_articles(articles).await;
    info!("📰 Added {} news articles", article_count);

    // Step 6: Start the orchestrator
    handle.start().await?;
    info!("🚀 Orchestrator started");

    // Wait for processing
    sleep(Duration::from_secs(2)).await;

    // Step 7: Manually process a market to show output format
    info!("\n=== MANUAL PROCESSING EXAMPLE ===\n");
    let market = &markets[0];
    let input = AgentInput {
        market: Arc::new(market.clone()),
        timestamp: Utc::now(),
        additional_data: None,
    };

    if let Some(output) = sentiment_agent.process_market(input).await? {
        print_agent_output(&output)?;
    }

    // Show example signal format
    info!("\n=== EXAMPLE SIGNAL FORMAT ===\n");
    print_example_signal()?;

    // Stop orchestrator
    handle.stop().await?;
    info!("\n✅ Orchestrator stopped");

    Ok(())
}

fn create_example_markets() -> Vec<Market> {
    vec![
        Market {
            id: Uuid::new_v4(),
            condition_id: "cond1".to_string(),
            question: "Will Donald Trump win the 2024 US Presidential Election?".to_string(),
            description: "This market resolves to YES if Donald Trump wins the 2024 US Presidential Election.".to_string(),
            category: "Politics".to_string(),
            outcomes: vec![
                common::Outcome {
                    id: "yes".to_string(),
                    name: "Yes".to_string(),
                    price: 0.52,
                    liquidity: 1000000.0,
                },
                common::Outcome {
                    id: "no".to_string(),
                    name: "No".to_string(),
                    price: 0.48,
                    liquidity: 1000000.0,
                },
            ],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        Market {
            id: Uuid::new_v4(),
            condition_id: "cond2".to_string(),
            question: "Will Bitcoin exceed $100,000 by end of 2024?".to_string(),
            description: "This market resolves to YES if Bitcoin (BTC) exceeds $100,000 USD on any major exchange by December 31, 2024.".to_string(),
            category: "Economics".to_string(),
            outcomes: vec![
                common::Outcome {
                    id: "yes".to_string(),
                    name: "Yes".to_string(),
                    price: 0.35,
                    liquidity: 500000.0,
                },
                common::Outcome {
                    id: "no".to_string(),
                    name: "No".to_string(),
                    price: 0.65,
                    liquidity: 500000.0,
                },
            ],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        Market {
            id: Uuid::new_v4(),
            condition_id: "cond3".to_string(),
            question: "Will Russia invade another country in 2024?".to_string(),
            description: "This market resolves to YES if Russian military forces invade any country not currently in conflict by December 31, 2024.".to_string(),
            category: "Geopolitics".to_string(),
            outcomes: vec![
                common::Outcome {
                    id: "yes".to_string(),
                    name: "Yes".to_string(),
                    price: 0.28,
                    liquidity: 750000.0,
                },
                common::Outcome {
                    id: "no".to_string(),
                    name: "No".to_string(),
                    price: 0.72,
                    liquidity: 750000.0,
                },
            ],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ]
}

fn create_example_articles() -> Vec<research_agents::sentiment::NewsArticle> {
    vec![
        research_agents::sentiment::NewsArticle {
            id: "article1".to_string(),
            url: "https://example.com/news1".to_string(),
            title: "Trump gains in latest polls as election approaches".to_string(),
            themes: "ELECTION;POLITICS;USA;PRESIDENT;CAMPAIGN".to_string(),
            tone: 30.0, // Slightly positive
            timestamp: Utc::now(),
        },
        research_agents::sentiment::NewsArticle {
            id: "article2".to_string(),
            url: "https://example.com/news2".to_string(),
            title: "Stock markets rally on positive economic data".to_string(),
            themes: "ECONOMY;MARKET;GROWTH;STOCK;FINANCE".to_string(),
            tone: 60.0, // Positive
            timestamp: Utc::now(),
        },
        research_agents::sentiment::NewsArticle {
            id: "article3".to_string(),
            url: "https://example.com/news3".to_string(),
            title: "Bitcoin surges past $95,000 as institutional demand grows".to_string(),
            themes: "CRYPTO;BITCOIN;FINANCE;ECONOMY;GROWTH".to_string(),
            tone: 70.0, // Strong positive
            timestamp: Utc::now(),
        },
        research_agents::sentiment::NewsArticle {
            id: "article4".to_string(),
            url: "https://example.com/news4".to_string(),
            title: "Tensions rise in Eastern Europe as military exercises increase".to_string(),
            themes: "MILITARY;RUSSIA;CONFLICT;GEOPOLITICS;WAR".to_string(),
            tone: -40.0, // Negative
            timestamp: Utc::now(),
        },
        research_agents::sentiment::NewsArticle {
            id: "article5".to_string(),
            url: "https://example.com/news5".to_string(),
            title: "Federal Reserve signals potential interest rate cuts".to_string(),
            themes: "ECONOMY;FED;INTEREST_RATE;MONETARY_POLICY".to_string(),
            tone: 50.0, // Positive
            timestamp: Utc::now(),
        },
    ]
}

fn print_agent_output(output: &research_agents::agent::AgentOutput) -> Result<()> {
    info!("📡 Agent Output:");
    info!("  Agent ID: {}", output.agent_id);
    info!("  Market ID: {}", output.market_id);
    info!("  Signal Type: {}", output.signal_type);
    info!("  Confidence: {:.2}%", output.confidence * 100.0);
    info!("  Processing Time: {}ms", output.processing_time_ms);
    info!("  Timestamp: {}", output.timestamp);

    // Parse and display the signal data
    if let Ok(signal) = serde_json::from_value::<SentimentSignal>(output.data.clone()) {
        info!("\n  Sentiment Signal:");
        info!("    Score: {:.2} (range: -1.0 to 1.0)", signal.sentiment.score);
        info!("    Magnitude: {:.2} (strength of sentiment)", signal.sentiment.magnitude);
        info!("    Confidence: {:.2}", signal.sentiment.confidence);
        info!("    Article Count: {}", signal.sentiment.article_count);
        info!("    Top Themes: {:?}", signal.top_themes);
        info!("    Sources: {} articles", signal.sources.len());
    }

    Ok(())
}

fn print_example_signal() -> Result<()> {
    let example_signal = SentimentSignal {
        market_id: Uuid::new_v4(),
        market_category: "Economics".to_string(),
        sentiment: research_agents::sentiment::SentimentScore {
            score: 0.65,
            magnitude: 0.65,
            confidence: 0.78,
            article_count: 12,
        },
        top_themes: vec![
            "bitcoin".to_string(),
            "growth".to_string(),
            "finance".to_string(),
            "crypto".to_string(),
            "economy".to_string(),
        ],
        timestamp: Utc::now(),
        sources: vec![
            "article1".to_string(),
            "article2".to_string(),
            "article3".to_string(),
        ],
    };

    info!("Example SentimentSignal JSON:");
    let json = serde_json::to_string_pretty(&example_signal)?;
    info!("\n{}\n", json);

    info!("Interpretation:");
    info!("  - Score 0.65: Strongly positive sentiment");
    info!("  - Magnitude 0.65: Strong signal (not weak/neutral)");
    info!("  - Confidence 0.78: Good confidence based on article count and consistency");
    info!("  - 12 articles analyzed: Good sample size");

    Ok(())
}
