//! Risk management configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Overall risk management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// Position-level and portfolio-level risk limits
    pub risk_limits: RiskLimits,

    /// Circuit breaker configuration
    pub circuit_breakers: CircuitBreakerConfig,

    /// Kelly criterion configuration
    #[serde(default = "default_kelly_multiplier")]
    pub kelly_multiplier: f64,

    /// Correlation monitoring settings
    #[serde(default)]
    pub correlation_threshold: f64,

    /// Risk metric calculation settings
    #[serde(default)]
    pub metrics: MetricsConfig,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            risk_limits: RiskLimits::default(),
            circuit_breakers: CircuitBreakerConfig::default(),
            kelly_multiplier: 0.25, // Conservative quarter-Kelly
            correlation_threshold: 0.7,
            metrics: MetricsConfig::default(),
        }
    }
}

fn default_kelly_multiplier() -> f64 {
    0.25
}

/// Risk limits at different levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    /// Maximum position size for a single market (in USD)
    pub max_position_size: f64,

    /// Maximum total portfolio exposure (in USD)
    pub max_total_exposure: f64,

    /// Maximum exposure per theme/category (in USD)
    pub max_theme_exposure: f64,

    /// Maximum number of positions
    pub max_positions: usize,

    /// Maximum percentage of portfolio in any single theme
    pub max_theme_percentage: f64,

    /// Daily loss limit (in USD) - triggers circuit breaker
    pub daily_loss_limit: f64,

    /// Stop loss percentage per position
    pub stop_loss_percentage: f64,

    /// Theme/category-specific limits
    #[serde(default)]
    pub theme_limits: HashMap<String, ThemeLimit>,
}

impl Default for RiskLimits {
    fn default() -> Self {
        let mut theme_limits = HashMap::new();
        theme_limits.insert("politics".to_string(), ThemeLimit {
            max_exposure: 500.0,
            max_positions: 5,
            max_percentage: 0.30,
        });
        theme_limits.insert("sports".to_string(), ThemeLimit {
            max_exposure: 300.0,
            max_positions: 3,
            max_percentage: 0.20,
        });
        theme_limits.insert("crypto".to_string(), ThemeLimit {
            max_exposure: 400.0,
            max_positions: 4,
            max_percentage: 0.25,
        });

        Self {
            max_position_size: 100.0,
            max_total_exposure: 1000.0,
            max_theme_exposure: 500.0,
            max_positions: 20,
            max_theme_percentage: 0.30,
            daily_loss_limit: 100.0,
            stop_loss_percentage: 0.20,
            theme_limits,
        }
    }
}

/// Theme-specific risk limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeLimit {
    /// Maximum exposure in this theme (in USD)
    pub max_exposure: f64,

    /// Maximum number of positions in this theme
    pub max_positions: usize,

    /// Maximum percentage of portfolio in this theme
    pub max_percentage: f64,
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Whether circuit breakers are enabled
    #[serde(default = "default_circuit_breakers_enabled")]
    pub enabled: bool,

    /// Stop trading if daily PnL drops below this (in USD)
    #[serde(default = "default_daily_loss_limit")]
    pub daily_loss_limit: f64,

    /// Stop trading if drawdown exceeds this percentage
    #[serde(default = "default_max_drawdown")]
    pub max_drawdown_percentage: f64,

    /// Stop trading if VaR (95%) exceeds this (in USD)
    #[serde(default = "default_var_limit")]
    pub var_95_limit: f64,

    /// Cooldown period after circuit breaker trigger (in minutes)
    #[serde(default = "default_cooldown_minutes")]
    pub cooldown_minutes: u64,

    /// Maximum number of violations per day before full halt
    #[serde(default = "default_max_violations")]
    pub max_violations_per_day: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            daily_loss_limit: 100.0,
            max_drawdown_percentage: 0.15,
            var_95_limit: 200.0,
            cooldown_minutes: 30,
            max_violations_per_day: 3,
        }
    }
}

fn default_circuit_breakers_enabled() -> bool {
    true
}

fn default_daily_loss_limit() -> f64 {
    100.0
}

fn default_max_drawdown() -> f64 {
    0.15
}

fn default_var_limit() -> f64 {
    200.0
}

fn default_cooldown_minutes() -> u64 {
    30
}

fn default_max_violations() -> usize {
    3
}

/// Risk metrics calculation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Number of samples for VaR calculation
    #[serde(default = "default_var_samples")]
    pub var_samples: usize,

    /// Confidence level for VaR (0.95 = 95%)
    #[serde(default = "default_var_confidence")]
    pub var_confidence: f64,

    /// Lookback period for Sharpe ratio calculation (in days)
    #[serde(default = "default_sharpe_lookback")]
    pub sharpe_lookback_days: u32,

    /// Risk-free rate for Sharpe ratio calculation (annualized)
    #[serde(default = "default_risk_free_rate")]
    pub risk_free_rate: f64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            var_samples: 100,
            var_confidence: 0.95,
            sharpe_lookback_days: 30,
            risk_free_rate: 0.05,
        }
    }
}

fn default_var_samples() -> usize {
    100
}

fn default_var_confidence() -> f64 {
    0.95
}

fn default_sharpe_lookback() -> u32 {
    30
}

fn default_risk_free_rate() -> f64 {
    0.05
}

/// Load configuration from TOML file
pub fn load_config(path: &str) -> anyhow::Result<RiskConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: RiskConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Save configuration to TOML file
pub fn save_config(config: &RiskConfig, path: &str) -> anyhow::Result<()> {
    let content = toml::to_string_pretty(config)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Create a default configuration file template
pub fn create_config_template(path: &str) -> anyhow::Result<()> {
    let template = "# Portfolio & Risk Management Configuration
# This file defines all risk limits and circuit breakers

[risk_limits]
# Maximum position size for a single market (USD)
max_position_size = 100.0

# Maximum total portfolio exposure (USD)
max_total_exposure = 1000.0

# Maximum exposure per theme/category (USD)
max_theme_exposure = 500.0

# Maximum number of open positions
max_positions = 20

# Maximum percentage of portfolio in any single theme
max_theme_percentage = 0.30

# Daily loss limit - triggers circuit breaker (USD)
daily_loss_limit = 100.0

# Stop loss percentage per position
stop_loss_percentage = 0.20

# Theme-specific limits
[risk_limits.theme_limits.politics]
max_exposure = 500.0
max_positions = 5
max_percentage = 0.30

[risk_limits.theme_limits.sports]
max_exposure = 300.0
max_positions = 3
max_percentage = 0.20

[risk_limits.theme_limits.crypto]
max_exposure = 400.0
max_positions = 4
max_percentage = 0.25

[circuit_breakers]
# Enable circuit breakers
enabled = true

# Stop trading if daily PnL drops below this (USD)
daily_loss_limit = 100.0

# Stop trading if drawdown exceeds this percentage
max_drawdown_percentage = 0.15

# Stop trading if VaR (95%) exceeds this (USD)
var_95_limit = 200.0

# Cooldown period after circuit breaker trigger (minutes)
cooldown_minutes = 30

# Maximum violations per day before full halt
max_violations_per_day = 3

# Kelly criterion multiplier (0.25 = quarter-Kelly, conservative)
kelly_multiplier = 0.25

correlation_threshold = 0.7

[metrics]
# Number of samples for VaR calculation
var_samples = 100

# Confidence level for VaR (0.95 = 95%)
var_confidence = 0.95

# Lookback period for Sharpe ratio (days)
sharpe_lookback_days = 30

# Risk-free rate for Sharpe (annualized)
risk_free_rate = 0.05
";

    std::fs::write(path, template)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RiskConfig::default();
        assert_eq!(config.kelly_multiplier, 0.25);
        assert!(config.risk_limits.max_position_size > 0.0);
    }

    #[test]
    fn test_config_serialization() {
        let config = RiskConfig::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: RiskConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(config.kelly_multiplier, deserialized.kelly_multiplier);
    }
}
