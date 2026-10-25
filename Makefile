# HARD RULE: every recipe sources ./env.sh so cargo's target dir and all
# temporary artifacts stay in in-memory tmpfs (/tmp). Never build without it -
# a ./target folder on the SSD is a bug; delete it and rebuild via this file.

# this project is under active development. so less ssd writing is considered here.

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

# Program icon source: libraries-48.png preferred, else the largest
# libraries-*.png (glob order is size order)
APP_ICON     := assets/app-icon.png
APP_ICON_SRC := $(or $(wildcard assets/icons/libraries-48.png),$(lastword $(sort $(wildcard assets/icons/libraries-*.png))))

.PHONY: all build check run test dist clean fmt size setup-toolchain


all: build

# setup-env:
# 	@ln -sf $(CARGO_TARGET_DIR) target
# 	@mkdir -p $(CARGO_HOME) $(RUSTUP_HOME)
# 	@if ! rustup toolchain list --no-self-update 2>/dev/null | grep -q "$(TOOLCHAIN)"; then \
# 		echo "==> Installing Rust toolchain '$(TOOLCHAIN)' into $(TMPROOT)..."; \
# 		rustup toolchain install $(TOOLCHAIN) --profile minimal --no-self-update; \
# 	fi
# 	@if ! rustup +$(TOOLCHAIN) target list --installed 2>/dev/null | grep -q "$(TARGET)"; then \
# 		echo "==> Adding target '$(TARGET)' for $(TOOLCHAIN)..."; \
# 		rustup +$(TOOLCHAIN) target add $(TARGET); \
# 	fi
setup-env:
	@ln -sf $(CARGO_TARGET_DIR) target
	@mkdir -p $(CARGO_HOME) $(RUSTUP_HOME)
	@# program icon: the proper assets/icons/libraries-*.png (48px
	@# preferred, else the largest) is copied to the canonical name the
	@# code embeds — the source never names a size-specific file
	@cp $(APP_ICON_SRC) $(APP_ICON)
	@if ! rustup toolchain list --no-self-update 2>/dev/null | grep -q "$(TOOLCHAIN)"; then \
		echo "==> Installing Rust toolchain '$(TOOLCHAIN)'..."; \
		n=1; while [ $$n -le 3 ]; do \
			if rustup toolchain install $(TOOLCHAIN) --profile minimal --no-self-update; then \
				break; \
			fi; \
			echo "==> Download failed, retrying ($$n/3)..."; \
			n=$$((n + 1)); \
			sleep 2; \
		done; \
		[ $$n -le 3 ] || exit 1; \
	fi
	@if ! rustup +$(TOOLCHAIN) target list --installed 2>/dev/null | grep -q "$(TARGET)"; then \
		echo "==> Adding target '$(TARGET)' for $(TOOLCHAIN)..."; \
		n=1; while [ $$n -le 3 ]; do \
			if rustup +$(TOOLCHAIN) target add $(TARGET); then \
				break; \
			fi; \
			echo "==> Download failed, retrying ($$n/3)..."; \
			n=$$((n + 1)); \
			sleep 2; \
		done; \
		[ $$n -le 3 ] || exit 1; \
	fi

build: setup-env
	cargo +$(TOOLCHAIN) build --release --target $(TARGET) $(if $(FEATURES),--features $(FEATURES),)

check: setup-env
	cargo +$(TOOLCHAIN) check --target $(TARGET) $(if $(FEATURES),--features $(FEATURES),)

run: build
	$(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN) $(ARGS)

test: setup-env
	cargo +$(TOOLCHAIN) test --target $(TARGET) $(if $(FEATURES),--features $(FEATURES),)

dist: build
	@mkdir -p $(DIST)
	-strip --strip-all $(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN) 2>/dev/null || true
	-./tools/upx  $(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN) || true
	@target="$$(realpath $(DIST)/$(BIN))"; \
	for p in /proc/[0-9]*; do \
		[ "$$(readlink "$$p/exe" 2>/dev/null)" = "$$target (deleted)" ] || \
		[ "$$(readlink -f "$$p/exe" 2>/dev/null)" = "$$target" ] || continue; \
		kill "$${p##*/}" 2>/dev/null || true; \
	done; \
	sleep 0.3
	cp $(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN) $(DIST)/$(BIN)
	@ls -l $(DIST)/$(BIN)
	@file $(DIST)/$(BIN)

size: dist
	@size $(DIST)/$(BIN) 2>/dev/null || true

fmt: setup-env
	cargo +$(TOOLCHAIN) fmt

clean:
	cargo clean || true
	rm -rf $(TMPROOT) $(DIST)
