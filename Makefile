# HARD RULE: every recipe sources ./env.sh so cargo's target dir and all
# temporary artifacts stay in in-memory tmpfs (/tmp). Never build without it -
# a ./target folder on the SSD is a bug; delete it and rebuild via this file.

SHELL := /bin/bash

TMPROOT   ?= /tmp/zramfs_inkodaily/explorer-rs-build
DIST      ?= $(CURDIR)/dist
BIN       := explorer


TOOLCHAIN   ?= stable
TARGET      ?= x86_64-unknown-linux-gnu
# Dependency: sudo pacman -S rustup


export CARGO_TARGET_DIR := $(TMPROOT)/target
export CARGO_INCREMENTAL := 0
export CARGO_HOME=$(TMPROOT)/cargo
export RUSTUP_HOME=$(TMPROOT)/rustup
export PATH := $(TMPROOT)/cargo/bin:$(PATH)

.PHONY: all build check run test dist clean fmt size setup-toolchain


all: build

setup-env:
	@mkdir -p $(CARGO_HOME) $(RUSTUP_HOME)
	@if ! rustup toolchain list --no-self-update 2>/dev/null | grep -q "$(TOOLCHAIN)"; then \
		echo "==> Installing Rust toolchain '$(TOOLCHAIN)' into $(TMPROOT)..."; \
		rustup toolchain install $(TOOLCHAIN) --profile minimal --no-self-update; \
	fi
	@if ! rustup +$(TOOLCHAIN) target list --installed 2>/dev/null | grep -q "$(TARGET)"; then \
		echo "==> Adding target '$(TARGET)' for $(TOOLCHAIN)..."; \
		rustup +$(TOOLCHAIN) target add $(TARGET); \
	fi

build: setup-env
	cargo +$(TOOLCHAIN) build --release --target $(TARGET)

check: setup-env
	cargo +$(TOOLCHAIN) check --target $(TARGET)

run: build
	$(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN) $(ARGS)

test: setup-env
	cargo +$(TOOLCHAIN) test --target $(TARGET)

dist: build
	@mkdir -p $(DIST)
	cp $(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN) $(DIST)/$(BIN)
	-strip --strip-all $(DIST)/$(BIN) 2>/dev/null || true
	@ls -l $(DIST)/$(BIN) | awk '{printf "dist: %s (%d bytes, %.1f MB)\n", $$NF, $$5, $$5/1048576}'
	@file $(DIST)/$(BIN)

size: dist
	@size $(DIST)/$(BIN) 2>/dev/null || true

fmt: setup-env
	cargo +$(TOOLCHAIN) fmt

clean:
	cargo clean || true
	rm -rf $(TMPROOT) $(DIST)
