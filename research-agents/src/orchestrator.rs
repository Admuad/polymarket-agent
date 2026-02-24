//! Orchestrator - Coordinates multiple specialist agents across markets
//!
//! The Orchestrator manages:
//! - Agent lifecycle (start, stop, pause, resume)
//! - Market distribution to agents
//! - Aggregation of signals from multiple agents
//! - Load balancing and scheduling
//!
//! Designed to handle ~10k markets efficiently.

use super::agent::{Agent, AgentInput, AgentOutput};
use super::bus::AgentBus;
use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock, Semaphore};
use tracing::{debug, error, info};
use uuid::Uuid;
use common::Market;

/// Configuration for the orchestrator
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Maximum number of markets to process concurrently
    pub max_concurrent_markets: usize,
    /// Maximum number of agents running concurrently
    pub max_concurrent_agents: usize,
    /// How often to re-scan markets for updates (seconds)
    pub scan_interval_secs: u64,
    /// Batch size for market distribution to agents
    pub market_batch_size: usize,
    /// Enable signal aggregation from multiple agents
    pub enable_aggregation: bool,
    /// Minimum confidence threshold for signals
    pub min_confidence_threshold: f64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_markets: 100,
            max_concurrent_agents: 10,
            scan_interval_secs: 30,
            market_batch_size: 50,
            enable_aggregation: true,
            min_confidence_threshold: 0.3,
        }
    }
}

/// Signal aggregation result
#[derive(Debug, Clone)]
pub struct AggregatedSignal {
    pub market_id: Uuid,
    pub signals: Vec<AgentOutput>,
    pub aggregated_confidence: f64,
    pub consensus_direction: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Status of the orchestrator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrchestratorStatus {
    Idle,
    Running,
    Paused,
    Error,
}

/// Orchestrator - coordinates multiple agents
pub struct Orchestrator {
    config: OrchestratorConfig,
    bus: Arc<AgentBus>,

    // Registered agents
    agents: DashMap<String, Box<dyn Agent>>,

    // Market cache
    markets: DashMap<Uuid, Arc<Market>>,

    // Signal storage
    signals: DashMap<Uuid, Vec<AgentOutput>>,

    // State
    status: Arc<RwLock<OrchestratorStatus>>,
    shutdown_tx: Option<mpsc::Sender<()>>,

    // Control channel
    control_tx: mpsc::Sender<ControlCommand>,
    control_rx: Arc<RwLock<Option<mpsc::Receiver<ControlCommand>>>>,
}

/// Commands to control the orchestrator
pub enum ControlCommand {
    Start,
    Stop,
    Pause,
    Resume,
    RegisterAgent {
        agent: Box<dyn Agent>,
    },
    UnregisterAgent {
        agent_id: String,
    },
    AddMarkets {
        markets: Vec<Market>,
    },
    GetStatus {
        respond_to: mpsc::Sender<OrchestratorStatus>,
    },
}

impl Orchestrator {
    /// Create a new orchestrator
    pub async fn new(config: OrchestratorConfig, bus: Arc<AgentBus>) -> Result<Self> {
        let (control_tx, control_rx) = mpsc::channel(100);

        Ok(Self {
            config,
            bus,
            agents: DashMap::new(),
            markets: DashMap::new(),
            signals: DashMap::new(),
            status: Arc::new(RwLock::new(OrchestratorStatus::Idle)),
            shutdown_tx: None,
            control_tx,
            control_rx: Arc::new(RwLock::new(Some(control_rx))),
        })
    }

    /// Get a handle to send control commands
    pub fn control_handle(&self) -> OrchestratorHandle {
        OrchestratorHandle {
            tx: self.control_tx.clone(),
        }
    }

    /// Run the orchestrator main loop
    pub async fn run(&mut self) -> Result<()> {
        let mut control_rx = self.control_rx.write().await.take()
            .ok_or_else(|| anyhow::anyhow!("Control receiver already taken"))?;

        info!("Orchestrator started");

        loop {
            tokio::select! {
                // Handle control commands
                Some(cmd) = control_rx.recv() => {
                    if let Err(e) = self.handle_command(cmd).await {
                        error!("Error handling command: {}", e);
                    }
                }

                // Main processing loop
                _ = tokio::time::sleep(Duration::from_secs(self.config.scan_interval_secs)) => {
                    if *self.status.read().await == OrchestratorStatus::Running {
                        if let Err(e) = self.process_markets().await {
                            error!("Error processing markets: {}", e);
                        }
                    }
                }

                // Shutdown signal
                else => break,
            }
        }

        info!("Orchestrator stopped");
        Ok(())
    }

    /// Handle a control command
    async fn handle_command(&self, cmd: ControlCommand) -> Result<()> {
        match cmd {
            ControlCommand::Start => {
                *self.status.write().await = OrchestratorStatus::Running;
                info!("Orchestrator started");
            }

            ControlCommand::Stop => {
                *self.status.write().await = OrchestratorStatus::Idle;
                info!("Orchestrator stopped");
            }

            ControlCommand::Pause => {
                *self.status.write().await = OrchestratorStatus::Paused;
                info!("Orchestrator paused");
            }

            ControlCommand::Resume => {
                *self.status.write().await = OrchestratorStatus::Running;
                info!("Orchestrator resumed");
            }

            ControlCommand::RegisterAgent { agent } => {
                let agent_id = agent.config().agent_id.clone();
                info!("Registering agent: {}", agent_id);

                // Start the agent
                if let Err(e) = agent.on_start().await {
                    error!("Failed to start agent {}: {}", agent_id, e);
                }

                self.agents.insert(agent_id.clone(), agent);
            }

            ControlCommand::UnregisterAgent { agent_id } => {
                info!("Unregistering agent: {}", agent_id);

                if let Some((_, agent)) = self.agents.remove(&agent_id) {
                    if let Err(e) = agent.on_stop().await {
                        error!("Failed to stop agent {}: {}", agent_id, e);
                    }
                }
            }

            ControlCommand::AddMarkets { markets } => {
                let count = markets.len();
                for market in markets {
                    let market_id = market.id;
                    self.markets.insert(market_id, Arc::new(market));
                }
                info!("Added {} markets, total: {}", count, self.markets.len());
            }

            ControlCommand::GetStatus { respond_to } => {
                let status = *self.status.read().await;
                let _ = respond_to.send(status).await;
            }
        }

        Ok(())
    }

    /// Process markets through all registered agents
    async fn process_markets(&self) -> Result<()> {
        let agent_ids: Vec<String> = self.agents.iter()
            .map(|entry| entry.key().clone())
            .collect();

        if agent_ids.is_empty() {
            debug!("No agents registered, skipping market processing");
            return Ok(());
        }

        let market_ids: Vec<Uuid> = self.markets.iter()
            .map(|entry| *entry.key())
            .collect();

        if market_ids.is_empty() {
            debug!("No markets to process");
            return Ok(());
        }

        debug!("Processing {} markets with {} agents", market_ids.len(), agent_ids.len());

        // Limit concurrent processing
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_markets));
        let mut tasks = Vec::new();

        for market_id in market_ids {
            let semaphore = semaphore.clone();
            let markets = self.markets.clone();
            let _config = self.config.clone();
            let _bus = self.bus.clone();
            let task_agent_ids = agent_ids.clone();

            let task = tokio::spawn(async move {
                let _permit = semaphore.acquire().await;

                let market_opt = markets.get(&market_id);
                if market_opt.is_none() {
                    return;
                }

                let market = market_opt.unwrap().value().clone();

                let _input = AgentInput {
                    market,
                    timestamp: Utc::now(),
                    additional_data: None,
                };

                // Note: In a full implementation, we would need to access the agents
                // This is a simplified version that demonstrates the architecture
                debug!("Would process market {} with agents: {:?}", market_id, task_agent_ids);
            });

            tasks.push(task);
        }

        // Wait for all tasks to complete
        for task in tasks {
            let _ = task.await;
        }

        Ok(())
    }

    /// Get aggregated signals for a market
    pub fn get_signals(&self, market_id: Uuid) -> Option<Vec<AgentOutput>> {
        self.signals.get(&market_id).map(|v| v.clone())
    }

    /// Get all market IDs
    pub fn market_ids(&self) -> Vec<Uuid> {
        self.markets.iter().map(|entry| *entry.key()).collect()
    }

    /// Get status
    pub async fn status(&self) -> OrchestratorStatus {
        *self.status.read().await
    }
}

/// Handle for controlling the orchestrator
pub struct OrchestratorHandle {
    tx: mpsc::Sender<ControlCommand>,
}

impl OrchestratorHandle {
    /// Start the orchestrator
    pub async fn start(&self) -> Result<()> {
        self.tx.send(ControlCommand::Start).await
            .map_err(|e| anyhow::anyhow!("Failed to send start command: {}", e))
    }

    /// Stop the orchestrator
    pub async fn stop(&self) -> Result<()> {
        self.tx.send(ControlCommand::Stop).await
            .map_err(|e| anyhow::anyhow!("Failed to send stop command: {}", e))
    }

    /// Pause the orchestrator
    pub async fn pause(&self) -> Result<()> {
        self.tx.send(ControlCommand::Pause).await
            .map_err(|e| anyhow::anyhow!("Failed to send pause command: {}", e))
    }

    /// Resume the orchestrator
    pub async fn resume(&self) -> Result<()> {
        self.tx.send(ControlCommand::Resume).await
            .map_err(|e| anyhow::anyhow!("Failed to send resume command: {}", e))
    }

    /// Register a new agent
    pub async fn register_agent(&self, agent: Box<dyn Agent>) -> Result<()> {
        self.tx.send(ControlCommand::RegisterAgent { agent }).await
            .map_err(|e| anyhow::anyhow!("Failed to register agent: {}", e))
    }

    /// Unregister an agent
    pub async fn unregister_agent(&self, agent_id: String) -> Result<()> {
        self.tx.send(ControlCommand::UnregisterAgent { agent_id }).await
            .map_err(|e| anyhow::anyhow!("Failed to unregister agent: {}", e))
    }

    /// Add markets to process
    pub async fn add_markets(&self, markets: Vec<Market>) -> Result<()> {
        self.tx.send(ControlCommand::AddMarkets { markets }).await
            .map_err(|e| anyhow::anyhow!("Failed to add markets: {}", e))
    }

    /// Get the current status
    pub async fn get_status(&self) -> Result<OrchestratorStatus> {
        let (tx, mut rx) = mpsc::channel(1);
        self.tx.send(ControlCommand::GetStatus { respond_to: tx }).await
            .map_err(|e| anyhow::anyhow!("Failed to get status: {}", e))?;

        rx.recv().await
            .ok_or_else(|| anyhow::anyhow!("Status response channel closed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let bus = Arc::new(AgentBus::new(
            crate::bus::AgentBusConfig::default()
        ).await.unwrap());
        let orchestrator = Orchestrator::new(
            OrchestratorConfig::default(),
            bus
        ).await.unwrap();
        assert_eq!(orchestrator.status().await, OrchestratorStatus::Idle);
    }
}
