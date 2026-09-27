#!/usr/bin/env bash
# Source this before running cargo / make:
#   source ./env.sh
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
