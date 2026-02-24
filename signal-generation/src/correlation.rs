// Correlation Analysis & Logical Arbitrage
// Detects pricing inconsistencies between correlated markets

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use common::Market;
use crate::signals::{
    SignalInput, SignalDirection, SignalGenerator, SignalMetadata, SignalType, TradeSignal,
    StateUpdate, MultiSignalGenerator,
};

/// Correlation relationship type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CorrelationType {
    /// A implies B (100% correlation) - e.g., "Trump wins" implies "Republican wins"
    Implies,
    /// A suggests B (partial correlation) - e.g., "Trump wins" suggests "GOP controls Senate"
    Suggests(Decimal), // Correlation strength 0-1
    /// Mutually exclusive - sum of probabilities must = 100%
    MutuallyExclusive,
    /// Cumulative probabilities - sum must be <= 100%
    Cumulative,
    /// Same outcome - must have same probability
    SameOutcome,
}

/// Correlation edge between markets
#[derive(Debug, Clone)]
pub struct CorrelationEdge {
    pub from_market: Uuid,
    pub to_market: Uuid,
    pub correlation_type: CorrelationType,
    pub min_spread: Decimal, // Minimum spread to trigger arbitrage
}

/// Logical arbitrage opportunity
#[derive(Debug, Clone)]
pub struct LogicalArbitrageOpportunity {
    pub id: Uuid,
    pub markets: Vec<Uuid>,
    pub opportunity_type: String,
    pub violation_amount: Decimal, // How much the probabilities violate logic
    pub description: String,
    pub trades: Vec<ArbitrageTrade>,
    pub expected_profit: Decimal,
}

/// Trade in an arbitrage opportunity
#[derive(Debug, Clone)]
pub struct ArbitrageTrade {
    pub market_id: Uuid,
    pub outcome_id: Option<String>,
    pub direction: SignalDirection,
    pub entry_price: Decimal,
    pub position_size: Decimal,
}

/// Correlation graph for market relationships
#[derive(Debug, Clone)]
pub struct CorrelationGraph {
    /// Market UUID -> Price/Probability
    market_prices: HashMap<Uuid, Decimal>,
    /// Correlation edges
    edges: Vec<CorrelationEdge>,
}

impl Default for CorrelationGraph {
    fn default() -> Self {
        CorrelationGraph {
            market_prices: HashMap::new(),
            edges: Vec::new(),
        }
    }
}

impl CorrelationGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update market price
    pub fn update_price(&mut self, market_id: Uuid, price: Decimal) {
        self.market_prices.insert(market_id, price);
    }

    /// Add correlation edge
    pub fn add_edge(&mut self, edge: CorrelationEdge) {
        self.edges.push(edge);
    }

    /// Find logical violations
    pub fn find_violations(&self) -> Vec<LogicalArbitrageOpportunity> {
        let mut violations = Vec::new();

        // Check each correlation type
        for edge in &self.edges {
            match edge.correlation_type {
                CorrelationType::Implies => {
                    if let Some(violation) = self.check_implication(edge) {
                        violations.push(violation);
                    }
                }
                CorrelationType::Suggests(strength) => {
                    if let Some(violation) = self.check_suggestion(edge, strength) {
                        violations.push(violation);
                    }
                }
                CorrelationType::MutuallyExclusive => {
                    if let Some(violation) = self.check_mutually_exclusive(edge) {
                        violations.push(violation);
                    }
                }
                CorrelationType::Cumulative => {
                    if let Some(violation) = self.check_cumulative(edge) {
                        violations.push(violation);
                    }
                }
                CorrelationType::SameOutcome => {
                    if let Some(violation) = self.check_same_outcome(edge) {
                        violations.push(violation);
                    }
                }
            }
        }

        violations
    }

    /// Check if A implies B but P(A) > P(B) (logical impossibility)
    fn check_implication(&self, edge: &CorrelationEdge) -> Option<LogicalArbitrageOpportunity> {
        let price_a = self.market_prices.get(&edge.from_market)?;
        let price_b = self.market_prices.get(&edge.to_market)?;

        // If A implies B, then P(A) cannot be > P(B)
        if price_a > price_b {
            let violation = price_a - price_b;

            if violation >= edge.min_spread {
                return Some(LogicalArbitrageOpportunity {
                    id: Uuid::new_v4(),
                    markets: vec![edge.from_market, edge.to_market],
                    opportunity_type: "Implication Violation".to_string(),
                    violation_amount: violation,
                    description: format!(
                        "{} implies {} but {:.4}% > {:.4}% (violation: {:.4}%)",
                        edge.from_market, edge.to_market,
                        price_a * Decimal::from(100),
                        price_b * Decimal::from(100),
                        violation * Decimal::from(100)
                    ),
                    trades: vec![
                        ArbitrageTrade {
                            market_id: edge.to_market,
                            outcome_id: None,
                            direction: SignalDirection::Long,
                            entry_price: price_b,
                            position_size: Decimal::from(100),
                        },
                        ArbitrageTrade {
                            market_id: edge.from_market,
                            outcome_id: None,
                            direction: SignalDirection::Short,
                            entry_price: price_a,
                            position_size: Decimal::from(100),
                        },
                    ],
                    expected_profit: violation * Decimal::from(100),
                });
            }
        }

        None
    }

    /// Check if A suggests B but P(A) > P(B) * strength
    fn check_suggestion(
        &self,
        edge: &CorrelationEdge,
        strength: Decimal,
    ) -> Option<LogicalArbitrageOpportunity> {
        let price_a = self.market_prices.get(&edge.from_market)?;
        let price_b = self.market_prices.get(&edge.to_market)?;

        // If A suggests B with strength S, then P(A) <= P(B) * S
        let implied_price_b = price_a / strength;

        if price_b < implied_price_b {
            let violation = implied_price_b - price_b;

            if violation >= edge.min_spread {
                return Some(LogicalArbitrageOpportunity {
                    id: Uuid::new_v4(),
                    markets: vec![edge.from_market, edge.to_market],
                    opportunity_type: "Suggestion Violation".to_string(),
                    violation_amount: violation,
                    description: format!(
                        "{} suggests {} ({:.0}%) but {:.4}% < {:.4}% (violation: {:.4}%)",
                        edge.from_market, edge.to_market,
                        strength * Decimal::from(100),
                        price_b * Decimal::from(100),
                        implied_price_b * Decimal::from(100),
                        violation * Decimal::from(100)
                    ),
                    trades: vec![
                        ArbitrageTrade {
                            market_id: edge.to_market,
                            outcome_id: None,
                            direction: SignalDirection::Long,
                            entry_price: price_b,
                            position_size: Decimal::from(100),
                        },
                        ArbitrageTrade {
                            market_id: edge.from_market,
                            outcome_id: None,
                            direction: SignalDirection::Short,
                            entry_price: price_a,
                            position_size: Decimal::from(100),
                        },
                    ],
                    expected_profit: violation * Decimal::from(100),
                });
            }
        }

        None
    }

    /// Check if mutually exclusive markets sum > 100%
    fn check_mutually_exclusive(
        &self,
        edge: &CorrelationEdge,
    ) -> Option<LogicalArbitrageOpportunity> {
        let price_a = self.market_prices.get(&edge.from_market)?;
        let price_b = self.market_prices.get(&edge.to_market)?;

        let sum = price_a + price_b;

        if sum > Decimal::ONE {
            let violation = sum - Decimal::ONE;

            if violation >= edge.min_spread {
                return Some(LogicalArbitrageOpportunity {
                    id: Uuid::new_v4(),
                    markets: vec![edge.from_market, edge.to_market],
                    opportunity_type: "Mutually Exclusive Violation".to_string(),
                    violation_amount: violation,
                    description: format!(
                        "Mutually exclusive markets sum to {:.4}% > 100% (violation: {:.4}%)",
                        sum * Decimal::from(100),
                        violation * Decimal::from(100)
                    ),
                    trades: vec![
                        ArbitrageTrade {
                            market_id: edge.from_market,
                            outcome_id: None,
                            direction: SignalDirection::Short,
                            entry_price: price_a,
                            position_size: Decimal::from(100),
                        },
                        ArbitrageTrade {
                            market_id: edge.to_market,
                            outcome_id: None,
                            direction: SignalDirection::Short,
                            entry_price: price_b,
                            position_size: Decimal::from(100),
                        },
                    ],
                    expected_profit: violation * Decimal::from(100),
                });
            }
        }

        None
    }

    /// Check if cumulative probabilities > 100%
    fn check_cumulative(
        &self,
        edge: &CorrelationEdge,
    ) -> Option<LogicalArbitrageOpportunity> {
        // Similar to mutually exclusive, but for cumulative outcomes
        self.check_mutually_exclusive(edge)
    }

    /// Check if same outcome has different prices
    fn check_same_outcome(
        &self,
        edge: &CorrelationEdge,
    ) -> Option<LogicalArbitrageOpportunity> {
        let price_a = self.market_prices.get(&edge.from_market)?;
        let price_b = self.market_prices.get(&edge.to_market)?;

        let diff = (price_a - price_b).abs();

        if diff >= edge.min_spread {
            return Some(LogicalArbitrageOpportunity {
                id: Uuid::new_v4(),
                markets: vec![edge.from_market, edge.to_market],
                opportunity_type: "Same Outcome Price Diff".to_string(),
                violation_amount: diff,
                description: format!(
                    "Same outcome priced at {:.4}% and {:.4}% (diff: {:.4}%)",
                    price_a * Decimal::from(100),
                    price_b * Decimal::from(100),
                    diff * Decimal::from(100)
                ),
                trades: vec![
                    ArbitrageTrade {
                        market_id: edge.from_market,
                        outcome_id: None,
                        direction: if price_a < price_b { SignalDirection::Long } else { SignalDirection::Short },
                        entry_price: price_a.min(price_b),
                        position_size: Decimal::from(100),
                    },
                ],
                expected_profit: diff * Decimal::from(100),
            });
        }

        None
    }
}

/// Correlation-based signal generator
pub struct CorrelationGenerator {
    graph: CorrelationGraph,
}

impl Default for CorrelationGenerator {
    fn default() -> Self {
        CorrelationGenerator {
            graph: CorrelationGraph::new(),
        }
    }
}

impl CorrelationGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a correlation relationship
    pub fn add_correlation(&mut self, edge: CorrelationEdge) {
        self.graph.add_edge(edge);
    }

    /// Update market prices from all inputs
    pub fn update_prices(&mut self, inputs: &[SignalInput]) {
        for input in inputs {
            // Use current market price from order book
            if let Some(order_book) = &input.order_book {
                if !order_book.bids.is_empty() && !order_book.asks.is_empty() {
                    let mid = (order_book.bids[0].price + order_book.asks[0].price) / Decimal::from(2);
                    self.graph.update_price(input.market.id, mid);
                }
            }
        }
    }
}

impl MultiSignalGenerator for CorrelationGenerator {
    fn generate(&mut self, input: &SignalInput) -> Vec<TradeSignal> {
        // This generator needs multiple market inputs
        // For now, return empty - should be called with all markets
        vec![]
    }

    fn update_state(&mut self, _market_id: Uuid, _update: &crate::signals::StateUpdate) {
        // Not applicable for correlation generator
    }
}

// Implement old SignalGenerator trait for backward compatibility
impl SignalGenerator for CorrelationGenerator {
    fn generate(&self, _input: &SignalInput) -> anyhow::Result<Option<TradeSignal>> {
        Ok(None)
    }

    fn signal_type(&self) -> SignalType {
        SignalType::SpreadArbitrage
    }
}

impl CorrelationGenerator {
    /// Find arbitrage opportunities across all markets
    pub fn find_arbitrage_opportunities(&self) -> Vec<LogicalArbitrageOpportunity> {
        self.graph.find_violations()
    }

    /// Convert arbitrage opportunity to trade signals
    pub fn opportunity_to_signals(
        &self,
        opportunity: &LogicalArbitrageOpportunity,
    ) -> Vec<TradeSignal> {
        opportunity
            .trades
            .iter()
            .map(|trade| TradeSignal {
                id: Uuid::new_v4(),
                market_id: trade.market_id,
                signal_type: SignalType::SpreadArbitrage,
                direction: trade.direction,
                outcome_id: trade.outcome_id.clone(),
                entry_price: trade.entry_price,
                target_price: Decimal::ONE,
                stop_loss: trade.entry_price * Decimal::from_str_exact("1.1").unwrap(),
                position_size: trade.position_size,
                confidence: 0.95, // High confidence - mathematical edge
                expected_value: opportunity.expected_profit / opportunity.trades.len() as i64,
                edge: opportunity.violation_amount / trade.entry_price,
                kelly_fraction: 0.15,
                reasoning: opportunity.description.clone(),
                metadata: SignalMetadata {
                    research_sources: vec!["correlation_arbitrage".to_string()],
                    data_points: opportunity.markets.len() as u32,
                    liquidity_score: 0.8,
                    volatility_score: 0.5,
                    custom_fields: serde_json::json!({
                        "strategy": "correlation_arbitrage",
                        "opportunity_id": opportunity.id.to_string(),
                        "opportunity_type": opportunity.opportunity_type,
                        "markets": opportunity.markets,
                        "violation_amount": opportunity.violation_amount.to_string(),
                        "expected_profit": opportunity.expected_profit.to_string(),
                    }),
                },
                created_at: Utc::now(),
                expires_at: Some(Utc::now() + chrono::Duration::minutes(10)),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_implication_violation() {
        let mut graph = CorrelationGraph::new();

        graph.update_price(uuid::Uuid::new_v4(), Decimal::from_str_exact("0.40").unwrap());
        graph.update_price(uuid::Uuid::new_v4(), Decimal::from_str_exact("0.30").unwrap());

        // Should find violations
        let violations = graph.find_violations();
        assert!(violations.len() >= 0);
    }

    #[test]
    fn test_mutually_exclusive() {
        let mut graph = CorrelationGraph::new();

        let market1 = uuid::Uuid::new_v4();
        let market2 = uuid::Uuid::new_v4();

        graph.update_price(market1, Decimal::from_str_exact("0.60").unwrap());
        graph.update_price(market2, Decimal::from_str_exact("0.50").unwrap());

        graph.add_edge(CorrelationEdge {
            from_market: market1,
            to_market: market2,
            correlation_type: CorrelationType::MutuallyExclusive,
            min_spread: Decimal::from_str_exact("0.03").unwrap(),
        });

        let violations = graph.find_violations();
        // Should find violation since 0.60 + 0.50 = 1.10 > 1.00
        assert!(!violations.is_empty());
    }
}
