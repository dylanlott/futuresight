# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

FutureSight is a Rust terminal dashboard for monitoring the Signet network. It displays real-time L1 (Host) and L2 (Rollup) blockchain metrics side-by-side using ratatui, including block history, EIP-1559/4844 gas analytics, connection status, and optional tx-pool monitoring.

## Build & Development Commands

```bash
make build           # Debug build
make release         # Optimized release build
make run             # Build + run against Parmigiana testnet defaults
make watch           # Continuous rebuild with cargo-watch
make fmt             # cargo fmt
make lint            # clippy with -D warnings
make test            # cargo test --all
make clean           # Clean build artifacts
```

Single test: `cargo test <test_name>`

Default RPC endpoints point to the Parmigiana testnet. Override with env vars or CLI flags:
```bash
HOST_RPC_URL=<url> ROLLUP_RPC_URL=<url> make run
cargo run -- --host-rpc-url <url> --rollup-rpc-url <url>
```

## Architecture

Four source files in `src/`:

- **main.rs** — CLI parsing (clap with env var support), terminal setup (crossterm raw mode + alternate screen), and the main event loop. The loop polls metrics at a configurable interval (default 5s), checks keyboard input every 100ms, and renders the dashboard each frame.

- **config.rs** — Constants: refresh intervals, staleness threshold (20s), block history limits, gas fee percentiles, alert thresholds. Tuning knobs live here.

- **data.rs** — Core business logic (~830 lines). Key types:
  - `SignetRpcClient` — JSON-RPC wrapper (block number, gas price, fee history, full block fetch)
  - `MetricsCollector` — Orchestrates per-cycle metric collection for one chain. Maintains `SignetMetrics` state with rolling `VecDeque<BlockInfo>` block history (newest-first, backfills up to 6 blocks/cycle)
  - `ConnectionStatus` enum — Connected/Stale/Disconnected/Error, drives UI coloring
  - `TxPoolClient` — Optional tx-pool-webservice integration (transactions, bundles, signed orders)
  - Fee suggestion algorithm: percentiles from `eth_feeHistory`, maxFee = nextBaseFee + 2× priorityFee

- **ui.rs** — Dashboard rendering (~630 lines). Two 50%-width panels (Host | Rollup), each with connection status, chain ID, block height, gas metrics, block delay alerts, and block history. Rollup panel additionally shows tx-pool summary and transaction list. Color scheme: green=healthy, yellow=stale/warning, red=error/alert, cyan=hashes/URLs.

## Data Flow

```
CLI args (clap) → Terminal setup → Two MetricsCollector instances (host + rollup)
→ Event loop: collect_metrics() → check_staleness() → render dashboard → poll input
```

Individual RPC call failures degrade gracefully (specific fields go None) without crashing the dashboard or flipping overall connection status.

## Key Design Decisions

- **VecDeque for block history**: O(1) push_front/pop_back, newest-first ordering
- **Lazy backfill**: Max 6 blocks per cycle to avoid burst RPC load
- **Stale detection**: 20s without successful update transitions to Stale (distinct from Disconnected/Error)
- **Separate collectors**: Host and rollup metrics are independent, allowing asymmetric configuration
- **Flexible tx-pool parsing**: Handles both array and object-with-nested-arrays JSON response shapes

## Dependencies

Core: `alloy` (Ethereum SDK), `tokio` (async runtime), `ratatui`/`crossterm` (TUI), `clap` (CLI), `serde`/`serde_json`, `reqwest` (HTTP). Signet-specific: `signet-constants`, `signet-tx-cache`.

Rust edition: 2024.
