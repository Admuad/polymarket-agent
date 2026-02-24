pub mod attribution;
pub mod calibration;
pub mod drift_detection;
pub mod metrics;
pub mod resolution;
pub mod ab_testing;
pub mod shadow_mode;

pub use attribution::{AttributionEngine, PnlAttribution, SignalOutcomeAnalysis};
pub use calibration::{CalibrationEngine, BrierScoreCalculator, BrierDecomposition};
pub use drift_detection::{DriftDetector, DriftDetectionConfig};
pub use metrics::{MetricsCalculator, StrategyComparison};
pub use resolution::{ResolutionMonitor, ResolutionTracker, ResolutionStats};
pub use ab_testing::{AbTestManager, AbTestEngine, AssignmentCounts};
pub use shadow_mode::{ShadowMode, PaperTrader, ShadowPerformance, ShadowRealComparison};

// Re-export from common
pub use common::{PerformanceMetrics, StrategyPerformance};
