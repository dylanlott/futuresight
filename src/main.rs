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
    let rpc_url = env::args()
        .nth(1)
        .unwrap_or_else(|| "http://localhost:8545".to_string());

    println!("Starting Ethereum RPC Monitor...");
    println!("RPC URL: {}", rpc_url);
    println!("Press 'q' to quit");

    let mut terminal = setup_terminal()?;
    let mut dashboard = Dashboard::new();
    // Optional: second CLI arg for block delay threshold seconds or env BLOCK_DELAY_SECS
    let cli_delay = env::args().nth(2).and_then(|s| s.parse::<u64>().ok());
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
