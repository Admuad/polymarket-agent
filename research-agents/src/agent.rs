//! Base Agent trait and core agent types
//!
//! All specialist agents implement the Agent trait for consistency and coordination.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use common::Market;

/// Base configuration for any agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub agent_id: String,
    pub name: String,
    pub enabled: bool,
    pub max_markets_per_batch: usize,
    pub processing_interval_secs: u64,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            agent_id: Uuid::new_v4().to_string(),
            name: "UnnamedAgent".to_string(),
            enabled: true,
            max_markets_per_batch: 100,
            processing_interval_secs: 60,
        }
    }
}

/// Input data for agent processing
#[derive(Debug, Clone)]
pub struct AgentInput {
    pub market: Arc<Market>,
    pub timestamp: DateTime<Utc>,
    pub additional_data: Option<serde_json::Value>,
}

/// Output from agent processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    pub agent_id: String,
    pub market_id: Uuid,
    pub signal_type: String,
    pub data: serde_json::Value,
    pub confidence: f64, // 0.0 to 1.0
    pub timestamp: DateTime<Utc>,
    pub processing_time_ms: u64,
}

/// Current status of an agent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Processing,
    Error,
    Paused,
}

/// Base trait that all specialist agents must implement
///
/// This trait provides a consistent interface for the orchestrator to manage
/// different types of analysis agents.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Get the agent's configuration
    fn config(&self) -> &AgentConfig;

    /// Get the current status of the agent
    fn status(&self) -> AgentStatus;

    /// Process a single market and generate a signal
    ///
    /// Returns None if the agent cannot generate a signal for this market
    async fn process_market(&self, input: AgentInput) -> anyhow::Result<Option<AgentOutput>>;

    /// Process multiple markets in batch (optional optimization)
    ///
    /// Default implementation calls process_market for each market.
    /// Implementations can override this for batch optimizations.
    async fn process_batch(&self, inputs: Vec<AgentInput>) -> anyhow::Result<Vec<AgentOutput>> {
        let mut outputs = Vec::new();
        for input in inputs {
            if let Some(output) = self.process_market(input).await? {
                outputs.push(output);
            }
        }
        Ok(outputs)
    }

    /// Handle a control message from the orchestrator
    async fn handle_control(&self, msg: ControlMessage) -> anyhow::Result<ControlResponse>;

    /// Called when the agent is started
    async fn on_start(&self) -> anyhow::Result<()>;

    /// Called when the agent is stopped
    async fn on_stop(&self) -> anyhow::Result<()>;
}

/// Control messages from orchestrator to agents
#[derive(Debug, Clone)]
pub enum ControlMessage {
    Pause,
    Resume,
    UpdateConfig(AgentConfig),
    HealthCheck,
    Shutdown,
}

/// Response to control messages
#[derive(Debug, Clone)]
pub enum ControlResponse {
    Ok,
    Error(String),
    HealthCheck { status: AgentStatus, uptime_secs: u64 },
}

/// Utility type for agent initialization
pub type AgentFactory = fn() -> anyhow::Result<Box<dyn Agent>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct DummyAgent {
        config: AgentConfig,
        status: AgentStatus,
    }

    #[async_trait]
    impl Agent for DummyAgent {
        fn config(&self) -> &AgentConfig {
            &self.config
        }

        fn status(&self) -> AgentStatus {
            self.status
        }

        async fn process_market(&self, _input: AgentInput) -> anyhow::Result<Option<AgentOutput>> {
            Ok(None)
        }

        async fn handle_control(&self, _msg: ControlMessage) -> anyhow::Result<ControlResponse> {
            Ok(ControlResponse::Ok)
        }

        async fn on_start(&self) -> anyhow::Result<()> {
            Ok(())
        }

        async fn on_stop(&self) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.max_markets_per_batch, 100);
        assert_eq!(config.processing_interval_secs, 60);
        assert!(config.enabled);
    }
}
