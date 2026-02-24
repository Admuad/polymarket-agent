// Pair Cost Arbitrage Generator (gabagool style)
// Generates signals based on maintaining avg_YES + avg_NO < 1.00

use chrono::{DateTime, Utc};
use common::{Market, OrderSide};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::signals::{
    SignalInput, SignalDirection, SignalGenerator, SignalMetadata, SignalType, TradeSignal,
    OrderBookSnapshot, StateUpdate, MultiSignalGenerator,
};

/// Pair cost configuration
#[derive(Debug, Clone)]
pub struct PairCostConfig {
    /// Target pair cost (must be < 1.00 for guaranteed profit)
    pub target_pair_cost: Decimal,
    /// Safety margin (e.g., 0.99 = 1% margin of safety)
    pub safety_margin: Decimal,
    /// Minimum position size per trade
    pub min_position_size: Decimal,
    /// Maximum position imbalance (YES/NO ratio)
    pub max_imbalance_ratio: Decimal,
    /// Maximum total position size
    pub max_total_size: Decimal,
    /// Minimum edge required to enter position
    pub min_edge: Decimal,
}

impl Default for PairCostConfig {
    fn default() -> Self {
        PairCostConfig {
            target_pair_cost: Decimal::from_str_exact("1.00").unwrap(),
            safety_margin: Decimal::from_str_exact("0.99").unwrap(), // 1% safety margin
            min_position_size: Decimal::from_str_exact("10").unwrap(), // $10 min trade
            max_imbalance_ratio: Decimal::from_str_exact("1.5").unwrap(), // 1.5:1 max ratio
            max_total_size: Decimal::from_str_exact("1000").unwrap(), // $1000 max position
            min_edge: Decimal::from_str_exact("0.01").unwrap(), // 1% minimum edge
        }
    }
}

/// Pair cost state for a market
#[derive(Debug, Clone)]
pub struct PairCostState {
    pub yes_qty: Decimal,
    pub no_qty: Decimal,
    pub yes_cost: Decimal,
    pub no_cost: Decimal,
    pub avg_yes_price: Decimal,
    pub avg_no_price: Decimal,
    pub pair_cost: Decimal,
    pub total_invested: Decimal,
}

impl Default for PairCostState {
    fn default() -> Self {
        PairCostState {
            yes_qty: Decimal::ZERO,
            no_qty: Decimal::ZERO,
            yes_cost: Decimal::ZERO,
            no_cost: Decimal::ZERO,
            avg_yes_price: Decimal::ZERO,
            avg_no_price: Decimal::ZERO,
            pair_cost: Decimal::ZERO,
            total_invested: Decimal::ZERO,
        }
    }
}

impl PairCostState {
    /// Calculate current pair cost
    pub fn calculate_pair_cost(&mut self) {
        if self.yes_qty > Decimal::ZERO {
            self.avg_yes_price = self.yes_cost / self.yes_qty;
        }
        if self.no_qty > Decimal::ZERO {
            self.avg_no_price = self.no_cost / self.no_qty;
        }
        self.pair_cost = self.avg_yes_price + self.avg_no_price;
    }

    /// Check if we have locked in profit
    pub fn has_locked_profit(&self, config: &PairCostConfig) -> bool {
        self.pair_cost > Decimal::ZERO && self.pair_cost < config.target_pair_cost * config.safety_margin
    }

    /// Calculate guaranteed profit
    pub fn guaranteed_profit(&self) -> Decimal {
        let min_qty = self.yes_qty.min(self.no_qty);
        let total_cost = self.yes_cost + self.no_cost;
        min_qty - total_cost
    }

    /// Check if we should add YES position
    pub fn should_buy_yes(&self, price: Decimal, config: &PairCostConfig) -> bool {
        // Check if adding YES at this price improves our position
        let new_yes_cost = self.yes_cost + (price * config.min_position_size);
        let new_yes_qty = self.yes_qty + config.min_position_size;
        let new_avg_yes = new_yes_cost / new_yes_qty;

        let new_pair_cost = new_avg_yes + self.avg_no_price;

        // Check if we exceed max size
        if self.yes_qty + config.min_position_size > config.max_total_size {
            return false;
        }

        // Check if imbalance is acceptable
        if self.no_qty > Decimal::ZERO {
            let new_ratio = new_yes_qty / self.no_qty;
            if new_ratio > config.max_imbalance_ratio {
                return false;
            }
        }

        // Check if this improves our pair cost
        new_pair_cost < self.pair_cost && new_pair_cost < config.safety_margin
    }

    /// Check if we should add NO position
    pub fn should_buy_no(&self, price: Decimal, config: &PairCostConfig) -> bool {
        // Check if adding NO at this price improves our position
        let new_no_cost = self.no_cost + (price * config.min_position_size);
        let new_no_qty = self.no_qty + config.min_position_size;
        let new_avg_no = new_no_cost / new_no_qty;

        let new_pair_cost = self.avg_yes_price + new_avg_no;

        // Check if we exceed max size
        if self.no_qty + config.min_position_size > config.max_total_size {
            return false;
        }

        // Check if imbalance is acceptable
        if self.yes_qty > Decimal::ZERO {
            let new_ratio = self.no_qty / self.yes_qty;
            if new_ratio > config.max_imbalance_ratio {
                return false;
            }
        }

        // Check if this improves our pair cost
        new_pair_cost < self.pair_cost && new_pair_cost < config.safety_margin
    }

    /// Update state after buying YES
    pub fn add_yes(&mut self, qty: Decimal, price: Decimal) {
        self.yes_qty += qty;
        self.yes_cost += qty * price;
        self.total_invested += qty * price;
        self.calculate_pair_cost();
    }

    /// Update state after buying NO
    pub fn add_no(&mut self, qty: Decimal, price: Decimal) {
        self.no_qty += qty;
        self.no_cost += qty * price;
        self.total_invested += qty * price;
        self.calculate_pair_cost();
    }
}

/// Pair cost arbitrage generator
pub struct PairCostGenerator {
    config: PairCostConfig,
    states: std::collections::HashMap<Uuid, PairCostState>,
}

impl PairCostGenerator {
    pub fn new(config: PairCostConfig) -> Self {
        PairCostGenerator {
            config,
            states: std::collections::HashMap::new(),
        }
    }

    /// Find optimal entry points for pair cost arbitrage
    fn find_entry_opportunity(
        &self,
        order_book: &OrderBookSnapshot,
        state: &PairCostState,
    ) -> (Option<TradeSignal>, Option<TradeSignal>) {
        let mut yes_signal = None;
        let mut no_signal = None;

        // Check YES entry (look for cheap YES)
        if let Some(best_ask) = order_book.asks.first() {
            if state.should_buy_yes(best_ask.price, &self.config) {
                yes_signal = Some(self.create_yes_signal(best_ask.price, state));
            }
        }

        // Check NO entry (look for cheap NO)
        if let Some(best_bid) = order_book.bids.first() {
            // NO price = 1 - YES price
            let no_price = Decimal::ONE - best_bid.price;
            if state.should_buy_no(no_price, &self.config) {
                no_signal = Some(self.create_no_signal(no_price, state));
            }
        }

        (yes_signal, no_signal)
    }

    fn create_yes_signal(&self, price: Decimal, state: &PairCostState) -> TradeSignal {
        let qty = self.config.min_position_size;
        let cost = qty * price;

        TradeSignal {
            id: Uuid::new_v4(),
            market_id: uuid::Uuid::new_v4(), // Will be set by caller
            signal_type: SignalType::SpreadArbitrage,
            direction: SignalDirection::Long,
            outcome_id: None,
            entry_price: price,
            target_price: Decimal::ONE, // Resolves to $1.00 if correct
            stop_loss: price * Decimal::from_str_exact("0.9").unwrap(),
            position_size: qty,
            confidence: 0.95, // High confidence - mathematical edge
            expected_value: cost * (Decimal::ONE - price),
            edge: (Decimal::ONE - price) / price,
            kelly_fraction: 0.2, // More aggressive for guaranteed profit
            reasoning: format!(
                "Pair Cost Arbitrage: Add YES @ {:.4}, current pair cost {:.4}, guaranteed if < {:.4}",
                price, state.pair_cost, self.config.safety_margin
            ),
            metadata: SignalMetadata {
                research_sources: vec!["pair_cost_arbitrage".to_string()],
                data_points: 1,
                liquidity_score: 0.85,
                volatility_score: 0.5,
                custom_fields: serde_json::json!({
                    "strategy": "pair_cost_arbitrage",
                    "current_pair_cost": state.pair_cost.to_string(),
                    "target_pair_cost": self.config.target_pair_cost.to_string(),
                    "yes_qty": state.yes_qty.to_string(),
                    "no_qty": state.no_qty.to_string(),
                    "guaranteed_profit": state.guaranteed_profit().to_string(),
                }),
            },
            created_at: Utc::now(),
            expires_at: Some(Utc::now() + chrono::Duration::minutes(15)), // 15 min validity
        }
    }

    fn create_no_signal(&self, price: Decimal, state: &PairCostState) -> TradeSignal {
        let qty = self.config.min_position_size;
        let cost = qty * price;

        TradeSignal {
            id: Uuid::new_v4(),
            market_id: uuid::Uuid::new_v4(),
            signal_type: SignalType::SpreadArbitrage,
            direction: SignalDirection::Short,
            outcome_id: None,
            entry_price: price,
            target_price: Decimal::ONE,
            stop_loss: price * Decimal::from_str_exact("1.1").unwrap(),
            position_size: qty,
            confidence: 0.95,
            expected_value: cost * (Decimal::ONE - price),
            edge: (Decimal::ONE - price) / price,
            kelly_fraction: 0.2,
            reasoning: format!(
                "Pair Cost Arbitrage: Add NO @ {:.4}, current pair cost {:.4}, guaranteed if < {:.4}",
                price, state.pair_cost, self.config.safety_margin
            ),
            metadata: SignalMetadata {
                research_sources: vec!["pair_cost_arbitrage".to_string()],
                data_points: 1,
                liquidity_score: 0.85,
                volatility_score: 0.5,
                custom_fields: serde_json::json!({
                    "strategy": "pair_cost_arbitrage",
                    "current_pair_cost": state.pair_cost.to_string(),
                    "target_pair_cost": self.config.target_pair_cost.to_string(),
                    "yes_qty": state.yes_qty.to_string(),
                    "no_qty": state.no_qty.to_string(),
                    "guaranteed_profit": state.guaranteed_profit().to_string(),
                }),
            },
            created_at: Utc::now(),
            expires_at: Some(Utc::now() + chrono::Duration::minutes(15)),
        }
    }
}

impl SignalGenerator for PairCostGenerator {
    fn generate(&mut self, input: &SignalInput) -> Vec<TradeSignal> {
        let order_book = match &input.order_book {
            Some(ob) => ob,
            None => return vec![],
        };

        // Get or create state
        let state = self.states
            .entry(input.market.id)
            .or_insert_with(PairCostState::default());

        // Check if we already have locked profit - no more entries needed
        if state.has_locked_profit(&self.config) {
            return vec![];
        }

        // Find entry opportunities
        let (yes_signal, no_signal) = self.find_entry_opportunity(order_book, state);

        let mut signals = Vec::new();

        if let Some(mut s) = yes_signal {
            s.market_id = input.market.id;
            s.outcome_id = input.market.outcomes.get(0).map(|o| o.id.clone());
            signals.push(s);
        }

        if let Some(mut s) = no_signal {
            s.market_id = input.market.id;
            s.outcome_id = input.market.outcomes.get(0).map(|o| o.id.clone());
            signals.push(s);
        }

        signals
    }

    fn update_state(&mut self, market_id: Uuid, update: &StateUpdate) {
        let state = self.states.entry(market_id).or_insert_with(PairCostState::default());

        match update {
            StateUpdate::TradeExecution { side, size, price, .. } => {
                match side {
                    OrderSide::Buy => {
                        // Determine if this is YES or NO based on price
                        // YES: price < 0.5 typically, NO: price > 0.5 typically
                        // But we need explicit signal from execution
                    }
                    OrderSide::Sell => {
                        // Selling YES = buying NO
                    }
                }
            }
            StateUpdate::PositionClosed { side, size, realized_pnl, .. } => {
                match side {
                    OrderSide::Buy => {
                        state.yes_qty -= *size;
                        state.total_invested += *realized_pnl;
                    }
                    OrderSide::Sell => {
                        state.no_qty -= *size;
                        state.total_invested += *realized_pnl;
                    }
                }
                state.calculate_pair_cost();
            }
            StateUpdate::VolatilityUpdate { .. } => {
                // Not used for pair cost arbitrage
            }
        }
    }
}

// Implement old SignalGenerator trait for backward compatibility
impl SignalGenerator for PairCostGenerator {
    fn generate(&self, input: &SignalInput) -> anyhow::Result<Option<TradeSignal>> {
        let mut temp_gen = PairCostGenerator {
            config: self.config.clone(),
            states: self.states.clone(),
        };
        let signals = temp_gen.generate(input);

        Ok(signals.into_iter().next())
    }

    fn signal_type(&self) -> SignalType {
        SignalType::SpreadArbitrage
    }
}
