use crate::signals::{SignalDirection, SignalInput, SignalMetadata, SignalType, TradeSignal};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use uuid::Uuid;

/// Spread Arbitrage Signal Generator
///
/// Detects price discrepancies across related outcomes and calculates arbitrage opportunities.
/// Uses Kelly criterion for optimal position sizing.
#[derive(Debug, Clone)]
pub struct SpreadArbitrageGenerator {
    min_edge_threshold: Decimal, // Minimum edge percentage to generate signal
    min_confidence: f64,
    min_liquidity: Decimal,
    kelly_risk_factor: f64, // Kelly fraction multiplier (0.0 to 1.0)
}

impl SpreadArbitrageGenerator {
    pub fn new(
        min_edge_threshold: f64,
        min_confidence: f64,
        min_liquidity: f64,
        kelly_risk_factor: f64,
    ) -> Self {
        Self {
            min_edge_threshold: Decimal::from_f64(min_edge_threshold).unwrap(),
            min_confidence,
            min_liquidity: Decimal::from_f64(min_liquidity).unwrap(),
            kelly_risk_factor: kelly_risk_factor.clamp(0.0, 1.0),
        }
    }

    pub fn default() -> Self {
        Self {
            min_edge_threshold: dec!(0.03), // 3% minimum edge
            min_confidence: 0.6,            // 60% minimum confidence
            min_liquidity: dec!(1000.0),    // $1000 minimum liquidity
            kelly_risk_factor: 0.25,         // 25% of full Kelly (quarter Kelly)
        }
    }

    /// Calculate spread arbitrage opportunity
    fn calculate_spread_opportunity(&self, input: &SignalInput) -> Option<SpreadOpportunity> {
        let outcomes = &input.market.outcomes;

        if outcomes.len() < 2 {
            return None;
        }

        // Calculate implied probabilities from prices
        let mut implied_probs: Vec<(String, Decimal)> = outcomes
            .iter()
            .map(|o| {
                let implied_prob = o.price / dec!(100.0);
                (o.id.clone(), implied_prob)
            })
            .collect();

        // Sort by implied probability
        implied_probs.sort_by(|a, b| a.1.cmp(&b.1));

        // Check for arbitrage: if sum of implied probabilities < 1.0, there's an opportunity
        let total_implied: Decimal = implied_probs.iter().map(|(_, p)| p).sum();

        if total_implied >= dec!(1.0) {
            return None; // No arbitrage
        }

        // Calculate the arbitrage edge
        let arbitrage_edge = (dec!(1.0) - total_implied) * dec!(100.0);

        if arbitrage_edge < self.min_edge_threshold {
            return None;
        }

        // Find the best outcome to bet on (lowest implied probability)
        let best_outcome_id = implied_probs[0].0.clone();
        let best_outcome = outcomes.iter().find(|o| o.id == best_outcome_id)?;

        // Calculate expected value
        let win_probability = best_outcome.price / dec!(100.0);
        let expected_value = (win_probability * dec!(100.0)) - best_outcome.price;

        // Apply Kelly criterion for position sizing
        // Kelly fraction = (bp - q) / b
        // where b = odds, p = win probability, q = lose probability
        let odds = dec!(100.0) / best_outcome.price;
        let p = win_probability;
        let q = dec!(1.0) - p;
        let kelly = (odds * p - q) / odds;

        // Apply risk factor (fractional Kelly)
        let adjusted_kelly = kelly.to_f64().unwrap_or(0.0) * self.kelly_risk_factor;

        // Estimate liquidity score
        let liquidity_score = self.calculate_liquidity_score(input);

        Some(SpreadOpportunity {
            outcome_id: best_outcome_id.clone(),
            edge: arbitrage_edge,
            expected_value,
            win_probability,
            entry_price: Decimal::from_f64(best_outcome.price).unwrap(),
            kelly_fraction: adjusted_kelly,
            confidence: self.calculate_confidence(input, arbitrage_edge, liquidity_score),
            liquidity_score,
        })
    }

    fn calculate_liquidity_score(&self, input: &SignalInput) -> f64 {
        // Normalize liquidity to 0-1 range
        // Use $10,000 as a reference for high liquidity
        let total_liquidity: Decimal = input
            .market
            .outcomes
            .iter()
            .map(|o| Decimal::from_f64(o.liquidity).unwrap())
            .sum();

        let normalized = (total_liquidity / dec!(10000.0)).min(dec!(1.0));
        normalized.to_f64().unwrap_or(0.0)
    }

    fn calculate_confidence(
        &self,
        input: &SignalInput,
        edge: Decimal,
        liquidity_score: f64,
    ) -> f64 {
        // Base confidence from edge size (larger edge = higher confidence)
        let edge_confidence = (edge / dec!(0.10)).to_f64().unwrap_or(0.0).min(1.0);

        // Incorporate research confidence
        let research_confidence = input.research_output.confidence;

        // Combine with liquidity
        let combined = edge_confidence * 0.5 + research_confidence * 0.3 + liquidity_score * 0.2;

        combined.min(1.0)
    }
}

impl crate::signals::SignalGenerator for SpreadArbitrageGenerator {
    fn generate(&self, input: &SignalInput) -> anyhow::Result<Option<TradeSignal>> {
        let opportunity = match self.calculate_spread_opportunity(input) {
            Some(opp) => opp,
            None => return Ok(None),
        };

        // Check thresholds
        if opportunity.confidence < self.min_confidence {
            tracing::debug!(
                "Signal rejected: confidence {:.2} < {:.2}",
                opportunity.confidence,
                self.min_confidence
            );
            return Ok(None);
        }

        // Get outcome details
        let outcome = input
            .market
            .outcomes
            .iter()
            .find(|o| o.id == opportunity.outcome_id)
            .ok_or_else(|| anyhow::anyhow!("Outcome not found"))?;

        // Check liquidity
        let outcome_liquidity = Decimal::from_f64(outcome.liquidity).unwrap();
        if outcome_liquidity < self.min_liquidity {
            tracing::debug!(
                "Signal rejected: liquidity {} < {}",
                outcome_liquidity,
                self.min_liquidity
            );
            return Ok(None);
        }

        // Calculate position size based on Kelly
        // For simplicity, use a base position size of $1000 * kelly_fraction
        let base_position = dec!(1000.0);
        let position_size = base_position * Decimal::from_f64(opportunity.kelly_fraction).unwrap();

        // Calculate target and stop loss
        let target_price = opportunity.entry_price * (dec!(1.0) + opportunity.edge / dec!(100.0));
        let stop_loss = opportunity.entry_price * dec!(0.95); // 5% stop loss

        // Build reasoning
        let reasoning = format!(
            "Spread arbitrage detected: {}% edge across {} outcomes. \
            Implied probabilities sum to {:.2}%, creating arbitrage opportunity. \
            Win probability: {:.2}%. Kelly fraction: {:.2}.",
            opportunity.edge,
            input.market.outcomes.len(),
            (dec!(100.0) - opportunity.edge),
            opportunity.win_probability * dec!(100.0),
            opportunity.kelly_fraction
        );

        let metadata = SignalMetadata {
            research_sources: input.research_output.key_factors.clone(),
            data_points: input.price_history.len() as u32,
            liquidity_score: opportunity.liquidity_score,
            volatility_score: 0.5, // Placeholder - could calculate from history
            custom_fields: serde_json::json!({
                "spread_edge": opportunity.edge.to_string(),
                "win_probability": opportunity.win_probability.to_string(),
                "total_outcomes": input.market.outcomes.len(),
            }),
        };

        let signal = TradeSignal {
            id: Uuid::new_v4(),
            market_id: input.market.id,
            signal_type: SignalType::SpreadArbitrage,
            direction: SignalDirection::Long,
            outcome_id: Some(opportunity.outcome_id.clone()),
            entry_price: opportunity.entry_price,
            target_price,
            stop_loss,
            position_size,
            confidence: opportunity.confidence,
            expected_value: opportunity.expected_value,
            edge: opportunity.edge,
            kelly_fraction: opportunity.kelly_fraction,
            reasoning,
            metadata,
            created_at: Utc::now(),
            expires_at: Some(Utc::now() + Duration::hours(1)), // Signal valid for 1 hour
        };

        tracing::info!(
            "Generated spread arbitrage signal for market {}: {:.2}% edge, confidence: {:.2}",
            signal.market_id,
            signal.edge,
            signal.confidence
        );

        Ok(Some(signal))
    }

    fn signal_type(&self) -> SignalType {
        SignalType::SpreadArbitrage
    }
}

#[derive(Debug, Clone)]
struct SpreadOpportunity {
    outcome_id: String,
    edge: Decimal,
    expected_value: Decimal,
    win_probability: Decimal,
    entry_price: Decimal,
    kelly_fraction: f64,
    confidence: f64,
    liquidity_score: f64,
}
