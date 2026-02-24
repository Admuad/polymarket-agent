use chrono::{DateTime, Utc};
use common::Market;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod spread_arbitrage;

pub use spread_arbitrage::SpreadArbitrageGenerator;

/// Signal type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SignalType {
    SpreadArbitrage,
    Momentum,
    MeanReversion,
    Value,
    Sentiment,
}

/// Signal direction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SignalDirection {
    Long,
    Short,
    Neutral,
}

/// Trade signal with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSignal {
    pub id: Uuid,
    pub market_id: Uuid,
    pub signal_type: SignalType,
    pub direction: SignalDirection,
    pub outcome_id: Option<String>,
    pub entry_price: Decimal,
    pub target_price: Decimal,
    pub stop_loss: Decimal,
    pub position_size: Decimal,
    pub confidence: f64, // 0.0 to 1.0
    pub expected_value: Decimal,
    pub edge: Decimal, // Edge percentage (e.g., 0.05 = 5%)
    pub kelly_fraction: f64,
    pub reasoning: String,
    pub metadata: SignalMetadata,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Additional metadata for the signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalMetadata {
    pub research_sources: Vec<String>,
    pub data_points: u32,
    pub liquidity_score: f64,
    pub volatility_score: f64,
    pub custom_fields: serde_json::Value,
}

/// Signal generation input
#[derive(Debug, Clone)]
pub struct SignalInput {
    pub market: Market,
    pub research_output: ResearchOutput,
    pub order_book: Option<OrderBookSnapshot>,
    pub price_history: Vec<PriceSnapshot>,
}

/// Research agent output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchOutput {
    pub market_id: Uuid,
    pub analysis: String,
    pub sentiment: SentimentScore,
    pub confidence: f64,
    pub probability_estimate: Option<f64>,
    pub key_factors: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

/// Sentiment score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentScore {
    pub overall: f64, // -1.0 to 1.0
    pub sources: Vec<SentimentSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentSource {
    pub name: String,
    pub score: f64,
    pub weight: f64,
}

/// Order book snapshot
#[derive(Debug, Clone)]
pub struct OrderBookSnapshot {
    pub market_id: Uuid,
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Level {
    pub outcome_id: String,
    pub price: Decimal,
    pub size: Decimal,
}

/// Price history snapshot
#[derive(Debug, Clone)]
pub struct PriceSnapshot {
    pub outcome_id: String,
    pub price: Decimal,
    pub volume: Decimal,
    pub liquidity: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Signal generator trait
pub trait SignalGenerator {
    fn generate(&self, input: &SignalInput) -> anyhow::Result<Option<TradeSignal>>;
    fn signal_type(&self) -> SignalType;
}
