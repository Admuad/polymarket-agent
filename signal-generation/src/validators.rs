// Signal Validators
// Validates signals before they are sent to execution

use super::signals::TradeSignal;
use anyhow::Result;
use rust_decimal::prelude::*;
use tracing::debug;

/// Trait for signal validators
#[async_trait::async_trait]
pub trait SignalValidator: Send + Sync {
    async fn validate(&self, signal: &TradeSignal) -> Result<bool>;
}

/// Configuration for edge threshold validator
#[derive(Debug, Clone)]
pub struct EdgeThresholdConfig {
    /// Minimum edge percentage (e.g., 0.05 = 5%)
    pub min_edge: Decimal,
}

impl Default for EdgeThresholdConfig {
    fn default() -> Self {
        Self {
            min_edge: Decimal::from_str_exact("0.05").unwrap(), // 5%
        }
    }
}

/// Validates that the signal has sufficient edge
pub struct EdgeThresholdValidator {
    config: EdgeThresholdConfig,
}

impl EdgeThresholdValidator {
    pub fn new(config: EdgeThresholdConfig) -> Self {
        Self { config }
    }

    pub fn default() -> Self {
        Self::new(EdgeThresholdConfig::default())
    }
}

#[async_trait::async_trait]
impl SignalValidator for EdgeThresholdValidator {
    async fn validate(&self, signal: &TradeSignal) -> Result<bool> {
        let passes = signal.edge >= self.config.min_edge;
        debug!(
            "Edge threshold validation: {} >= {}? {}",
            signal.edge, self.config.min_edge, passes
        );
        Ok(passes)
    }
}

/// Configuration for confidence validator
#[derive(Debug, Clone)]
pub struct ConfidenceValidatorConfig {
    /// Minimum confidence level (0.0 to 1.0)
    pub min_confidence: f64,
}

impl Default for ConfidenceValidatorConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.7,
        }
    }
}

/// Validates that the signal has sufficient confidence
pub struct ConfidenceValidator {
    config: ConfidenceValidatorConfig,
}

impl ConfidenceValidator {
    pub fn new(config: ConfidenceValidatorConfig) -> Self {
        Self { config }
    }

    pub fn default() -> Self {
        Self::new(ConfidenceValidatorConfig::default())
    }
}

#[async_trait::async_trait]
impl SignalValidator for ConfidenceValidator {
    async fn validate(&self, signal: &TradeSignal) -> Result<bool> {
        let passes = signal.confidence >= self.config.min_confidence;
        debug!(
            "Confidence validation: {:.2} >= {:.2}? {}",
            signal.confidence, self.config.min_confidence, passes
        );
        Ok(passes)
    }
}

/// Configuration for liquidity validator
#[derive(Debug, Clone)]
pub struct LiquidityValidatorConfig {
    /// Minimum liquidity score (0.0 to 1.0)
    pub min_liquidity_score: f64,
    /// Minimum position size relative to liquidity (e.g., 0.1 = max 10% of liquidity)
    pub max_position_liquidity_ratio: f64,
}

impl Default for LiquidityValidatorConfig {
    fn default() -> Self {
        Self {
            min_liquidity_score: 0.3,
            max_position_liquidity_ratio: 0.1,
        }
    }
}

/// Validates that there is sufficient liquidity for the position
pub struct LiquidityValidator {
    config: LiquidityValidatorConfig,
}

impl LiquidityValidator {
    pub fn new(config: LiquidityValidatorConfig) -> Self {
        Self { config }
    }

    pub fn default() -> Self {
        Self::new(LiquidityValidatorConfig::default())
    }
}

#[async_trait::async_trait]
impl SignalValidator for LiquidityValidator {
    async fn validate(&self, signal: &TradeSignal) -> Result<bool> {
        let liquidity_passes = signal.metadata.liquidity_score >= self.config.min_liquidity_score;

        // Calculate position size relative to liquidity
        let position_ratio = signal.position_size.to_f64().unwrap_or(0.0)
            / (signal.metadata.liquidity_score * 10000.0).max(1.0); // Normalize to rough dollar value
        let size_passes = position_ratio <= self.config.max_position_liquidity_ratio;

        let passes = liquidity_passes && size_passes;

        debug!(
            "Liquidity validation: score={:.2}, ratio={:.2}? {}",
            signal.metadata.liquidity_score, position_ratio, passes
        );

        Ok(passes)
    }
}

/// Configuration for expected value validator
#[derive(Debug, Clone)]
pub struct ExpectedValueValidatorConfig {
    /// Minimum expected value (in dollars)
    pub min_expected_value: Decimal,
}

impl Default for ExpectedValueValidatorConfig {
    fn default() -> Self {
        Self {
            min_expected_value: Decimal::from_str_exact("5.0").unwrap(), // $5
        }
    }
}

/// Validates that the signal has positive expected value above threshold
pub struct ExpectedValueValidator {
    config: ExpectedValueValidatorConfig,
}

impl ExpectedValueValidator {
    pub fn new(config: ExpectedValueValidatorConfig) -> Self {
        Self { config }
    }

    pub fn default() -> Self {
        Self::new(ExpectedValueValidatorConfig::default())
    }
}

#[async_trait::async_trait]
impl SignalValidator for ExpectedValueValidator {
    async fn validate(&self, signal: &TradeSignal) -> Result<bool> {
        let passes = signal.expected_value >= self.config.min_expected_value
            && signal.expected_value > Decimal::ZERO;
        debug!(
            "Expected value validation: {} >= {}? {}",
            signal.expected_value, self.config.min_expected_value, passes
        );
        Ok(passes)
    }
}

/// Combines multiple validators with AND logic
pub struct CompositeValidator {
    validators: Vec<Box<dyn SignalValidator + Send + Sync>>,
}

impl CompositeValidator {
    pub fn new() -> Self {
        Self {
            validators: Vec::new(),
        }
    }

    pub fn add_validator(mut self, validator: Box<dyn SignalValidator + Send + Sync>) -> Self {
        self.validators.push(validator);
        self
    }
}

impl Default for CompositeValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SignalValidator for CompositeValidator {
    async fn validate(&self, signal: &TradeSignal) -> Result<bool> {
        for validator in &self.validators {
            if !validator.validate(signal).await? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_signal(edge: f64, confidence: f64, liquidity_score: f64) -> TradeSignal {
        use super::super::signals::{SignalMetadata, SignalType, SignalDirection};
        use uuid::Uuid;

        TradeSignal {
            id: Uuid::new_v4(),
            market_id: Uuid::new_v4(),
            signal_type: SignalType::SpreadArbitrage,
            direction: SignalDirection::Long,
            outcome_id: Some("test".to_string()),
            entry_price: Decimal::from_f64(0.5).unwrap(),
            target_price: Decimal::from_f64(0.6).unwrap(),
            stop_loss: Decimal::from_f64(0.4).unwrap(),
            position_size: Decimal::from_f64(100.0).unwrap(),
            confidence,
            expected_value: Decimal::from_f64(10.0).unwrap(),
            edge: Decimal::from_f64(edge).unwrap(),
            kelly_fraction: 0.1,
            reasoning: "test".to_string(),
            metadata: SignalMetadata {
                research_sources: vec![],
                data_points: 10,
                liquidity_score,
                volatility_score: 0.5,
                custom_fields: serde_json::json!({}),
            },
            created_at: Utc::now(),
            expires_at: None,
        }
    }

    #[tokio::test]
    async fn test_edge_validator() {
        let validator = EdgeThresholdValidator::default();

        let good_signal = create_test_signal(0.06, 0.8, 0.5);
        assert!(validator.validate(&good_signal).await.unwrap());

        let bad_signal = create_test_signal(0.03, 0.8, 0.5);
        assert!(!validator.validate(&bad_signal).await.unwrap());
    }

    #[tokio::test]
    async fn test_confidence_validator() {
        let validator = ConfidenceValidator::default();

        let good_signal = create_test_signal(0.06, 0.8, 0.5);
        assert!(validator.validate(&good_signal).await.unwrap());

        let bad_signal = create_test_signal(0.06, 0.6, 0.5);
        assert!(!validator.validate(&bad_signal).await.unwrap());
    }

    #[tokio::test]
    async fn test_liquidity_validator() {
        let validator = LiquidityValidator::default();

        let good_signal = create_test_signal(0.06, 0.8, 0.8);
        assert!(validator.validate(&good_signal).await.unwrap());

        let bad_signal = create_test_signal(0.06, 0.8, 0.2);
        assert!(!validator.validate(&bad_signal).await.unwrap());
    }
}
