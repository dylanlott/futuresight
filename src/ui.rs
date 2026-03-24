use alloy::primitives::{Address, B256, U256};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Sparkline, Table, Wrap},
};
use std::time::{Duration, Instant};

use crate::config::{GAS_ALERT_HIGH_GWEI, GAS_SPIKE_MULTIPLIER, STALE_AFTER};
use crate::data::{ConnectionStatus, SignetMetrics, SuggestedFeeTier, TxPoolMetrics, TxPoolTx};

#[derive(Clone, Copy)]
enum ChainTipSyncStatus {
    Synced,
    Diverged { direction: &'static str, diff: u64 },
    Unknown,
}

pub struct Dashboard {
    pub should_quit: bool,
    refresh_interval: u64,
}

impl Dashboard {
    pub fn new(refresh_interval: u64) -> Self {
        Self {
            should_quit: false,
            refresh_interval,
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn render(&self, frame: &mut Frame, host: &SignetMetrics, rollup: &SignetMetrics) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(18),
                Constraint::Length(3),
            ])
            .split(frame.area());

        self.render_header(frame, outer[0], host, rollup);

        let panels = if outer[1].width >= 160 {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(outer[1])
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(outer[1])
        };

        self.render_host_panel(frame, panels[0], host);
        self.render_rollup_panel(frame, panels[1], host, rollup);
        self.render_footer(frame, outer[2], host, rollup);
    }

    fn render_header(
        &self,
        frame: &mut Frame,
        area: Rect,
        host: &SignetMetrics,
        rollup: &SignetMetrics,
    ) {
        let host_status = status_badge(&host.connection_status);
        let rollup_status = status_badge(&rollup.connection_status);
        let host_block = metric_or_na(host.chain_height());
        let rollup_block = metric_or_na(rollup.chain_height());

        let lines = vec![
            Line::from(vec![
                Span::styled(
                    " FUTURESIGHT ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    "production RPC telemetry cockpit",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("v{}", env!("CARGO_PKG_VERSION")),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(vec![
                status_chip("HOST", host_status.0, host_status.1),
                Span::raw(" "),
                Span::styled(
                    format!("#{}  age {}", host_block, block_age(host)),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw("    "),
                status_chip("ROLLUP", rollup_status.0, rollup_status.1),
                Span::raw(" "),
                Span::styled(
                    format!("#{}  age {}", rollup_block, block_age(rollup)),
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ];

        let paragraph = Paragraph::new(lines)
            .block(shell_block("Mission Control".to_string(), Color::Blue))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_host_panel(&self, frame: &mut Frame, area: Rect, metrics: &SignetMetrics) {
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Length(8),
                Constraint::Min(8),
            ])
            .split(area);

        self.render_summary(frame, sections[0], metrics, "Host", None);
        self.render_gas(frame, sections[1], metrics, "Host");
        self.render_block_history(frame, sections[2], metrics, "Host");
    }

    fn render_rollup_panel(
        &self,
        frame: &mut Frame,
        area: Rect,
        host: &SignetMetrics,
        metrics: &SignetMetrics,
    ) {
        let sections = if area.height >= 34 {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6),
                    Constraint::Length(8),
                    Constraint::Length(12),
                    Constraint::Min(8),
                ])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6),
                    Constraint::Length(8),
                    Constraint::Length(10),
                    Constraint::Min(6),
                ])
                .split(area)
        };

        self.render_summary(frame, sections[0], metrics, "Rollup", Some(host));
        self.render_gas(frame, sections[1], metrics, "Rollup");
        self.render_txpool(frame, sections[2], metrics, "Rollup");
        self.render_block_history(frame, sections[3], metrics, "Rollup");
    }

    fn render_summary(
        &self,
        frame: &mut Frame,
        area: Rect,
        metrics: &SignetMetrics,
        label: &str,
        host: Option<&SignetMetrics>,
    ) {
        let accent = panel_accent(label);
        let sync_status = host.map(|host_metrics| chain_tip_sync_status(host_metrics, metrics));
        let feed_accent = match sync_status {
            Some(ChainTipSyncStatus::Diverged { .. }) => Color::Red,
            _ => accent,
        };
        let (status_text, status_style) = status_badge(&metrics.connection_status);
        let rpc_width = area.width.saturating_sub(14) as usize;
        let delay = block_delay(metrics);
        let delay_style = match delay {
            Some(value) if value > metrics.block_delay_threshold => {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            }
            Some(value) if value > metrics.block_delay_threshold / 2 => {
                Style::default().fg(Color::Yellow)
            }
            Some(_) => Style::default().fg(Color::Green),
            None => Style::default().fg(Color::DarkGray),
        };

        let mut lines = vec![
            Line::from(vec![
                status_chip(label, status_text, status_style),
                Span::raw(" "),
                Span::styled(
                    format!("updated {}", relative_age(metrics.last_updated.elapsed())),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("stale after {}", relative_age(STALE_AFTER)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(vec![
                Span::styled("RPC ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    trim_middle(&metrics.rpc_url, rpc_width.max(24)),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                kv_span("Chain", metric_or_na(metrics.chain_id), Color::Cyan),
                Span::raw("  "),
                kv_span("Block", metric_or_na(metrics.chain_height()), accent),
                Span::raw("  "),
                kv_span("Age", block_age(metrics), Color::Yellow),
                Span::raw("  "),
                Span::styled("Delay ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    delay
                        .map(|value| format!("{}s", value))
                        .unwrap_or_else(|| "--".to_string()),
                    delay_style,
                ),
            ]),
        ];

        if let Some(host_metrics) = host {
            lines.push(chain_tip_comparison_line(
                host_metrics,
                metrics,
                sync_status.unwrap_or(ChainTipSyncStatus::Unknown),
            ));
        }

        let paragraph = Paragraph::new(lines)
            .block(shell_block(format!("{} Feed", label), feed_accent))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_gas(&self, frame: &mut Frame, area: Rect, metrics: &SignetMetrics, label: &str) {
        let accent = panel_accent(label);
        let block = shell_block(format!("{} Gas Deck", label), accent);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height < 4 {
            return;
        }

        let gas_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(inner);

        let trend_mult = metrics.gas_volatility_5m.map(|value| 1.0 + value.max(-1.0));
        let trend_style = match trend_mult {
            Some(multiplier) if multiplier >= GAS_SPIKE_MULTIPLIER => {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            }
            Some(_) => Style::default().fg(Color::Green),
            None => Style::default().fg(Color::DarkGray),
        };

        let gas_lines = vec![
            Line::from(vec![
                kv_span(
                    "Base",
                    fmt_gwei_opt(metrics.base_fee_per_gas),
                    fee_style(metrics.base_fee_per_gas),
                ),
                Span::raw("  "),
                kv_span(
                    "Next",
                    fmt_gwei_opt(metrics.next_base_fee_per_gas),
                    Color::Cyan,
                ),
                Span::raw("  "),
                kv_span("Legacy", fmt_gwei_opt(metrics.gas_price), Color::Gray),
            ]),
            Line::from(vec![
                kv_span(
                    "RPC tip",
                    fmt_gwei_opt(metrics.max_priority_fee_suggested),
                    Color::Magenta,
                ),
                Span::raw("  "),
                kv_span(
                    "Safe",
                    fmt_fee_tier(metrics.suggested_fees.as_ref().map(|fees| &fees.safe)),
                    Color::Green,
                ),
                Span::raw("  "),
                kv_span(
                    "Fast",
                    fmt_fee_tier(metrics.suggested_fees.as_ref().map(|fees| &fees.fast)),
                    Color::Red,
                ),
            ]),
            Line::from(vec![
                Span::styled("Trend ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    trend_mult
                        .map(|value| format!("x{:.2}", value))
                        .unwrap_or_else(|| "--".to_string()),
                    trend_style,
                ),
            ]),
        ];

        frame.render_widget(
            Paragraph::new(gas_lines).wrap(Wrap { trim: true }),
            gas_layout[0],
        );

        let utilization = metrics
            .gas_utilization_ma_n
            .unwrap_or_default()
            .clamp(0.0, 100.0);
        frame.render_widget(
            Gauge::default()
                .ratio(utilization / 100.0)
                .label(format!("utilization {:.0}%", utilization))
                .gauge_style(
                    Style::default()
                        .fg(accent)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                ),
            gas_layout[1],
        );

        let trend_points = sparkline_points(metrics);
        let sparkline = Sparkline::default()
            .data(&trend_points)
            .style(Style::default().fg(accent))
            .max(trend_points.iter().copied().max().unwrap_or(1));
        frame.render_widget(sparkline, gas_layout[2]);
    }

    fn render_txpool(&self, frame: &mut Frame, area: Rect, metrics: &SignetMetrics, label: &str) {
        let accent = panel_accent(label);
        let block = shell_block(format!("{} Flow Radar", label), accent);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height < 3 {
            return;
        }

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(inner);

        match &metrics.txpool {
            Some(txpool) => {
                let summary = txpool_summary_lines(txpool, inner.width as usize);
                frame.render_widget(Paragraph::new(summary).wrap(Wrap { trim: true }), layout[0]);
                self.render_txpool_table(frame, layout[1], txpool);
            }
            None => {
                let lines = vec![
                    Line::from(vec![Span::styled(
                        "No tx-pool service configured.",
                        Style::default().fg(Color::Gray),
                    )]),
                    Line::from(vec![Span::styled(
                        "Use --rollup-txpool-url to enable rollup flow metrics.",
                        Style::default().fg(Color::DarkGray),
                    )]),
                ];
                frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
            }
        }
    }

    fn render_txpool_table(&self, frame: &mut Frame, area: Rect, txpool: &TxPoolMetrics) {
        if area.height < 3 {
            return;
        }

        if txpool.transactions.is_empty() {
            let mut lines = vec![Line::from(vec![Span::styled(
                "(no transactions in view)",
                Style::default().fg(Color::DarkGray),
            )])];
            if let Some(error) = &txpool.error {
                lines.push(Line::from(vec![Span::styled(
                    trim_middle(error, area.width.saturating_sub(2) as usize),
                    Style::default().fg(Color::Red),
                )]));
            }
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
            return;
        }

        let header = Row::new(vec!["hash", "route", "value", "fee", "gas", "n", "ty"]).style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

        let max_rows = area.height.saturating_sub(2) as usize;
        let rows = txpool
            .transactions
            .iter()
            .take(max_rows)
            .map(tx_row)
            .collect::<Vec<_>>();

        let table = Table::new(
            rows,
            [
                Constraint::Length(14),
                Constraint::Min(18),
                Constraint::Length(10),
                Constraint::Length(12),
                Constraint::Length(8),
                Constraint::Length(6),
                Constraint::Length(4),
            ],
        )
        .header(header)
        .column_spacing(1);

        frame.render_widget(table, area);
    }

    fn render_block_history(
        &self,
        frame: &mut Frame,
        area: Rect,
        metrics: &SignetMetrics,
        label: &str,
    ) {
        let accent = panel_accent(label);
        let block = shell_block(format!("{} Block Tape", label), accent);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height < 3 {
            return;
        }

        if metrics.block_history.is_empty() {
            frame.render_widget(
                Paragraph::new("(no blocks yet)").style(Style::default().fg(Color::DarkGray)),
                inner,
            );
            return;
        }

        let header = Row::new(vec!["blk", "age", "tx", "gas", "base", "hash"]).style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

        let max_rows = inner.height.saturating_sub(2) as usize;
        let rows = metrics
            .block_history
            .iter()
            .take(max_rows)
            .enumerate()
            .map(|(index, block)| {
                let row_style = if index == 0 {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                let gas_ratio = if block.gas_limit > 0 {
                    (block.gas_used as f64 / block.gas_limit as f64) * 100.0
                } else {
                    0.0
                };
                let base_fee = block
                    .base_fee_per_gas
                    .map(|value| fmt_gwei_opt(Some(value)))
                    .unwrap_or_else(|| "--".to_string());

                Row::new(vec![
                    Cell::from(format!("#{}", block.number)),
                    Cell::from(relative_age_from_ts(block.timestamp)),
                    Cell::from(block.tx_count.to_string()),
                    Cell::from(format!("{:.0}%", gas_ratio)),
                    Cell::from(base_fee),
                    Cell::from(trim_middle(&block.hash, 14)),
                ])
                .style(row_style)
            })
            .collect::<Vec<_>>();

        let table = Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(8),
                Constraint::Length(6),
                Constraint::Length(7),
                Constraint::Length(10),
                Constraint::Min(12),
            ],
        )
        .header(header)
        .column_spacing(1);

        frame.render_widget(table, inner);
    }

    fn render_footer(
        &self,
        frame: &mut Frame,
        area: Rect,
        host: &SignetMetrics,
        rollup: &SignetMetrics,
    ) {
        let lines = vec![Line::from(vec![
            Span::styled("Controls ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::styled(" quit", Style::default().fg(Color::Gray)),
            Span::raw("  "),
            Span::styled(
                format!("refresh {}s", self.refresh_interval),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("  "),
            Span::styled(
                format!(
                    "host {} | rollup {}",
                    relative_age(host.last_updated.elapsed()),
                    relative_age(rollup.last_updated.elapsed())
                ),
                Style::default().fg(Color::Gray),
            ),
        ])];

        frame.render_widget(
            Paragraph::new(lines).block(shell_block("Flight Notes".to_string(), Color::DarkGray)),
            area,
        );
    }
}

fn shell_block(title: String, accent: Color) -> Block<'static> {
    Block::default()
        .title(Line::from(vec![Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(Color::Black)
                .bg(accent)
                .add_modifier(Modifier::BOLD),
        )]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
}

fn panel_accent(label: &str) -> Color {
    match label {
        "Host" => Color::Cyan,
        "Rollup" => Color::LightMagenta,
        _ => Color::Blue,
    }
}

fn status_badge(status: &ConnectionStatus) -> (&'static str, Style) {
    match status {
        ConnectionStatus::Connected => (
            "LIVE",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        ConnectionStatus::Stale => (
            "STALE",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        ConnectionStatus::Disconnected => (
            "DOWN",
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        ConnectionStatus::Error(_) => (
            "ERROR",
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
    }
}

fn status_chip(label: &str, status: &str, status_style: Style) -> Span<'static> {
    Span::styled(
        format!(" {} {} ", label.to_ascii_uppercase(), status),
        status_style,
    )
}

fn kv_span(label: &str, value: String, value_color: Color) -> Span<'static> {
    Span::styled(
        format!("{} {}", label, value),
        Style::default().fg(value_color),
    )
}

fn chain_tip_sync_status(host: &SignetMetrics, metrics: &SignetMetrics) -> ChainTipSyncStatus {
    let host_tip = host.chain_height();
    let rollup_tip = metrics.chain_height();

    match (host_tip, rollup_tip) {
        (Some(host_height), Some(rollup_height)) if host_height == rollup_height => {
            ChainTipSyncStatus::Synced
        }
        (Some(host_height), Some(rollup_height)) => {
            let (direction, diff) = if rollup_height < host_height {
                ("behind", host_height - rollup_height)
            } else {
                ("ahead", rollup_height - host_height)
            };
            ChainTipSyncStatus::Diverged { direction, diff }
        }
        _ => ChainTipSyncStatus::Unknown,
    }
}

fn chain_tip_comparison_line(
    host: &SignetMetrics,
    metrics: &SignetMetrics,
    sync_status: ChainTipSyncStatus,
) -> Line<'static> {
    let host_tip = host.chain_height();
    let rollup_tip = metrics.chain_height();

    let sync_spans = match sync_status {
        ChainTipSyncStatus::Synced => vec![
            Span::styled("Synced ", Style::default().fg(Color::Green)),
            Span::styled("at tip", Style::default().fg(Color::Green)),
        ],
        ChainTipSyncStatus::Diverged { direction, diff } => vec![
            Span::styled(
                format!(
                    "{direction} by {diff} block{}",
                    if diff == 1 { "" } else { "s" }
                ),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("Rollup {}", metric_or_na(rollup_tip)),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ],
        ChainTipSyncStatus::Unknown => vec![Span::styled(
            "Sync unknown",
            Style::default().fg(Color::DarkGray),
        )],
    };

    let mut spans = vec![
        Span::styled("Host tip ", Style::default().fg(Color::DarkGray)),
        Span::styled(metric_or_na(host_tip), Style::default().fg(Color::Cyan)),
        Span::raw("  "),
    ];
    spans.extend(sync_spans);
    Line::from(spans)
}

fn txpool_summary_lines(txpool: &TxPoolMetrics, width: usize) -> Vec<Line<'static>> {
    let health_style = if txpool.healthy {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .bg(Color::Red)
            .add_modifier(Modifier::BOLD)
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                format!(
                    " {} ",
                    if txpool.healthy {
                        "HEALTHY"
                    } else {
                        "DEGRADED"
                    }
                ),
                health_style,
            ),
            Span::raw(" "),
            Span::styled(
                trim_middle(&txpool.base_url, width.saturating_sub(20).max(22)),
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(vec![
            kv_span("Tx", metric_or_na(txpool.transactions_cache), Color::Green),
            Span::raw("  "),
            kv_span(
                "Bundles",
                metric_or_na(txpool.bundles_cache),
                Color::Magenta,
            ),
            Span::raw("  "),
            kv_span(
                "Orders",
                metric_or_na(txpool.signed_orders_cache),
                Color::Blue,
            ),
            Span::raw("  "),
            kv_span(
                "Updated",
                relative_age(txpool.last_updated.elapsed()),
                Color::Yellow,
            ),
        ]),
    ];

    if let Some(error) = &txpool.error {
        lines.push(Line::from(vec![Span::styled(
            trim_middle(error, width.saturating_sub(2)),
            Style::default().fg(Color::Red),
        )]));
    }

    lines
}

fn tx_row(tx: &TxPoolTx) -> Row<'static> {
    let route = format!(
        "{} -> {}",
        short_addr(&tx.from),
        tx.to
            .as_ref()
            .map(short_addr)
            .unwrap_or_else(|| "--".to_string())
    );

    Row::new(vec![
        Cell::from(short_hash(&tx.hash)).style(Style::default().fg(Color::Cyan)),
        Cell::from(trim_middle(&route, 26)).style(Style::default().fg(Color::White)),
        Cell::from(fmt_eth_short(&tx.value)).style(Style::default().fg(Color::Yellow)),
        Cell::from(fmt_tx_fee_gwei(tx)).style(Style::default().fg(Color::Magenta)),
        Cell::from(
            tx.gas_limit
                .map(|value| value.to_string())
                .unwrap_or_else(|| "--".to_string()),
        )
        .style(Style::default().fg(Color::Gray)),
        Cell::from(tx.nonce.to_string()).style(Style::default().fg(Color::Gray)),
        Cell::from(
            tx.tx_type
                .map(|value| value.to_string())
                .unwrap_or_else(|| "--".to_string()),
        )
        .style(Style::default().fg(Color::DarkGray)),
    ])
}

fn sparkline_points(metrics: &SignetMetrics) -> Vec<u64> {
    metrics
        .fee_history
        .as_ref()
        .map(|history| {
            history
                .base_fees
                .iter()
                .rev()
                .take(24)
                .rev()
                .map(|value| (*value / 1_000_000_000).max(1) as u64)
                .collect::<Vec<_>>()
        })
        .filter(|points| !points.is_empty())
        .unwrap_or_else(|| vec![0])
}

fn metric_or_na<T: std::fmt::Display>(value: Option<T>) -> String {
    value
        .map(|inner| inner.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

fn fee_style(value: Option<u128>) -> Color {
    match value.map(|wei| (wei as f64) / 1_000_000_000.0) {
        Some(gwei) if gwei >= GAS_ALERT_HIGH_GWEI => Color::Red,
        Some(gwei) if gwei >= GAS_ALERT_HIGH_GWEI * 0.5 => Color::Yellow,
        Some(_) => Color::Green,
        None => Color::DarkGray,
    }
}

fn fmt_fee_tier(tier: Option<&SuggestedFeeTier>) -> String {
    match tier {
        Some(value) if value.max_fee_per_gas > 0 && value.max_priority_fee_per_gas > 0 => format!(
            "{}/{}",
            fmt_gwei_opt(Some(value.max_fee_per_gas)),
            fmt_gwei_opt(Some(value.max_priority_fee_per_gas))
        ),
        _ => "N/A".to_string(),
    }
}

fn fmt_gwei_opt(wei: Option<u128>) -> String {
    match wei {
        Some(value) => {
            let gwei = (value as f64) / 1_000_000_000.0;
            if gwei >= 100.0 {
                format!("{:.0}g", gwei)
            } else {
                format!("{:.1}g", gwei)
            }
        }
        None => "N/A".to_string(),
    }
}

fn fmt_eth_short(value: &U256) -> String {
    let wei = value.to::<u128>();
    let eth = (wei as f64) / 1_000_000_000_000_000_000.0;
    if eth >= 1.0 {
        format!("{:.3}", eth)
    } else {
        format!("{:.5}", eth)
    }
}

fn fmt_tx_fee_gwei(tx: &TxPoolTx) -> String {
    if let Some(max_fee) = tx.max_fee_per_gas {
        let priority = tx.max_priority_fee_per_gas.unwrap_or(0);
        format!(
            "{}/{}",
            fmt_gwei_opt(Some(max_fee)),
            fmt_gwei_opt(Some(priority))
        )
    } else if let Some(gas_price) = tx.gas_price {
        fmt_gwei_opt(Some(gas_price))
    } else {
        "N/A".to_string()
    }
}

fn short_hash(hash: &B256) -> String {
    trim_middle(&format!("{:#x}", hash), 12)
}

fn short_addr(address: &Address) -> String {
    trim_middle(&format!("{:#x}", address), 14)
}

fn trim_middle(value: &str, max: usize) -> String {
    if value.len() <= max || max <= 6 {
        return value.to_string();
    }

    let left = (max.saturating_sub(2)) / 2;
    let right = max.saturating_sub(left + 2);
    format!("{}..{}", &value[..left], &value[value.len() - right..])
}

fn block_delay(metrics: &SignetMetrics) -> Option<u64> {
    metrics.latest_block_timestamp.map(seconds_since)
}

fn block_age(metrics: &SignetMetrics) -> String {
    metrics
        .latest_block_timestamp
        .map(relative_age_from_ts)
        .unwrap_or_else(|| "--".to_string())
}

fn relative_age_from_ts(timestamp: u64) -> String {
    relative_age(Duration::from_secs(seconds_since(timestamp)))
}

fn seconds_since(timestamp: u64) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    now.saturating_sub(timestamp)
}

fn relative_age(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3_600 {
        format!("{}m", seconds / 60)
    } else {
        format!("{}h{}m", seconds / 3_600, (seconds % 3_600) / 60)
    }
}

#[allow(dead_code)]
fn instant_age(instant: Instant) -> String {
    relative_age(instant.elapsed())
}
