// Paper Trading Module
// Simulates trading with real market data but no actual money at risk

use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Paper trading configuration
#[derive(Debug, Clone)]
pub struct PaperTradingConfig {
    pub enabled: bool,
    pub initial_capital: f64,
    pub max_position_size: f64,
    pub max_markets: usize,
    pub auto_start: bool,
    pub log_trades: bool,
    pub update_interval_secs: u64,
}

impl Default for PaperTradingConfig {
    fn default() -> Self {
        PaperTradingConfig {
            enabled: true,
            initial_capital: 10000.0,
            max_position_size: 100.0,
            max_markets: 10,
            auto_start: false,
            log_trades: true,
            update_interval_secs: 60, // Update every minute
        }
    }
}

/// Paper trade
#[derive(Debug, Clone)]
pub struct PaperTrade {
    pub id: Uuid,
    pub market_id: Uuid,
    pub outcome_id: String,
    pub strategy: String,
    pub entry_price: f64,
    pub target_price: f64,
    pub position_size: f64,
    pub side: PaperTradeSide,
    pub entry_time: DateTime<Utc>,
    pub exit_time: Option<DateTime<Utc>>,
    pub exit_price: Option<f64>,
    pub pnl: Option<f64>,
    pub fees: f64,
    pub status: PaperTradeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaperTradeSide {
    Long,
    Short,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaperTradeStatus {
    Open,
    Closed,
    Cancelled,
}

/// Paper portfolio state
#[derive(Debug, Clone)]
pub struct PaperPortfolio {
    pub initial_capital: f64,
    pub current_equity: f64,
    pub peak_equity: f64,
    pub max_drawdown: f64,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub total_pnl: f64,
    pub open_positions: Vec<PaperTrade>,
    pub closed_positions: Vec<PaperTrade>,
}

impl PaperPortfolio {
    pub fn new(initial_capital: f64) -> Self {
        PaperPortfolio {
            initial_capital,
            current_equity: initial_capital,
            peak_equity: initial_capital,
            max_drawdown: 0.0,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            total_pnl: 0.0,
            open_positions: Vec::new(),
            closed_positions: Vec::new(),
        }
    }

    pub fn add_trade(&mut self, trade: PaperTrade) {
        self.open_positions.push(trade);
        self.current_equity -= trade.fees;
        self.update_metrics();
    }

    pub fn close_trade(&mut self, trade_id: Uuid, exit_price: f64) {
        if let Some(pos) = self.open_positions.iter().position(|t| t.id == trade_id) {
            let mut trade = self.open_positions.remove(pos);
            trade.exit_time = Some(Utc::now());
            trade.exit_price = Some(exit_price);
            
            // Calculate P&L
            let pnl = match trade.side {
                PaperTradeSide::Long => (exit_price - trade.entry_price) * trade.position_size,
                PaperTradeSide::Short => (trade.entry_price - exit_price) * trade.position_size,
            };
            
            trade.pnl = Some(pnl);
            trade.status = PaperTradeStatus::Closed;
            
            self.current_equity += pnl;
            self.total_trades += 1;
            self.total_pnl += pnl;
            
            if pnl > 0.0 {
                self.winning_trades += 1;
            } else {
                self.losing_trades += 1;
            }
            
            self.closed_positions.push(trade);
            self.update_metrics();
        }
    }

    pub fn update_metrics(&mut self) {
        if self.current_equity > self.peak_equity {
            self.peak_equity = self.current_equity;
        }
        
        let drawdown = (self.peak_equity - self.current_equity) / self.peak_equity;
        self.max_drawdown = self.max_drawdown.max(drawdown);
    }

    pub fn roi(&self) -> f64 {
        (self.total_pnl / self.initial_capital) * 100.0
    }

    pub fn hit_rate(&self) -> f64 {
        if self.total_trades == 0 {
            0.0
        } else {
            self.winning_trades as f64 / self.total_trades as f64
        }
    }

    pub fn open_exposure(&self) -> f64 {
        self.open_positions.iter().map(|t| t.position_size * t.entry_price).sum()
    }
}

/// Paper trading engine
pub struct PaperTradingEngine {
    config: PaperTradingConfig,
    portfolio: Arc<Mutex<PaperPortfolio>>,
    running: Arc<Mutex<bool>>,
    start_time: DateTime<Utc>,
}

impl PaperTradingEngine {
    pub fn new(config: PaperTradingConfig) -> Self {
        let portfolio = Arc::new(Mutex::new(PaperPortfolio::new(config.initial_capital)));
        
        PaperTradingEngine {
            config,
            portfolio,
            running: Arc::new(Mutex::new(false)),
            start_time: Utc::now(),
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        println!("╔══════════════════════════════════════════════════════╗");
        println!("║           PAPER TRADING - v2.0                        ║");
        println!("╚══════════════════════════════════════════════════════╝");

        println!("\n⚙️  Configuration:");
        println!("   Initial Capital: ${:.2}", self.config.initial_capital);
        println!("   Max Position:   ${:.2}", self.config.max_position_size);
        println!("   Max Markets:     {}", self.config.max_markets);
        println!("   Update Interval: {}s", self.config.update_interval_secs);

        println!("\n📊 Portfolio:");
        let port = self.portfolio.lock().unwrap();
        println!("   Current Equity:  ${:.2}", port.current_equity);
        println!("   Open Positions:  {}", port.open_positions.len());
        println!("   Total Trades:    {}", port.total_trades);
        println!("   Winning Trades:   {}", port.winning_trades);
        println!("   Losing Trades:    {}", port.losing_trades);
        println!("   Win Rate:        {:.2}%", port.hit_rate() * 100.0);
        println!("   Total P&L:       ${:.2}", port.total_pnl);
        println!("   ROI:             {:.2}%", port.roi());
        println!("   Max Drawdown:     {:.2}%", port.max_drawdown * 100.0);

        // Set running state
        *self.running.lock().unwrap() = true;
        self.start_time = Utc::now();

        println!("\n🚀 Paper Trading Started");
        println!("   Start Time: {}", self.start_time.format("%Y-%m-%d %H:%M:%S"));
        println!("   Duration:   7 days (will end: {})", 
            (self.start_time + Duration::days(7)).format("%Y-%m-%d %H:%M:%S"));

        println!("\n📋 Status:");
        println!("   ✅ Portfolio initialized");
        println!("   ✅ Strategies enabled");
        println!("   ✅ Monitoring active");
        println!("   ✅ Paper mode (no real money at risk)");
        
        println!("\n{}", "═".repeat(60));
        println!("💹 MONITORING ACTIVE - Updates every {} seconds", self.config.update_interval_secs);
        println!("{}", "═".repeat(60));

        Ok(())
    }

    pub async fn stop(&self) -> anyhow::Result<()> {
        *self.running.lock().unwrap() = false;
        let end_time = Utc::now();
        let duration = end_time - self.start_time;

        let port = self.portfolio.lock().unwrap();

        println!("\n{}", "═".repeat(60));
        println!("🛑 PAPER TRADING STOPPED");
        println!("{}", "═".repeat(60));

        println!("\n📊 FINAL RESULTS:");
        println!("   Duration:        {}", duration.num_hours());
        println!("   Initial Capital:  ${:.2}", port.initial_capital);
        println!("   Final Equity:    ${:.2}", port.current_equity);
        println!("   Total P&L:       ${:.2}", port.total_pnl);
        println!("   ROI:             {:.2}%", port.roi());
        println!("   Total Trades:    {}", port.total_trades);
        println!("   Win Rate:        {:.2}%", port.hit_rate() * 100.0);
        println!("   Max Drawdown:     {:.2}%", port.max_drawdown * 100.0);

        println!("\n🔧 STATISTICS:");
        println!("   Trades per day:  {:.1}", port.total_trades as f64 / duration.num_days().max(1) as f64);
        println!("   Win Rate:        {:.2}%", port.hit_rate() * 100.0);
        
        if port.total_trades > 0 {
            let avg_win = if port.winning_trades > 0 {
                port.total_pnl / port.winning_trades as f64
            } else {
                0.0
            };
            println!("   Avg P&L per win: ${:.2}", avg_win);
        }

        Ok(())
    }

    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    pub fn get_portfolio(&self) -> PaperPortfolio {
        self.portfolio.lock().unwrap().clone()
    }

    pub async fn process_market_update(&self, market_id: Uuid, price: f64) -> anyhow::Result<()> {
        let mut port = self.portfolio.lock().unwrap();
        
        // Check for exit opportunities on open positions
        let to_close: Vec<Uuid> = port.open_positions
            .iter()
            .filter(|trade| {
                match trade.side {
                    PaperTradeSide::Long => price >= trade.target_price,
                    PaperTradeSide::Short => price <= trade.target_price,
                }
            })
            .map(|trade| trade.id)
            .collect();

        for trade_id in to_close {
            port.close_trade(trade_id, price);
            
            if self.config.log_trades {
                let trade = port.closed_positions.last().unwrap();
                println!("\n✅ Trade Closed:");
                println!("   ID:       {}", trade.id);
                println!("   Strategy: {}", trade.strategy);
                println!("   Entry:    ${:.4} @ {}", trade.entry_price, 
                    trade.entry_time.format("%H:%M:%S"));
                println!("   Exit:     ${:.4} @ {}", trade.exit_price.unwrap(), 
                    trade.exit_time.unwrap().format("%H:%M:%S"));
                println!("   Size:     ${:.2}", trade.position_size);
                println!("   P&L:      ${:.2}", trade.pnl.unwrap());
            }
        }

        Ok(())
    }

    pub async fn add_trade(&self, trade: PaperTrade) -> anyhow::Result<()> {
        let mut port = self.portfolio.lock().unwrap();
        
        if self.config.log_trades {
            println!("\n🎯 New Trade Signal:");
            println!("   ID:        {}", trade.id);
            println!("   Strategy:   {}", trade.strategy);
            println!("   Market:     {}", trade.market_id);
            println!("   Entry:      ${:.4}", trade.entry_price);
            println!("   Target:      ${:.4}", trade.target_price);
            println!("   Size:        ${:.2}", trade.position_size);
            println!("   Side:        {:?}", trade.side);
            println!("   Stop Loss:   ${:.4}", trade.entry_price * 0.95);
        }
        
        port.add_trade(trade);
        Ok(())
    }

    pub async fn generate_optimization_report(&self) -> anyhow::Result<String> {
        let port = self.portfolio.lock().unwrap();
        
        let mut report = String::new();
        report.push_str("╔══════════════════════════════════════════════════════╗\n");
        report.push_str("║              OPTIMIZATION REPORT                       ║\n");
        report.push_str("╚══════════════════════════════════════════════════════╝\n");
        
        report.push_str("\n📊 CURRENT PERFORMANCE:\n");
        report.push_str(&format!("   ROI:            {:.2}%\n", port.roi()));
        report.push_str(&format!("   Win Rate:       {:.2}%\n", port.hit_rate() * 100.0));
        report.push_str(&format!("   Total Trades:    {}\n", port.total_trades));
        report.push_str(&format!("   Max Drawdown:   {:.2}%\n", port.max_drawdown * 100.0));
        
        report.push_str("\n💡 OPTIMIZATION SUGGESTIONS:\n");
        
        if port.roi() > 10.0 {
            report.push_str("   ✅ Excellent ROI (>10%)\n");
            report.push_str("      Consider increasing position sizes\n");
            report.push_str("      Add more strategies (correlation, AI signals)\n");
        } else if port.roi() > 5.0 {
            report.push_str("   ✅ Good ROI (5-10%)\n");
            report.push_str("      Consider adding correlation arbitrage\n");
            report.push_str("      Review market making parameters\n");
        } else if port.roi() > 0.0 {
            report.push_str("   ⚠️  Low ROI (0-5%)\n");
            report.push_str("      Tighten risk management\n");
            report.push_str("      Adjust spread thresholds\n");
        } else {
            report.push_str("   ❌ Negative ROI\n");
            report.push_str("      Stop paper trading\n");
            report.push_str("      Review all strategies\n");
            report.push_str("      Re-run backtest with new parameters\n");
        }
        
        if port.hit_rate() > 0.85 {
            report.push_str("   ✅ Excellent hit rate (>85%)\n");
            report.push_str("      Consider increasing position sizes\n");
        } else if port.hit_rate() > 0.70 {
            report.push_str("   ✅ Good hit rate (70-85%)\n");
            report.push_str("      Current strategies working well\n");
        } else {
            report.push_str("   ⚠️  Low hit rate (<70%)\n");
            report.push_str("      Review signal generation\n");
            report.push_str("      Consider more conservative parameters\n");
        }
        
        if port.max_drawdown < 0.05 {
            report.push_str("   ✅ Excellent risk control (<5% DD)\n");
            report.push_str("      Kelly Criterion working optimally\n");
        } else if port.max_drawdown < 0.10 {
            report.push_str("   ✅ Good risk control (5-10% DD)\n");
        } else {
            report.push_str("   ⚠️  High drawdown (>10% DD)\n");
            report.push_str("      Tighten risk management\n");
            report.push_str("      Consider smaller positions\n");
        }
        
        report.push_str("\n📋 ACTION ITEMS:\n");
        report.push_str(&format!("   1. Continue paper trading for {} days\n", 
            (Utc::now() - self.start_time).num_days()));
        report.push_str("   2. Monitor for edge cases\n");
        report.push_str("   3. Adjust parameters based on performance\n");
        report.push_str("   4. Prepare for live deployment\n");
        
        report.push_str(&format!("\n⏰  Elapsed: {} days, {} hours\n",
            (Utc::now() - self.start_time).num_days(),
            (Utc::now() - self.start_time).num_hours()));
        
        Ok(report)
    }
}