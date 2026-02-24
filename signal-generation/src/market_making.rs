// Market Making Signal Generator
// Generates signals for providing liquidity on both sides of markets

use chrono::{DateTime, Utc};
use common::{Market, OrderSide};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::signals::{
    SignalInput, SignalDirection, SignalGenerator, SignalMetadata, SignalType, TradeSignal,
    OrderBookSnapshot, Level, StateUpdate, MultiSignalGenerator
};

/// Market making configuration
#[derive(Debug, Clone)]
pub struct MarketMakingConfig {
    /// Minimum spread required to provide liquidity
    pub min_spread: Decimal,
    /// Maximum inventory imbalance (0.3 = never hold >30% on one side)
    pub max_inventory_imbalance: Decimal,
    /// Base position size for each side
    pub base_position_size: Decimal,
    /// Spread multiplier during high volatility
    pub volatility_multiplier: Decimal,
    /// Inventory adjustment factor
    pub inventory_adjustment: Decimal,
    /// Widen spreads during breaking news
    pub news_spread_multiplier: Decimal,
}

impl Default for MarketMakingConfig {
    fn default() -> Self {
        MarketMakingConfig {
            min_spread: Decimal::from_str_exact("0.02").unwrap(), // 2% spread
            max_inventory_imbalance: Decimal::from_str_exact("0.3").unwrap(), // 30% max imbalance
            base_position_size: Decimal::from_str_exact("100").unwrap(), // $100 base size
            volatility_multiplier: Decimal::from_str_exact("1.5").unwrap(), // 1.5x spread in high vol
            inventory_adjustment: Decimal::from_str_exact("0.1").unwrap(), // 10% adjustment per imbalance
            news_spread_multiplier: Decimal::from_str_exact("2.0").unwrap(), // 2x spread during news
        }
    }
}

/// Market making state
#[derive(Debug, Clone)]
pub struct MarketMakingState {
    pub yes_inventory: Decimal,
    pub no_inventory: Decimal,
    pub total_invested: Decimal,
    pub last_spread: Decimal,
    pub volatility_score: f64,
}

impl Default for MarketMakingState {
    fn default() -> Self {
        MarketMakingState {
            yes_inventory: Decimal::ZERO,
            no_inventory: Decimal::ZERO,
            total_invested: Decimal::ZERO,
            last_spread: Decimal::ZERO,
            volatility_score: 0.0,
        }
    }
}

/// Market making signal generator
pub struct MarketMakingGenerator {
    config: MarketMakingConfig,
    states: std::collections::HashMap<Uuid, MarketMakingState>,
}

impl MarketMakingGenerator {
    pub fn new(config: MarketMakingConfig) -> Self {
        MarketMakingGenerator {
            config,
            states: std::collections::HashMap::new(),
        }
    }

    /// Calculate inventory imbalance (-1 to 1)
    fn calculate_imbalance(&self, state: &MarketMakingState) -> Decimal {
        if state.yes_inventory + state.no_inventory == Decimal::ZERO {
            return Decimal::ZERO;
        }

        (state.yes_inventory - state.no_inventory)
            / (state.yes_inventory + state.no_inventory)
    }

    /// Calculate adjusted spread based on inventory imbalance
    fn calculate_adjusted_spread(&self, base_spread: Decimal, imbalance: Decimal) -> Decimal {
        // Widen spread on the side with more inventory
        let adjustment = imbalance.abs() * self.config.inventory_adjustment;
        base_spread + adjustment
    }

    /// Check if we should provide liquidity on a side
    fn should_provide_liquidity(
        &self,
        state: &MarketMakingState,
        side: OrderSide,
        imbalance: Decimal,
    ) -> bool {
        let imbalance_threshold = self.config.max_inventory_imbalance;

        match side {
            OrderSide::Buy => {
                // Don't buy YES if we have too much YES inventory
                if imbalance > imbalance_threshold {
                    return false;
                }
                true
            }
            OrderSide::Sell => {
                // Don't sell NO if we have too much NO inventory
                if imbalance < -imbalance_threshold {
                    return false;
                }
                true
            }
        }
    }

    /// Generate limit order prices
    fn generate_order_prices(
        &self,
        market_price: Decimal,
        spread: Decimal,
    ) -> (Decimal, Decimal) {
        let half_spread = spread / Decimal::from_str_exact("2").unwrap();

        let yes_price = market_price - half_spread;
        let no_price = Decimal::ONE - yes_price;

        let min_price = Decimal::from_str_exact("0.01").unwrap();
        (yes_price.max(min_price), no_price.max(min_price))
    }
}

impl SignalGenerator for MarketMakingGenerator {
    fn generate(&mut self, input: &SignalInput) -> Vec<TradeSignal> {
        let order_book = match &input.order_book {
            Some(ob) => ob,
            None => return vec![], // Need order book for market making
        };

        // Get or create state
        let state = self.states
            .entry(input.market.id)
            .or_insert_with(MarketMakingState::default);

        // Calculate current market price (midpoint)
        let mid_price = if !order_book.bids.is_empty() && !order_book.asks.is_empty() {
            let best_bid = order_book.bids.first().unwrap().price;
            let best_ask = order_book.asks.first().unwrap().price;
            (best_bid + best_ask) / Decimal::from(2)
        } else {
            return vec![];
        };

        // Calculate inventory imbalance
        let imbalance = self.calculate_imbalance(state);

        // Calculate spread
        let base_spread = if state.volatility_score > 0.7 {
            self.config.min_spread * self.config.volatility_multiplier
        } else {
            self.config.min_spread
        };

        let adjusted_spread = self.calculate_adjusted_spread(base_spread, imbalance);

        // Generate order prices
        let (yes_price, no_price) = self.generate_order_prices(mid_price, adjusted_spread);

        let mut signals = Vec::new();

        // Generate YES liquidity signal (buy YES at lower price)
        if self.should_provide_liquidity(state, OrderSide::Buy, imbalance) {
            let yes_signal = TradeSignal {
                id: Uuid::new_v4(),
                market_id: input.market.id,
                signal_type: SignalType::MeanReversion, // Using MeanReversion for liquidity
                direction: SignalDirection::Long,
                outcome_id: input.market.outcomes.get(0).map(|o| o.id.clone()),
                entry_price: yes_price,
                target_price: mid_price,
                stop_loss: yes_price * Decimal::from_str_exact("0.95").unwrap(), // 5% stop loss
                position_size: self.config.base_position_size,
                confidence: 0.85, // High confidence for market making
                expected_value: (mid_price - yes_price) * self.config.base_position_size,
                edge: (mid_price - yes_price) / yes_price,
                kelly_fraction: 0.1, // Conservative position sizing
                reasoning: format!(
                    "Market making: providing YES liquidity at {:.4}, current mid {:.4}, spread {:.4}%",
                    yes_price, mid_price, adjusted_spread * Decimal::from(100)
                ),
                metadata: SignalMetadata {
                    research_sources: vec!["market_making".to_string()],
                    data_points: 1,
                    liquidity_score: 0.9,
                    volatility_score: state.volatility_score,
                    custom_fields: serde_json::json!({
                        "strategy": "market_making",
                        "inventory_imbalance": imbalance.to_string(),
                        "spread": adjusted_spread.to_string(),
                        "yes_inventory": state.yes_inventory.to_string(),
                        "no_inventory": state.no_inventory.to_string(),
                    }),
                },
                created_at: Utc::now(),
                expires_at: Some(Utc::now() + chrono::Duration::minutes(30)), // 30 min validity
            };
            signals.push(yes_signal);
        }

        // Generate NO liquidity signal (buy NO at lower price = sell YES)
        if self.should_provide_liquidity(state, OrderSide::Sell, imbalance) {
            let no_signal = TradeSignal {
                id: Uuid::new_v4(),
                market_id: input.market.id,
                signal_type: SignalType::MeanReversion,
                direction: SignalDirection::Short,
                outcome_id: input.market.outcomes.get(0).map(|o| o.id.clone()),
                entry_price: no_price,
                target_price: Decimal::ONE - mid_price,
                stop_loss: no_price * Decimal::from_str_exact("1.05").unwrap(), // 5% stop loss
                position_size: self.config.base_position_size,
                confidence: 0.85,
                expected_value: ((Decimal::ONE - mid_price) - no_price) * self.config.base_position_size,
                edge: ((Decimal::ONE - mid_price) - no_price) / no_price,
                kelly_fraction: 0.1,
                reasoning: format!(
                    "Market making: providing NO liquidity at {:.4}, current NO price {:.4}, spread {:.4}%",
                    no_price, Decimal::ONE - mid_price, adjusted_spread * Decimal::from(100)
                ),
                metadata: SignalMetadata {
                    research_sources: vec!["market_making".to_string()],
                    data_points: 1,
                    liquidity_score: 0.9,
                    volatility_score: state.volatility_score,
                    custom_fields: serde_json::json!({
                        "strategy": "market_making",
                        "inventory_imbalance": imbalance.to_string(),
                        "spread": adjusted_spread.to_string(),
                        "yes_inventory": state.yes_inventory.to_string(),
                        "no_inventory": state.no_inventory.to_string(),
                    }),
                },
                created_at: Utc::now(),
                expires_at: Some(Utc::now() + chrono::Duration::minutes(30)),
            };
            signals.push(no_signal);
        }

        state.last_spread = adjusted_spread;

        signals
    }

    fn update_state(&mut self, market_id: Uuid, update: &crate::signals::StateUpdate) {
        let state = self.states.entry(market_id).or_insert_with(MarketMakingState::default());

        match update {
            crate::signals::StateUpdate::TradeExecution { side, size, price, .. } => {
                let cost = *size * *price;

                match side {
                    OrderSide::Buy => {
                        state.yes_inventory += *size;
                        state.total_invested += cost;
                    }
                    OrderSide::Sell => {
                        state.no_inventory += *size;
                        state.total_invested += cost;
                    }
                }
            }
            crate::signals::StateUpdate::PositionClosed { side, size, realized_pnl, .. } => {
                match side {
                    OrderSide::Buy => {
                        state.yes_inventory -= *size;
                    }
                    OrderSide::Sell => {
                        state.no_inventory -= *size;
                    }
                }
                state.total_invested += *realized_pnl;
            }
            crate::signals::StateUpdate::VolatilityUpdate { score } => {
                state.volatility_score = *score;
            }
        }
    }
}

// Implement old SignalGenerator trait for backward compatibility
impl SignalGenerator for MarketMakingGenerator {
    fn generate(&self, input: &SignalInput) -> anyhow::Result<Option<TradeSignal>> {
        let order_book = match &input.order_book {
            Some(ob) => ob,
            None => return Ok(None),
        };

        // Calculate current market price (midpoint)
        let mid_price = if !order_book.bids.is_empty() && !order_book.asks.is_empty() {
            let best_bid = order_book.bids.first().unwrap().price;
            let best_ask = order_book.asks.first().unwrap().price;
            (best_bid + best_ask) / Decimal::from_str_exact("2").unwrap()
        } else {
            return Ok(None);
        };

        // Get state (non-mutable for old trait)
        let state = self.states.get(&input.market.id);

        // Just return first signal if any
        let mut temp_gen = MarketMakingGenerator {
            config: self.config.clone(),
            states: self.states.clone(),
        };
        let signals = temp_gen.generate(input);

        Ok(signals.into_iter().next())
    }

    fn signal_type(&self) -> SignalType {
        SignalType::MeanReversion
    }
}
