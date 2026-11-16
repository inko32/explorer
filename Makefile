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
ICED_REV     := b2fbd3cb6289603f8cbbb2387a5242231f509d13
ICED_PATCH   := $(CURDIR)/deps/iced-raw-handle.patch
# (same dir as CARGO_TARGET_DIR; spelled via TMPROOT because this block
# is parsed before the CARGO_TARGET_DIR export below)
ICED_OVERLAY := $(TMPROOT)/target/iced-overlay


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
.PHONY: setup-iced
setup-iced:
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

setup-env: setup-iced
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
	cargo +$(TOOLCHAIN) build --release --target $(TARGET) $(if $(FEATURES),--features $(FEATURES),)
	@$(STATIC_TARGET_DIR)/$(TARGET)/release/$(BIN) --version >/dev/null 2>&1 || true
	@echo "==> ldd of the mostly-static binary:"
	@ldd $(STATIC_TARGET_DIR)/$(TARGET)/release/$(BIN) || true

# dist variant of the mostly-static build; mirrors `dist` but takes the
# binary from target-static (same strip/upx staging discipline).
dist-static: static
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
		./tools/upx $(STATIC_TARGET_DIR)/dist/$(BIN) || true; \
	fi
	cp $(STATIC_TARGET_DIR)/dist/$(BIN) $(DIST)/$(BIN)
	@ls -l $(DIST)/$(BIN)

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

	@ls -l $(DIST)/$(BIN)
	@file $(DIST)/$(BIN)

# CPU-renderer dist: builds with the `cpu-render` cargo feature so the
# binary defaults to tiny-skia instead of wgpu. Same staging discipline
# as `dist`, shipped as explorer-cpu so it never clobbers the
# GPU-default dist/explorer.
dist-cpu: FEATURES := cpu-render
dist-cpu: build
	@mkdir -p $(DIST)
	@target="$$(realpath $(DIST)/explorer-cpu)"; \
	for p in /proc/[0-9]*; do \
		[ "$$(readlink "$${p}/exe" 2>/dev/null)" = "$$target (deleted)" ] || \
		[ "$$(readlink -f "$${p}/exe" 2>/dev/null)" = "$$target" ] || continue; \
		kill "$${p##*/}" 2>/dev/null || true; \
	done; \
	sleep 0.3
	@mkdir -p $(CARGO_TARGET_DIR)/dist
	cp $(CARGO_TARGET_DIR)/$(TARGET)/release/$(BIN) $(CARGO_TARGET_DIR)/dist/explorer-cpu
	@if [ "$(COMPRESS)" != "0" ]; then \
		strip --strip-all $(CARGO_TARGET_DIR)/dist/explorer-cpu 2>/dev/null || true; \
		./tools/upx $(CARGO_TARGET_DIR)/dist/explorer-cpu || true; \
	fi
	cp $(CARGO_TARGET_DIR)/dist/explorer-cpu $(DIST)/explorer-cpu
	@ls -l $(DIST)/explorer-cpu
	@file $(DIST)/explorer-cpu

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
