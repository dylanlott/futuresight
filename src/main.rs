mod config;
mod data;
mod ui;

use crate::data::Config;
use data::MetricsCollector;
use ui::Dashboard;

use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io::{Stdout, stdout},
    time::Duration,
};
use tokio::time;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

type CrosstermTerminal = Terminal<CrosstermBackend<Stdout>>;

#[derive(Parser, Debug)]
#[command(
    name = env!("CARGO_PKG_NAME"),
    version = env!("CARGO_PKG_VERSION"),
    about = "FutureSight is a terminal dashboard showing Ethereum RPC metrics."
)]
struct Cli {
    /// Ethereum JSON-RPC endpoint
    #[arg(default_value = "http://rpc.pecorino.signet.sh", env = "RPC_URL")]
    rpc_url: String,

    /// Seconds before block delay alert is displayed
    #[arg(default_value_t = crate::config::BLOCK_DELAY_DEFAULT, env = "BLOCK_DELAY_SECS")]
    block_delay_secs: u64,

    #[arg(long, short, default_value_t = 5u64, env = "REFRESH_INTERVAL")]
    refresh_interval: u64,

    /// Base URL for tx-pool-webservice (example: http://localhost:8080)
    #[arg(long, env = "TXPOOL_URL")]
    txpool_url: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help(&args[0]);
        return Ok(());
    }

    println!(
        "FutureSight {} - Signet terminal dashboard",
        env!("CARGO_PKG_VERSION")
    );
    println!("=== Connecting to RPC URL: {} ===", cli.rpc_url);
    if let Some(url) = &cli.txpool_url {
        println!("=== Monitoring tx-pool-webservice: {} ===", url);
    }
    println!("Press 'q' to quit. Use --help for options.");

    let mut terminal = setup_terminal()?;
    let mut dashboard = Dashboard::new();

    // create a metrics collector with the given configs
    let mut collector = MetricsCollector::new_with_txpool(Config {
        rpc_url: cli.rpc_url.clone(),
        block_delay_threshold: cli.block_delay_secs,
    }, cli.txpool_url.clone());

    // collect metrics at startup to prime the dashboard
    collector.collect_metrics().await;

    let mut last_update = std::time::Instant::now();

    // Loop every
    loop {
        if last_update.elapsed() >= Duration::from_secs(cli.refresh_interval) {
            collector.collect_metrics().await;
            last_update = std::time::Instant::now();
        }

        // Update staleness if no successful updates for threshold.
        collector.check_staleness();

        let metrics = collector.get_metrics();
        terminal.draw(|frame| dashboard.render(frame, metrics))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        dashboard.quit();
                        break;
                    }
                    _ => {}
                }
            }
        }

        if dashboard.should_quit {
            break;
        }

        time::sleep(Duration::from_millis(100)).await;
    }

    cleanup_terminal(&mut terminal)?;
    println!("Goodbye!");
    Ok(())
}

fn setup_terminal() -> Result<CrosstermTerminal> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn cleanup_terminal(terminal: &mut CrosstermTerminal) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn print_help(program: &str) {
    let version = env!("CARGO_PKG_VERSION");
    println!("{} {}\n", env!("CARGO_PKG_NAME"), version);
    println!("Usage:");
    println!("  {program} [RPC_URL] [BLOCK_DELAY_SECS]");
    println!("  {program} --help");
    println!("  {program} --version\n");
    println!("Args:");
    println!("  RPC_URL              Ethereum JSON-RPC endpoint (default: http://localhost:8545)");
    println!(
        "  BLOCK_DELAY_SECS     Seconds before block delay alert (default: 60 or env BLOCK_DELAY_SECS)\n"
    );
    println!("Environment:");
    println!(
        "  BLOCK_DELAY_SECS     Override block delay alert threshold when second arg omitted\n"
    );
    println!("  TXPOOL_URL           Optional tx-pool-webservice base URL for cache metrics (e.g. http://localhost:8080)\n");
    println!("Flags:");
    println!("  -h, --help           Show this help and exit");
    println!("  -V, --version        Show version information and exit\n");
    println!("Description:");
    println!(
        "  FutureSight is a terminal dashboard showing Ethereum RPC metrics: connection status, chain id, block\n  height, gas price, recent block history ({} entries), staleness & block delay alerts. When TXPOOL_URL is set,\n  it also shows tx-pool-webservice cache metrics for transactions, bundles, and signed orders.",
        config::MAX_BLOCK_HISTORY
    );
    println!("Update Interval: 5s metrics poll.");
}
