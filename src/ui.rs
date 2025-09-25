use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::time::Duration;

use crate::config::{STALE_AFTER, GAS_ALERT_HIGH_GWEI, GAS_SPIKE_MULTIPLIER};
use crate::data::{ConnectionStatus, SignetMetrics};

pub struct Dashboard {
    pub should_quit: bool,
}

impl Dashboard {
    pub fn new() -> Self {
        Self { should_quit: false }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn render(&self, frame: &mut Frame, metrics: &SignetMetrics) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // connection
                Constraint::Length(3), // chain id
                Constraint::Length(3), // block
                Constraint::Length(7), // gas (expanded)
                Constraint::Length(3), // block alert
                Constraint::Length(6), // tx-pool
                Constraint::Min(8),    // history
                Constraint::Length(5), // help
            ])
            .split(frame.area());

        self.render_connection_status(frame, chunks[0], metrics);
        self.render_chain_id(frame, chunks[1], metrics);
        self.render_block_height(frame, chunks[2], metrics);
        self.render_gas_price(frame, chunks[3], metrics);
    self.render_block_delay_alert(frame, chunks[4], metrics);
    self.render_txpool(frame, chunks[5], metrics);
    self.render_block_history(frame, chunks[6], metrics);
    self.render_help(frame, chunks[7]);
    }

    fn render_connection_status(&self, frame: &mut Frame, area: Rect, metrics: &SignetMetrics) {
        let status_text = match &metrics.connection_status {
            ConnectionStatus::Connected => "Connected".to_string(),
            ConnectionStatus::Stale => "Stale".to_string(),
            ConnectionStatus::Disconnected => "Disconnected".to_string(),
            ConnectionStatus::Error(err) => format!("Error: {}", err),
        };

        let status_style = match &metrics.connection_status {
            ConnectionStatus::Connected => Style::default().fg(Color::Green),
            ConnectionStatus::Stale => Style::default().fg(Color::Yellow),
            ConnectionStatus::Disconnected | ConnectionStatus::Error(_) => {
                Style::default().fg(Color::Red)
            }
        };

        let elapsed = metrics.last_updated.elapsed();
        let last_update = if elapsed < Duration::from_secs(1) {
            "< 1s ago".to_string()
        } else {
            format!("{}s ago", elapsed.as_secs())
        };

        let mut line_parts = vec![
            Span::styled("Status: ", Style::default()),
            Span::styled(&status_text, status_style),
            Span::styled(" | ", Style::default()),
            Span::styled("RPC: ", Style::default()),
            Span::styled(&metrics.rpc_url, Style::default().fg(Color::Cyan)),
            Span::styled(" | ", Style::default()),
            Span::styled("Updated: ", Style::default()),
            Span::styled(last_update.clone(), Style::default().fg(Color::Yellow)),
        ];

        if matches!(metrics.connection_status, ConnectionStatus::Stale) {
            let threshold_secs = STALE_AFTER.as_secs();
            line_parts.push(Span::styled(" | Stale > ", Style::default()));
            line_parts.push(Span::styled(
                format!("{}s", threshold_secs),
                Style::default().fg(Color::Yellow),
            ));
        }

        let content = vec![Line::from(line_parts)];

        let paragraph = Paragraph::new(content)
            .block(Block::default().title("Connection").borders(Borders::ALL));

        frame.render_widget(paragraph, area);
    }

    fn render_block_height(&self, frame: &mut Frame, area: Rect, metrics: &SignetMetrics) {
        let block_text = match metrics.block_number {
            Some(block) => format!("{}", block),
            None => "N/A".to_string(),
        };

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let age = metrics
            .latest_block_timestamp
            .map(|ts| now_secs.saturating_sub(ts));
        let age_text = age
            .map(|a| format!("{}s ago", a))
            .unwrap_or("--".to_string());

        let content = vec![Line::from(vec![
            Span::styled("Current Block: ", Style::default()),
            Span::styled(block_text, Style::default().fg(Color::Green)),
            Span::styled("  (", Style::default()),
            Span::styled(age_text, Style::default().fg(Color::Yellow)),
            Span::styled(")", Style::default()),
        ])];

        let paragraph = Paragraph::new(content)
            .block(Block::default().title("Block Height").borders(Borders::ALL));

        frame.render_widget(paragraph, area);
    }

    fn render_gas_price(&self, frame: &mut Frame, area: Rect, metrics: &SignetMetrics) {
        fn fmt_gwei_opt(v: Option<u128>) -> String {
            match v {
                Some(wei) => format!("{:.2} Gwei", (wei as f64) / 1_000_000_000.0),
                None => "N/A".to_string(),
            }
        }
        fn fmt_gwei_pair(max_fee: u128, prio: u128) -> String {
            let max_g = (max_fee as f64) / 1_000_000_000.0;
            let prio_g = (prio as f64) / 1_000_000_000.0;
            format!("{:.1}/{:.1} g", max_g, prio_g)
        }
        fn fmt_opt_pair(tier: Option<&crate::data::SuggestedFeeTier>) -> String {
            match tier {
                Some(t) if t.max_fee_per_gas > 0 && t.max_priority_fee_per_gas > 0 => {
                    fmt_gwei_pair(t.max_fee_per_gas, t.max_priority_fee_per_gas)
                }
                _ => "N/A".to_string(),
            }
        }

        // Line 1: Base fee, Next base, Legacy gas price
        let base_fee_val_gwei = metrics.base_fee_per_gas.map(|w| (w as f64) / 1_000_000_000.0);
        let base_fee = fmt_gwei_opt(metrics.base_fee_per_gas);
        let next_base = fmt_gwei_opt(metrics.next_base_fee_per_gas);
        let legacy = fmt_gwei_opt(metrics.gas_price);

        let mut lines: Vec<Line> = Vec::new();
        let base_style = match base_fee_val_gwei {
            Some(v) if v >= GAS_ALERT_HIGH_GWEI => Style::default().fg(Color::Red),
            Some(v) if v >= GAS_ALERT_HIGH_GWEI * 0.5 => Style::default().fg(Color::Yellow),
            Some(_) => Style::default().fg(Color::Green),
            None => Style::default().fg(Color::Gray),
        };
        lines.push(Line::from(vec![
            Span::styled("Base: ", Style::default()),
            Span::styled(base_fee, base_style),
            Span::raw("  |  Next: "),
            Span::styled(next_base, Style::default().fg(Color::Cyan)),
            Span::raw("  |  Legacy: "),
            Span::styled(legacy, Style::default().fg(Color::Gray)),
        ]));

        // Line 2: Priority suggestion (RPC) and Utilization MA
        let prio_rpc = fmt_gwei_opt(metrics.max_priority_fee_suggested);
        let util = metrics
            .gas_utilization_ma_n
            .map(|u| format!("{:.0}%", u))
            .unwrap_or_else(|| "N/A".to_string());
        lines.push(Line::from(vec![
            Span::styled("Prio (RPC): ", Style::default()),
            Span::styled(prio_rpc, Style::default().fg(Color::Magenta)),
            Span::raw("  |  Util MA: "),
            Span::styled(util, Style::default().fg(Color::Green)),
        ]));

        // Line 3: Suggested tiers (safe/standard/fast)
        let (safe_s, std_s, fast_s) = if let Some(sug) = &metrics.suggested_fees {
            (
                fmt_opt_pair(Some(&sug.safe)),
                fmt_opt_pair(Some(&sug.standard)),
                fmt_opt_pair(Some(&sug.fast)),
            )
        } else {
            ("N/A".to_string(), "N/A".to_string(), "N/A".to_string())
        };
        lines.push(Line::from(vec![
            Span::styled("Suggested: ", Style::default()),
            Span::styled("Safe ", Style::default()),
            Span::styled(safe_s, Style::default().fg(Color::Green)),
            Span::raw("  |  Std "),
            Span::styled(std_s, Style::default().fg(Color::Yellow)),
            Span::raw("  |  Fast "),
            Span::styled(fast_s, Style::default().fg(Color::Red)),
        ]));

        // Line 4: Spike indicator based on volatility vs multiplier (1+vol > multiplier)
        if let Some(vol) = metrics.gas_volatility_5m {
            let mult = 1.0 + vol.max(-1.0);
            let spike = mult >= GAS_SPIKE_MULTIPLIER;
            let label = if spike { "Spike" } else { "Stable" };
            let style = if spike { Style::default().fg(Color::Red) } else { Style::default().fg(Color::Green) };
            lines.push(Line::from(vec![
                Span::styled("Trend: ", Style::default()),
                Span::styled(label, style),
                Span::raw("  ("),
                Span::styled(format!("x{:.2}", mult), Style::default().fg(Color::Gray)),
                Span::raw(")"),
            ]));
        }

        // Line 4 (optional): blob metrics if present
        if metrics.blob_base_fee.is_some() || metrics.blob_gas_utilization_ma_n.is_some() {
            let blob_fee = fmt_gwei_opt(metrics.blob_base_fee);
            let blob_util = metrics
                .blob_gas_utilization_ma_n
                .map(|u| format!("{:.0}%", u))
                .unwrap_or_else(|| "N/A".to_string());
            lines.push(Line::from(vec![
                Span::styled("Blob fee: ", Style::default()),
                Span::styled(blob_fee, Style::default().fg(Color::Cyan)),
                Span::raw("  |  Blob Util MA: "),
                Span::styled(blob_util, Style::default().fg(Color::Blue)),
            ]));
        }

        let paragraph = Paragraph::new(lines)
            .block(Block::default().title("Gas").borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    fn render_chain_id(&self, frame: &mut Frame, area: Rect, metrics: &SignetMetrics) {
        let chain_text = match metrics.chain_id {
            Some(id) => format!("{}", id),
            None => "N/A".to_string(),
        };

        let content = vec![Line::from(vec![
            Span::styled("Chain ID: ", Style::default()),
            Span::styled(chain_text, Style::default().fg(Color::Blue)),
        ])];

        let paragraph =
            Paragraph::new(content).block(Block::default().title("Network").borders(Borders::ALL));
        frame.render_widget(paragraph, area);
    }

    fn render_block_history(&self, frame: &mut Frame, area: Rect, metrics: &SignetMetrics) {
        // Show newest first (already stored newest at front)
        let mut lines: Vec<Line> = Vec::new();
    for (idx, block) in metrics.block_history.iter().enumerate() {
            if idx >= 50 {
                break;
            } // safety cap for rendering
            let age_style = if idx == 0 {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Gray)
            };
            let num_span = Span::styled(format!("#{}", block.number), age_style);
            let hash_short = if block.hash.len() > 10 {
                &block.hash[0..10]
            } else {
                &block.hash
            };
            let hash_span =
                Span::styled(format!(" {}", hash_short), Style::default().fg(Color::Cyan));
            let tx_span = Span::styled(
                format!(" tx:{}", block.tx_count),
                Style::default().fg(Color::Yellow),
            );
            let gas_ratio = if block.gas_limit > 0 {
                (block.gas_used as f64 / block.gas_limit as f64) * 100.0
            } else {
                0.0
            };
            let gas_span = Span::styled(
                format!(" gas:{:.0}%", gas_ratio),
                Style::default().fg(Color::Magenta),
            );
            // Optional short base fee hint for newest blocks if available
            let bf_span = if let Some(bf) = block.base_fee_per_gas {
                let g = (bf as f64) / 1_000_000_000.0;
                Span::styled(format!(" bf:{:.0}g", g), Style::default().fg(Color::Blue))
            } else {
                Span::raw("")
            };
            lines.push(Line::from(vec![num_span, hash_span, tx_span, gas_span, bf_span]));
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no blocks yet)",
                Style::default().fg(Color::DarkGray),
            )));
        }

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true }).block(
            Block::default()
                .title("Recent Blocks (newest first) ")
                .borders(Borders::ALL),
        );
        frame.render_widget(paragraph, area);
    }

    fn render_block_delay_alert(&self, frame: &mut Frame, area: Rect, metrics: &SignetMetrics) {
        // Determine delay
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let delay = metrics
            .latest_block_timestamp
            .map(|ts| now_secs.saturating_sub(ts));
        let threshold = metrics.block_delay_threshold;
        let exceeded = delay.map(|d| d > threshold).unwrap_or(false);

        let (title, style, msg) = if exceeded {
            (
                "ALERT",
                Style::default().fg(Color::Red),
                format!(
                    "No new block for {}s (threshold {}s). Network or node may be stalled.",
                    delay.unwrap_or(0),
                    threshold
                ),
            )
        } else {
            (
                "Block Delay",
                Style::default().fg(Color::Green),
                format!(
                    "Last block {}s ago (threshold {}s).",
                    delay.unwrap_or(0),
                    threshold
                ),
            )
        };

        let content = vec![Line::from(vec![Span::styled(msg, style)])];
        let paragraph =
            Paragraph::new(content).block(Block::default().title(title).borders(Borders::ALL));
        frame.render_widget(paragraph, area);
    }

    fn render_txpool(&self, frame: &mut Frame, area: Rect, metrics: &SignetMetrics) {
        let title = "Tx Pool";
        if let Some(tp) = &metrics.txpool {
            let health_style = if tp.healthy {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red)
            };
            let updated = tp.last_updated.elapsed();
            let updated_text = if updated < Duration::from_secs(1) {
                "< 1s ago".to_string()
            } else {
                format!("{}s ago", updated.as_secs())
            };

            let mut line1 = vec![
                Span::styled("Health: ", Style::default()),
                Span::styled(if tp.healthy { "OK" } else { "Down" }, health_style),
                Span::raw(" | URL: "),
                Span::styled(&tp.base_url, Style::default().fg(Color::Cyan)),
                Span::raw(" | Updated: "),
                Span::styled(updated_text, Style::default().fg(Color::Yellow)),
            ];
            if let Some(err) = &tp.error {
                line1.push(Span::raw(" | Error: "));
                line1.push(Span::styled(err, Style::default().fg(Color::Red)));
            }

            let mut lines = vec![Line::from(line1)];

            let tx = tp
                .transactions_cache
                .map(|v| v.to_string())
                .unwrap_or_else(|| "N/A".to_string());
            let bundles = tp
                .bundles_cache
                .map(|v| v.to_string())
                .unwrap_or_else(|| "N/A".to_string());
            let orders = tp
                .signed_orders_cache
                .map(|v| v.to_string())
                .unwrap_or_else(|| "N/A".to_string());

            lines.push(Line::from(vec![
                Span::styled("Transactions: ", Style::default()),
                Span::styled(tx, Style::default().fg(Color::Green)),
                Span::raw("  |  Bundles: "),
                Span::styled(bundles, Style::default().fg(Color::Magenta)),
                Span::raw("  |  Signed Orders: "),
                Span::styled(orders, Style::default().fg(Color::Blue)),
            ]));

            let paragraph = Paragraph::new(lines)
                .block(Block::default().title(title).borders(Borders::ALL));
            frame.render_widget(paragraph, area);
        } else {
            let lines = vec![Line::from(vec![
                Span::styled(
                    "Set TXPOOL_URL to enable tx-pool-webservice metrics.",
                    Style::default().fg(Color::DarkGray),
                ),
            ])];
            let paragraph = Paragraph::new(lines)
                .block(Block::default().title(title).borders(Borders::ALL));
            frame.render_widget(paragraph, area);
        }
    }

    fn render_help(&self, frame: &mut Frame, area: Rect) {
        let help_text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("Press ", Style::default()),
                Span::styled("'q'", Style::default().fg(Color::Yellow)),
                Span::styled(" to quit", Style::default()),
            ]),
            Line::from(vec![
                Span::styled("Updates every ", Style::default()),
                Span::styled("5 seconds", Style::default().fg(Color::Cyan)),
            ]),
        ];

        let paragraph =
            Paragraph::new(help_text).block(Block::default().title("Help").borders(Borders::ALL));

        frame.render_widget(paragraph, area);
    }
}
