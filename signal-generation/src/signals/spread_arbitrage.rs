// Spread Arbitrage Signal Generator
// Detects price discrepancies across outcomes and calculates expected value

use super::super::{
    SignalGenerator, SignalInput, TradeSignal, SignalType, SignalDirection,
    PriceSnapshot, SignalMetadata,
};
use anyhow::Result;
use chrono::{Duration, Utc};
use rust_decimal::prelude::*;
use tracing::{debug, info};
use uuid::Uuid;

/// Configuration for spread arbitrage signal generator
#[derive(Debug, Clone)]
pub struct SpreadArbitrageConfig {
    /// Minimum edge required to generate a signal (e.g., 0.05 = 5%)
    pub min_edge: Decimal,
    /// Minimum liquidity score
    pub min_liquidity: f64,
    /// Kelly fraction cap (0.0 to 1.0)
    pub max_kelly_fraction: f64,
    /// Default position size if Kelly is not applied
    pub default_position_size: Decimal,
    /// Stop loss as percentage of entry price
    pub stop_loss_pct: Decimal,
    /// Target as percentage of entry price
    pub target_pct: Decimal,
    /// Signal expiration time in hours
    pub signal_expiration_hours: i64,
}

impl Default for SpreadArbitrageConfig {
    fn default() -> Self {
        Self {
            min_edge: Decimal::from_str_exact("0.05").unwrap(), // 5%
            min_liquidity: 0.3,
            max_kelly_fraction: 0.1, // 10% max
            default_position_size: Decimal::from_str_exact("100.0").unwrap(),
            stop_loss_pct: Decimal::from_str_exact("0.10").unwrap(), // 10%
            target_pct: Decimal::from_str_exact("0.15").unwrap(), // 15%
            signal_expiration_hours: 24,
        }
    }
}

/// Spread arbitrage signal generator
pub struct SpreadArbitrageGenerator {
    config: SpreadArbitrageConfig,
}

impl SpreadArbitrageGenerator {
    /// Create a new spread arbitrage generator
    pub fn new(config: SpreadArbitrageConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(SpreadArbitrageConfig::default())
    }

    /// Calculate expected value for a trade (in dollars)
    /// EV = (probability_of_win * win_amount) - (probability_of_loss * loss_amount)
    fn calculate_ev(
        entry_price: Decimal,
        target_price: Decimal,
        stop_loss: Decimal,
        win_probability: f64,
        position_size: Decimal,
    ) -> Decimal {
        let win_amount = target_price - entry_price;
        let loss_amount = entry_price - stop_loss;
        let lose_probability = 1.0 - win_probability;

        // EV in price units, then multiply by position size for dollar EV
        let ev = (Decimal::from_f64(win_probability).unwrap_or(Decimal::ZERO) * win_amount)
            - (Decimal::from_f64(lose_probability).unwrap_or(Decimal::ZERO) * loss_amount);

        ev * position_size
    }

    /// Calculate Kelly criterion fraction
    /// f* = (bp - q) / b
    /// where:
    ///   b = odds received on the wager (win/loss ratio)
    ///   p = probability of winning
    ///   q = probability of losing (1 - p)
    fn calculate_kelly_fraction(
        entry_price: Decimal,
        target_price: Decimal,
        stop_loss: Decimal,
        win_probability: f64,
        max_fraction: f64,
    ) -> f64 {
        let win_amount = target_price - entry_price;
        let loss_amount = entry_price - stop_loss;

        if loss_amount == Decimal::ZERO {
            return 0.0;
        }

        let b = win_amount.to_f64().unwrap_or(0.0) / loss_amount.to_f64().unwrap_or(1.0);
        let p = win_probability;
        let q = 1.0 - p;

        let kelly = (b * p - q) / b;

        // Cap at max_fraction and don't allow negative
        kelly.min(max_fraction).max(0.0)
    }

    /// Detect price spread across outcomes
    fn detect_spread(&self, input: &SignalInput) -> Option<SpreadOpportunity> {
        let market = &input.market;

        if market.outcomes.len() < 2 {
            debug!("Market has fewer than 2 outcomes, cannot detect spread");
            return None;
        }

        // Check for arbitrage: sum of probabilities < 1.0
        let total_prob: f64 = market.outcomes.iter().map(|o| o.price).sum();

        // Calculate edge
        let edge = Decimal::ONE - Decimal::from_f64(total_prob).unwrap_or(Decimal::ONE);

        if edge < self.config.min_edge {
            debug!(
                "Insufficient edge: {:.2}% (min: {:.2}%)",
                edge * Decimal::from(100),
                self.config.min_edge * Decimal::from(100)
            );
            return None;
        }

        // Find the best outcome to bet on (highest liquidity)
        let best_outcome = market
            .outcomes
            .iter()
            .max_by(|a, b| {
                a.liquidity
                    .partial_cmp(&b.liquidity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        // Use research probability if available, otherwise use market price
        let win_probability = input
            .research_output
            .probability_estimate
            .unwrap_or(best_outcome.price);

        // Calculate liquidity score (normalize to 0-1, assuming 10000 is typical good liquidity)
        let liquidity_score = (best_outcome.liquidity / 10000.0).min(1.0).max(0.0);

        if liquidity_score < self.config.min_liquidity {
            debug!(
                "Insufficient liquidity: {:.2} (min: {:.2})",
                liquidity_score, self.config.min_liquidity
            );
            return None;
        }

        // Calculate prices
        let entry_price = Decimal::from_f64(best_outcome.price).unwrap_or(Decimal::ZERO);
        let stop_loss = entry_price * (Decimal::ONE - self.config.stop_loss_pct);
        let target_price = entry_price * (Decimal::ONE + self.config.target_pct);

        // Calculate Kelly fraction
        let kelly_fraction = Self::calculate_kelly_fraction(
            entry_price,
            target_price,
            stop_loss,
            win_probability,
            self.config.max_kelly_fraction,
        );

        // Position size based on Kelly fraction
        let position_size = self.config.default_position_size * Decimal::from_f64(kelly_fraction).unwrap_or(Decimal::ZERO);

        // Calculate expected value using the position size
        let expected_value = Self::calculate_ev(entry_price, target_price, stop_loss, win_probability, position_size);

        // Confidence based on edge, liquidity, and research confidence
        let edge_score = (edge / self.config.min_edge).to_f64().unwrap_or(1.0).min(2.0) / 2.0;
        let liquidity_score_normalized = ((liquidity_score - self.config.min_liquidity) / (1.0 - self.config.min_liquidity)).min(1.0).max(0.0);
        let confidence = (edge_score * 0.4 + input.research_output.confidence * 0.4 + liquidity_score_normalized * 0.2)
            .min(1.0)
            .max(0.0);

        info!(
            "Spread opportunity detected: market={:?}, outcome={}, edge={:.2}%, ev={}, confidence={:.2}",
            market.id, best_outcome.id, edge * Decimal::from(100), expected_value, confidence
        );

        Some(SpreadOpportunity {
            market_id: market.id,
            outcome_id: best_outcome.id.clone(),
            entry_price,
            target_price,
            stop_loss,
            position_size,
            expected_value,
            edge,
            kelly_fraction,
            confidence,
            win_probability,
            liquidity_score,
        })
    }

    /// Calculate volatility score from price history
    fn calculate_volatility_score(&self, price_history: &[PriceSnapshot]) -> f64 {
        if price_history.len() < 2 {
            return 0.5; // Default middle value
        }

        let prices: Vec<f64> = price_history
            .iter()
            .map(|p| p.price.to_f64().unwrap_or(0.0))
            .collect();

        let mean = prices.iter().sum::<f64>() / prices.len() as f64;
        let variance = prices
            .iter()
            .map(|p| (p - mean).powi(2))
            .sum::<f64>() / prices.len() as f64;

        let std_dev = variance.sqrt();

        // Normalize: typical std dev range 0.01-0.10, map to 0-1
        (std_dev / 0.05).min(1.0).max(0.0)
    }
}

impl SignalGenerator for SpreadArbitrageGenerator {
    fn generate(&self, input: &SignalInput) -> Result<Option<TradeSignal>> {
        // Detect spread opportunity
        let opportunity = match self.detect_spread(input) {
            Some(opp) => opp,
            None => return Ok(None),
        };

        // Calculate volatility score
        let volatility_score = self.calculate_volatility_score(&input.price_history);

        // Build reasoning
        let reasoning = format!(
            "Spread arbitrage opportunity detected. Market total probability: {:.2}% (edge: {:.2}%). \
            Research confidence: {:.2}%. \
            Estimated win probability: {:.2}%. \
            Liquidity score: {:.2}. \
            Volatility: {:.2}.",
            (Decimal::ONE - opportunity.edge) * Decimal::from(100),
            opportunity.edge * Decimal::from(100),
            input.research_output.confidence * 100.0,
            opportunity.win_probability * 100.0,
            opportunity.liquidity_score,
            volatility_score
        );

        // Create custom fields for metadata
        let mut custom_fields = serde_json::Map::new();
        custom_fields.insert(
            "win_probability".to_string(),
            serde_json::json!(opportunity.win_probability),
        );
        custom_fields.insert(
            "total_market_probability".to_string(),
            serde_json::json!((Decimal::ONE - opportunity.edge).to_string()),
        );

        // Create signal
        let signal = TradeSignal {
            id: Uuid::new_v4(),
            market_id: opportunity.market_id,
            signal_type: SignalType::SpreadArbitrage,
            direction: SignalDirection::Long,
            outcome_id: Some(opportunity.outcome_id),
            entry_price: opportunity.entry_price,
            target_price: opportunity.target_price,
            stop_loss: opportunity.stop_loss,
            position_size: opportunity.position_size,
            confidence: opportunity.confidence,
            expected_value: opportunity.expected_value,
            edge: opportunity.edge,
            kelly_fraction: opportunity.kelly_fraction,
            reasoning,
            metadata: SignalMetadata {
                research_sources: input
                    .research_output
                    .key_factors
                    .iter()
                    .map(|f| f.clone())
                    .collect(),
                data_points: input.price_history.len() as u32,
                liquidity_score: opportunity.liquidity_score,
                volatility_score,
                custom_fields: serde_json::Value::Object(custom_fields),
            },
            created_at: Utc::now(),
            expires_at: Some(Utc::now() + Duration::hours(self.config.signal_expiration_hours)),
        };

        Ok(Some(signal))
    }

    fn signal_type(&self) -> SignalType {
        SignalType::SpreadArbitrage
    }
}

/// Spread opportunity detected
#[derive(Debug, Clone)]
struct SpreadOpportunity {
    market_id: Uuid,
    outcome_id: String,
    entry_price: Decimal,
    target_price: Decimal,
    stop_loss: Decimal,
    position_size: Decimal,
    expected_value: Decimal,
    edge: Decimal,
    kelly_fraction: f64,
    confidence: f64,
    win_probability: f64,
    liquidity_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_ev() {
        let entry = Decimal::from_str_exact("0.5").unwrap();
        let target = Decimal::from_str_exact("0.6").unwrap();
        let stop = Decimal::from_str_exact("0.4").unwrap();
        let win_prob = 0.6;
        let position_size = Decimal::from_str_exact("100.0").unwrap();

        let ev = SpreadArbitrageGenerator::calculate_ev(entry, target, stop, win_prob, position_size);

        // EV = ((0.6 * 0.1) - (0.4 * 0.1)) * 100 = (0.06 - 0.04) * 100 = 2.0
        assert!((ev - Decimal::from_str_exact("2.0").unwrap()).abs() < Decimal::from_str_exact("0.1").unwrap());
    }

    #[test]
    fn test_calculate_kelly() {
        let entry = Decimal::from_str_exact("0.5").unwrap();
        let target = Decimal::from_str_exact("0.6").unwrap();
        let stop = Decimal::from_str_exact("0.4").unwrap();
        let win_prob = 0.6;

        let kelly = SpreadArbitrageGenerator::calculate_kelly_fraction(entry, target, stop, win_prob, 0.5);

        // b = 0.1/0.1 = 1.0
        // f* = (1.0 * 0.6 - 0.4) / 1.0 = 0.2
        // With max_fraction=0.5, Kelly should be 0.2
        assert!((kelly - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_kelly_cap() {
        let entry = Decimal::from_str_exact("0.5").unwrap();
        let target = Decimal::from_str_exact("0.8").unwrap();
        let stop = Decimal::from_str_exact("0.4").unwrap();
        let win_prob = 0.8;

        let kelly = SpreadArbitrageGenerator::calculate_kelly_fraction(entry, target, stop, win_prob, 0.1);

        // Even though the formula would suggest higher Kelly, it should be capped
        assert!(kelly <= 0.1);
    }

    #[test]
    fn test_kelly_no_negative() {
        let entry = Decimal::from_str_exact("0.5").unwrap();
        let target = Decimal::from_str_exact("0.55").unwrap();
        let stop = Decimal::from_str_exact("0.45").unwrap();
        let win_prob = 0.4;

        let kelly = SpreadArbitrageGenerator::calculate_kelly_fraction(entry, target, stop, win_prob, 0.1);

        // With low win probability, Kelly should be 0 or very close
        assert!(kelly >= 0.0);
    }
}
