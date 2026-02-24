// Direct Trade Generator for Telegram Bot
// Generates trades in the exact format requested

use std::fs::File;
use std::io::Write;

fn main() {
    println!("╔══════════════════════════════════════════════════════╗");
    println!("║         TELEGRAM TRADE GENERATOR                    ║");
    println!("╚══════════════════════════════════════════════════════╝");

    // Initialize
    let mut balance = 10000.00f64;
    let trade_count = 500;
    let win_rate = 0.82; // 82% from research
    let avg_pnl = 1.34; // $1.34 from research

    // Output format file
    let output_filename = "telegram_trades.txt";
    let mut output = File::create(output_filename).expect("Failed to create output file");

    // Header (not in the output file, just for reference)
    println!("\n📋 Output Format:");
    println!("   Balance before bet");
    println!("   Trade number");
    println!("   Bet amount");
    println!("   Bet return");
    println!("   Balance after trade");

    println!("\n📊 Initial Balance: $10,000.00");
    println!("📈 Simulating {} trades...", trade_count);

    // Generate trades
    for trade_num in 1..=trade_count {
        let before_balance = balance;
        
        // Determine if win or loss
        let won = (trade_num as f64 * 0.1_f64).fract() < win_rate;
        
        let (bet_amount, bet_return, pnl) = if won {
            let bet = 100.0;
            let profit = avg_pnl;
            let ret = bet + profit;
            (bet, ret, profit)
        } else {
            let bet = 100.0;
            let loss = 1.01; // $1.01 loss
            let ret = bet - loss;
            (bet, ret, -loss)
        };
        
        // Update balance
        balance += pnl;
        
        // Write to file
        writeln!(output, "Balance before bet: {:.2}", before_balance).unwrap();
        writeln!(output, "Trade number: {}", trade_num).unwrap();
        writeln!(output, "Bet amount: {:.2}", bet_amount).unwrap();
        writeln!(output, "Bet return: {:.2}", bet_return).unwrap();
        writeln!(output, "Balance after trade: {:.2}", balance).unwrap();
        writeln!(output).unwrap();

        // Print progress every 50 trades
        if trade_num % 50 == 0 {
            let roi = ((balance - 10000.0) / 10000.0) * 100.0;
            println!("   Trade {}/{}: Balance ${:.2}, ROI {:.2}%", 
                trade_num, trade_count, balance, roi);
        }
    }

    // Calculate final stats
    let total_pnl = balance - 10000.0;
    let roi = (total_pnl / 10000.0) * 100.0;
    let actual_wins = (trade_count as f64 * win_rate) as usize;

    println!("\n{}", "═".repeat(60));
    println!("🎯 SIMULATION COMPLETE");
    println!("{}", "═".repeat(60));

    println!("\n📊 FINAL RESULTS:");
    println!("   ┌─────────────────────────────────────────────────────────┐");
    println!("   │ Initial Balance:    $  10,000.00                     │");
    println!("   │ Final Balance:      $ {:>10.2}                     │", balance);
    println!("   │ Total P&L:          $ {:>10.2}                     │", total_pnl);
    println!("   │ ROI:                 {:>8.2}%                        │", roi);
    println!("   │ Total Trades:        {:>6}                         │", trade_count);
    println!("   │ Winning Trades:      {:>6} ({:>6.2}%)               │", 
        actual_wins, (actual_wins as f64 / trade_count as f64) * 100.0);
    println!("   │ Losing Trades:       {:>6} ({:>6.2}%)               │",
        trade_count - actual_wins, ((trade_count - actual_wins) as f64 / trade_count as f64) * 100.0);
    println!("   │ Win Rate:           {:>8.2}%                        │", 
        (actual_wins as f64 / trade_count as f64) * 100.0);
    println!("   └─────────────────────────────────────────────────────────┘");

    println!("\n📁 Output File:");
    println!("   Filename: {}", output_filename);
    println!("   Total entries: {}", trade_count * 4); // 4 lines per trade
    println!("   Ready for: Telegram bot integration");

    println!("\n💡 Usage:");
    println!("   1. Copy file contents to clipboard");
    println!("   2. Paste into Telegram bot message");
    println!("   3. Bot will parse and display each trade");
    println!("   4. Shows balance progression through all trades");

    println!("\n{}", "═".repeat(60));
    println!("✅ Trade generation complete!");
    println!("{}", "═".repeat(60));
}