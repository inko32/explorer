# Demo Makefile — mirrors the explorer repo's build discipline:
# every artifact (toolchain, registry, target dir, iced overlay, mingw
# toolchain, wine prefix) lives in the /tmp tmpfs; nothing is written
# to $HOME.

SHELL := /bin/bash

TMPROOT   ?= /tmp/wine-port-build
BIN       := wine_port

TOOLCHAIN ?= stable
# Cross target: Windows PE via mingw-w64, run under Wine.
TARGET    ?= x86_64-pc-windows-gnu

export CARGO_TARGET_DIR := $(TMPROOT)/target
export CARGO_HOME := $(TMPROOT)/cargo
export RUSTUP_HOME := $(TMPROOT)/rustup
export CARGO_INCREMENTAL := 0
# The mingw-w64 toolchain is fetched as Arch package tarballs (no root
# needed) and extracted into the tmpfs — see setup-mingw below.
MINGW_ROOT := $(TMPROOT)/mingw
export PATH := $(MINGW_ROOT)/usr/bin:$(TMPROOT)/cargo/bin:$(PATH)

# Keep Wine's prefix out of $HOME too.
WINEPREFIX := $(TMPROOT)/wine-prefix

# --- fonts -------------------------------------------------------------------
# iced's text stack (cosmic-text -> fontdb) loads *system* fonts on Windows
# by scanning %SYSTEMROOT%\Fonts directly. Wine's prefix has no real font
# files there (wine syncs its font list into the registry, pointing at host
# paths — which fontdb never reads), so without this the app renders no
# text at all. Copy a minimal host font set into the prefix once.
# Override HOST_FONTS to point at a bigger collection.
HOST_FONTS  ?= /usr/share/fonts/liberation
FONT_MARKER := $(WINEPREFIX)/drive_c/windows/Fonts/.copied-from-host

# --- mingw-w64 toolchain (linker for x86_64-pc-windows-gnu) ------------------
# Same discipline as seed-iced: fetch into the tmpfs once per tmpfs
# lifetime (needs network), extract, reuse after that. The pinned
# versions are part of the build inputs; bump them together with a
# lockfile refresh.
MINGW_MIRROR := https://mirror.rackspace.com/archlinux/extra/os/x86_64
MINGW_PKGS   := \
	mingw-w64-binutils-2.47-1-x86_64 \
	mingw-w64-headers-14.0.0-1-any \
	mingw-w64-crt-14.0.0-1-any \
	mingw-w64-winpthreads-14.0.0-1-any \
	mingw-w64-gcc-16.2.0-2-x86_64

.PHONY: all setup-env setup-toolchain setup-mingw setup-fonts seed-iced patch-iced unpatch-iced build check run clippy clean

all: build

setup-env:
	@ln -sf $(CARGO_TARGET_DIR) target
	@mkdir -p $(CARGO_HOME) $(RUSTUP_HOME) $(CARGO_TARGET_DIR)

# Rust stdlib for the cross target, installed into the tmpfs rustup home
# (nothing touches the system rustup or $HOME).
.PHONY: setup-toolchain
setup-toolchain: setup-env
	@if ! rustup toolchain list --no-self-update 2>/dev/null | grep -q "$(TOOLCHAIN)"; then \
		echo "==> Installing Rust toolchain '$(TOOLCHAIN)' into $(TMPROOT)..."; \
		rustup toolchain install $(TOOLCHAIN) --profile minimal --no-self-update; \
	fi
	@if ! rustup +$(TOOLCHAIN) target list --installed 2>/dev/null | grep -q "$(TARGET)"; then \
		echo "==> Adding target '$(TARGET)' for $(TOOLCHAIN)..."; \
		rustup +$(TOOLCHAIN) target add $(TARGET); \
	fi

# mingw-w64-gcc + binutils + CRT, unpacked from Arch packages into the
# tmpfs. No root: curl the .pkg.tar.zst, tar -xf into $(MINGW_ROOT).
.PHONY: setup-mingw
setup-mingw:
	@if [ ! -x $(MINGW_ROOT)/usr/bin/x86_64-w64-mingw32-gcc ]; then \
		mkdir -p $(MINGW_ROOT) $(TMPROOT)/pkgs; \
		for p in $(MINGW_PKGS); do \
			if [ ! -f $(TMPROOT)/pkgs/$$p.pkg.tar.zst ]; then \
				echo "==> downloading $$p"; \
				curl -fsSL -o $(TMPROOT)/pkgs/$$p.pkg.tar.zst $(MINGW_MIRROR)/$$p.pkg.tar.zst; \
			fi; \
			echo "==> extracting $$p"; \
			tar -xf $(TMPROOT)/pkgs/$$p.pkg.tar.zst -C $(MINGW_ROOT); \
		done; \
	fi

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

build: setup-toolchain setup-mingw seed-iced
	cargo +$(TOOLCHAIN) build --release --target $(TARGET)

check: setup-toolchain setup-mingw seed-iced
	cargo +$(TOOLCHAIN) check --target $(TARGET)

# Populate C:\Windows\Fonts in the (possibly not yet created) prefix.
# Creating the dir before wineboot is fine — wineboot only adds files.
setup-fonts:
	@if [ ! -f $(FONT_MARKER) ]; then \
		mkdir -p $(WINEPREFIX)/drive_c/windows/Fonts && \
		cp $(HOST_FONTS)/*.tt[fc] $(WINEPREFIX)/drive_c/windows/Fonts/ && \
		touch $(FONT_MARKER); \
	fi

# Wine runs the PE binary; its prefix lives in the tmpfs, not ~/.wine.
run: build setup-fonts
	WINEPREFIX=$(WINEPREFIX) wine $(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN).exe

clippy: setup-toolchain setup-mingw seed-iced
	cargo +$(TOOLCHAIN) clippy --target $(TARGET)

clean:
	rm -rf $(TMPROOT) target
