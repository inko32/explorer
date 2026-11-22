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
COMPRESS  ?= 0


TOOLCHAIN   ?= stable
TARGET      ?= x86_64-unknown-linux-gnu
# Dependency: sudo pacman -S rustup
# Crate features: TEMPORARY on this branch: no default features (vello-cpu
# software renderer + Wayland only). NO_DEFAULT is now a no-op; add
# FEATURES=wgpu to build the GPU backend.

# --- local iced overlay ---------------------------------------------------
# The Wayland DnD side-channel (src/dnd_wayland.rs) needs the window's raw
# wl_display/wl_surface, which iced does not expose upstream. Instead of
# maintaining a forked tree, we keep a small additive patch
# (deps/iced-raw-handle.patch) and generate the overlay here: upstream iced
# at ICED_REV, patched, living entirely in tmpfs. Cargo.toml's [patch]
# section reaches it through the existing `target/` symlink
# ("target/iced-overlay"), so the source tree carries no generated
# paths at all.
#
# When iced upstream updates: bump ICED_REV, `make setup-iced` re-seeds and
# re-applies the patch. The patch is purely additive, so it should apply
# cleanly; if not, git apply reports the conflict and it has to be
# re-placed by hand (rare; ~30 lines).
ICED_REV     := 98bd48fe011efc0ec5819fee0fa7673a7169ff11
ICED_PATCH   := $(CURDIR)/deps/iced-raw-handle.patch
# (same dir as CARGO_TARGET_DIR; spelled via TMPROOT because this block
# is parsed before the CARGO_TARGET_DIR export below)
ICED_OVERLAY := $(TMPROOT)/target/iced-overlay

# --- local softbuffer overlay ----------------------------------------------
# The translucent Win8.1 border (win81theme.yaml border_opacity < 1) needs
# pixel alpha to reach the Wayland compositor. iced's tiny-skia (software)
# compositor presents through softbuffer, whose Wayland backend hardcodes
# wl_shm Xrgb8888 ("alpha ignored"), so the border composites opaque in
# cpu-render mode. The buffer bytes are already premultiplied-argb-ready
# (iced swaps R/B into tiny-skia; tiny-skia is premultiplied), so
# relabeling the shm buffer Argb8888 is the whole fix — see
# deps/softbuffer-argb.md. Same overlay pattern as iced above, but seeded
# from the crates.io source and idempotency-checked via grep (no git).
SB_VER     := 0.4.8
SB_PATCH   := $(CURDIR)/deps/softbuffer-argb.patch
SB_OVERLAY := $(TMPROOT)/target/softbuffer-overlay

# --- upx (dist compression) -------------------------------------------------
# dist/dist-static compress the staging binary with upx when COMPRESS != 0.
# The binary is fetched once from the pinned GitHub release into tmpfs —
# UPX_VER is hard-coded on purpose: upx output is baked into the shipped
# artifacts, so its version is part of the build inputs. Supersedes the
# previously vendored ./tools/upx (5.2.1).
UPX_VER := 5.3.0
UPX_DIR := $(TMPROOT)/tools/upx
UPX_BIN := $(UPX_DIR)/upx
UPX_URL := https://github.com/upx/upx/releases/download/v$(UPX_VER)/upx-$(UPX_VER)-amd64_linux.tar.xz


export CARGO_TARGET_DIR := $(TMPROOT)/target
export CARGO_INCREMENTAL := 0
export CARGO_HOME=$(TMPROOT)/cargo
export RUSTUP_HOME=$(TMPROOT)/rustup
export PATH := $(TMPROOT)/cargo/bin:$(PATH)

.PHONY: all build check run test dist dist-static clean fmt size setup-toolchain bloat tree clippy clippy-json saferun% static static-sysroot


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
.PHONY: seed-iced
seed-iced:
	@mkdir -p $(ICED_OVERLAY)
	@if [ ! -d $(ICED_OVERLAY)/.git ]; then \
		rev7=$$(echo $(ICED_REV) | cut -c1-7); \
		cache=$$(echo $(CARGO_HOME)/git/checkouts/iced-*/$$rev7); \
		if [ -d "$$cache" ]; then \
			echo "==> seeding iced overlay from cargo cache"; \
			cp -a "$$cache"/. $(ICED_OVERLAY)/; \
		else \
			echo "==> cloning iced $(ICED_REV)"; \
			git clone --no-checkout https://github.com/iced-rs/iced $(ICED_OVERLAY); \
			git -C $(ICED_OVERLAY) checkout -q $(ICED_REV); \
		fi; \
	fi
	@# record the pristine baseline once (clone case has no commit yet)
	@if ! git -C $(ICED_OVERLAY) rev-parse -q --verify HEAD >/dev/null; then \
		git -C $(ICED_OVERLAY) add -A; \
		git -C $(ICED_OVERLAY) -c user.email=local@explorer -c user.name=explorer \
			commit -qm "baseline: iced @ $(ICED_REV) (upstream)"; \
	fi

# --- manual patch toggle ----------------------------------------------------
# Cargo.toml's [patch] section always points at the overlay, so switching
# between patched/pristine only touches the overlay's working tree — no
# Cargo.toml edits, no dependency re-resolution, and cargo rebuilds the
# overlay crates on the next build either way.
#   make patch-iced    apply deps/iced-raw-handle.patch (explorer builds:
#                      raw Wayland handles, tiny-skia quad snap, and the
#                      window::resize viewport sync)
#   make unpatch-iced  revert the overlay to pristine upstream iced (used
#                      to reproduce upstream bugs, e.g. demo/iced_resize)
.PHONY: patch-iced
patch-iced: seed-iced
	@# apply the patch unless already applied
	@if git -C $(ICED_OVERLAY) apply --check $(ICED_PATCH) 2>/dev/null; then \
		git -C $(ICED_OVERLAY) apply $(ICED_PATCH); \
		echo "==> applied deps/iced-raw-handle.patch"; \
	elif git -C $(ICED_OVERLAY) apply --reverse --check $(ICED_PATCH) 2>/dev/null; then \
		echo "==> iced overlay already patched"; \
	else \
		echo "==> ERROR: iced patch does not apply to this revision"; \
		echo "==> bump ICED_REV or refresh deps/iced-raw-handle.patch"; \
		exit 1; \
	fi

.PHONY: unpatch-iced
unpatch-iced: seed-iced
	@# revert the overlay to pristine upstream unless already pristine
	@if git -C $(ICED_OVERLAY) apply --reverse --check $(ICED_PATCH) 2>/dev/null; then \
		git -C $(ICED_OVERLAY) apply --reverse $(ICED_PATCH); \
		echo "==> iced overlay reverted to pristine upstream"; \
	elif git -C $(ICED_OVERLAY) apply --check $(ICED_PATCH) 2>/dev/null; then \
		echo "==> iced overlay is already pristine"; \
	else \
		echo "==> ERROR: overlay state matches neither the patch nor pristine"; \
		echo "==> re-seed it: rm -rf $(ICED_OVERLAY) && make seed-iced"; \
		exit 1; \
	fi

.PHONY: setup-iced
setup-iced: patch-iced

.PHONY: setup-softbuffer
setup-softbuffer:
	@if [ ! -d $(SB_OVERLAY)/src ]; then \
		cache=$$(echo $(CARGO_HOME)/registry/src/*/softbuffer-$(SB_VER)); \
		if [ -d "$$cache" ]; then \
			echo "==> seeding softbuffer overlay from cargo cache"; \
			mkdir -p $(SB_OVERLAY); \
			cp -a "$$cache"/. $(SB_OVERLAY)/; \
		else \
			echo "==> softbuffer $(SB_VER) not in cargo cache; downloading from crates.io"; \
			mkdir -p $(SB_OVERLAY) $(TMPROOT)/dl; \
			curl -fL --retry 3 -o $(TMPROOT)/dl/softbuffer.crate \
				https://static.crates.io/crates/softbuffer/softbuffer-$(SB_VER).crate && \
			tar -xzf $(TMPROOT)/dl/softbuffer.crate -C $(SB_OVERLAY) --strip-components=1; \
		fi; \
	fi
	@chmod -R u+w $(SB_OVERLAY)
	@if grep -q 'wl_shm::Format::Argb8888' $(SB_OVERLAY)/src/backends/wayland/buffer.rs 2>/dev/null; then \
		echo "==> softbuffer overlay already patched"; \
	elif git -C $(SB_OVERLAY) apply --check $(SB_PATCH) 2>/dev/null; then \
		git -C $(SB_OVERLAY) apply $(SB_PATCH); \
		echo "==> applied deps/softbuffer-argb.patch"; \
	else \
		echo "==> ERROR: softbuffer patch does not apply to $(SB_VER)"; \
		echo "==> bump SB_VER or refresh deps/softbuffer-argb.patch"; \
		exit 1; \
	fi

.PHONY: setup-upx
setup-upx:
	@if [ ! -x $(UPX_BIN) ]; then \
		echo "==> downloading upx $(UPX_VER)"; \
		mkdir -p $(UPX_DIR) $(TMPROOT)/dl; \
		curl -fL --retry 3 -o $(TMPROOT)/dl/upx.tar.xz $(UPX_URL) && \
		tar -xJf $(TMPROOT)/dl/upx.tar.xz -C $(UPX_DIR) --strip-components=1; \
	fi

# --- plain-cargo support ----------------------------------------------------
# Cargo.toml's [patch] sections resolve the iced/softbuffer overlays through
# the repo-root `target/` symlink, so once the overlays exist a bare
# `cargo build` works in the user's OWN environment — no toolchain install,
# no CARGO_HOME/RUSTUP_HOME redirection:
#
#   make setup-overlays    # one-time: seed + patch overlays, create target/
#   cargo build --release  # artifacts land in tmpfs through the symlink
#
# `make setup-env` extends this with the tmpfs toolchain for the make flow.
.PHONY: setup-overlays
setup-overlays: patch-iced setup-softbuffer
	@mkdir -p $(CARGO_TARGET_DIR)
	@if [ -e target ] && [ ! -L target ]; then \
		echo "==> ERROR: ./target is a real directory;"; \
		echo "==> move it away before letting make manage the symlink"; \
		exit 1; \
	fi
	@ln -sfn $(CARGO_TARGET_DIR) target
	@echo "==> overlays ready under $(TMPROOT); plain 'cargo build' will pick them up"

setup-env: setup-overlays
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

# --- mostly-static build (branch `static`) --------------------------------
# Goal: the only dynamic deps left are the glibc runtime (libc/libm/
# libgcc_s/ld-linux) — which the dlopened glibc 7z.so plugin requires
# anyway. The whole glib stack (gio/glib/gobject/gmodule + pcre2/ffi/z/
# mount/blkid) links statically against deps/sysroot.
# musl is NOT usable here: a static musl binary cannot dlopen at all, and
# two libcs (musl exe + glibc plugin) clash on the x86-64 TLS layout.
SYSROOT   ?= $(TMPROOT)/sysroot
STATIC_TARGET_DIR := $(CARGO_TARGET_DIR)-static

# gtk-rs -sys probes fed manually (no pkg-config): full ordered archive
# chain in gio's LIB so static link order is correct by construction.
STATIC_DEPS_ENV := \
	SYSTEM_DEPS_LINK=static \
	SYSTEM_DEPS_GIO_2_0_NO_PKG_CONFIG=1 \
	SYSTEM_DEPS_GIO_2_0_LIB="gio-2.0 gobject-2.0 gmodule-2.0 glib-2.0 ffi pcre2-8 z mount blkid sysprof-stub" \
	SYSTEM_DEPS_GLIB_2_0_NO_PKG_CONFIG=1 \
	SYSTEM_DEPS_GLIB_2_0_LIB="glib-2.0" \
	SYSTEM_DEPS_GOBJECT_2_0_NO_PKG_CONFIG=1 \
	SYSTEM_DEPS_GOBJECT_2_0_LIB="gobject-2.0" \
	SYSTEM_DEPS_GMODULE_2_0_NO_PKG_CONFIG=1 \
	SYSTEM_DEPS_GMODULE_2_0_LIB="gmodule-2.0"

static-sysroot:
	bash deps/build-sysroot.sh "$(SYSROOT)"

static: setup-env static-sysroot
	$(STATIC_DEPS_ENV) \
	RUSTFLAGS="-L native=$(SYSROOT)/lib -L native=/usr/lib" \
	CARGO_TARGET_DIR="$(STATIC_TARGET_DIR)" \
	cargo +$(TOOLCHAIN) build --release --target $(TARGET) $(if $(NO_DEFAULT),--no-default-features,) $(if $(FEATURES),--features $(FEATURES),)
	@$(STATIC_TARGET_DIR)/$(TARGET)/release/$(BIN) --version >/dev/null 2>&1 || true
	@echo "==> ldd of the mostly-static binary:"
	@ldd $(STATIC_TARGET_DIR)/$(TARGET)/release/$(BIN) || true

# dist variant of the mostly-static build; mirrors `dist` but takes the
# binary from target-static (same strip/upx staging discipline).
dist-static: static setup-upx
	@mkdir -p $(DIST)
	@target="$$(realpath $(DIST)/$(BIN))"; \
	for p in /proc/[0-9]*; do \
		[ "$$(readlink "$${p}/exe" 2>/dev/null)" = "$$target (deleted)" ] || \
		[ "$$(readlink -f "$${p}/exe" 2>/dev/null)" = "$$target" ] || continue; \
		kill "$${p##*/}" 2>/dev/null || true; \
	done; \
	sleep 0.3
	@mkdir -p $(STATIC_TARGET_DIR)/dist
	cp $(STATIC_TARGET_DIR)/$(TARGET)/release/$(BIN) $(STATIC_TARGET_DIR)/dist/$(BIN)
	@if [ "$(COMPRESS)" != "0" ]; then \
		strip --strip-all $(STATIC_TARGET_DIR)/dist/$(BIN) 2>/dev/null || true; \
		$(UPX_BIN) $(STATIC_TARGET_DIR)/dist/$(BIN) || true; \
	fi
	cp $(STATIC_TARGET_DIR)/dist/$(BIN) $(DIST)/$(BIN)
	@ls -l $(DIST)/$(BIN)

build: setup-env
	cargo +$(TOOLCHAIN) build --release --target $(TARGET) $(if $(NO_DEFAULT),--no-default-features,) $(if $(FEATURES),--features $(FEATURES),)

check: setup-env
	cargo +$(TOOLCHAIN) check --target $(TARGET) $(if $(NO_DEFAULT),--no-default-features,) $(if $(FEATURES),--features $(FEATURES),)

run: build
	$(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN) $(ARGS)

# --- Windows cross-build + Wine (targets below are additive; the Linux
# --- build above is untouched) ---------------------------------------------
# Same toolchain as demo/wine_port: x86_64-pc-windows-gnu via a tmpfs-local
# mingw-w64 (fetched as Arch package tarballs, no root), run with a wine
# prefix that also lives in the tmpfs. `run-wine`/`dist-wine` copy host
# fonts into the prefix (fontdb scans %SYSTEMROOT%\Fonts, which wine
# leaves empty — without it no text renders) and, when present, a real
# 7z.dll next to the exe (the archive backend loads "7z.dll" through
# LoadLibrary's search order).
TARGET_WINE := x86_64-pc-windows-gnu
MINGW_ROOT  := $(TMPROOT)/mingw
WINEPREFIX  := $(TMPROOT)/wine-prefix
HOST_FONTS  ?= /usr/share/fonts/liberation
FONT_MARKER := $(WINEPREFIX)/drive_c/windows/Fonts/.copied-from-host
WINE_EXE    := $(CARGO_TARGET_DIR)/$(TARGET_WINE)/release/$(BIN).exe

MINGW_MIRROR := https://mirror.rackspace.com/archlinux/extra/os/x86_64
MINGW_PKGS   := \
	mingw-w64-binutils-2.47-1-x86_64 \
	mingw-w64-headers-14.0.0-1-any \
	mingw-w64-crt-14.0.0-1-any \
	mingw-w64-winpthreads-14.0.0-1-any \
	mingw-w64-gcc-16.2.0-2-x86_64

# Windows stdlib for the tmpfs toolchain (idempotent; same retry
# discipline as setup-env).
.PHONY: setup-wine-toolchain
setup-wine-toolchain: setup-env
	@if ! rustup +$(TOOLCHAIN) target list --installed 2>/dev/null | grep -q "$(TARGET_WINE)"; then \
		echo "==> Adding target '$(TARGET_WINE)' for $(TOOLCHAIN)..."; \
		n=1; while [ $$n -le 3 ]; do \
			if rustup +$(TOOLCHAIN) target add $(TARGET_WINE); then \
				break; \
			fi; \
			echo "==> Download failed, retrying ($$n/3)..."; \
			n=$$((n + 1)); \
			sleep 2; \
		done; \
		[ $$n -le 3 ] || exit 1; \
	fi

# mingw-w64 gcc/binutils/CRT, unpacked from Arch packages into the
# tmpfs. No root: curl the .pkg.tar.zst, tar -xf into $(MINGW_ROOT).
# One fetch per tmpfs lifetime (needs network).
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

# Populate C:\Windows\Fonts in the (possibly not yet created) wine
# prefix. Creating the dir before wineboot is fine — wineboot only adds
# files. Without this the app finds zero fonts and renders no text.
# Wine never copies its OWN bundled fonts into the prefix (it keeps
# them behind registry entries pointing at Z: paths, which the app's
# font stack — fontdb — does not read), so copy both wine's bundled
# TTFs (Tahoma is Wine's default UI font) and a host font set; the
# fonts.yaml fallback list names Tahoma/Arial first among the always
# available ones.
WINE_FONTS ?= /usr/share/wine/fonts
.PHONY: setup-fonts
setup-fonts:
	@if [ ! -f $(FONT_MARKER) ]; then \
		mkdir -p $(WINEPREFIX)/drive_c/windows/Fonts && \
		[ -d $(WINE_FONTS) ] && cp $(WINE_FONTS)/*.ttf $(WINEPREFIX)/drive_c/windows/Fonts/ || true; \
		cp $(HOST_FONTS)/*.tt[fc] $(WINEPREFIX)/drive_c/windows/Fonts/ && \
		touch $(FONT_MARKER); \
	fi

# --- 7z.dll for the Windows build -------------------------------------------
# The archive backend dlopens "7z.dll" (LoadLibrary search: exe dir
# first), so the build fetches one from the pinned 7-Zip-zstd installer
# (a 7z self-extracting archive; version+URL hardcoded on purpose — the
# dll is a build input). Extraction: `7z x` when the CLI is available,
# else a silent (/S) install into the tmpfs wine prefix. Any failure
# only warns — archives still open via the tar/CLI backends.
7Z_VER   := 26.05-v1.5.7-R1
7Z_EXE   := 7z26.05-zstd-x64.exe
7Z_URL   := https://github.com/mcmilk/7-Zip-zstd/releases/download/v$(7Z_VER)/$(7Z_EXE)
7Z_STAGE := $(TMPROOT)/7z-installer
# Last-resort fallback: a locally available dll (e.g. _real_win10).
7Z_DLL   ?= $(CURDIR)/_real_win10/7z.dll

.PHONY: setup-7z-dll
setup-7z-dll: build-wine
	@if [ ! -f $(CARGO_TARGET_DIR)/$(TARGET_WINE)/release/7z.dll ]; then \
		mkdir -p $(7Z_STAGE); \
		if [ ! -f $(7Z_STAGE)/$(7Z_EXE) ]; then \
			echo "==> downloading $(7Z_URL)"; \
			curl -fsSL --retry 3 -o $(7Z_STAGE)/$(7Z_EXE) $(7Z_URL) || true; \
		fi; \
		if [ -f $(7Z_STAGE)/$(7Z_EXE) ]; then \
			if command -v 7z >/dev/null 2>&1; then \
				7z x -y -o$(7Z_STAGE)/x $(7Z_STAGE)/$(7Z_EXE) >/dev/null 2>&1 || true; \
			fi; \
			if [ ! -f $(7Z_STAGE)/x/7z.dll ]; then \
				echo "==> 7z CLI failed; silent-installing into the wine prefix"; \
				WINEPREFIX=$(WINEPREFIX) wine $(7Z_STAGE)/$(7Z_EXE) /S >/dev/null 2>&1 || true; \
				dll=$$(find $(WINEPREFIX)/drive_c -name 7z.dll 2>/dev/null | head -1); \
				[ -n "$$dll" ] && cp "$$dll" $(7Z_STAGE)/x/7z.dll; \
			fi; \
			if [ -f $(7Z_STAGE)/x/7z.dll ]; then \
				cp $(7Z_STAGE)/x/7z.dll $(CARGO_TARGET_DIR)/$(TARGET_WINE)/release/7z.dll; \
				echo "==> 7z.dll from $(7Z_EXE) ($(7Z_VER)) ready"; \
			fi; \
		fi; \
		if [ ! -f $(CARGO_TARGET_DIR)/$(TARGET_WINE)/release/7z.dll ] && [ -f "$(7Z_DLL)" ]; then \
			cp "$(7Z_DLL)" $(CARGO_TARGET_DIR)/$(TARGET_WINE)/release/7z.dll; \
		echo "==> fell back to $(7Z_DLL)"; \
		fi; \
		if [ ! -f $(CARGO_TARGET_DIR)/$(TARGET_WINE)/release/7z.dll ]; then \
			echo "==> WARNING: no 7z.dll — archives via tar/CLI only"; \
		fi; \
	fi

# Feature set for the wine builds. dist-wine embeds Selawik (SIL OFL,
# free to ship) so the distributed exe renders text in ANY wine prefix
# — a double-clicked binary runs in the user's default prefix, whose
# C:\Windows\Fonts is usually empty (wine only registers fonts in the
# registry, which the app's font stack never reads). run-wine keeps the
# feature off so the local test path exercises the real font fallback.
WINE_FEATURES ?= $(FEATURES)

check-wine: setup-wine-toolchain setup-mingw setup-overlays
	PATH="$(MINGW_ROOT)/usr/bin:$(PATH)" cargo +$(TOOLCHAIN) check --target $(TARGET_WINE) $(if $(NO_DEFAULT),--no-default-features,) $(if $(WINE_FEATURES),--features $(WINE_FEATURES),)

build-wine: setup-wine-toolchain setup-mingw setup-overlays
	PATH="$(MINGW_ROOT)/usr/bin:$(PATH)" cargo +$(TOOLCHAIN) build --release --target $(TARGET_WINE) $(if $(NO_DEFAULT),--no-default-features,) $(if $(WINE_FEATURES),--features $(WINE_FEATURES),)

run-wine: build-wine setup-fonts setup-7z-dll
	WINEPREFIX=$(WINEPREFIX) wine $(WINE_EXE) $(ARGS)

# Windows sibling of `dist`: the release PE plus 7z.dll (when present),
# optionally upx-compressed with the same COMPRESS discipline. Builds
# with embed-selawik (see WINE_FEATURES) unless FEATURES is given.
dist-wine: WINE_FEATURES = $(or $(FEATURES),embed-selawik)
dist-wine: build-wine setup-7z-dll setup-upx
	@mkdir -p $(DIST)/wine
	@cp $(WINE_EXE) $(DIST)/wine/$(BIN).exe
	@if [ -f $(CARGO_TARGET_DIR)/$(TARGET_WINE)/release/7z.dll ]; then \
		cp $(CARGO_TARGET_DIR)/$(TARGET_WINE)/release/7z.dll $(DIST)/wine/; \
	fi
	@if [ "$(COMPRESS)" != "0" ]; then \
		strip --strip-all $(DIST)/wine/$(BIN).exe 2>/dev/null || true; \
		$(UPX_BIN) $(DIST)/wine/$(BIN).exe || true; \
	fi
	@ls -l $(DIST)/wine/

test: setup-env
	cargo +$(TOOLCHAIN) test --target $(TARGET) $(if $(NO_DEFAULT),--no-default-features,) $(if $(FEATURES),--features $(FEATURES),)

clippy: setup-env
	$(CLIPPY_ENSURE)
	cargo +$(TOOLCHAIN) clippy --target $(TARGET) $(if $(NO_DEFAULT),--no-default-features,) $(if $(FEATURES),--features $(FEATURES),)

# machine-readable clippy (one JSON diagnostic per line) — for tooling
# that sorts/filters the warning list.
clippy-json: setup-env
	$(CLIPPY_ENSURE)
	@# grep exits 1 on zero matches (clean run) — not an error here
	-cargo +$(TOOLCHAIN) clippy --target $(TARGET) --message-format=json $(if $(NO_DEFAULT),--no-default-features,) $(if $(FEATURES),--features $(FEATURES),) 2>/dev/null | grep '"reason":"compiler-message"' > /tmp/clippy.jsonl || true
	@echo "wrote /tmp/clippy.jsonl"

dist: build setup-upx
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
		$(UPX_BIN) $(CARGO_TARGET_DIR)/dist/$(BIN) || true; \
	fi
	cp $(CARGO_TARGET_DIR)/dist/$(BIN) $(DIST)/$(BIN)
	@ls -l $(DIST)/$(BIN)
	@file $(DIST)/$(BIN)

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
	cargo +$(TOOLCHAIN) bloat --release --target $(TARGET) --crates $(if $(NO_DEFAULT),--no-default-features,) $(if $(FEATURES),--features $(FEATURES),)

# dependency graph inspection; pass args via ARGS, e.g.
# make tree ARGS="-i exr"  (inverse: what pulls in exr)
tree: setup-env
	cargo +$(TOOLCHAIN) tree --target $(TARGET) $(if $(NO_DEFAULT),--no-default-features,) $(if $(FEATURES),--features $(FEATURES),) $(ARGS)

fmt: setup-env
	cargo +$(TOOLCHAIN) fmt

clean:
	cargo clean || true
	rm -rf $(TMPROOT) $(DIST)

saferun%:
	$(eval ARGS := $(subst :, ,$(patsubst saferun%,%,$@)))
	bwrap	--unshare-all \
		--dev /dev \
		--proc /proc \
		--ro-bind /usr /usr \
		--ro-bind /lib64 /lib64 \
		--symlink usr/bin /bin \
		--symlink usr/lib /lib \
		--bind /tmp /tmp \
		--dev-bind /dev/dri /dev/dri \
		--ro-bind "$$XDG_RUNTIME_DIR/$$WAYLAND_DISPLAY" "$$XDG_RUNTIME_DIR/$$WAYLAND_DISPLAY" \
		--setenv XDG_RUNTIME_DIR "$$XDG_RUNTIME_DIR" \
		--setenv WAYLAND_DISPLAY "$$WAYLAND_DISPLAY" \
		--bind ./dist /tmp/dist \
		-- /tmp/dist/explorer "$(ARGS)"
