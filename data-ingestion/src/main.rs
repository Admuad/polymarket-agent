use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber;

mod event_bus;
mod connectors;
mod databases;

use event_bus::KafkaProducer;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🚀 Starting Polymarket Data Ingestion Service");

    // Initialize Kafka producer
    let kafka_producer = KafkaProducer::new("localhost:9092").await?;

    // Start connectors
    tokio::select! {
        result = connectors::polymarket::PolymarketConnector::run(&kafka_producer) => {
            result?
        }
        _ = tokio::signal::ctrl_c() => {
            info!("👋 Shutting down gracefully...");
        }
    }

    Ok(())
}
