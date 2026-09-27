SHELL := /bin/bash
ENV := source ./env.sh &&

.PHONY: run check build snapshot dist

run:
	@$(ENV) cargo run

check:
	@$(ENV) cargo check

build:
	@$(ENV) cargo build

# Headless verification render (no window needed).
snapshot:
	@$(ENV) cargo run -- --snapshot /tmp/explorer_snapshot.bmp

# Release binary, stripped, copied to the SSD under dist/.
dist:
	@$(ENV) cargo build --release
	@mkdir -p dist
	@source ./env.sh && strip -o dist/explorer "$$CARGO_TARGET_DIR/release/explorer"
	@echo "dist/explorer ($(shell du -h dist/explorer | cut -f1))"
