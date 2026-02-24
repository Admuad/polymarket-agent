//! Risk metrics calculation module

use serde::{Deserialize, Serialize};

/// Risk metrics for portfolio analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    /// Total portfolio value in USD
    pub total_value: f64,

    /// Number of open positions
    pub positions_count: usize,

    /// Current unrealized PnL
    pub unrealized_pnl: f64,

    /// Total realized PnL
    pub realized_pnl: f64,

    /// Maximum drawdown (as a percentage, 0.0 to 1.0)
    pub max_drawdown: f64,

    /// Sharpe ratio (annualized)
    pub sharpe_ratio: Option<f64>,

    /// Value at Risk at 95% confidence level
    pub var_95: Option<f64>,

    /// Value at Risk at 99% confidence level
    pub var_99: Option<f64>,

    /// Expected Shortfall (average of worst 5%)
    pub expected_shortfall: Option<f64>,
}

impl RiskMetrics {
    /// Create empty metrics
    pub fn empty() -> Self {
        Self {
            total_value: 0.0,
            positions_count: 0,
            unrealized_pnl: 0.0,
            realized_pnl: 0.0,
            max_drawdown: 0.0,
            sharpe_ratio: None,
            var_95: None,
            var_99: None,
            expected_shortfall: None,
        }
    }

    /// Get total PnL (realized + unrealized)
    pub fn total_pnl(&self) -> f64 {
        self.realized_pnl + self.unrealized_pnl
    }

    /// Get return on investment
    pub fn roi(&self) -> f64 {
        if self.total_value > 0.0 {
            self.total_pnl() / self.total_value
        } else {
            0.0
        }
    }

    /// Check if portfolio is in a risky state
    pub fn is_risky(&self) -> bool {
        self.max_drawdown > 0.10 || self.sharpe_ratio.map_or(false, |s| s < 0.0)
    }
}

impl Default for RiskMetrics {
    fn default() -> Self {
        Self::empty()
    }
}

/// Value at Risk calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaRResult {
    /// VaR at 95% confidence level
    pub var_95: Option<f64>,

    /// VaR at 99% confidence level
    pub var_99: Option<f64>,

    /// Expected Shortfall (Conditional VaR)
    pub expected_shortfall: Option<f64>,
}

impl VaRResult {
    /// Create empty result
    pub fn empty() -> Self {
        Self {
            var_95: None,
            var_99: None,
            expected_shortfall: None,
        }
    }
}

impl Default for VaRResult {
    fn default() -> Self {
        Self::empty()
    }
}

/// Drawdown calculation helper
#[derive(Debug, Clone)]
pub struct DrawdownCalculator {
    peak: f64,
    max_drawdown: f64,
    current_value: f64,
}

impl DrawdownCalculator {
    pub fn new() -> Self {
        Self {
            peak: 0.0,
            max_drawdown: 0.0,
            current_value: 0.0,
        }
    }

    /// Update with new portfolio value
    pub fn update(&mut self, value: f64) {
        self.current_value = value;

        if value > self.peak {
            self.peak = value;
        }

        let drawdown = (self.peak - value) / self.peak.max(1.0);
        if drawdown > self.max_drawdown {
            self.max_drawdown = drawdown;
        }
    }

    /// Get current maximum drawdown
    pub fn max_drawdown(&self) -> f64 {
        self.max_drawdown
    }

    /// Get current drawdown
    pub fn current_drawdown(&self) -> f64 {
        if self.peak > 0.0 {
            (self.peak - self.current_value) / self.peak
        } else {
            0.0
        }
    }

    /// Reset the calculator
    pub fn reset(&mut self) {
        self.peak = self.current_value;
        self.max_drawdown = 0.0;
    }
}

impl Default for DrawdownCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Sharpe ratio calculation helper
#[derive(Debug, Clone)]
pub struct SharpeCalculator {
    returns: Vec<f64>,
    risk_free_rate: f64,
    lookback_period: usize,
}

impl SharpeCalculator {
    pub fn new(risk_free_rate: f64, lookback_period: usize) -> Self {
        Self {
            returns: Vec::new(),
            risk_free_rate,
            lookback_period,
        }
    }

    /// Add a return value
    pub fn add_return(&mut self, return_value: f64) {
        self.returns.push(return_value);

        // Keep only lookback period
        if self.returns.len() > self.lookback_period {
            self.returns.remove(0);
        }
    }

    /// Calculate current Sharpe ratio
    pub fn calculate(&self) -> Option<f64> {
        if self.returns.len() < 2 {
            return None;
        }

        let mean: f64 = self.returns.iter().sum::<f64>() / self.returns.len() as f64;

        let variance: f64 = self
            .returns
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / self.returns.len() as f64;

        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return Some(0.0);
        }

        // Annualize (assuming daily returns)
        let annualized_mean = mean * 365.0;
        let annualized_std = std_dev * (365.0_f64).sqrt();

        Some((annualized_mean - self.risk_free_rate) / annualized_std)
    }

    /// Reset the calculator
    pub fn reset(&mut self) {
        self.returns.clear();
    }
}

/// Volatility calculator using standard deviation
#[derive(Debug, Clone)]
pub struct VolatilityCalculator {
    values: Vec<f64>,
    window_size: usize,
}

impl VolatilityCalculator {
    pub fn new(window_size: usize) -> Self {
        Self {
            values: Vec::new(),
            window_size,
        }
    }

    /// Add a value
    pub fn add_value(&mut self, value: f64) {
        self.values.push(value);

        if self.values.len() > self.window_size {
            self.values.remove(0);
        }
    }

    /// Calculate volatility (standard deviation of returns)
    pub fn calculate_volatility(&self) -> Option<f64> {
        if self.values.len() < 2 {
            return None;
        }

        // Calculate returns
        let returns: Vec<f64> = self
            .values
            .windows(2)
            .map(|w| {
                if w[0] > 0.0 {
                    (w[1] - w[0]) / w[0]
                } else {
                    0.0
                }
            })
            .collect();

        if returns.is_empty() {
            return None;
        }

        let mean: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance: f64 = returns
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;

        Some(variance.sqrt())
    }

    /// Calculate annualized volatility
    pub fn calculate_annualized_volatility(&self) -> Option<f64> {
        self.calculate_volatility().map(|v| v * (365.0_f64).sqrt())
    }
}

/// Position-level risk metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionRiskMetrics {
    /// Position value
    pub value: f64,

    /// Unrealized PnL
    pub unrealized_pnl: f64,

    /// Return on position
    pub roi: f64,

    /// Time held (in hours)
    pub hours_held: f64,

    /// Current price vs entry price
    pub price_change_pct: f64,

    /// Risk score (0.0 to 1.0)
    pub risk_score: f64,
}

impl PositionRiskMetrics {
    /// Calculate risk score based on multiple factors
    pub fn calculate_risk_score(&self) -> f64 {
        let mut score: f64 = 0.0;

        // Factor in PnL (negative PnL increases risk)
        if self.unrealized_pnl < 0.0 {
            score += 0.3;
        }

        // Factor in holding period (longer = potentially more risk)
        if self.hours_held > 24.0 {
            score += 0.2;
        }

        // Factor in price change (large moves increase risk)
        let price_change_abs = self.price_change_pct.abs();
        if price_change_abs > 0.1 {
            score += 0.3;
        } else if price_change_abs > 0.05 {
            score += 0.15;
        }

        score.min(1.0)
    }
}

/// Portfolio risk scoring
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskScore {
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
}

impl RiskScore {
    /// Calculate risk score from metrics
    pub fn from_metrics(metrics: &RiskMetrics) -> Self {
        let mut score = 0;

        // Consider drawdown
        if metrics.max_drawdown > 0.20 {
            score += 3;
        } else if metrics.max_drawdown > 0.10 {
            score += 2;
        } else if metrics.max_drawdown > 0.05 {
            score += 1;
        }

        // Consider Sharpe ratio
        if let Some(sharpe) = metrics.sharpe_ratio {
            if sharpe < -0.5 {
                score += 3;
            } else if sharpe < 0.0 {
                score += 2;
            } else if sharpe < 0.5 {
                score += 1;
            }
        }

        // Consider VaR
        if let Some(var) = metrics.var_95 {
            let var_pct = var / metrics.total_value.max(1.0);
            if var_pct > 0.15 {
                score += 3;
            } else if var_pct > 0.10 {
                score += 2;
            } else if var_pct > 0.05 {
                score += 1;
            }
        }

        match score {
            s if s >= 7 => RiskScore::VeryHigh,
            s if s >= 5 => RiskScore::High,
            s if s >= 3 => RiskScore::Medium,
            s if s >= 1 => RiskScore::Low,
            _ => RiskScore::VeryLow,
        }
    }

    /// Get risk level as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskScore::VeryLow => "Very Low",
            RiskScore::Low => "Low",
            RiskScore::Medium => "Medium",
            RiskScore::High => "High",
            RiskScore::VeryHigh => "Very High",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drawdown_calculator() {
        let mut calc = DrawdownCalculator::new();

        calc.update(100.0);
        assert_eq!(calc.max_drawdown(), 0.0);

        calc.update(90.0);
        assert_eq!(calc.current_drawdown(), 0.1);
        assert_eq!(calc.max_drawdown(), 0.1);

        calc.update(95.0);
        assert_eq!(calc.current_drawdown(), 0.05);
        assert_eq!(calc.max_drawdown(), 0.1); // Max stays at 0.1

        calc.update(110.0);
        assert_eq!(calc.current_drawdown(), 0.0);
    }

    #[test]
    fn test_sharpe_calculator() {
        let mut calc = SharpeCalculator::new(0.05, 30);

        calc.add_return(0.01);
        calc.add_return(0.02);
        calc.add_return(-0.01);

        let sharpe = calc.calculate();
        assert!(sharpe.is_some());
    }

    #[test]
    fn test_volatility_calculator() {
        let mut calc = VolatilityCalculator::new(20);

        calc.add_value(100.0);
        calc.add_value(105.0);
        calc.add_value(102.0);
        calc.add_value(108.0);

        let vol = calc.calculate_volatility();
        assert!(vol.is_some());
        assert!(vol.unwrap() > 0.0);
    }

    #[test]
    fn test_risk_score_from_metrics() {
        let mut metrics = RiskMetrics::empty();
        metrics.total_value = 1000.0;
        metrics.max_drawdown = 0.05;

        let score = RiskScore::from_metrics(&metrics);
        assert_eq!(score, RiskScore::VeryLow);

        metrics.max_drawdown = 0.25;
        metrics.sharpe_ratio = Some(-1.0); // Poor Sharpe ratio
        metrics.var_95 = Some(-200.0); // Large VaR

        let score = RiskScore::from_metrics(&metrics);
        assert!(score >= RiskScore::High);
    }
}
