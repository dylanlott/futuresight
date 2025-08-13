# FutureSight

A minimal terminal dashboard for monitoring Ethereum JSON RPC servers using ratatui.

![Terminal Dashboard](https://img.shields.io/badge/Terminal-Dashboard-blue) ![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white) ![Ethereum](https://img.shields.io/badge/Ethereum-3C3C3D?logo=ethereum&logoColor=white)

## Features

- **Real-time Monitoring**: Track block height, gas prices, and peer count
- **Connection Status**: Visual indication of RPC server connectivity
- **Clean Terminal UI**: Built with ratatui for a responsive interface
- **Minimal Resource Usage**: Lightweight monitoring with 5-second update intervals
- **Error Handling**: Graceful handling of RPC connection issues

## Installation

Clone the repository and build with Cargo:

```bash
git clone <repository-url>
cd futuresight
cargo build --release
```

## Usage

Run with default settings (connects to `http://localhost:8545`):

```bash
cargo run
```

Connect to a custom RPC endpoint:

```bash
cargo run -- http://your-ethereum-node:8545
cargo run -- https://mainnet.infura.io/v3/your-api-key
```

### Controls

- **q** or **Esc**: Quit the application

## Dashboard Information

The dashboard displays:

- **Connection Status**: Current RPC connection state and last update time
- **Block Height**: Latest block number from the network
- **Gas Price**: Current gas price in both Gwei and wei
- **Network Peers**: Number of connected peers (if supported by the RPC)

## Architecture

The project maintains clean separation between layers:

- **Data Layer** (`src/data.rs`): RPC client and metrics collection
- **Presentation Layer** (`src/ui.rs`): Terminal UI components and rendering
- **Main Application** (`src/main.rs`): Event loop and terminal management

## Requirements

- Rust 1.70+
- Access to an Ethereum JSON RPC endpoint

## Dependencies

- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal user interface
- [tokio](https://tokio.rs/) - Async runtime
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal control

## License

This project is open source and available under the MIT License.