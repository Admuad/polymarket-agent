//! Research Agents Framework - Layer 1
//!
//! This crate provides a modular agent architecture for analyzing Polymarket data.
//! It includes:
//! - Base Agent trait for implementing specialist agents
//! - Orchestrator for coordinating multiple agents across thousands of markets
//! - Communication bus for agent-to-agent messaging
//! - Specialist agent implementations (Sentiment, etc.)

pub mod agent;
pub mod orchestrator;
pub mod bus;
pub mod sentiment;

// Re-export commonly used types
pub use agent::{Agent, AgentConfig, AgentInput, AgentOutput, AgentStatus};
pub use orchestrator::{Orchestrator, OrchestratorConfig};
pub use bus::{AgentBus, AgentBusConfig, AgentBusHandle, AgentMessage, MessagePriority};
pub use sentiment::{SentimentAgent, SentimentAgentConfig, SentimentSignal, SentimentScore};

// Re-export common types for convenience
pub use common::{Market, MarketEvent, PriceTick};
