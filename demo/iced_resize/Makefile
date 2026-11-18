# Demo Makefile — mirrors the explorer repo's build discipline:
# every artifact (toolchain, registry, target dir, iced overlay) lives in
# the /tmp tmpfs; nothing is written to $HOME.

SHELL := /bin/bash

TMPROOT   ?= /tmp/iced-resize-build
BIN       := iced_resize

TOOLCHAIN ?= stable
TARGET    ?= x86_64-unknown-linux-gnu

export CARGO_TARGET_DIR := $(TMPROOT)/target
export CARGO_HOME := $(TMPROOT)/cargo
export RUSTUP_HOME := $(TMPROOT)/rustup
export CARGO_INCREMENTAL := 0
export PATH := $(TMPROOT)/cargo/bin:$(PATH)

# --- iced overlay ------------------------------------------------------------
# Same pattern as the explorer repo: Cargo.toml's [patch] section always
# points at ./target/iced-overlay (target is a symlink into the tmpfs).
# The overlay is cloned from GitHub at the pinned rev; the patch toggle
# only touches its working tree.
#
#   make patch-iced    apply deps/iced-resize-viewport-sync.patch (fixed)
#   make unpatch-iced  revert the overlay to pristine upstream (buggy)
#
# Builds (make build/run/...) depend only on seed-iced and never change
# the patch state: a freshly seeded overlay is pristine, so a plain
# `make run` reproduces the bug; run `make patch-iced` once to verify
# the fix.
ICED_REV     := e5c350f271a8046f875c6662d452fde9e462d486
ICED_REPO    := https://github.com/iced-rs/iced
DEMO_OVERLAY := $(CARGO_TARGET_DIR)/iced-overlay
DEMO_PATCH   := $(CURDIR)/deps/iced-resize-viewport-sync.patch

.PHONY: all setup-env seed-iced patch-iced unpatch-iced build check run clippy clean

all: build

setup-env:
	@ln -sf $(CARGO_TARGET_DIR) target
	@mkdir -p $(CARGO_HOME) $(RUSTUP_HOME) $(CARGO_TARGET_DIR)

.PHONY: seed-iced
seed-iced: setup-env
	@if [ ! -d $(DEMO_OVERLAY)/.git ]; then \
		echo "==> cloning $(ICED_REPO) @ $(ICED_REV) into the overlay"; \
		git clone -q --no-checkout $(ICED_REPO) $(DEMO_OVERLAY); \
		git -C $(DEMO_OVERLAY) checkout -q $(ICED_REV); \
	fi

.PHONY: patch-iced
patch-iced: seed-iced
	@# apply the patch unless already applied
	@if git -C $(DEMO_OVERLAY) apply --check $(DEMO_PATCH) 2>/dev/null; then \
		git -C $(DEMO_OVERLAY) apply $(DEMO_PATCH); \
		echo "==> applied deps/iced-resize-viewport-sync.patch"; \
	elif git -C $(DEMO_OVERLAY) apply --reverse --check $(DEMO_PATCH) 2>/dev/null; then \
		echo "==> demo iced overlay already patched"; \
	else \
		echo "==> ERROR: patch does not apply to this revision"; \
		echo "==> re-seed the overlay: rm -rf $(DEMO_OVERLAY) && make seed-iced"; \
		exit 1; \
	fi

.PHONY: unpatch-iced
unpatch-iced: seed-iced
	@# revert the overlay to pristine upstream unless already pristine
	@if git -C $(DEMO_OVERLAY) apply --reverse --check $(DEMO_PATCH) 2>/dev/null; then \
		git -C $(DEMO_OVERLAY) apply --reverse $(DEMO_PATCH); \
		echo "==> demo iced overlay reverted to pristine upstream"; \
	elif git -C $(DEMO_OVERLAY) apply --check $(DEMO_PATCH) 2>/dev/null; then \
		echo "==> demo iced overlay is already pristine"; \
	else \
		echo "==> ERROR: overlay state matches neither the patch nor pristine"; \
		echo "==> re-seed it: rm -rf $(DEMO_OVERLAY) && make seed-iced"; \
		exit 1; \
	fi

build: setup-env seed-iced
	cargo +$(TOOLCHAIN) build --release --target $(TARGET)

check: setup-env seed-iced
	cargo +$(TOOLCHAIN) check --target $(TARGET)

run: build
	$(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN)

clippy: setup-env seed-iced
	cargo +$(TOOLCHAIN) clippy --target $(TARGET)

clean:
	rm -rf $(TMPROOT) target
