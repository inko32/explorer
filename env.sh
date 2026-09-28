#!/usr/bin/env bash
# Source this before running cargo / make:
#   source ./env.sh
#
# !!! HARD RULE (owner's directive) !!!
# ALL temporary artifacts go to IN-MEMORY tmpfs ($EXPLORER_TMP under /tmp):
# the cargo registry, AND the `target` build directory. NEVER let a `target/`
# folder appear in this project directory - the SSD carries source code only.
# If you see ./target here, a cargo command ran WITHOUT this file sourced:
# delete it immediately (`rm -rf target`) and redo the command properly.
#
# All build intermediates (cargo registry, target dir) live in a zram-backed
# tmpfs so the SSD only carries source code. Recreated automatically if the
# machine was rebooted.

export EXPLORER_TMP="${EXPLORER_TMP:-/tmp/zramfs_inkodaily/explorer_tmp_077de390}"
mkdir -p "$EXPLORER_TMP/cargo" "$EXPLORER_TMP/target" "$EXPLORER_TMP/rustup"

export CARGO_HOME="$EXPLORER_TMP/cargo"
export CARGO_TARGET_DIR="$EXPLORER_TMP/target"
export RUSTUP_HOME="$EXPLORER_TMP/rustup"

echo "explorer build env: CARGO_HOME=$CARGO_HOME"
echo "                    CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
