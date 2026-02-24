// Real-time Monitoring and Optimization System
// Tracks paper trading performance and provides optimization suggestions

use chrono::{DateTime, Utc, Duration};
use std::time::Instant;
use std::collections::HashMap;

/// Monitoring configuration
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    pub update_interval_secs: u64,
    pub alert_thresholds: AlertThresholds,
    pub optimization_check_interval: u64,
}

#[derive(Debug, Clone)]
pub struct AlertThresholds {
    pub max_drawdown_warning: f64, // Warning at X% drawdown
    pub max_drawdown_critical: f64, // Critical at Y% drawdown
    pub roi_warning_low: f64, // Warning if ROI below X%
    pub roi_target: f64, // Target ROI
    pub win_rate_warning_low: f64, // Warning if win rate below X%
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        MonitoringConfig {
            update_interval_secs: 60, // Every minute
            alert_thresholds: AlertThresholds {
                max_drawdown_warning: 0.05, // 5%
                max_drawdown_critical: 0.10, // 10%
                roi_warning_low: 0.0, // Warning at 0% or negative
                roi_target: 0.05, // Target 5% ROI
                win_rate_warning_low: 0.70, // Warning below 70%
            },
            optimization_check_interval: 3600, // Every hour
        }
    }
}

/// Real-time performance metrics
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub timestamp: DateTime<Utc>,
    pub equity: f64,
    pub equity_change: f64,
    pub roi: f64,
    pub win_rate: f64,
    pub drawdown: f64,
    pub open_positions: usize,
    pub total_trades: usize,
}

/// Alert level
#[derive(Debug, Clone, PartialEq)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

/// Alert
#[derive(Debug, Clone)]
pub struct Alert {
    pub timestamp: DateTime<Utc>,
    pub level: AlertLevel,
    pub message: String,
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
    pub action_suggestion: String,
}

/// Optimization suggestion
#[derive(Debug, Clone)]
pub struct OptimizationSuggestion {
    pub timestamp: DateTime<Utc>,
    pub category: OptimizationCategory,
    pub priority: OptimizationPriority,
    pub title: String,
    pub description: String,
    pub expected_impact: String,
    pub implementation_effort: ImplementationEffort,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptimizationCategory {
    StrategyAdjustment,
    RiskManagement,
    PositionSizing,
    NewStrategy,
    Infrastructure,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptimizationPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImplementationEffort {
    Trivial,
    Easy,
    Medium,
    Hard,
}

/// Monitoring engine
pub struct MonitoringEngine {
    config: MonitoringConfig,
    start_time: DateTime<Utc>,
    metrics_history: Vec<PerformanceMetrics>,
    alerts: Vec<Alert>,
    optimizations: Vec<OptimizationSuggestion>,
}

impl MonitoringEngine {
    pub fn new(config: MonitoringConfig) -> Self {
        MonitoringEngine {
            config,
            start_time: Utc::now(),
            metrics_history: Vec::new(),
            alerts: Vec::new(),
            optimizations: Vec::new(),
        }
    }

    pub fn add_metric(&mut self, metric: PerformanceMetrics) {
        self.metrics_history.push(metric.clone());
        
        // Check for alerts
        self.check_alerts(&metric);
        
        // Check for optimizations
        self.check_optimizations();
    }

    fn check_alerts(&mut self, metric: &PerformanceMetrics) {
        // Check drawdown
        if metric.drawdown >= self.config.alert_thresholds.max_drawdown_critical {
            self.add_alert(Alert {
                timestamp: Utc::now(),
                level: AlertLevel::Critical,
                message: format!("Critical drawdown of {:.2}% detected!", metric.drawdown * 100.0),
                metric: "Max Drawdown".to_string(),
                value: metric.drawdown,
                threshold: self.config.alert_thresholds.max_drawdown_critical,
                action_suggestion: "IMMEDIATE: Stop all trading, review risk parameters".to_string(),
            });
        } else if metric.drawdown >= self.config.alert_thresholds.max_drawdown_warning {
            self.add_alert(Alert {
                timestamp: Utc::now(),
                level: AlertLevel::Warning,
                message: format!("Warning: Drawdown at {:.2}%", metric.drawdown * 100.0),
                metric: "Max Drawdown".to_string(),
                value: metric.drawdown,
                threshold: self.config.alert_thresholds.max_drawdown_warning,
                action_suggestion: "Reduce position sizes, check inventory imbalance".to_string(),
            });
        }
        
        // Check ROI
        if metric.roi < self.config.alert_thresholds.roi_warning_low {
            self.add_alert(Alert {
                timestamp: Utc::now(),
                level: AlertLevel::Warning,
                message: format!("Warning: ROI at {:.2}%", metric.roi),
                metric: "ROI".to_string(),
                value: metric.roi,
                threshold: self.config.alert_thresholds.roi_warning_low,
                action_suggestion: "Review strategy performance, consider parameter adjustment".to_string(),
            });
        }
        
        // Check win rate
        if metric.win_rate < self.config.alert_thresholds.win_rate_warning_low {
            self.add_alert(Alert {
                timestamp: Utc::now(),
                level: AlertLevel::Warning,
                message: format!("Warning: Win rate at {:.2}%", metric.win_rate * 100.0),
                metric: "Win Rate".to_string(),
                value: metric.win_rate,
                threshold: self.config.alert_thresholds.win_rate_warning_low,
                action_suggestion: "Review signal quality, tighten entry criteria".to_string(),
            });
        }
    }

    fn check_optimizations(&mut self) {
        if self.metrics_history.len() < 10 {
            return; // Need at least 10 data points
        }
        
        let recent = &self.metrics_history[self.metrics_history.len() - 10..];
        let current = self.metrics_history.last().unwrap();
        
        // Calculate recent averages
        let avg_roi: f64 = recent.iter().map(|m| m.roi).sum::<f64>() / 10.0;
        let avg_win_rate: f64 = recent.iter().map(|m| m.win_rate).sum::<f64>() / 10.0;
        let avg_drawdown: f64 = recent.iter().map(|m| m.drawdown).sum::<f64>() / 10.0;
        
        // Check for optimization opportunities
        
        // 1. ROI below target
        if current.roi < self.config.alert_thresholds.roi_target {
            self.add_optimization(OptimizationSuggestion {
                timestamp: Utc::now(),
                category: OptimizationCategory::StrategyAdjustment,
                priority: OptimizationPriority::High,
                title: "Low ROI Detected".to_string(),
                description: format!("Current ROI {:.2}% is below target {:.2}%", 
                    current.roi * 100.0, self.config.alert_thresholds.roi_target * 100.0),
                expected_impact: format!("Potential improvement: +{} to +{} pp", 
                    (self.config.alert_thresholds.roi_target - current.roi) * 100.0,
                    (self.config.alert_thresholds.roi_target - current.roi) * 200.0),
                implementation_effort: ImplementationEffort::Easy,
            });
        }
        
        // 2. Win rate declining
        if current.win_rate < avg_win_rate - 0.10 {
            self.add_optimization(OptimizationSuggestion {
                timestamp: Utc::now(),
                category: OptimizationCategory::StrategyAdjustment,
                priority: OptimizationPriority::Medium,
                title: "Declining Win Rate".to_string(),
                description: format!("Win rate dropped from {:.2}% to {:.2}%",
                    avg_win_rate * 100.0, current.win_rate * 100.0),
                expected_impact: "Could recover 5-10% of lost performance".to_string(),
                implementation_effort: ImplementationEffort::Easy,
            });
        }
        
        // 3. High drawdown
        if current.drawdown > 0.08 {
            self.add_optimization(OptimizationSuggestion {
                timestamp: Utc::now(),
                category: OptimizationCategory::RiskManagement,
                priority: OptimizationPriority::Critical,
                title: "High Drawdown Detected".to_string(),
                description: format!("Drawdown at {:.2}% exceeds acceptable range", current.drawdown * 100.0),
                expected_impact: "Protect capital from further losses".to_string(),
                implementation_effort: ImplementationEffort::Trivial,
            });
        }
        
        // 4. Low win rate
        if current.win_rate < 0.65 {
            self.add_optimization(OptimizationSuggestion {
                timestamp: Utc::now(),
                category: OptimizationCategory::NewStrategy,
                priority: OptimizationPriority::High,
                title: "Low Win Rate".to_string(),
                description: format!("Win rate of {:.2}% is below acceptable threshold", current.win_rate * 100.0),
                expected_impact: "Adding correlation arbitrage could improve to 70-80% win rate".to_string(),
                implementation_effort: ImplementationEffort::Medium,
            });
        }
        
        // 5. Consistent performance (good)
        if current.roi > 5.0 && avg_roi > 4.5 && current.drawdown < 0.05 {
            self.add_optimization(OptimizationSuggestion {
                timestamp: Utc::now(),
                category: OptimizationCategory::PositionSizing,
                priority: OptimizationPriority::Medium,
                title: "Optimal Performance Detected".to_string(),
                description: "System performing within optimal parameters".to_string(),
                expected_impact: "Consider increasing position sizes to maximize returns".to_string(),
                implementation_effort: ImplementationEffort::Easy,
            });
        }
    }

    fn add_alert(&mut self, alert: Alert) {
        // Check if we already have a similar recent alert
        let recent_alerts = self.alerts.iter()
            .filter(|a| a.timestamp > Utc::now() - Duration::minutes(5))
            .collect::<Vec<_>>();
        
        let is_duplicate = recent_alerts.iter().any(|a| a.message == alert.message);
        
        if !is_duplicate {
            self.alerts.push(alert.clone());
            
            match alert.level {
                AlertLevel::Critical => {
                    println!("\n🚨 CRITICAL ALERT:");
                    println!("   {}", alert.message);
                    println!("   Action: {}", alert.action_suggestion);
                }
                AlertLevel::Warning => {
                    println!("\n⚠️  WARNING:");
                    println!("   {}", alert.message);
                    println!("   Action: {}", alert.action_suggestion);
                }
                AlertLevel::Info => {
                    println!("\nℹ️  INFO:");
                    println!("   {}", alert.message);
                }
            }
        }
    }

    fn add_optimization(&mut self, opt: OptimizationSuggestion) {
        // Check if we already have a similar recent optimization
        let recent_opts = self.optimizations.iter()
            .filter(|o| o.timestamp > Utc::now() - Duration::hours(1))
            .collect::<Vec<_>>();
        
        let is_duplicate = recent_opts.iter().any(|o| o.title == opt.title);
        
        if !is_duplicate {
            self.optimizations.push(opt.clone());
            
            let priority_icon = match opt.priority {
                OptimizationPriority::Critical => "🔴",
                OptimizationPriority::High => "🟠",
                OptimizationPriority::Medium => "🟡",
                OptimizationPriority::Low => "🟢",
            };
            
            let effort_icon = match opt.implementation_effort {
                ImplementationEffort::Trivial => "⚡",
                ImplementationEffort::Easy => "✓",
                ImplementationEffort::Medium => "◉",
                ImplementationEffort::Hard => "⏳",
            };
            
            println!("\n{} OPTIMIZATION SUGGESTION [{}] [{}]",
                priority_icon, effort_icon);
            println!("   📌 {}", opt.title);
            println!("   📝 {}", opt.description);
            println!("   💰 Impact: {}", opt.expected_impact);
            println!("   📊 Category: {:?}", opt.category);
        }
    }

    pub fn generate_summary(&self) -> String {
        let current = self.metrics_history.last();
        
        if current.is_none() {
            return "No metrics available yet".to_string();
        }
        
        let current = current.unwrap();
        let elapsed = (Utc::now() - self.start_time).num_hours();
        
        let mut summary = String::new();
        summary.push_str("╔══════════════════════════════════════════════════════╗\n");
        summary.push_str("║              MONITORING SUMMARY                     ║\n");
        summary.push_str("╚════════════════════════════════════════════════════════╝\n");
        
        summary.push_str(&format!("\n⏰  Session Info:\n"));
        summary.push_str(&format!("   Elapsed:      {} hours\n", elapsed));
        summary.push_str(&format!("   Start Time:   {}\n", self.start_time.format("%Y-%m-%d %H:%M:%S")));
        summary.push_str(&format!("   Current Time: {}\n", Utc::now().format("%Y-%m-%d %H:%M:%S")));
        
        if let Some(current) = current {
            summary.push_str(&format!("\n📊 Current Metrics:\n"));
            summary.push_str(&format!("   Equity:        ${:.2}\n", current.equity));
            summary.push_str(&format!("   ROI:            {:.2}%\n", current.roi * 100.0));
            summary.push_str(&format!("   Win Rate:       {:.2}%\n", current.win_rate * 100.0));
            summary.push_str(&format!("   Drawdown:       {:.2}%\n", current.drawdown * 100.0));
            summary.push_str(&format!("   Open Positions:  {}\n", current.open_positions));
            summary.push_str(&format!("   Total Trades:    {}\n", current.total_trades));
        }
        
        summary.push_str(&format!("\n🚨 Alerts: {} ({} critical)\n",
            self.alerts.len(), self.alerts.iter().filter(|a| a.level == AlertLevel::Critical).count()));
        
        for alert in self.alerts.iter().take(5) {
            let icon = match alert.level {
                AlertLevel::Critical => "🔴",
                AlertLevel::Warning => "🟠",
                AlertLevel::Info => "🔵",
            };
            summary.push_str(&format!("   {} {}\n", icon, alert.message));
        }
        
        if self.alerts.len() > 5 {
            summary.push_str(&format!("   ... and {} more alerts\n", self.alerts.len() - 5));
        }
        
        summary.push_str(&format!("\n💡 Optimizations: {} suggestions\n", self.optimizations.len()));
        
        for opt in self.optimizations.iter().take(5) {
            let priority = match opt.priority {
                OptimizationPriority::Critical => "CRITICAL",
                OptimizationPriority::High => "HIGH",
                OptimizationPriority::Medium => "MEDIUM",
                OptimizationPriority::Low => "LOW",
            };
            summary.push_str(&format!("   [{}] {}\n", priority, opt.title));
        }
        
        if self.optimizations.len() > 5 {
            summary.push_str(&format!("   ... and {} more suggestions\n", self.optimizations.len() - 5));
        }
        
        summary.push_str(&format!("\n{}", "═".repeat(60)));
        
        summary
    }
}

/// Standalone monitoring display
pub fn display_live_dashboard(portfolio: &crate::paper_trading::PaperPortfolio, 
                                   monitoring: &MonitoringEngine) {
    let elapsed = (Utc::now() - monitoring.start_time).num_hours();
    let hours = elapsed as u64;
    let days = hours / 24;
    let remaining_hours = hours % 24;
    
    println!("\n{}", "═".repeat(68));
    println!("📊 PAPER TRADING DASHBOARD - LIVE");
    println!("{}", "═".repeat(68));
    
    println!("\n⏰  Session Duration: {} days, {} hours", days, remaining_hours);
    
    println!("\n💰 Portfolio:");
    println!("   ┌─────────────────────────────────────────────────────────┐");
    println!("   │ Initial Capital:    ${:>10.2}                     │", portfolio.initial_capital);
    println!("   │ Current Equity:    ${:>10.2}                     │", portfolio.current_equity);
    println!("   │ Total P&L:         ${:>10.2}                     │", portfolio.total_pnl);
    println!("   │ ROI:                {:>8.2}%                        │", portfolio.roi());
    println!("   │ Peak Equity:       ${:>10.2}                     │", portfolio.peak_equity);
    println!("   │ Max Drawdown:      {:>8.2}%                        │", portfolio.max_drawdown * 100.0);
    println!("   └─────────────────────────────────────────────────────────┘");
    
    println!("\n📈 Statistics:");
    println!("   ┌─────────────────────────────────────────────────────────┐");
    println!("   │ Total Trades:        {:>6}                         │", portfolio.total_trades);
    println!("   │ Winning Trades:      {:>6} ({:>6.2}%)               │", 
        portfolio.winning_trades, portfolio.hit_rate() * 100.0);
    println!("   │ Losing Trades:       {:>6} ({:>6.2}%)               │",
        portfolio.losing_trades, (1.0 - portfolio.hit_rate()) * 100.0);
    println!("   │ Open Positions:      {:>6}                         │", portfolio.open_positions.len());
    println!("   │ Exposure:            ${:>10.2}                     │", portfolio.open_exposure());
    println!("   └─────────────────────────────────────────────────────────┘");
    
    println!("\n🔔 Activity (Last Hour):");
    let recent_trades = portfolio.closed_positions.iter()
        .filter(|t| t.exit_time.map_or(false, |et| et > Utc::now() - Duration::hours(1)))
        .count();
    let recent_wins = portfolio.closed_positions.iter()
        .filter(|t| t.exit_time.map_or(false, |et| et > Utc::now() - Duration::hours(1)))
        .filter(|t| t.pnl.map_or(false, |p| p > 0.0))
        .count();
    
    println!("   Trades in last hour: {}", recent_trades);
    if recent_trades > 0 {
        println!("   Win rate last hour:  {:.1}%", (recent_wins as f64 / recent_trades as f64) * 100.0);
    } else {
        println!("   Win rate last hour:  N/A");
    }
    
    println!("\n{}", "═".repeat(68));
    println!("💡 Press Ctrl+C to stop paper trading");
    println!("{}", "═".repeat(68));
}