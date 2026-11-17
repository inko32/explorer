# deps/iced-raw-handle.patch

Tiny **additive** patch on top of the pinned iced rev, applied automatically by
`make setup-iced` to a generated overlay living entirely in tmpfs (never a
hand-maintained fork). It adds exactly one public API:

```rust
iced::window::raw_handle(id) -> Task<Option<(usize, usize, usize)>>
//                                                     wl_surface, wl_display, xdg_toplevel
```

The opaque pointers are wrapped into typed Wayland proxies by
`src/raw_handle_wayland.rs` and power the whole Wayland side-channel:

| module              | handle it needs                              | feature                          |
|---------------------|----------------------------------------------|----------------------------------|
| `dnd_wayland.rs`    | `wl_display` (wrap winit's connection)        | drag-out / drag-in (`wl_data_device`) |
| `dnd_wayland.rs`    | `wl_surface` (the window surface)             | drag origin; drop events route only to this client's surface |
| `popup_wayland.rs`  | `xdg_toplevel` (via the sctk window wrapper)  | off-window menu popups (`xdg_popup` parent chain) |
| `icon_wayland.rs`   | `xdg_toplevel`                                | taskbar/window-switcher icon (`xdg_toplevel_icon_v1`) |

## Why the patch has to exist

1. **No public escape hatch.** iced never hands out the winit `Window` and has
   no raw-handle API at any level. The only place that can reach
   `window.raw` is inside `iced_winit::run_action` — that is where the patch
   hooks (`winit/src/lib.rs`, `Action::GetRawHandle`).

2. **A second Wayland connection cannot substitute.** The compositor routes
   drop events and drag grabs only to the *same client* that owns the focused
   surface, and popups/icon must be parented to objects of the same client.
   So the app must drive its own event queue **on winit's connection**, and
   there is no way to reach that connection from the app without the patch.

3. **`xdg_toplevel` is not available upstream at all.** The method comes from
   iced's own winit fork extension (`WindowExtWayland::xdg_toplevel`); upstream
   winit 0.30 has no such API. iced pins the fork either way, so exposing the
   pointer costs one match arm.

## What you lose by dropping it

The Wayland side-channel cannot start (`raw_handle_wayland::wrap` never gets
its handles), so: no OS-level file drag-out/drag-in (iced has no Wayland
data-device at all), no off-window `xdg_popup` menus, no toplevel icon.
In-window iced overlays keep working — only the real compositor objects need
the handles.

## Maintenance

- The patch is purely additive (~30 lines); on iced bumps, refresh `ICED_REV`
  in the Makefile and re-run `make setup-iced`. `git apply` reports any
  conflict; the patch then has to be re-placed by hand (rare).
- Cargo.toml's `[patch."https://github.com/iced-rs/iced"]` section points at
  `target/iced-overlay` (a symlink into tmpfs created by `make setup-env`).
