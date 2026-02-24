use anyhow::{anyhow, Result};
use chrono::{DateTime, TimeZone, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::time::{interval, sleep};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

use crate::event_bus::KafkaProducer;
use common::{Market, MarketEvent, Order, OrderBook, OrderSide, Outcome, PriceTick, Trade};

/// Polymarket CLOB WebSocket connector
pub struct PolymarketConnector {
    ws_url: String,
    asset_ids: Vec<String>,
    reconnect_delay: u64,
    heartbeat_interval_secs: u64,
}

/// Subscription message for market channel
#[derive(Debug, Serialize)]
struct MarketSubscription {
    assets_ids: Vec<String>,
    #[serde(rename = "type")]
    msg_type: String,
    custom_feature_enabled: bool,
}

/// Dynamic subscription message
#[derive(Debug, Serialize)]
struct DynamicSubscription {
    assets_ids: Vec<String>,
    operation: String,
}

/// Polymarket WebSocket message types
#[derive(Debug, Deserialize)]
struct WsMessage {
    #[serde(flatten)]
    content: WsMessageContent,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsMessageContent {
    Book(BookMessage),
    PriceChange(PriceChangeMessage),
    TickSizeChange(TickSizeChangeMessage),
    LastTradePrice(LastTradePriceMessage),
    BestBidAsk(BestBidAskMessage),
    NewMarket(NewMarketMessage),
    MarketResolved(MarketResolvedMessage),
}

/// Full orderbook snapshot
#[derive(Debug, Deserialize)]
struct BookMessage {
    asset_id: String,
    bids: Vec<OrderLevel>,
    asks: Vec<OrderLevel>,
    #[serde(default)]
    timestamp: i64,
}

/// Price level updates
#[derive(Debug, Deserialize)]
struct PriceChangeMessage {
    asset_id: String,
    price: f64,
    #[serde(default)]
    timestamp: i64,
}

/// Tick size changes
#[derive(Debug, Deserialize)]
struct TickSizeChangeMessage {
    asset_id: String,
    tick_size: f64,
}

/// Trade executions
#[derive(Debug, Deserialize)]
struct LastTradePriceMessage {
    asset_id: String,
    price: f64,
    size: f64,
    side: String,
    #[serde(default)]
    timestamp: i64,
}

/// Best prices update
#[derive(Debug, Deserialize)]
struct BestBidAskMessage {
    asset_id: String,
    best_bid: Option<PriceLevel>,
    best_ask: Option<PriceLevel>,
}

#[derive(Debug, Clone, Deserialize)]
struct PriceLevel {
    price: f64,
    size: f64,
}

/// New market created
#[derive(Debug, Deserialize)]
struct NewMarketMessage {
    condition_id: String,
    question: String,
    description: String,
    outcomes: Vec<OutcomeData>,
    #[serde(default)]
    created_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct OutcomeData {
    id: String,
    name: String,
    #[serde(default)]
    price: Option<f64>,
    #[serde(default)]
    liquidity: Option<f64>,
}

/// Market resolution
#[derive(Debug, Deserialize)]
struct MarketResolvedMessage {
    condition_id: String,
    winning_outcome_id: String,
}

/// Order level for orderbook
#[derive(Debug, Deserialize)]
struct OrderLevel {
    price: f64,
    size: f64,
}

impl PolymarketConnector {
    pub fn new() -> Self {
        Self {
            // Polymarket CLOB WebSocket endpoint for market data
            ws_url: "wss://ws-subscriptions-clob.polymarket.com/ws/market".to_string(),
            asset_ids: Vec::new(),
            reconnect_delay: 5,
            heartbeat_interval_secs: 10,
        }
    }

    pub fn ws_url(&self) -> &str {
        &self.ws_url
    }

    pub fn with_assets(asset_ids: Vec<String>) -> Self {
        Self {
            ws_url: "wss://ws-subscriptions-clob.polymarket.com/ws/market".to_string(),
            asset_ids,
            reconnect_delay: 5,
            heartbeat_interval_secs: 10,
        }
    }

    pub async fn run(producer: &KafkaProducer) -> Result<()> {
        let connector = Self::new();

        // Main reconnection loop
        loop {
            match connector.connect_and_run(producer).await {
                Ok(_) => {
                    info!("Polymarket WebSocket connection closed normally");
                }
                Err(e) => {
                    error!("Polymarket WebSocket connection failed: {}", e);
                    info!("Reconnecting in {} seconds...", connector.reconnect_delay);
                    sleep(tokio::time::Duration::from_secs(connector.reconnect_delay)).await;
                }
            }
        }
    }

    async fn connect_and_run(&self, producer: &KafkaProducer) -> Result<()> {
        info!("Connecting to Polymarket CLOB WebSocket at {}", self.ws_url);

        let (ws_stream, response) = tokio_tungstenite::connect_async(&self.ws_url).await?;

        info!("✅ Connected to Polymarket CLOB WebSocket");
        debug!("Response status: {:?}", response.status());

        let (mut write, mut read) = ws_stream.split();

        // Send subscription message immediately
        let subscribe_msg = MarketSubscription {
            assets_ids: self.asset_ids.clone(),
            msg_type: "market".to_string(),
            custom_feature_enabled: true,
        };

        let subscribe_json = serde_json::to_string(&subscribe_msg)?;
        info!("Sending subscription: {}", subscribe_json);
        write.send(Message::Text(subscribe_json)).await?;

        // Start heartbeat task (simplified - no separate thread for now)
        let heartbeat_interval = self.heartbeat_interval_secs;
        let mut heartbeat_ticker = interval(tokio::time::Duration::from_secs(heartbeat_interval));

        // Process incoming messages with heartbeat
        loop {
            tokio::select! {
                msg_result = read.next() => {
                    match msg_result {
                        Some(Ok(msg)) => {
                            match msg {
                                Message::Text(text) => {
                                    if let Err(e) = self.handle_text_message(&text, producer).await {
                                        error!("Failed to handle text message: {}", e);
                                    }
                                }
                                Message::Pong(_) => {
                                    debug!("Received PONG");
                                }
                                Message::Ping(_) => {
                                    // Respond to server pings
                                    if let Err(e) = write.send(Message::Pong(vec![])).await {
                                        error!("Failed to send PONG response: {}", e);
                                        break;
                                    }
                                }
                                Message::Close(_) => {
                                    info!("WebSocket closed by server");
                                    break;
                                }
                                Message::Binary(data) => {
                                    warn!("Received unexpected binary message: {} bytes", data.len());
                                }
                                Message::Frame(_) => {
                                    // Raw frame - ignore for now
                                    debug!("Received raw frame");
                                }
                            }
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            return Err(anyhow!("WebSocket stream error: {}", e));
                        }
                        None => {
                            info!("WebSocket stream ended");
                            break;
                        }
                    }
                }
                _ = heartbeat_ticker.tick() => {
                    // Send heartbeat
                    if let Err(e) = write.send(Message::Ping(vec![])).await {
                        error!("Failed to send heartbeat: {}", e);
                        break;
                    }
                    debug!("Sent PING heartbeat");
                }
            }
        }

        Ok(())
    }

    async fn handle_text_message(&self, text: &str, producer: &KafkaProducer) -> Result<()> {
        debug!("Received message: {}", text);

        // Try to parse as WebSocket message
        if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(text) {
            match ws_msg.content {
                WsMessageContent::Book(msg) => {
                    self.handle_orderbook_update(msg, producer).await?;
                }
                WsMessageContent::PriceChange(msg) => {
                    self.handle_price_change(msg, producer).await?;
                }
                WsMessageContent::LastTradePrice(msg) => {
                    self.handle_trade(msg, producer).await?;
                }
                WsMessageContent::BestBidAsk(msg) => {
                    self.handle_best_bid_ask(msg, producer).await?;
                }
                WsMessageContent::NewMarket(msg) => {
                    self.handle_new_market(msg, producer).await?;
                }
                WsMessageContent::MarketResolved(msg) => {
                    self.handle_market_resolved(msg, producer).await?;
                }
                WsMessageContent::TickSizeChange(msg) => {
                    debug!("Tick size change for {}: {}", msg.asset_id, msg.tick_size);
                }
            }
        } else {
            // Try to parse as raw JSON for debugging
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                debug!("Unparsed JSON message: {:?}", json);
            } else {
                warn!("Failed to parse message: {}", text);
            }
        }

        Ok(())
    }

    async fn handle_orderbook_update(&self, msg: BookMessage, producer: &KafkaProducer) -> Result<()> {
        let timestamp = if msg.timestamp > 0 {
            Utc.timestamp_opt(msg.timestamp / 1000, 0).single().unwrap_or(Utc::now())
        } else {
            Utc::now()
        };

        // Convert asset_id to UUID (simplified - in production you'd map asset IDs to market IDs)
        let market_id = uuid::Uuid::new_v4();

        let bids: Vec<Order> = msg.bids
            .into_iter()
            .map(|level| Order {
                outcome_id: msg.asset_id.clone(),
                price: level.price,
                size: level.size,
            })
            .collect();

        let asks: Vec<Order> = msg.asks
            .into_iter()
            .map(|level| Order {
                outcome_id: msg.asset_id.clone(),
                price: level.price,
                size: level.size,
            })
            .collect();

        let orderbook = OrderBook {
            market_id,
            timestamp,
            bids,
            asks,
        };

        debug!("Orderbook update for {}: {} bids, {} asks",
            msg.asset_id, orderbook.bids.len(), orderbook.asks.len());

        producer.publish("order-book-updates", &MarketEvent::OrderBookUpdate(orderbook)).await?;
        Ok(())
    }

    async fn handle_price_change(&self, msg: PriceChangeMessage, producer: &KafkaProducer) -> Result<()> {
        let timestamp = if msg.timestamp > 0 {
            Utc.timestamp_opt(msg.timestamp / 1000, 0).single().unwrap_or(Utc::now())
        } else {
            Utc::now()
        };

        let market_id = uuid::Uuid::new_v4();
        let price_tick = PriceTick {
            market_id,
            outcome_id: msg.asset_id.clone(),
            price: msg.price,
            volume_24h: 0.0, // Not provided in price_change message
            liquidity: 0.0,   // Not provided in price_change message
            timestamp,
        };

        debug!("Price change for {}: {}", msg.asset_id, msg.price);
        producer.publish("price-ticks", &MarketEvent::PriceTick(price_tick)).await?;
        Ok(())
    }

    async fn handle_trade(&self, msg: LastTradePriceMessage, producer: &KafkaProducer) -> Result<()> {
        let timestamp = if msg.timestamp > 0 {
            Utc.timestamp_opt(msg.timestamp / 1000, 0).single().unwrap_or(Utc::now())
        } else {
            Utc::now()
        };

        let market_id = uuid::Uuid::new_v4();
        let side = match msg.side.to_uppercase().as_str() {
            "BUY" => OrderSide::Buy,
            "SELL" => OrderSide::Sell,
            _ => OrderSide::Buy, // Default to Buy if unknown
        };

        let trade = Trade {
            id: uuid::Uuid::new_v4(),
            market_id,
            outcome_id: msg.asset_id.clone(),
            price: msg.price,
            size: msg.size,
            side,
            timestamp,
        };

        debug!("Trade for {}: {} {} @ {}",
            msg.asset_id, msg.side, msg.size, msg.price);
        producer.publish("trades", &MarketEvent::Trade(trade)).await?;
        Ok(())
    }

    async fn handle_best_bid_ask(&self, msg: BestBidAskMessage, producer: &KafkaProducer) -> Result<()> {
        let timestamp = Utc::now();
        let market_id = uuid::Uuid::new_v4();
        let asset_id = msg.asset_id.clone();

        // Publish as both bid and ask price ticks
        if let Some(bid) = msg.best_bid.clone() {
            let price_tick = PriceTick {
                market_id,
                outcome_id: asset_id.clone(),
                price: bid.price,
                volume_24h: 0.0,
                liquidity: bid.size,
                timestamp,
            };
            producer.publish("price-ticks", &MarketEvent::PriceTick(price_tick)).await?;
        }

        if let Some(ask) = msg.best_ask.clone() {
            let price_tick = PriceTick {
                market_id,
                outcome_id: asset_id,
                price: ask.price,
                volume_24h: 0.0,
                liquidity: ask.size,
                timestamp,
            };
            producer.publish("price-ticks", &MarketEvent::PriceTick(price_tick)).await?;
        }

        debug!("Best bid/ask for {}: bid={:?}, ask={:?}",
            msg.asset_id, msg.best_bid, msg.best_ask);
        Ok(())
    }

    async fn handle_new_market(&self, msg: NewMarketMessage, producer: &KafkaProducer) -> Result<()> {
        let created_at = msg.created_at
            .and_then(|ts| Utc.timestamp_opt(ts / 1000, 0).single())
            .unwrap_or(Utc::now());

        let outcomes: Vec<Outcome> = msg.outcomes
            .into_iter()
            .map(|o| Outcome {
                id: o.id,
                name: o.name,
                price: o.price.unwrap_or(0.0),
                liquidity: o.liquidity.unwrap_or(0.0),
            })
            .collect();

        let market = Market {
            id: uuid::Uuid::new_v4(),
            condition_id: msg.condition_id.clone(),
            question: msg.question.clone(),
            description: msg.description,
            category: "unknown".to_string(), // Not provided in message
            outcomes,
            created_at,
            updated_at: created_at,
        };

        info!("New market created: {}", msg.question);
        producer.publish("market-events", &MarketEvent::MarketCreated(market)).await?;
        Ok(())
    }

    async fn handle_market_resolved(&self, msg: MarketResolvedMessage, producer: &KafkaProducer) -> Result<()> {
        // Simplified - in production you'd map condition_id to UUID
        let market_id = uuid::Uuid::new_v4();

        info!("Market resolved: {} -> {}", msg.condition_id, msg.winning_outcome_id);
        producer.publish("market-events", &MarketEvent::MarketResolved {
            market_id,
            outcome_id: msg.winning_outcome_id,
        }).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_book_message() {
        let json = r#"{
            "type": "book",
            "asset_id": "21742633143463906290569050155826241533067272736897614950488156847949938836455",
            "bids": [{"price": 0.55, "size": 100}],
            "asks": [{"price": 0.60, "size": 150}],
            "timestamp": 1708627200000
        }"#;

        let ws_msg: WsMessage = serde_json::from_str(json).unwrap();
        match ws_msg.content {
            WsMessageContent::Book(msg) => {
                assert_eq!(msg.bids.len(), 1);
                assert_eq!(msg.bids[0].price, 0.55);
                assert_eq!(msg.asks.len(), 1);
                assert_eq!(msg.asks[0].price, 0.60);
            }
            _ => panic!("Expected Book message"),
        }
    }

    #[test]
    fn test_parse_last_trade_price() {
        let json = r#"{
            "type": "last_trade_price",
            "asset_id": "21742633143463906290569050155826241533067272736897614950488156847949938836455",
            "price": 0.57,
            "size": 50,
            "side": "BUY",
            "timestamp": 1708627200000
        }"#;

        let ws_msg: WsMessage = serde_json::from_str(json).unwrap();
        match ws_msg.content {
            WsMessageContent::LastTradePrice(msg) => {
                assert_eq!(msg.price, 0.57);
                assert_eq!(msg.size, 50.0);
                assert_eq!(msg.side, "BUY");
            }
            _ => panic!("Expected LastTradePrice message"),
        }
    }

    #[test]
    fn test_parse_price_change() {
        let json = r#"{
            "type": "price_change",
            "asset_id": "21742633143463906290569050155826241533067272736897614950488156847949938836455",
            "price": 0.58,
            "timestamp": 1708627200000
        }"#;

        let ws_msg: WsMessage = serde_json::from_str(json).unwrap();
        match ws_msg.content {
            WsMessageContent::PriceChange(msg) => {
                assert_eq!(msg.price, 0.58);
            }
            _ => panic!("Expected PriceChange message"),
        }
    }

    #[test]
    fn test_parse_best_bid_ask() {
        let json = r#"{
            "type": "best_bid_ask",
            "asset_id": "21742633143463906290569050155826241533067272736897614950488156847949938836455",
            "best_bid": {"price": 0.56, "size": 200},
            "best_ask": {"price": 0.59, "size": 180}
        }"#;

        let ws_msg: WsMessage = serde_json::from_str(json).unwrap();
        match ws_msg.content {
            WsMessageContent::BestBidAsk(msg) => {
                assert!(msg.best_bid.is_some());
                assert_eq!(msg.best_bid.unwrap().price, 0.56);
                assert!(msg.best_ask.is_some());
                assert_eq!(msg.best_ask.unwrap().price, 0.59);
            }
            _ => panic!("Expected BestBidAsk message"),
        }
    }
}
