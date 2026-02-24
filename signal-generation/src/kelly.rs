// Kelly Criterion Position Sizing
// Mathematical optimal bet sizing based on edge and win probability

use rust_decimal::Decimal;

/// Kelly Criterion calculator
#[derive(Debug, Clone)]
pub struct KellyCalculator {
    /// Maximum fraction of bankroll to risk per trade
    pub max_fraction: Decimal,
    /// Safety multiplier (e.g., 0.5 = half Kelly for reduced volatility)
    pub safety_multiplier: Decimal,
    /// Minimum fraction to allocate (avoid over-trading with tiny edges)
    pub min_fraction: Decimal,
}

impl Default for KellyCalculator {
    fn default() -> Self {
        KellyCalculator {
            max_fraction: Decimal::from_str_exact("0.25").unwrap(), // 25% max
            safety_multiplier: Decimal::from_str_exact("0.5").unwrap(), // Half Kelly (quarter Kelly criterion)
            min_fraction: Decimal::from_str_exact("0.01").unwrap(), // 1% min
        }
    }
}

impl KellyCalculator {
    pub fn new(max_fraction: Decimal, safety_multiplier: Decimal, min_fraction: Decimal) -> Self {
        KellyCalculator {
            max_fraction,
            safety_multiplier,
            min_fraction,
        }
    }

    /// Calculate Kelly fraction
    /// Kelly = (bp - q) / b
    /// where:
    ///   b = odds received on bet (b to 1)
    ///   p = probability of winning
    ///   q = probability of losing (1 - p)
    ///   bp = expected value
    pub fn calculate(&self, win_probability: f64, avg_win: Decimal, avg_loss: Decimal) -> Decimal {
        // Convert to Decimal
        let p = Decimal::from_f64(win_probability).unwrap_or(Decimal::ZERO);
        let q = Decimal::ONE - p;
        let b = avg_win / avg_loss.abs();

        // Kelly formula: (b*p - q) / b
        let bp = b * p;
        let numerator = bp - q;

        if b == Decimal::ZERO {
            return Decimal::ZERO;
        }

        let kelly = numerator / b;

        // Apply safety multiplier (half Kelly is common)
        let safe_kelly = kelly * self.safety_multiplier;

        // Clamp to min/max bounds
        safe_kelly
            .max(self.min_fraction)
            .min(self.max_fraction)
            .max(Decimal::ZERO)
    }

    /// Calculate position size based on bankroll
    pub fn position_size(&self, bankroll: Decimal, kelly_fraction: Decimal) -> Decimal {
        bankroll * kelly_fraction
    }

    /// Calculate Kelly fraction from expected value
    /// Simplified version when we have EV and edge percentage
    pub fn from_edge(&self, edge: Decimal, win_probability: f64) -> Decimal {
        let p = Decimal::from_f64(win_probability).unwrap_or(Decimal::ZERO);
        let q = Decimal::ONE - p;

        // Simplified Kelly using edge
        let kelly = (edge * p - (Decimal::ONE - p) * edge) / edge;

        let safe_kelly = kelly * self.safety_multiplier;

        safe_kelly
            .max(self.min_fraction)
            .min(self.max_fraction)
            .max(Decimal::ZERO)
    }

    /// Calculate optimal position size with bankroll and edge
    pub fn optimal_position(
        &self,
        bankroll: Decimal,
        win_probability: f64,
        avg_win: Decimal,
        avg_loss: Decimal,
    ) -> Decimal {
        let kelly = self.calculate(win_probability, avg_win, avg_loss);
        self.position_size(bankroll, kelly)
    }

    /// Calculate position size from edge percentage
    pub fn position_from_edge(
        &self,
        bankroll: Decimal,
        edge: Decimal,
        win_probability: f64,
    ) -> Decimal {
        let kelly = self.from_edge(edge, win_probability);
        self.position_size(bankroll, kelly)
    }
}

/// Position sizing strategies
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositionSizingStrategy {
    /// Full Kelly (most aggressive, highest volatility)
    FullKelly,
    /// Half Kelly (recommended, good balance)
    HalfKelly,
    /// Quarter Kelly (conservative)
    QuarterKelly,
    /// Fixed fractional (e.g., always 2%)
    FixedFraction(Decimal),
    /// Volatility-adjusted (reduce size when volatility is high)
    VolatilityAdjusted,
}

impl Default for PositionSizingStrategy {
    fn default() -> Self {
        PositionSizingStrategy::HalfKelly
    }
}

/// Advanced position calculator with multiple strategies
#[derive(Debug, Clone)]
pub struct PositionCalculator {
    kelly: KellyCalculator,
    strategy: PositionSizingStrategy,
}

impl Default for PositionCalculator {
    fn default() -> Self {
        PositionCalculator {
            kelly: KellyCalculator::default(),
            strategy: PositionSizingStrategy::HalfKelly,
        }
    }
}

impl PositionCalculator {
    pub fn new(kelly: KellyCalculator, strategy: PositionSizingStrategy) -> Self {
        PositionCalculator { kelly, strategy }
    }

    /// Calculate optimal position size based on configured strategy
    pub fn calculate_position(
        &self,
        bankroll: Decimal,
        win_probability: f64,
        avg_win: Decimal,
        avg_loss: Decimal,
        volatility: f64,
    ) -> Decimal {
        let base_position = self.kelly.optimal_position(bankroll, win_probability, avg_win, avg_loss);

        match self.strategy {
            PositionSizingStrategy::FullKelly => base_position,
            PositionSizingStrategy::HalfKelly => base_position * Decimal::from_str_exact("0.5").unwrap(),
            PositionSizingStrategy::QuarterKelly => base_position * Decimal::from_str_exact("0.25").unwrap(),
            PositionSizingStrategy::FixedFraction(frac) => bankroll * frac,
            PositionSizingStrategy::VolatilityAdjusted => {
                // Reduce position when volatility is high
                let vol_adjustment = if volatility > 0.7 {
                    Decimal::from_str_exact("0.5").unwrap() // Cut in half
                } else if volatility > 0.5 {
                    Decimal::from_str_exact("0.75").unwrap() // Reduce by 25%
                } else {
                    Decimal::ONE // Full size
                };

                base_position * vol_adjustment
            }
        }
    }

    /// Calculate position size from edge
    pub fn calculate_from_edge(
        &self,
        bankroll: Decimal,
        edge: Decimal,
        win_probability: f64,
        volatility: f64,
    ) -> Decimal {
        let base_position = self.kelly.position_from_edge(bankroll, edge, win_probability);

        match self.strategy {
            PositionSizingStrategy::VolatilityAdjusted => {
                let vol_adjustment = if volatility > 0.7 {
                    Decimal::from_str_exact("0.5").unwrap()
                } else if volatility > 0.5 {
                    Decimal::from_str_exact("0.75").unwrap()
                } else {
                    Decimal::ONE
                };

                base_position * vol_adjustment
            }
            _ => base_position,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kelly_calculation() {
        let calc = KellyCalculator::default();

        // 60% win rate, 2:1 reward/risk
        let kelly = calc.calculate(0.6, Decimal::from(2), Decimal::from(1));
        assert!(kelly > Decimal::ZERO);
        assert!(kelly < calc.max_fraction);
    }

    #[test]
    fn test_kelly_limits() {
        let calc = KellyCalculator::default();

        // Very low edge should give min fraction
        let kelly = calc.calculate(0.51, Decimal::from(1.01), Decimal::from(1));
        assert_eq!(kelly, calc.min_fraction);

        // High edge should be capped at max
        let kelly = calc.calculate(0.8, Decimal::from(3), Decimal::from(1));
        assert!(kelly <= calc.max_fraction);
    }

    #[test]
    fn test_position_size() {
        let calc = KellyCalculator::default();

        let bankroll = Decimal::from(10000);
        let kelly_fraction = Decimal::from_str_exact("0.1").unwrap();

        let position = calc.position_size(bankroll, kelly_fraction);
        assert_eq!(position, Decimal::from(1000));
    }

    #[test]
    fn test_volatility_adjusted() {
        let kelly = KellyCalculator::default();
        let calc = PositionCalculator::new(kelly, PositionSizingStrategy::VolatilityAdjusted);

        let base_size = Decimal::from(1000);

        // Low volatility
        let pos = calc.calculate_from_edge(base_size, Decimal::from(10), 0.05, 0.3);
        assert_eq!(pos, base_size); // Full size

        // High volatility
        let pos = calc.calculate_from_edge(base_size, Decimal::from(10), 0.05, 0.8);
        assert_eq!(pos, base_size / Decimal::from(2)); // Half size
    }
}
