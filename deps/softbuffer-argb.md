# deps/softbuffer-argb.patch

Two-line **format relabel** on the pinned softbuffer crates.io version
(currently 0.4.8), applied automatically by `make setup-softbuffer` to a
generated overlay living entirely in tmpfs (same pattern as the iced
overlay — never a hand-maintained fork).

```rust
// src/backends/wayland/buffer.rs (both create_buffer call sites)
- wl_shm::Format::Xrgb8888,
+ wl_shm::Format::Argb8888,
```

## Why the patch has to exist

The translucent Win8.1 border (win81theme.yaml `border_opacity` < 1) needs
the pixel alpha to reach the Wayland compositor. That works on the wgpu
backend (iced negotiates `CompositeAlphaMode::PreMultiplied`), but iced's
tiny-skia **software** compositor presents through softbuffer, whose
Wayland backend hardcodes `wl_shm::Format::Xrgb8888` — "X" = alpha ignored,
the surface composites opaque. The border's alpha therefore never reaches
the compositor in cpu-render mode, whatever the app does.

Upstream softbuffer is *deliberately* an opaque window-presenter (its own
README example writes `0x00RRGGBB` pixels); transparency is an
unimplemented feature there, not a defect — so this stays a local patch
until softbuffer grows a transparency story.

## Why two lines are enough

Everything else is already in place:

1. **Byte layout.** iced's tiny-skia `into_color` swaps R/B
   (`from_rgba(color.b, color.g, color.r, color.a)`), so the pixmap bytes
   land in memory as `[B, G, R, A]` — exactly the `Argb8888` wire layout,
   identical to `Xrgb8888` except byte 3 is alpha.
2. **Alpha content.** tiny-skia pixmaps are premultiplied (what
   `Argb8888` expects), and the app clears with a transparent background,
   so byte 3 already holds the right premultiplied alpha.
3. **Compositing opt-in.** winit is told `transparent: true` at window
   creation, so it calls `wl_surface.set_opaque_region(None)` and the
   compositor will blend the buffer.

`Argb8888`/`Xrgb8888` are the two mandatory wl_shm formats — no runtime
negotiation, no fallback needed.

## What you lose by dropping it

Nothing on X11 (softbuffer's X11 backend has no per-pixel alpha either
way) or on the wgpu backend (unaffected). Only: the translucent border
stops working in `--render cpu` / `cpu-render` builds — it composites
opaque there.

## Maintenance

- The patch is 2 changed lines; on softbuffer bumps, bump `SB_VER` in the
  Makefile and refresh this patch (`git diff` format, applies with
  `git apply` outside a repo).
- Idempotency is a `grep Argb8888` check, so the overlay must be seeded
  pristine before the first apply (`make setup-softbuffer` does both).
