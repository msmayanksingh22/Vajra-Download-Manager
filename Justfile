# Project Vajra Justfile

default: help

# Show available commands
help:
	@just --list

# Install all dependencies (Node, Rust)
install:
	npm install
	cargo fetch

# Run the full Tauri application in development mode
dev:
	npm run tauri dev

# Run only the UI frontend
dev-ui:
	npm run dev

# Run only the Vajra Daemon
dev-daemon:
	cargo run --bin vajra-daemon

# Build the complete release package
build:
	npm run tauri build

# Build the release daemon
build-daemon:
	cargo build --release --bin vajra-daemon

# Run all tests
test: test-rust test-ui

# Run Rust tests
test-rust:
	cargo test

# Run UI tests
test-ui:
	npm run test

# Run Playwright E2E tests
test-e2e:
	npm run test:e2e
