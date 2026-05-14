# Copyright 2026 H0llyW00dzZ
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

.PHONY: all header build test test-doc test-cover clippy fmt fmt-fix doc clean deps

# Print ASCII art banner.
header:
	@printf '%s\n' '  ____        _               _        '
	@printf '%s\n' ' |  _ \ __ _ | | __ __ _ ___ (_) _ __  '
	@printf '%s\n' ' | |_) / _` || |/ // _` / __|| || '"'"'__| '
	@printf '%s\n' ' |  __/ (_| ||   <| (_| \__ \| || |    '
	@printf '%s\n' ' |_|   \__,_||_|\_\\__,_|___/|_||_|    '
	@printf '%s\n' '                                        '
	@printf '%s\n' '  Rust SDK by H0llyW00dzZ (@github.com/H0llyW00dzZ)'
	@echo ""

# Default target.
all: header build test

## ──────────────────────────────────────────────
## Build
## ──────────────────────────────────────────────

# Build the crate and every target (binaries, tests, examples, benches).
build: header
	@echo "==> Building all targets..."
	cargo build --all-targets
	@echo "==> Done."

## ──────────────────────────────────────────────
## Quality
## ──────────────────────────────────────────────

# Run unit and integration tests.
test: header
	@echo "==> Running tests..."
	cargo test --all-targets
	@echo "==> Done."

# Run doc tests (the rustdoc examples).
test-doc: header
	@echo "==> Running doc tests..."
	cargo test --doc --all-features
	@echo "==> Done."

# Run tests with line coverage using cargo-llvm-cov.
# To view in browser: cargo llvm-cov --open
test-cover: header
	@echo "==> Running tests with coverage..."
	cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
	cargo llvm-cov --all-features --workspace --summary-only
	@echo "==> Done. (lcov.info written; cargo llvm-cov --open for HTML)"

# Run clippy with warnings promoted to errors. This is the Rust analog of
# `go vet` from the original Go project.
clippy: header
	@echo "==> Running clippy..."
	cargo clippy --all-targets --all-features -- -D warnings
	@echo "==> Done."

# Check formatting (CI mode — fails on diff, does not modify files).
fmt: header
	@echo "==> Checking rustfmt..."
	cargo fmt --all -- --check
	@echo "==> Done."

# Apply formatting. Convenience target for local use.
fmt-fix: header
	@echo "==> Applying rustfmt..."
	cargo fmt --all
	@echo "==> Done."

# Build the crate documentation locally.
# To view in browser: cargo doc --no-deps --open
doc: header
	@echo "==> Building docs..."
	cargo doc --no-deps
	@echo "==> Done. (cargo doc --no-deps --open to view)"

## ──────────────────────────────────────────────
## Cleanup
## ──────────────────────────────────────────────

# Remove build artifacts and coverage files.
clean: header
	cargo clean
	rm -f lcov.info
	@echo "==> Cleaned."

## ──────────────────────────────────────────────
## Dependencies
## ──────────────────────────────────────────────

# Install the cargo extensions used by the test-cover target.
# cargo-llvm-cov needs the llvm-tools-preview component; if it is missing run:
#   rustup component add llvm-tools-preview
deps: header
	cargo install cargo-llvm-cov
	@echo "==> Done."
