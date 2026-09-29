# explorer

A Windows 8 style file explorer for POSIX and Windows — POSIX/Wayland first.

**Status: branch `iced` — Ribbon milestone "90% complete" (commit `040351a`).**
iced 0.14 + tiny-skia (CPU renderer): borderless window with self-drawn
frame (Win8 double border / Win10 1px, toggleable), QAT title bar with
contextual tab blocks, YAML-configurable ribbon (tabs, groups, small/big/
split/checkbox/gallery commands, menus, contextual rules), breadcrumb
address bar, nav pane, details/large-icon file view with inline rename,
sorting, file ops + internal clipboard, status bar, archives open as
folders via 7-Zip, light/dark theme, ~0% idle CPU.

The egui implementation is preserved on `master` (docs/DECISIONS.md D11
explains the switch).

## Build & run

```sh
source ./env.sh        # build intermediates go to zram, not the SSD
cargo run              # opens the explorer window (GPU, CPU fallback)
cargo run -- --render cpu   # force CPU (tiny-skia); --render gpu is strict
cargo run -- --ui-debug     # outline every UI element (colored 1px borders)
cargo run -- --dpi=125      # absolute content scale (percent of 96 dpi)
cargo run -- --size=900x560 # window size in logical px (small screens)
make dist              # stripped release binary in dist/
```

## Docs

- `docs/ARCHITECTURE.md` — living architecture map (module status + boundaries)
- `docs/UI-SPEC.md`      — pixel measurements from the win8_screenshots
- `docs/DECISIONS.md`    — decision log (D11: why iced, not egui)
