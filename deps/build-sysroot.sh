#!/bin/bash
# Build the static system libraries the explorer link needs that Arch does
# not ship as .a: libffi, zlib, libmount, libblkid (the last two without
# systemd, so the -lsystemd/-lsysprof tail of `pkg-config --static` is gone).
#
# Artifacts land in $1 (default $TMPROOT/sysroot); everything stays in
# tmpfs per the Makefile HARD RULE. Idempotent: skips on the stamp file.
set -euo pipefail

SYSROOT="${1:-${TMPROOT:-/tmp/zramfs_inkodaily/explorer-rs-build}/sysroot}"
DISTFILES="$(dirname "$SYSROOT")/distfiles"
STAMP="$SYSROOT/.stamp"
JOBS="$(nproc)"

ZLIB_VER=1.3.1
FFI_VER=3.4.6
UTILLINUX_VER=2.40.4

mkdir -p "$SYSROOT" "$DISTFILES"

# Stub for the sysprof-capture symbols Arch's static libglib-2.0.a
# references (see deps/sysprof-stub.c). Rebuilt every run; cheap, and
# lives outside the stamp because it was added after sysroots existed.
echo "==> building sysprof stub"
gcc -O2 -c "$(dirname "$0")/sysprof-stub.c" -o "$DISTFILES/sysprof-stub.o"
ar rcs "$SYSROOT/lib/libsysprof-stub.a" "$DISTFILES/sysprof-stub.o"

if [ -f "$STAMP" ]; then
    echo "==> sysroot already built ($STAMP), nothing to do"
    exit 0
fi

fetch() { # fetch <url> <tarball-name>
    local url="$1" out="$DISTFILES/$2"
    if [ ! -f "$out" ]; then
        echo "==> downloading $2"
        curl -fL --retry 3 -o "$out" "$url"
    fi
}

build_zlib() {
    fetch "https://zlib.net/fossils/zlib-$ZLIB_VER.tar.gz" "zlib-$ZLIB_VER.tar.gz"
    echo "==> building zlib $ZLIB_VER (static)"
    rm -rf "$DISTFILES/zlib-$ZLIB_VER"
    tar -xzf "$DISTFILES/zlib-$ZLIB_VER.tar.gz" -C "$DISTFILES"
    ( cd "$DISTFILES/zlib-$ZLIB_VER"
      ./configure --static --prefix="$SYSROOT"
      make -j"$JOBS" libz.a
      make install )
}

build_libffi() {
    fetch "https://github.com/libffi/libffi/releases/download/v$FFI_VER/libffi-$FFI_VER.tar.gz" "libffi-$FFI_VER.tar.gz"
    echo "==> building libffi $FFI_VER (static)"
    rm -rf "$DISTFILES/libffi-$FFI_VER"
    tar -xzf "$DISTFILES/libffi-$FFI_VER.tar.gz" -C "$DISTFILES"
    ( cd "$DISTFILES/libffi-$FFI_VER"
      ./configure --prefix="$SYSROOT" --enable-static --disable-shared \
          --disable-docs
      make -j"$JOBS"
      make install )
}

build_libmount() {
    fetch "https://www.kernel.org/pub/linux/utils/util-linux/v${UTILLINUX_VER%.*}/util-linux-$UTILLINUX_VER.tar.xz" "util-linux-$UTILLINUX_VER.tar.xz"
    echo "==> building util-linux $UTILLINUX_VER (libmount+libblkid, static, no systemd)"
    rm -rf "$DISTFILES/util-linux-$UTILLINUX_VER"
    tar -xJf "$DISTFILES/util-linux-$UTILLINUX_VER.tar.xz" -C "$DISTFILES"
    ( cd "$DISTFILES/util-linux-$UTILLINUX_VER"
      ./configure --prefix="$SYSROOT" \
          --disable-all-programs \
          --enable-libmount --enable-libblkid \
          --enable-static --disable-shared \
          --disable-nls --disable-bash-completion --disable-dependency-tracking \
          --without-systemd --without-udev --without-libeconf --without-libz \
          --without-tinfo --without-readline
      make -j"$JOBS" libmount.la libblkid.la
      make install-strip ) || { make -j"$JOBS" && make install; }
}

build_zlib
build_libffi
build_libmount

touch "$STAMP"
echo "==> sysroot ready: $SYSROOT"
ls -la "$SYSROOT/lib" | grep -E "\.a$"
