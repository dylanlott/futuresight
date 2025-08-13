# Simple Makefile for FutureSight

CARGO ?= cargo
BIN_NAME := futuresight

# Default block delay threshold (overridden by env or CLI)
BLOCK_DELAY_SECS ?= 60
RPC_URL ?= http://localhost:8545
SIGNET_RPC_URL ?= http://rpc.pecorino.signet.sh

.PHONY: help build run dev release fmt lint clean test watch

help:
	@echo "FutureSight Make Targets"
	@echo "  make signet         - Run with Signet RPC defaults"
	@echo "  make build          - Build debug binary"
	@echo "  make release        - Build optimized release binary"
	@echo "  make run            - Run (debug) with optional RPC_URL and BLOCK_DELAY_SECS env vars"
	@echo "  make dev            - Run with cargo watch (requires cargo-watch)"
	@echo "  make test           - Run tests (none yet)"
	@echo "  make fmt            - Format code"
	@echo "  make lint           - Run clippy lints"
	@echo "  make clean          - Clean target directory"
	@echo "  make help           - Show this help"

build:
	$(CARGO) build

release:
	$(CARGO) build --release

run: build
	BLOCK_DELAY_SECS=$(BLOCK_DELAY_SECS) $(CARGO) run -- $(RPC_URL) $(BLOCK_DELAY_SECS)

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

signet:
	$(CARGO) run -- $(SIGNET_RPC_URL) $(BLOCK_DELAY_SECS)