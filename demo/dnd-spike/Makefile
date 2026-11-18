# Wayland DnD demo. Mirrors the explorer Makefile discipline: all build
# artifacts stay on tmpfs (/tmp), never on the SSD.
#
# Reuses the explorer's CARGO_HOME/RUSTUP_HOME so every crate (including the
# cosmic-text git fork) is already cached in tmpfs — no downloads.

SHELL := /bin/bash

TMPROOT ?= /tmp/zramfs_inkodaily/dnd-spike-build
EXPLORER_BUILD ?= /tmp/zramfs_inkodaily/explorer-rs-build
BIN            := dnd-spike
TOOLCHAIN      ?= stable
TARGET         ?= x86_64-unknown-linux-gnu

TMPROOT       ?= /tmp/zramfs_inkodaily/dnd-spike-build
export CARGO_TARGET_DIR := $(TMPROOT)
export CARGO_HOME       := $(EXPLORER_BUILD)/cargo
export RUSTUP_HOME      := $(EXPLORER_BUILD)/rustup
export PATH             := $(EXPLORER_BUILD)/cargo/bin:$(PATH)

.PHONY: all build check run clean setup-env

all: build

setup-env:
	@ln -sfn $(CARGO_TARGET_DIR) target
	@mkdir -p $(CARGO_TARGET_DIR)
	@if ! rustup toolchain list --no-self-update 2>/dev/null | grep -q "$(TOOLCHAIN)"; then \
		rustup toolchain install $(TOOLCHAIN) --profile minimal --no-self-update; \
	fi

build: setup-env
	cargo +$(TOOLCHAIN) build --release --target $(TARGET)

check: setup-env
	cargo +$(TOOLCHAIN) check --target $(TARGET)

run: build
	$(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN)

clean:
	rm -rf $(TMPROOT ?= /tmp/zramfs_inkodaily/dnd-spike-build
