mod config;
mod data;
mod ui;

use alloy::primitives::Address;
use clap::{Parser, value_parser};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use data::{Config, MetricsCollector};
use eyre::Result;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io::{Stdout, stdout},
    time::{Duration, Instant},
};
use ui::Dashboard;

type CrosstermTerminal = Terminal<CrosstermBackend<Stdout>>;

#[derive(Parser, Debug)]
#[command(
    name = env!("CARGO_PKG_NAME"),
    version = env!("CARGO_PKG_VERSION"),
    about = "FutureSight is a terminal dashboard for Ethereum RPC and tx-pool telemetry.",
    next_line_help = true
)]
struct Cli {
    /// Host (L1) JSON-RPC endpoint.
    #[arg(
        long = "host-rpc-url",
        env = "HOST_RPC_URL",
        default_value = "https://host-rpc.parmigiana.signet.sh"
    )]
    host_rpc_url: String,

    /// Rollup (L2) JSON-RPC endpoint.
    #[arg(
        long = "rollup-rpc-url",
        env = "ROLLUP_RPC_URL",
        default_value = "https://rpc.parmigiana.signet.sh"
    )]
    rollup_rpc_url: String,

    /// Seconds before a block delay alert is shown.
    #[arg(
        long = "block-delay-secs",
        env = "BLOCK_DELAY_SECS",
        default_value_t = crate::config::BLOCK_DELAY_DEFAULT,
        value_parser = value_parser!(u64).range(1..)
    )]
    block_delay_secs: u64,

    /// Metrics refresh interval in seconds.
    #[arg(
        long,
        short,
        env = "REFRESH_INTERVAL",
        default_value_t = crate::config::DEFAULT_REFRESH_INTERVAL,
        value_parser = value_parser!(u64).range(1..)
    )]
    refresh_interval: u64,

    /// Base URL for the host tx-pool service.
    #[arg(long, env = "TXPOOL_URL")]
    txpool_url: Option<String>,

    /// Base URL for the rollup tx-pool service.
    #[arg(long, env = "ROLLUP_TXPOOL_URL")]
    rollup_txpool_url: Option<String>,

    /// Maximum tx-pool rows rendered per panel.
    #[arg(
        long = "txpool-max-rows",
        env = "TXPOOL_MAX_ROWS",
        default_value_t = crate::config::DEFAULT_TXPOOL_MAX_ROWS
    )]
    txpool_max_rows: usize,

    /// Disable fetching and displaying tx-pool transactions.
    #[arg(long = "no-txpool-list", default_value_t = false)]
    txpool_disable_list: bool,

    /// Number of recent blocks retained in memory.
    #[arg(
        long = "max-block-history",
        env = "MAX_BLOCK_HISTORY",
        default_value_t = crate::config::DEFAULT_MAX_BLOCK_HISTORY
    )]
    max_block_history: usize,

    /// Restrict host tx-pool rows to calls into these contracts.
    #[arg(long = "host-contracts", env = "HOST_CONTRACTS", value_delimiter = ',')]
    host_contracts: Vec<Address>,
}

#[tokio::main]
async fn main() -> Result<()> {
    run(Cli::parse()).await
}

async fn run(cli: Cli) -> Result<()> {
    if cli.txpool_max_rows == 0 {
        return Err(eyre::eyre!("--txpool-max-rows must be at least 1"));
    }
    if cli.max_block_history == 0 {
        return Err(eyre::eyre!("--max-block-history must be at least 1"));
    }

    let mut dashboard = Dashboard::new(cli.refresh_interval);
    let mut terminal = TerminalSession::enter()?;

    let mut host_collector = MetricsCollector::new_with_txpool(
        Config {
            rpc_url: cli.host_rpc_url.clone(),
            block_delay_threshold: cli.block_delay_secs,
            max_block_history: cli.max_block_history,
            txpool_max_rows: cli.txpool_max_rows,
            txpool_fetch_list: !cli.txpool_disable_list,
            txpool_filter_contracts: cli.host_contracts.clone(),
        },
        cli.txpool_url.clone(),
    )?;

    let mut rollup_collector = MetricsCollector::new_with_txpool(
        Config {
            rpc_url: cli.rollup_rpc_url.clone(),
            block_delay_threshold: cli.block_delay_secs,
            max_block_history: cli.max_block_history,
            txpool_max_rows: cli.txpool_max_rows,
            txpool_fetch_list: !cli.txpool_disable_list,
            txpool_filter_contracts: Vec::new(),
        },
        cli.rollup_txpool_url.clone(),
    )?;

    tokio::join!(
        host_collector.collect_metrics(),
        rollup_collector.collect_metrics()
    );

    let refresh_every = Duration::from_secs(cli.refresh_interval);
    let ui_tick = Duration::from_millis(200);
    let mut last_refresh = Instant::now();

    loop {
        if last_refresh.elapsed() >= refresh_every {
            tokio::join!(
                host_collector.collect_metrics(),
                rollup_collector.collect_metrics()
            );
            last_refresh = Instant::now();
        }

        host_collector.check_staleness();
        rollup_collector.check_staleness();

        terminal.draw(|frame| {
            dashboard.render(
                frame,
                host_collector.get_metrics(),
                rollup_collector.get_metrics(),
            )
        })?;

        if event::poll(ui_tick)?
            && let Event::Key(key) = event::read()?
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            dashboard.quit();
        }

        if dashboard.should_quit {
            break;
        }
    }

    Ok(())
}

struct TerminalSession {
    terminal: CrosstermTerminal,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(out);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    fn draw<F>(&mut self, render_fn: F) -> Result<()>
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        self.terminal.draw(render_fn)?;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}
