// Signal Generation Pipeline
// Orchestrates signal generation from research outputs and market data

use super::signals::{SignalGenerator, SignalInput, TradeSignal};
use super::validators::SignalValidator;
use super::storage::SignalStorage;
use anyhow::Result;
use rust_decimal::prelude::*;
use tracing::{debug, info, warn};

/// Configuration for the signal generation pipeline
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Enable/disable signal generation
    pub enabled: bool,
    /// Maximum number of signals to generate per cycle
    pub max_signals_per_cycle: usize,
    /// Minimum confidence threshold for any signal
    pub min_confidence: f64,
    /// Minimum edge threshold for any signal
    pub min_edge: Decimal,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_signals_per_cycle: 10,
            min_confidence: 0.6,
            min_edge: Decimal::from_str_exact("0.03").unwrap(), // 3%
        }
    }
}

/// Signal generation pipeline
pub struct SignalPipeline {
    generators: Vec<Box<dyn SignalGenerator + Send + Sync>>,
    validators: Vec<Box<dyn SignalValidator + Send + Sync>>,
    storage: Option<Box<dyn SignalStorage + Send + Sync>>,
    config: PipelineConfig,
}

impl SignalPipeline {
    /// Create a new signal pipeline
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            generators: Vec::new(),
            validators: Vec::new(),
            storage: None,
            config,
        }
    }

    /// Add a signal generator
    pub fn add_generator(mut self, generator: Box<dyn SignalGenerator + Send + Sync>) -> Self {
        info!("Adding signal generator: {:?}", generator.signal_type());
        self.generators.push(generator);
        self
    }

    /// Add a signal validator
    pub fn add_validator(mut self, validator: Box<dyn SignalValidator + Send + Sync>) -> Self {
        info!("Adding signal validator");
        self.validators.push(validator);
        self
    }

    /// Set signal storage
    pub fn with_storage(mut self, storage: Box<dyn SignalStorage + Send + Sync>) -> Self {
        info!("Setting signal storage");
        self.storage = Some(storage);
        self
    }

    /// Process a signal input and generate signals
    pub async fn process(&self, input: &SignalInput) -> Result<Vec<TradeSignal>> {
        if !self.config.enabled {
            debug!("Pipeline is disabled, skipping signal generation");
            return Ok(Vec::new());
        }

        let mut signals = Vec::new();

        // Generate signals from all generators
        for generator in &self.generators {
            match generator.generate(input) {
                Ok(Some(signal)) => {
                    debug!("Generated signal: {:?} for market {:?}", signal.signal_type, signal.market_id);
                    signals.push(signal);
                }
                Ok(None) => {
                    debug!("No signal generated from {:?}", generator.signal_type());
                }
                Err(e) => {
                    warn!("Error generating signal from {:?}: {}", generator.signal_type(), e);
                }
            }
        }

        // Apply global filters
        signals = signals
            .into_iter()
            .filter(|s| s.confidence >= self.config.min_confidence)
            .filter(|s| s.edge >= self.config.min_edge)
            .collect();

        // Validate signals
        let mut validated_signals = Vec::new();
        for signal in &signals {
            if self.validate(signal).await? {
                validated_signals.push(signal.clone());
            } else {
                debug!("Signal rejected by validators: {:?}", signal.signal_type);
            }
        }

        // Limit number of signals
        validated_signals.sort_by(|a, b| {
            // Sort by expected value * confidence
            let a_score = a.expected_value * Decimal::from_f64(a.confidence).unwrap_or(Decimal::ZERO);
            let b_score = b.expected_value * Decimal::from_f64(b.confidence).unwrap_or(Decimal::ZERO);
            b_score.partial_cmp(&a_score).unwrap_or(std::cmp::Ordering::Equal)
        });

        validated_signals.truncate(self.config.max_signals_per_cycle);

        // Store signals if storage is configured
        if let Some(storage) = &self.storage {
            for signal in &validated_signals {
                if let Err(e) = storage.store(signal).await {
                    warn!("Failed to store signal {:?}: {}", signal.id, e);
                }
            }
        }

        info!("Generated {} valid signals", validated_signals.len());
        Ok(validated_signals)
    }

    /// Validate a signal against all validators
    async fn validate(&self, signal: &TradeSignal) -> Result<bool> {
        for validator in &self.validators {
            if !validator.validate(signal).await? {
                debug!("Signal rejected by validator");
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Get the number of registered generators
    pub fn generator_count(&self) -> usize {
        self.generators.len()
    }

    /// Get the number of registered validators
    pub fn validator_count(&self) -> usize {
        self.validators.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PipelineConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_signals_per_cycle, 10);
    }
}
