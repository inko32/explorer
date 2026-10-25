# explorer

A Windows 8 style file explorer for POSIX and Windows — POSIX/Wayland first.

Rust + iced (git `cd04b74`, 0.15-dev; wgpu GPU renderer with tiny-skia CPU
fallback). Borderless window with self-drawn frame (Win8 double border /
Win10 1px, toggleable), QAT title bar with contextual tab blocks,
YAML-configurable ribbon (tabs, groups, buttons/split/dropdown/toggle
commands, menus, galleries, contextual rules), breadcrumb address bar,
resizable nav pane, and a folder view with a 48-step Ctrl+scroll zoom scale
(Content/Tiles/Details/List/small-icon/icon-on-top families — see
`SPEC/FOLDER-SPEC.MD` + `SPEC/FOLDER-VIEW.md`).

More: YAML-driven context menus (`SPEC/MENU_SPEC.MD`), preview/details side
pane (mutually exclusive, resizable, auto-hides when narrow), compact
Details mode with adjustable columns, multi-select (Ctrl/Shift+click,
rubber-band), Windows-style scrollbars with end buttons, touch input,
inline rename, sorting, file ops + internal clipboard, archives open as
folders via 7-Zip, light/dark theme, ~0% idle CPU.

## Build & run

Always build via the Makefile — it keeps every artifact in in-memory tmpfs

```sh
make run                # build (release) + run; GPU, CPU fallback
make check / make test  # type check / unit tests
make dist               # stripped + upxed binary in dist/

# flags (after --):
make run ARGS="--render cpu"   # force CPU renderer (--render gpu is strict)
make run ARGS="--ui-debug"     # outline every UI element
make run ARGS="--dpi=125"      # absolute content scale (percent of 96 dpi)
make run ARGS="--size=900x560" # window size in logical px
make run ARGS="/usr/bin"       # initial folder (default $HOME)

make build FEATURES=embed-selawik # embed the UI font (Selawik, SIL OFL; off by default)
```

## Configuration

All runtime configuration is YAML, overridable per-user under
`~/.config/explorer/`:

- `assets/ribbon.yaml`      — ribbon markup + UI metrics + window sacrifices
- `assets/folder-view.yaml` — the 48-step zoom scale + layout geometry
- `assets/menu.yaml`        — context menus, file types, open handlers
- `assets/fonts.yaml`       — font sizes (all 12px at 100% dpi) + family fallback list
- `assets/icons.yaml`       — icon registry (PNG sets / builtin SVG)

## Docs

- `SPEC/ARCHITECTURE.md` — canonical architecture map (module status + boundaries)
- `SPEC/RIBBON-SPEC.MD`  — the ribbon framework (Windows Ribbon API in-tree)
- `SPEC/FOLDER-SPEC.MD`  — the 48-step zoom scale (normative)
- `SPEC/FOLDER-VIEW.md`  — folder view implementation notes
- `SPEC/MENU_SPEC.MD`    — context menus
