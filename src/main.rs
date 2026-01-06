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
use alloy::primitives::Address;
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
    #[arg(default_value = "http://rpc.parmigiana.signet.sh", env = "RPC_URL")]
    rpc_url: String,

    /// Host (L1) JSON-RPC endpoint (left pane). Defaults to RPC_URL when unset.
    #[arg(long = "host-rpc-url", env = "HOST_RPC_URL")]
    host_rpc_url: Option<String>,

    /// L2 rollup JSON-RPC endpoint (right pane). Defaults to RPC_URL when unset.
    #[arg(long = "rollup-rpc-url", env = "ROLLUP_RPC_URL")]
    rollup_rpc_url: Option<String>,

    /// Seconds before block delay alert is displayed
    #[arg(default_value_t = crate::config::BLOCK_DELAY_DEFAULT, env = "BLOCK_DELAY_SECS")]
    block_delay_secs: u64,

    /// How often metric data is refreshed
    #[arg(long, short, default_value_t = crate::config::DEFAULT_REFRESH_INTERVAL, env = "REFRESH_INTERVAL")]
    refresh_interval: u64,

    /// Base URL for tx-pool-webservice (example: http://localhost:8080)
    #[arg(long, env = "TXPOOL_URL")]
    txpool_url: Option<String>,

    /// Base URL for rollup tx-pool-webservice (example: http://localhost:8080)
    #[arg(long, env = "ROLLUP_TXPOOL_URL")]
    rollup_txpool_url: Option<String>,

    /// Maximum number of tx-pool transactions to keep and display
    #[arg(long = "txpool-max-rows", env = "TXPOOL_MAX_ROWS", default_value_t = crate::config::DEFAULT_TXPOOL_MAX_ROWS)]
    txpool_max_rows: usize,

    /// Disable fetching and displaying tx list from tx-pool
    #[arg(long = "no-txpool-list", default_value_t = false)]
    txpool_disable_list: bool,

    /// Number of recent blocks to keep in memory/display
    #[arg(long = "max-block-history", env = "MAX_BLOCK_HISTORY", default_value_t = crate::config::DEFAULT_MAX_BLOCK_HISTORY)]
    max_block_history: usize,

    /// Restrict host view tx list to calls into these contracts (comma-separated addresses)
    #[arg(long = "host-contracts", env = "HOST_CONTRACTS", value_delimiter = ',')]
    host_contracts: Vec<Address>,
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
    let host_rpc = cli.host_rpc_url.clone().unwrap_or_else(|| cli.rpc_url.clone());
    let rollup_rpc = cli.rollup_rpc_url.clone().unwrap_or_else(|| cli.rpc_url.clone());
    println!("=== Host RPC: {} ===", host_rpc);
    println!("=== Rollup RPC: {} ===", rollup_rpc);
    if let Some(url) = &cli.txpool_url {
        println!("=== Monitoring host tx-pool-webservice: {} ===", url);
    }
    if let Some(url) = &cli.rollup_txpool_url {
        println!("=== Monitoring rollup tx-pool-webservice: {} ===", url);
    }
    if !cli.host_contracts.is_empty() {
        let filters: Vec<String> = cli.host_contracts.iter().map(|a| format!("{:#x}", a)).collect();
        println!("Host tx filter (contracts): {}", filters.join(", "));
    }
    println!("Press 'q' to quit. Use --help for options.");

    let mut terminal = setup_terminal()?;
    let mut dashboard = Dashboard::new();

    // create a metrics collector with the given configs
    let mut host_collector = MetricsCollector::new_with_txpool(
        Config {
            rpc_url: host_rpc.clone(),
            block_delay_threshold: cli.block_delay_secs,
            max_block_history: cli.max_block_history,
            txpool_max_rows: cli.txpool_max_rows,
            txpool_fetch_list: !cli.txpool_disable_list,
            txpool_filter_contracts: cli.host_contracts.clone(),
        },
        cli.txpool_url.clone(),
    );

    let mut rollup_collector = MetricsCollector::new_with_txpool(
        Config {
            rpc_url: rollup_rpc.clone(),
            block_delay_threshold: cli.block_delay_secs,
            max_block_history: cli.max_block_history,
            txpool_max_rows: cli.txpool_max_rows,
            txpool_fetch_list: !cli.txpool_disable_list,
            txpool_filter_contracts: Vec::new(),
        },
        cli.rollup_txpool_url.clone(),
    );

    // collect metrics at startup to prime the dashboard
    host_collector.collect_metrics().await;
    rollup_collector.collect_metrics().await;

    let mut last_update = std::time::Instant::now();

    // Loop every
    loop {
        if last_update.elapsed() >= Duration::from_secs(cli.refresh_interval) {
            host_collector.collect_metrics().await;
            rollup_collector.collect_metrics().await;
            last_update = std::time::Instant::now();
        }

        // Update staleness if no successful updates for threshold.
        host_collector.check_staleness();
        rollup_collector.check_staleness();

        let host_metrics = host_collector.get_metrics();
        let rollup_metrics = rollup_collector.get_metrics();
        terminal.draw(|frame| dashboard.render(frame, host_metrics, rollup_metrics))?;

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
    println!("Options:");
    println!("  --host-rpc-url URL    Host RPC URL (defaults to RPC_URL or positional)");
    println!("  --rollup-rpc-url URL  Rollup RPC URL (defaults to RPC_URL or positional)");
    println!("  --rollup-txpool-url   Rollup tx-pool-webservice base URL");
    println!("  --host-contracts ADDR[,ADDR...]  Filter host tx list to calls into listed contracts\n");
    println!(
        "  --max-block-history N  Number of recent blocks to keep (default: {} or env MAX_BLOCK_HISTORY)\n",
        crate::config::DEFAULT_MAX_BLOCK_HISTORY
    );
    println!("Environment:");
    println!(
        "  BLOCK_DELAY_SECS     Override block delay alert threshold when second arg omitted\n"
    );
    println!("  MAX_BLOCK_HISTORY     Configure how many recent blocks to keep and display\n");
    println!(
        "  TXPOOL_URL           Optional tx-pool-webservice base URL for cache metrics (e.g. http://localhost:8080)\n"
    );
    println!(
        "  ROLLUP_TXPOOL_URL    Optional rollup tx-pool-webservice base URL (e.g. http://localhost:8080)\n"
    );
    println!("  HOST_RPC_URL          Override host RPC URL (falls back to RPC_URL)\n");
    println!("  ROLLUP_RPC_URL        Configure rollup RPC URL (falls back to RPC_URL)\n");
    println!("  HOST_CONTRACTS        Comma-separated contract addresses to filter host tx list\n");
    println!("Flags:");
    println!("  -h, --help           Show this help and exit");
    println!("  -V, --version        Show version information and exit\n");
    println!("Description:");
    println!(
        "  FutureSight is a terminal dashboard showing Ethereum RPC metrics: connection status, chain id, block\n  height, gas price, recent block history (configurable entries), staleness & block delay alerts. When TXPOOL_URL is set,\n  it also shows tx-pool-webservice cache metrics for transactions, bundles, and signed orders."
    );
    println!("Update Interval: 5s metrics poll.");
}
