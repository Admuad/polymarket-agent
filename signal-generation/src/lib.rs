// Signal Generation Framework (Layer 2)
// Processes research outputs and market data to generate trade signals

pub mod pipeline;
pub mod signals;
pub mod validators;
pub mod storage;

pub use pipeline::{PipelineConfig, SignalPipeline};
pub use signals::{
    TradeSignal, SignalInput, ResearchOutput, SignalGenerator, SignalType, SignalDirection,
    SpreadArbitrageGenerator, SignalMetadata, PriceSnapshot, OrderBookSnapshot, Level, SentimentScore, SentimentSource
};
pub use validators::{SignalValidator, EdgeThresholdValidator, EdgeThresholdConfig, ConfidenceValidator, ConfidenceValidatorConfig, LiquidityValidator, LiquidityValidatorConfig, ExpectedValueValidator, ExpectedValueValidatorConfig, CompositeValidator};
pub use storage::{SignalStorage, InMemoryStorage, StorageStats, ExecutionStorage, InMemoryExecutionStorage, SignalExecutionResult, ExitReason, BacktestStats, SignalTypeStats};
