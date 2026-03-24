BIN_NAME := futuresight
CARGO ?= cargo

## Defaults are set for the Parmigiana Signet environment
MAX_BLOCK_HISTORY ?= 24
BLOCK_DELAY_SECS ?= 60
HOST_RPC_URL ?= https://host-rpc.parmigiana.signet.sh
ROLLUP_RPC_URL ?= https://rpc.parmigiana.signet.sh
REFRESH_INTERVAL ?= 2
TXPOOL_MAX_ROWS ?= 12
TXPOOL_URL ?=
ROLLUP_TXPOOL_URL ?=
HOST_CONTRACTS ?=
RUN_ARGS ?=

.PHONY: help build run dev release fmt lint clean test watch parmigiana mainnet

help:
	@echo "FutureSight Make Targets"
	@echo "------------------------"
	@echo "  make build          - Build debug binary"
	@echo "  make release        - Build optimized release binary"
	@echo "  make run            - Run (debug) with optional *_RPC_URL, MAX_BLOCK_HISTORY, BLOCK_DELAY_SECS, REFRESH_INTERVAL, TXPOOL_MAX_ROWS, TXPOOL_URL, ROLLUP_TXPOOL_URL, HOST_CONTRACTS, and RUN_ARGS overrides"
	@echo "  make dev            - Alias for make watch"
	@echo "  make watch          - Run with cargo-watch (requires cargo-watch)"
	@echo "  make parmigiana     - Run with parmigiana defaults"
	@echo "  make mainnet        - Run with mainnet defaults"
	@echo "  make test           - Run tests"
	@echo "  make fmt            - Format code"
	@echo "  make lint           - Run clippy lints"
	@echo "  make clean          - Clean target directory"
	@echo "  make help           - Show this help"

build:
	$(CARGO) build

release:
	$(CARGO) build --release

run: build
	BLOCK_DELAY_SECS=$(BLOCK_DELAY_SECS) MAX_BLOCK_HISTORY=$(MAX_BLOCK_HISTORY) REFRESH_INTERVAL=$(REFRESH_INTERVAL) TXPOOL_MAX_ROWS=$(TXPOOL_MAX_ROWS) HOST_RPC_URL=$(HOST_RPC_URL) ROLLUP_RPC_URL=$(ROLLUP_RPC_URL) TXPOOL_URL=$(TXPOOL_URL) ROLLUP_TXPOOL_URL=$(ROLLUP_TXPOOL_URL) HOST_CONTRACTS=$(HOST_CONTRACTS) $(CARGO) run -- $(RUN_ARGS)

dev: watch

# Requires cargo-watch: cargo install cargo-watch
watch:
	BLOCK_DELAY_SECS=$(BLOCK_DELAY_SECS) MAX_BLOCK_HISTORY=$(MAX_BLOCK_HISTORY) REFRESH_INTERVAL=$(REFRESH_INTERVAL) TXPOOL_MAX_ROWS=$(TXPOOL_MAX_ROWS) HOST_RPC_URL=$(HOST_RPC_URL) ROLLUP_RPC_URL=$(ROLLUP_RPC_URL) TXPOOL_URL=$(TXPOOL_URL) ROLLUP_TXPOOL_URL=$(ROLLUP_TXPOOL_URL) HOST_CONTRACTS=$(HOST_CONTRACTS) cargo watch -x "run -- $(RUN_ARGS)"

parmigiana: build
	BLOCK_DELAY_SECS=$(BLOCK_DELAY_SECS) MAX_BLOCK_HISTORY=$(MAX_BLOCK_HISTORY) REFRESH_INTERVAL=$(REFRESH_INTERVAL) TXPOOL_MAX_ROWS=$(TXPOOL_MAX_ROWS) HOST_RPC_URL=https://host-rpc.parmigiana.signet.sh ROLLUP_RPC_URL=https://rpc.parmigiana.signet.sh TXPOOL_URL=$(TXPOOL_URL) ROLLUP_TXPOOL_URL=$(ROLLUP_TXPOOL_URL) HOST_CONTRACTS=$(HOST_CONTRACTS) $(CARGO) run -- $(RUN_ARGS)

mainnet: build
	BLOCK_DELAY_SECS=$(BLOCK_DELAY_SECS) MAX_BLOCK_HISTORY=$(MAX_BLOCK_HISTORY) REFRESH_INTERVAL=$(REFRESH_INTERVAL) TXPOOL_MAX_ROWS=$(TXPOOL_MAX_ROWS) HOST_RPC_URL=https://rpc.flashbots.net ROLLUP_RPC_URL=https://rpc.mainnet.signet.sh TXPOOL_URL=$(TXPOOL_URL) ROLLUP_TXPOOL_URL=$(ROLLUP_TXPOOL_URL) HOST_CONTRACTS=$(HOST_CONTRACTS) $(CARGO) run -- $(RUN_ARGS)

fmt:
	$(CARGO) fmt --all

lint:
	$(CARGO) clippy -- -D warnings

clean:
	$(CARGO) clean

test:
	$(CARGO) test --all
