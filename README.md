# FutureSight

FutureSight is a Rust terminal dashboard for monitoring Ethereum RPC health, recent block flow, fee pressure, and optional tx-pool activity across a host chain and a rollup.

![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white) ![Ethereum](https://img.shields.io/badge/Ethereum-3C3C3D?logo=ethereum&logoColor=white)

![parmigiana dashboard screenshot](parmigiana.png)

## What It Shows

- Connection state with stale and error signaling
- Current chain ID and best observed chain height
- Rollup tip sync status against the host tip, including block difference when out of sync and a red rollup feed box when unsynced
- Block age and chain halt alerts
- EIP-1559 fee telemetry, fee suggestions, utilization gauge, and fee trend sparkline
- Rolling block tape with gas usage and base fee context
- Optional tx-pool service health, cache counts, and recent transactions in the rollup panel

## Build

Prerequisites: Rust toolchain. `make` is optional and only needed for the helper targets below.

```bash
git clone https://github.com/dylanlott/futuresight
cd futuresight
cargo build --release
```

## Run

The built-in CLI defaults target the Parmigiana Signet environment.

```bash
cargo run
```

Show the full CLI surface:

```bash
cargo run -- --help
```

Tune refresh rate and history depth:

```bash
cargo run -- \
  --refresh-interval 3 \
  --max-block-history 40 \
  --block-delay-secs 90
```

Enable tx-pool telemetry:

```bash
cargo run -- \
  --txpool-url https://transactions.parmigiana.signet.sh
```

Disable tx-pool transaction lists while keeping tx-pool summary counts:

```bash
cargo run -- \
  --txpool-url https://transactions.parmigiana.signet.sh \
  --no-txpool-list
```

Use the Makefile wrappers:

```bash
make run
make dev
make parmigiana
make mainnet
```

Notes:

- `cargo run` uses the CLI defaults shown in the table below.
- `make run` falls back to the same Parmigiana endpoint defaults as the CLI, but existing shell env vars or `make` variable overrides still win.
- `make parmigiana` forces `HOST_RPC_URL=https://host-rpc.parmigiana.signet.sh` and `ROLLUP_RPC_URL=https://rpc.parmigiana.signet.sh`.
- `make mainnet` forces `HOST_RPC_URL=https://rpc.flashbots.net` and `ROLLUP_RPC_URL=https://rpc.mainnet.signet.sh`.
- `make run RUN_ARGS="--no-txpool-list"` is the Makefile path for flags that do not have env-var equivalents.

## Configuration

Most runtime flags can also be set through environment variables.

Flag                       | Env                 | Default
-------------------------- | ------------------- | -------------------------------------------
`--host-rpc-url`           | `HOST_RPC_URL`      | `https://host-rpc.parmigiana.signet.sh`
`--rollup-rpc-url`         | `ROLLUP_RPC_URL`    | `https://rpc.parmigiana.signet.sh`
`--txpool-url`             | `TXPOOL_URL`        | `https://transactions.parmigiana.signet.sh`
`--block-delay-secs`       | `BLOCK_DELAY_SECS`  | `60`
`--refresh-interval`, `-r` | `REFRESH_INTERVAL`  | `2`
`--max-block-history`      | `MAX_BLOCK_HISTORY` | `24`
`--txpool-max-rows`        | `TXPOOL_MAX_ROWS`   | `12`
`--no-txpool-list`         | none                | `false`

Notes:

- `--txpool-url` powers the rollup panel's Flow Radar section.
- `--txpool-url` is a network-level tx-pool endpoint. For known Parmigiana and mainnet hosts, FutureSight auto-populates the watched host system contracts from Signet constants instead of taking a manual contract list.
- `--no-txpool-list` disables fetching transaction rows while keeping tx-pool summary requests enabled.

## Controls

- `q`
- `Esc`

## Make Targets

```bash
make build
make clean
make dev
make fmt
make help
make lint
make mainnet
make parmigiana
make release
make run
make test
make watch
```

`make dev` is an alias for `make watch`. `make watch` requires `cargo-watch`.
