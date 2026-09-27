# explorer

A Windows 8 style file explorer for POSIX and Windows — POSIX/Wayland first.

**Status: iced spike validated on branch `iced`.** iced 0.14 + tiny-skia
(CPU renderer): borderless window with self-drawn title bar, details file
list with hover/selection, breadcrumb address bar, nav pane, status bar,
light/dark theme, ~0% idle CPU, crisp text at fractional DPI. Next: port the
GIO backend and ops from `master`, then rebuild the ribbon.

The egui implementation is preserved on `master` (docs/DECISIONS.md D11
explains the switch).

## Build & run

```sh
source ./env.sh        # build intermediates go to zram, not the SSD
cargo run              # opens the explorer window
make dist              # stripped release binary in dist/
```

## Docs

- `docs/ARCHITECTURE.md` — living architecture map (module status + boundaries)
- `docs/UI-SPEC.md`      — pixel measurements from the win8_screenshots
- `docs/DECISIONS.md`    — decision log (D11: why iced, not egui)
