# Demo Makefile — mirrors the explorer repo's build discipline:
# every artifact (toolchain, registry, target dir) lives in the /tmp
# tmpfs; nothing is written to $HOME.

SHELL := /bin/bash

TMPROOT   ?= /tmp/zramfs_inkodaily/textmetrics-build
BIN       := textmetrics

TOOLCHAIN ?= stable
TARGET    ?= x86_64-unknown-linux-gnu

# Reuse the explorer spike's tmpfs cargo/rustup homes so the toolchain
# and the git/registry caches are shared instead of re-downloaded.
SHARED    := /tmp/zramfs_inkodaily/explorer-rs-build

export CARGO_TARGET_DIR := $(TMPROOT)/target
export CARGO_HOME := $(SHARED)/cargo
export RUSTUP_HOME := $(SHARED)/rustup
export CARGO_INCREMENTAL := 0
export PATH := $(SHARED)/cargo/bin:$(PATH)

.PHONY: all setup-env build check run clippy clean

all: build

setup-env:
	@ln -sf $(CARGO_TARGET_DIR) target
	@mkdir -p $(CARGO_HOME) $(RUSTUP_HOME) $(CARGO_TARGET_DIR)

build: setup-env
	cargo +$(TOOLCHAIN) build --release --target $(TARGET)

check: setup-env
	cargo +$(TOOLCHAIN) check --target $(TARGET)

run: build
	$(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN)

clippy: setup-env
	cargo +$(TOOLCHAIN) clippy --target $(TARGET)

clean:
	rm -rf $(TMPROOT) target
