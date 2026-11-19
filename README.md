# explorer

A Windows-style file explorer for Linux/Wayland.

[explorer.webm](https://github.com/user-attachments/assets/c4b9e91c-192c-4e86-b766-e70463d42550)

Rust + iced (git `e5c350f`, 0.15-dev; wgpu GPU renderer with tiny-skia CPU
fallback). Borderless window with self-drawn frame (Win8 double border /
Win10 1px, toggleable), QAT title bar with contextual tab blocks,
YAML-configurable ribbon (tabs, groups, buttons/split/dropdown/toggle
commands, menus, in-ribbon galleries, contextual rules), breadcrumb address
bar, resizable nav pane, and a folder view with a 48-step Ctrl+scroll zoom
scale (Content/Tiles/Details/List/small-icon/icon-on-top families).

More: YAML-driven context menus, preview/details side pane (mutually
exclusive, resizable, auto-hides when narrow), compact Details mode with
adjustable and reorderable columns, multi-select (Ctrl/Shift+click,
rubber-band with edge auto-scroll), Windows-style scrollbars with end
buttons, touch input, inline rename, sorting, file ops (internal clipboard,
trash via GIO) on a dedicated fs worker thread, archives open as folders via
7-Zip (7z.so → tar → 7z CLI), drag & drop (internal + a Wayland data-device
side-channel that also drives the native taskbar icon), light/dark theme,
~0% idle CPU.

## Design sources

The design is a deliberate blend of Windows versions past and present, not a
transplant of any single release — and not a wholesale return to Windows 8.
It is best described as "measured", in both senses: values are taken from
real measurements of the shipped systems, and the result is judged on its
own merits rather than by provenance.

- Window frame / borders — Windows 8 (the double border) and Windows 10
  (1px frame), toggleable. The Win8 chrome is simply the most space-
  efficient full-frame look Microsoft shipped, and ages well.
- Ribbon — measured from Windows 10 and 11. This includes QAT behaviour,
  contextual tab blocks, and the newer command layouts.
- Folder view & file-operation (copy/move) dialogs — measured from Windows 11.
- Future features — some are planned from the Windows Vista / 7 era, where
  that generation did things well.
- New ground — parts are original work with no Windows counterpart, notably
  a custom file-system abstraction layer.

Choosing each piece individually is the point. No single Windows release got
everything right; taking what holds up, from whichever version it came, and
building a few things that never existed is a different goal than re-creating
any one OS — including the current one.

## Why iced is patched

The build does not use upstream iced directly: `make setup-iced` materializes
a local overlay (`target/iced-overlay`, gitignored) from upstream iced at a
pinned rev plus one small additive patch, `deps/iced-raw-handle.patch`.
Upstream iced/winit exposes no Wayland data-device API, so the app cannot
reach its own `wl_display`/`wl_surface` — which the drag & drop side-channel
(`src/dnd_wayland.rs`) and the native taskbar icon (`xdg-toplevel-icon-v1`)
need. The patch adds one `window::raw_handle()` action returning those
opaque pointers. It is ~30 lines, purely additive, and re-applied by
`make setup-iced` whenever `ICED_REV` moves.

## Build & run

cargo is the build tool — the Makefile only prepares the two patched crate
overlays (see above) and, for its own targets, redirects cargo's target dir
and caches into in-memory tmpfs to spare the SSD. Everything below is plain
cargo underneath.

```sh
make run                # build (release) + run; GPU, CPU fallback
make check / make test  # type check / unit tests
make dist               # stripped + upxed binary in dist/
make static             # mostly-static build (only glibc stays dynamic)
make dist-static        # its dist counterpart

# flags (after --):
make run ARGS="--render cpu"   # force CPU renderer (--render gpu is strict)
make run ARGS="--dpi=125"      # absolute content scale (percent of 96 dpi)
make run ARGS="--size=900x560" # window size in logical px
make run ARGS="/usr/bin"       # initial folder (default $HOME)

make build FEATURES=embed-selawik # embed the UI font (Selawik, SIL OFL; off by default)
make saferun:/usr/bin            # run dist/ in a bubblewrap sandbox
```

### With plain cargo

If you'd rather use your own toolchain/environment, cargo works directly —
the only make-specific step is materializing the patched overlays that
`Cargo.toml`'s `[patch]` sections point at:

```sh
make setup-overlays    # one-time: seed + patch iced/softbuffer into tmpfs,
                       # create the target/ symlink cargo resolves them through

cargo build --release  # your env, your toolchain; artifacts land in the
                       # tmpfs target/ symlink, never a ./target on disk
```

`setup-overlays` touches nothing else: no toolchain install, no
CARGO_HOME/RUSTUP_HOME redirection. After that, any cargo command
(`build`, `check`, `test`, `clippy`, ...) behaves as usual.

## Configuration

All runtime configuration is YAML, overridable per-user under
`~/.config/explorer/`:

- `assets/ribbon.yaml`      — ribbon markup + UI metrics + window sacrifices
- `assets/folder-view.yaml` — the 48-step zoom scale + layout geometry
- `assets/menu.yaml`        — context menus, file types, open handlers
- `assets/fonts.yaml`       — font sizes (all 12px at 100% dpi) + family fallback list
- `assets/icons.yaml`       — icon registry (builtin SVGs / freedesktop names)
