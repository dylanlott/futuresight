# FutureSight 🔮

A minimal terminal dashboard for interacting with and observing the [Signet](https://signet.sh) network.

![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white) ![Ethereum](https://img.shields.io/badge/Ethereum-3C3C3D?logo=ethereum&logoColor=white)

## Dashboard

![FutureSight Dashboard](./assets/futuresight-dashboard.png)

FutureSight currently displays the following data:

- **Connection Status**: Current RPC connection state and last update time
- **Chain Halt Detection**: Alerts if a new block hasn't been detected within the configured threshold
- **Recent Blocks**: Rolling history of the latest blocks with tx count & gas utilization
- **Block Height**: Displays the latest block number from the network
- **Gas Price**: Current gas price displayed in gwei and wei
- **Alerts**: Stale connection and block delay warnings

## Installation

*Pre-requisites: `make` and `rust` tooling*

Clone the repository and build with Cargo:

```bash
git clone https://github.com/dylanlott/futuresight
cd futuresight
cargo build --release && cargo run
```

## Usage

`tl;dr` `make run`  starts the dashboard for the Pecorino test net.

```bash
# equivalent to make run
cargo run -- https://rpc.pecorino.signet.sh 30 
```

### Make Commands

Common shortcuts:

```bash
make build        # debug build
make release      # optimized build
make run          # run FutureSight (targets Pecorino test network by default)
make fmt          # run cargo fmt
make lint         # run clippy
make test         # run tests
```

### Controls

- **q** or **Esc**: Quit the application

## License

This project is open source and available under the MIT License.