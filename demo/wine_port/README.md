# wine_port — iced_resize cross-built for Windows, run under Wine

Port of `demo/iced_resize` to `x86_64-pc-windows-gnu` (PE binary via a
tmpfs-local mingw-w64 toolchain), run with Wine. Feasibility test for
shipping the explorer app as a Windows binary.

Everything (rust toolchain, cargo registry, mingw, wine prefix) lives in
`/tmp/wine-port-build`; nothing is written to `$HOME`.

## Run

```sh
make run
```

First run bootstraps, all into the tmpfs (needs network once per tmpfs
lifetime):

1. `setup-toolchain` — rustup `stable` + the `x86_64-pc-windows-gnu` stdlib
   (into `RUSTUP_HOME=$(TMPROOT)/rustup`).
2. `setup-mingw` — mingw-w64 gcc/binutils/CRT fetched as Arch package
   tarballs with curl (no root) and extracted into `$(TMPROOT)/mingw`.
3. `seed-iced` — iced overlay clone (same rev/patch toggle as
   `demo/iced_resize`).
4. `setup-fonts` — copies host TTFs (`/usr/share/fonts/liberation`,
   override with `HOST_FONTS=...`) into the wine prefix's
   `C:\Windows\Fonts`.

## Why the fonts step is required

iced's text stack (cosmic-text → fontdb) loads system fonts on Windows
by scanning `%SYSTEMROOT%\Fonts` *directly*. Wine's prefix has no real
font files there — wine instead syncs its font list into the registry,
pointing at host paths via fontconfig, which fontdb never reads. Without
real files in `C:\Windows\Fonts` the app finds zero fonts and renders
no text at all (verified with a fontdb probe binary: 0 faces before the
copy, 12 after).

## Expected

Same behavior as a native Windows build: after ~2 s the window resizes
300x150 → 300x300 and the readout flips to
`OK: iced sees the new size 300x300` — the Wayland-only stale-viewport
bug this demo reproduces does not apply (on Windows winit applies the
resize asynchronously and a `Resized` event arrives), so both pristine
and `make patch-iced` overlays show OK. stderr:

```
[repro] sending window::resize -> 300x300 (window::size believes: Size { width: 300.0, height: 150.0 })
[repro] window::size now reports: Size { width: 300.0, height: 300.0 }
```

## Notes

- Software rendering only (`tiny-skia`); no GPU/DXVK needed under Wine.
  The `wayland` winit feature is dropped — irrelevant on Windows and it
  would pull Linux-only deps into the cross build.
- `make patch-iced` / `make unpatch-iced` work as in `demo/iced_resize`.
- If the tmpfs was wiped (reboot), the next `make run` re-seeds
  everything from the network.
