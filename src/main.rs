mod data;
mod ui;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    env,
    io::{stdout, Stdout},
    time::Duration,
};
use tokio::time;

use data::{MetricsCollector, BLOCK_DELAY_DEFAULT};
use ui::Dashboard;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type CrosstermTerminal = Terminal<CrosstermBackend<Stdout>>;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let program = args.get(0).cloned().unwrap_or_else(|| "futuresight".into());

    // Simple flag handling
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help(&program);
        return Ok(());
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Positional args: [1] rpc_url, [2] block_delay_threshold
    let rpc_url = args.get(1).cloned().unwrap_or_else(|| "http://localhost:8545".to_string());

    println!("FutureSight {} - Ethereum RPC terminal dashboard", env!("CARGO_PKG_VERSION"));
    println!("RPC URL: {}", rpc_url);
    println!("Press 'q' to quit. Use --help for options.");

    let mut terminal = setup_terminal()?;
    let mut dashboard = Dashboard::new();
    // Optional: second CLI arg for block delay threshold seconds or env BLOCK_DELAY_SECS
    let cli_delay = args.get(2).and_then(|s| s.parse::<u64>().ok());
    let env_delay = std::env::var("BLOCK_DELAY_SECS").ok().and_then(|s| s.parse::<u64>().ok());
    let block_delay_threshold = cli_delay.or(env_delay).unwrap_or(BLOCK_DELAY_DEFAULT);

    let mut collector = MetricsCollector::new(rpc_url, block_delay_threshold);

    let mut last_update = std::time::Instant::now();
    
    loop {
        if last_update.elapsed() >= Duration::from_secs(5) {
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
    println!("  BLOCK_DELAY_SECS     Seconds before block delay alert (default: 60 or env BLOCK_DELAY_SECS)\n");
    println!("Environment:");
    println!("  BLOCK_DELAY_SECS     Override block delay alert threshold when second arg omitted\n");
    println!("Flags:");
    println!("  -h, --help           Show this help and exit");
    println!("  -V, --version        Show version information and exit\n");
    println!("Description:");
    println!("  FutureSight is a terminal dashboard showing Ethereum RPC metrics: connection status, chain id, block\n  height, gas price, peer count, recent block history ({} entries), staleness & block delay alerts.", data::MAX_BLOCK_HISTORY);
    println!("Update Interval: 5s metrics poll.");
}
