# explorer

A Windows 8 style file explorer for POSIX and Windows — POSIX/Wayland first.
Early skeleton stage.

## Build & run

```sh
source ./env.sh        # build intermediates go to zram, not the SSD
cargo run              # opens the explorer window
cargo run -- --snapshot /tmp/shot.bmp   # headless verification render
make dist              # stripped release binary in dist/
```

## Docs

- `docs/ARCHITECTURE.md` — living architecture map (module status + boundaries)
- `docs/UI-SPEC.md`      — pixel measurements from the win8_screenshots
- `docs/DECISIONS.md`    — why egui, why no eframe, GIO seam, CPU rasterizer, …

## Status

Skeleton: self-rendered border + title bar, ribbon (Home wired), breadcrumb
address bar, navigation pane (favorites / This PC / Network), details +
large-icon file views, status bar, light/dark theme, GIO filesystem backend on
a worker thread, software rendering. See ARCHITECTURE.md for what's stubbed.
