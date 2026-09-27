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

Skeleton **runs on Wayland**: self-rendered border + title bar (drag,
double-click maximize, compositor resize), ribbon (Home wired: clipboard /
organize / new / open / select), breadcrumb address bar, navigation pane
(favorites / This PC via GIO volumes / Network stub), details + large-icon
file views with selection/sorting/inline-rename, status bar, light/dark theme,
GIO filesystem on a worker thread, software rendering (no GPU needed), hi-dpi
verified, internal clipboard + copy/cut/paste/delete-to-trash with progress.

Verify layouts headlessly: `cargo run -- --snapshot out.bmp [--dark] [--scale 1.75]`
See ARCHITECTURE.md for the full module status table.
