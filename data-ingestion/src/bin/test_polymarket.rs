use data_ingestion::connectors::polymarket::PolymarketConnector;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info};
use tracing_subscriber::fmt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting Polymarket WebSocket connection test...");

    // Note: This test connects to the real Polymarket WebSocket
    // It will receive data but won't publish to Kafka (KafkaProducer is mocked)
    // Run with: cargo run --bin test_polymarket

    // Create connector (will connect without asset IDs to get all market data)
    let connector = PolymarketConnector::new();

    // Connect to WebSocket
    info!("Connecting to Polymarket WebSocket...");
    let (mut ws_stream, response) = tokio_tungstenite::connect_async(connector.ws_url()).await?;

    info!("✅ Connected to Polymarket WebSocket!");
    info!("Response status: {:?}", response.status());

    // Send subscription message
    let subscribe_msg = serde_json::json!({
        "assets_ids": [],
        "type": "market",
        "custom_feature_enabled": true
    });

    let subscribe_json = serde_json::to_string(&subscribe_msg)?;
    info!("Sending subscription: {}", subscribe_json);
    ws_stream.send(Message::Text(subscribe_json)).await?;

    // Receive messages for 30 seconds
    info!("Listening for messages (will run for 30 seconds)...");
    let mut message_count = 0;
    let start = std::time::Instant::now();

    while start.elapsed().as_secs() < 30 {
        match ws_stream.next().await {
            Some(Ok(msg)) => {
                match msg {
                    Message::Text(text) => {
                        message_count += 1;
                        info!("Message #{}: {}", message_count, text);

                        // Try to parse and show message type
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(msg_type) = json.get("type").and_then(|v| v.as_str()) {
                                info!("  → Message type: {}", msg_type);
                            }
                        }
                    }
                    Message::Pong(_) => {
                        info!("Received PONG");
                    }
                    Message::Ping(_) => {
                        info!("Received PING from server");
                    }
                    Message::Close(_) => {
                        info!("Server closed connection");
                        break;
                    }
                    _ => {}
                }
            }
            Some(Err(e)) => {
                error!("WebSocket error: {}", e);
                break;
            }
            None => {
                info!("WebSocket stream ended");
                break;
            }
        }
    }

    info!("Test complete. Received {} messages in 30 seconds.", message_count);
    Ok(())
}
