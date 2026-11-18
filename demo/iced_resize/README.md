# iced_resize — `window::resize` viewport bug (Wayland)

Minimal repro for an iced_winit bug: after a programmatic `window::resize`
on Wayland, the window geometry changes but iced keeps rendering at the
old size, because iced's internal viewport is never updated.

Pinned to iced `e5c350f271a8046f875c6662d452fde9e462d486` via its own
overlay (same pattern as the explorer repo: `Cargo.toml`'s `[patch]`
section always points at `target/iced-overlay`, cloned from GitHub into
the tmpfs build dir — never into the source tree).

## Run

```sh
make run          # needs a Wayland session; window must be floating
make patch-iced   # apply the viewport-sync fix to the overlay
make unpatch-iced # revert the overlay to pristine upstream iced
```

A 300x150 non-resizable window opens; ~2 s later the app resizes it to
300x300 via `window::resize`. A freshly seeded overlay is pristine, so a
plain `make run` reproduces the bug; `make patch-iced` once, rebuild,
and the same demo shows the fixed behavior. Builds never change the patch
state and the explorer overlay is never touched.

If the overlay is missing (e.g. after a reboot — it lives in tmpfs and
does not survive a restart), the next `make build` seeds it by cloning
`https://github.com/iced-rs/iced` at the pinned rev, so seeding needs
network access once per tmpfs lifetime.

## Expected

The content re-renders at 300x300; the red hairline stays at y=150 with
window background below it; the readout flips to `OK: iced sees the new
size 300x300`.

## Actual (Wayland, e.g. KWin)

The window frame grows to 300x300 (bottom edge extends) but iced keeps
submitting the old 150 px-tall buffer, so the compositor stretches the
content. Textual proof without screenshots: the readout stays

```
BUG: iced thinks the size is still 300x150 (stale)
```

because `window::size` also reads the stale viewport. stderr shows the
same: after `[repro] sending window::resize -> 300x300`,
`window::size` never reports the new size.

## Cause

On Wayland (floating window ⇒ "stateless" xdg configures), winit's
`request_inner_size` applies the size synchronously and returns
`Some(size)` — documented as "applied, **no** `Resized` event will
follow". `iced_winit`'s `window::Action::Resize` handler does
`let _ = window.raw.request_inner_size(...)`, discarding exactly the
signal that tells it to update `WindowState.viewport` itself. The
viewport is only written by `WindowEvent::Resized` /
`ScaleFactorChanged`, which never arrive on this path.

Possible fix (carried here as `deps/iced-resize-viewport-sync.patch`,
and also in explorer's `deps/iced-raw-handle.patch`): when
`request_inner_size` returns `Some(new_size)`, update the viewport and
bump `surface_version` in the action handler, mirroring the `Resized`
event branch. Platforms that apply resizes asynchronously
(Windows/X11/macOS → `None`) are unaffected.

Verified with this demo on KWin/Wayland:

- pristine: `[repro] sending window::resize -> 300x300` and
  `window::size` never reports anything else (stale at 300x150);
- patched: one second after the resize,
  `[repro] window::size now reports: Size { width: 300.0, height: 300.0 }`.
