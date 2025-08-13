# FutureSight 🔮

> A minimal terminal dashboard for interacting with and observing the Signet network.

![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white) ![Ethereum](https://img.shields.io/badge/Ethereum-3C3C3D?logo=ethereum&logoColor=white)

## Features

- **Realtime Block Monitoring**: Tracks a list of observed block heights, gas prices, and transaction counts
- **Connection Status**: Visual indication of RPC server connectivity
- **Chain Halt Detection**: Alerts if a new block hasn't been detected within the configured threshold

## Installation

Clone the repository and build with Cargo:

```bash
git clone https://github.com/dylanlott/futuresight
cd futuresight
cargo build --release
```

## Usage

Run with default settings (connects to `http://localhost:8545`):

```bash
cargo run
```

Connect to a Signet RPC endpoint:

```bash
# the pecorino test net
cargo run -- https://rpc.pecorino.signet.sh
```

Specify a 30 second block delay alert threshold (seconds) either as a second argument or env variable:

```bash
cargo run -- https://rpc.pecorino.signet.sh 30 
BLOCK_DELAY_SECS=120 cargo run -- https://mainnet.infura.io/v3/your-api-key
```

Show the help text:

```bash
cargo run -- --help
cargo run -- --version
```

### Using the Makefile

Common shortcuts:

```bash
make signet		  # run futuresight
make build        # debug build
make release      # optimized build
make run          # run (uses RPC_URL & BLOCK_DELAY_SECS env vars)
make fmt          # run cargo fmt
make lint         # run clippy
make test         # run tests
```

### Controls

- **q** or **Esc**: Quit the application

## Dashboard

FutureSight currently displays the following data:

- **Connection Status**: Current RPC connection state and last update time
- **Block Height**: Latest block number from the network
  - Shows time since last block and displays an alert if it's past the configured threshold
  - Shows a list of blocks as they're received with minimal block info attached
- **Gas Price**: Current gas price displayed in gwei and wei
- **Recent Blocks**: Rolling history of the latest blocks with tx count & gas utilization
- **Alerts**: Stale connection and block delay warnings



## License

This project is open source and available under the MIT License.