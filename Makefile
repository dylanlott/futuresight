BIN_NAME := futuresight
CARGO ?= cargo

## Defaults are set for Parmigiana test net
MAX_BLOCK_HISTORY ?= 40
BLOCK_DELAY_SECS ?= 60
HOST_RPC_URL ?= https://rpc-host.parmigiana.signet.sh
ROLLUP_RPC_URL ?= https://rpc.parmigiana.signet.sh

.PHONY: help build run dev release fmt lint clean test watch parmigiana

help:
	@echo "FutureSight Make Targets"
	@echo "------------------------"
	@echo "  make build          - Build debug binary"
	@echo "  make release        - Build optimized release binary"
	@echo "  make run            - Run (debug) with optional HOST_RPC_URL, ROLLUP_RPC_URL, MAX_BLOCK_HISTORY, and BLOCK_DELAY_SECS env vars"
	@echo "  make dev            - Run with cargo watch (requires cargo-watch)"
	@echo "  make test           - Run tests (none yet)"
	@echo "  make watch          - Runs FutureSight with cargo-watch; Requires cargo-watch to be installed"
	@echo "  make fmt            - Format code"
	@echo "  make lint           - Run clippy lints"
	@echo "  make clean          - Clean target directory"
	@echo "  make help           - Show this help"

build:
	$(CARGO) build

release:
	$(CARGO) build --release

run: build
	BLOCK_DELAY_SECS=$(BLOCK_DELAY_SECS) MAX_BLOCK_HISTORY=$(MAX_BLOCK_HISTORY) HOST_RPC_URL=$(HOST_RPC_URL) ROLLUP_RPC_URL=$(ROLLUP_RPC_URL) $(CARGO) run

# Requires cargo-watch: cargo install cargo-watch
watch:
	HOST_RPC_URL=$(HOST_RPC_URL) ROLLUP_RPC_URL=$(ROLLUP_RPC_URL) cargo watch -x run

parmigiana: build
	BLOCK_DELAY_SECS=$(BLOCK_DELAY_SECS) MAX_BLOCK_HISTORY=$(MAX_BLOCK_HISTORY) HOST_RPC_URL=https://host-rpc.parmigiana.signet.sh ROLLUP_RPC_URL=https://rpc.parmigiana.signet.sh $(CARGO) run

fmt:
	$(CARGO) fmt --all || true

lint:
	$(CARGO) clippy -- -D warnings || true

clean:
	$(CARGO) clean

test:
	$(CARGO) test --all
