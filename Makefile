BIN_NAME := futuresight
CARGO ?= cargo

## Defaults are set for Pecorino test net
MAX_BLOCK_HISTORY ?= 40
BLOCK_DELAY_SECS ?= 60
RPC_URL ?= http://rpc.pecorino.signet.sh

.PHONY: help build run dev release fmt lint clean test watch pecorino

help:
	@echo "FutureSight Make Targets"
	@echo "------------------------"
	@echo "  make build          - Build debug binary"
	@echo "  make release        - Build optimized release binary"
	@echo "  make run            - Run (debug) with optional RPC_URL, MAX_BLOCK_HISTORY, and BLOCK_DELAY_SECS env vars"
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
	BLOCK_DELAY_SECS=$(BLOCK_DELAY_SECS) MAX_BLOCK_HISTORY=$(MAX_BLOCK_HISTORY) $(CARGO) run -- $(RPC_URL) $(BLOCK_DELAY_SECS)

# Requires cargo-watch: cargo install cargo-watch
watch:
	cargo watch -x 'run -- $(RPC_URL) $(BLOCK_DELAY_SECS)'

dev: run

fmt:
	$(CARGO) fmt --all || true

lint:
	$(CARGO) clippy -- -D warnings || true

clean:
	$(CARGO) clean

test:
	$(CARGO) test --all
