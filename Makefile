# HARD RULE: every recipe sources ./env.sh so cargo's target dir and all
# temporary artifacts stay in in-memory tmpfs (/tmp). Never build without it -
# a ./target folder on the SSD is a bug; delete it and rebuild via this file.

SHELL := /bin/bash
ENV := source ./env.sh &&

.PHONY: run check build dist

run:
	@$(ENV) cargo run

check:
	@$(ENV) cargo check

build:
	@$(ENV) cargo build

# Release binary, stripped, copied to the SSD under dist/.
dist:
	@$(ENV) cargo build --release
	@mkdir -p dist
	@source ./env.sh && strip -o dist/explorer "$$CARGO_TARGET_DIR/release/explorer"
	@echo "dist/explorer ($$(du -h dist/explorer | cut -f1))"
