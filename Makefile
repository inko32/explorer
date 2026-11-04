# HARD RULE: every recipe sources ./env.sh so cargo's target dir and all
# temporary artifacts stay in in-memory tmpfs (/tmp). Never build without it -
# a ./target folder on the SSD is a bug; delete it and rebuild via this file.

# this project is under active development. so less ssd writing is considered here.

SHELL := /bin/bash

TMPROOT   ?= /tmp/zramfs_inkodaily/explorer-rs-build
DIST      ?= $(CURDIR)/dist
BIN       := explorer
# COMPRESS=0 keeps the dist binary unstripped and uncompressed (for
# size/symbol analysis); default applies strip + upx.
COMPRESS  ?= 1


TOOLCHAIN   ?= stable
TARGET      ?= x86_64-unknown-linux-gnu
# Dependency: sudo pacman -S rustup


export CARGO_TARGET_DIR := $(TMPROOT)/target
export CARGO_INCREMENTAL := 0
export CARGO_HOME=$(TMPROOT)/cargo
export RUSTUP_HOME=$(TMPROOT)/rustup
export PATH := $(TMPROOT)/cargo/bin:$(PATH)

.PHONY: all build check run test dist clean fmt size setup-toolchain bloat tree clippy clippy-json


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

# minimal profile excludes clippy; install the component on first use
# (same on-demand pattern as the bloat target)
define CLIPPY_ENSURE
	@if ! rustup +$(TOOLCHAIN) component list --installed 2>/dev/null | grep -q '^clippy'; then \
		echo "==> Adding component 'clippy' for $(TOOLCHAIN)..."; \
		n=1; while [ $$n -le 3 ]; do \
			if rustup +$(TOOLCHAIN) component add clippy; then \
				break; \
			fi; \
			echo "==> Download failed, retrying ($$n/3)..."; \
			n=$$((n + 1)); \
			sleep 2; \
		done; \
		[ $$n -le 3 ] || exit 1; \
	fi
endef

build: setup-env
	cargo +$(TOOLCHAIN) build --release --target $(TARGET) $(if $(FEATURES),--features $(FEATURES),)

check: setup-env
	cargo +$(TOOLCHAIN) check --target $(TARGET) $(if $(FEATURES),--features $(FEATURES),)

run: build
	$(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN) $(ARGS)

test: setup-env
	cargo +$(TOOLCHAIN) test --target $(TARGET) $(if $(FEATURES),--features $(FEATURES),)

clippy: setup-env
	$(CLIPPY_ENSURE)
	cargo +$(TOOLCHAIN) clippy --target $(TARGET) $(if $(FEATURES),--features $(FEATURES),)

# machine-readable clippy (one JSON diagnostic per line) — for tooling
# that sorts/filters the warning list.
clippy-json: setup-env
	$(CLIPPY_ENSURE)
	@# grep exits 1 on zero matches (clean run) — not an error here
	-cargo +$(TOOLCHAIN) clippy --target $(TARGET) --message-format=json $(if $(FEATURES),--features $(FEATURES),) 2>/dev/null | grep '"reason":"compiler-message"' > /tmp/clippy.jsonl || true
	@echo "wrote /tmp/clippy.jsonl"

dist: build
	@mkdir -p $(DIST)
	@target="$$(realpath $(DIST)/$(BIN))"; \
	for p in /proc/[0-9]*; do \
		[ "$$(readlink "$$p/exe" 2>/dev/null)" = "$$target (deleted)" ] || \
		[ "$$(readlink -f "$$p/exe" 2>/dev/null)" = "$$target" ] || continue; \
		kill "$${p##*/}" 2>/dev/null || true; \
	done; \
	sleep 0.3
	@# strip + upx rewrite the binary in place (several full passes),
	@# so they run on a staging copy in the tmpfs — the SSD dist/ only
	@# ever receives the finished artifact (one sequential write)
	@mkdir -p $(CARGO_TARGET_DIR)/dist
	cp $(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN) $(CARGO_TARGET_DIR)/dist/$(BIN)
	@if [ "$(COMPRESS)" != "0" ]; then \
		strip --strip-all $(CARGO_TARGET_DIR)/dist/$(BIN) 2>/dev/null || true; \
		./tools/upx $(CARGO_TARGET_DIR)/dist/$(BIN) || true; \
	fi
	cp $(CARGO_TARGET_DIR)/dist/$(BIN) $(DIST)/$(BIN)
	@ls -l $(DIST)/$(BIN)
	@file $(DIST)/$(BIN)

size: dist
	@size $(DIST)/$(BIN) 2>/dev/null || true

# Per-crate .text attribution of the release binary. Installs cargo-bloat
# into the tmpfs CARGO_HOME on first use; touch forces a fresh link so the
# analyzed binary is not the stale stripped one from an earlier 'dist'.
bloat: setup-env
	@if ! command -v cargo-bloat >/dev/null 2>&1; then \
		echo "==> Installing cargo-bloat into $(CARGO_HOME)..."; \
		n=1; while [ $$n -le 3 ]; do \
			if cargo +$(TOOLCHAIN) install --locked cargo-bloat; then \
				break; \
			fi; \
			echo "==> Install failed, retrying ($$n/3)..."; \
			n=$$((n + 1)); \
			sleep 2; \
		done; \
		[ $$n -le 3 ] || exit 1; \
	fi
	@find src -name '*.rs' -exec touch {} +
	cargo +$(TOOLCHAIN) bloat --release --target $(TARGET) --crates $(if $(FEATURES),--features $(FEATURES),)

# dependency graph inspection; pass args via ARGS, e.g.
# make tree ARGS="-i exr"  (inverse: what pulls in exr)
tree: setup-env
	cargo +$(TOOLCHAIN) tree --target $(TARGET) $(if $(FEATURES),--features $(FEATURES),) $(ARGS)

fmt: setup-env
	cargo +$(TOOLCHAIN) fmt

clean:
	cargo clean || true
	rm -rf $(TMPROOT) $(DIST)
