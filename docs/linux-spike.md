# Spike G0: the GTK4 page widget at a fractional scale

Spike G0 of the Linux plan asks whether a custom-drawn page widget in GTK4 can show an image of exactly device-pixel size one to one at a fractional scale, with no resampling or blur, and what the right way to do it is. The code is `linux/quill-gtk` (its own Cargo workspace); the numbers below come from running it.

## Table of contents

- [Answer](#answer)
- [The machine](#the-machine)
- [Method](#method)
- [Results](#results)
- [Why the plain texture blurs](#why-the-plain-texture-blurs)
- [The recipe](#the-recipe)
- [Text](#text)
- [Input method](#input-method)
- [File dialog](#file-dialog)
- [Dark mode](#dark-mode)
- [Recommendation](#recommendation)
- [What the viewer needs](#what-the-viewer-needs)
- [Not verified](#not-verified)
- [Reproducing it](#reproducing-it)

## Answer

**Yes.** A `gtk::Widget` subclass that appends a `gdk::MemoryTexture` of exactly (logical size × scale) device pixels with `snapshot.append_texture` is pixel-exact at a scale of 1.5, checkerboard, ruler and text included (100.00 % of the pixels equal the source in the compositor's output), **provided the widget's top-left sits on a whole device pixel of the surface**. GTK does not guarantee that. A widget's position is a whole number of logical pixels, and on this machine the surface sits 25 logical pixels (37.5 device pixels) into its window for the client-side shadow, so everything in the window is half a device pixel off: a plain `append_texture` is bilinear-sampled at a half-pixel phase and the one-pixel checkerboard turns into flat grey (0 % pure black and white pixels). The page widget must measure its own offset to the surface and translate the snapshot by the difference (`snap_offset` in `linux/quill-gtk/src/page.rs`). With that, the texture path is exact, and no fallback is needed.

## The machine

- Arch Linux, GNOME Shell 51.1 and mutter 51.1 on Wayland, GTK 4.24.1, libadwaita 1.10.0, cairo 1.18.6, gtk4-rs 0.11.5 with the `v4_20` feature.
- Two monitors, each **3840 × 2160** at scale 1.5 (2560 × 1440 logical), 144 Hz. The brief said a 5120 × 2880 panel; these are the two panels the compositor reports.
- GTK reports `gdk::Surface::scale()` = **1.5** and `scale_factor()` = **2** (the integer ceiling). Use `scale()`; `scale_factor()` is wrong for fractional work. Both monitors report the same, `gdk::Monitor::scale()` = 1.5.
- The renderer is Vulkan; the unsnapped results were the same with the GL and Cairo renderers (`GSK_RENDERER=gl` and `cairo`), so the blur does not come from one renderer.

## Method

One window holds panels that show the same 288 × 144 device-pixel image (192 × 96 logical) through different GTK paths: a one-device-pixel checkerboard, one-pixel lines, a ruler and a line of text. The grid behind them is a flat `#123456`, which locates the window in a frame. The frame is a real compositor frame: mutter's `org.gnome.Mutter.ScreenCast` D-Bus API gives a PipeWire stream of the monitor at its native 3840 × 2160, and `gst-launch-1.0` saves one frame; no permission prompt, no resampling. `tools/analyse.py` crops each panel and compares it with `source.png`, which the app writes: how many pixels are exactly equal, the largest channel difference, and the share of checkerboard pixels that are pure black or white and alternate with their neighbours. A resampled checkerboard has none of either.

## Results

Compositor frame, scale 1.5, Vulkan renderer, widget at a whole device pixel unless noted. The same ranking held in two runs with the window in two places.

| Path | Pixels equal to source | Checker pure black or white | Verdict |
|---|---|---|---|
| `append_texture`, plain | 59.1 % | 0 % | Flat grey: blurred |
| **`append_texture` after `snap_offset`** | **100.00 %** | **100 %** | **Exact** |
| `append_texture` after `snap_offset`, widget at an odd logical margin (4.5 device pixels) | **100.00 %** | **100 %** | **Exact** |
| `append_texture` at an odd logical margin, no snap | 72.3 % | 0 % | Blurred |
| `append_scaled_texture` with `ScalingFilter::Nearest`, with or without the snap | 58.7 % to 59.5 % | 11 % to 44 % | Aliased, worse than linear |
| `snapshot.scale(1/scale)` with the texture in device units, no snap | 59.1 % | 0 % | Blurred |
| `gtk::Picture` with a paintable of logical size | 59.1 % | 0 % | Blurred (no way to snap) |
| cairo `DrawingArea`, image surface with `set_device_scale(scale, scale)` | 59.1 % | 0 % | Blurred |
| the same, after `cr.translate` by `snap_offset` | 99.0 % | 100 % | Exact inside; one edge column and the last row differ (1 %) |
| cairo `DrawingArea`, one antialiasing-off rectangle per device pixel | 81.5 % | 97.8 % | Mostly crisp but a pixel off; slow |
| control: texture of logical size (scale 1) stretched by GTK | 53.2 % | 0 % | Resampled, as expected |

`docs/media/gtk-spike-checker-zoom.png` shows the top-left of each panel at 8× (the source, the plain texture, the snapped texture, Nearest, the device-unit snapshot, the control, and the two cairo paths). `gtk-spike-aligned-panel.png` is the snapped panel at 1:1 and `gtk-spike-unaligned-panel.png` is the plain one.

Other measurements:

- With GTK's own renderer drawing the grid offscreen at a scale of 1.5 with no window around it (`self.png`, from `GskRenderer::render_texture`), the plain texture, the device-unit snapshot, the cairo image surface and the `Picture` are all exactly 100 %. So GTK's sampling is right; only the half-pixel origin inside the real window breaks it.
- `gtk::Image` takes a `pixel_size` that sets a square box, so a page-shaped paintable cannot be placed in it one to one; it is for icons and was left out of the final grid.
- A cairo draw function sees a target with device scale 1.0 at a surface scale of 1.5: cairo is told nothing about the fraction, so the code must read `gdk::Surface::scale()` itself.

## Why the plain texture blurs

`gtk::Native::surface_transform()` is `(25, 25)` here: the client-side decoration (shadow and border) puts the surface's content 25 logical pixels in, which is 37.5 device pixels. Every widget in the window sits at a fractional device offset; solid fills are snapped to the pixel grid by GSK, so the panel backgrounds look sharp, but textures are not snapped, so they are filtered at a half-pixel phase in both axes (a checkerboard averages to 127). Widget positions in a grid add their own half pixels at odd logical offsets. The compositor does not resample: the snapped panels are exact in its frames, and the frames and GTK's own offscreen render agree.

The offset depends on the shadow, the headerbar and the layout, so it cannot be assumed; it has to be measured for each widget when it draws, and it changes when the window is maximized (no shadow) or the scale changes.

## The recipe

In `snapshot`:

1. `scale = native.surface().scale()` (an `f64`; Wayland sends it in 1/120ths).
2. The widget's device size is `round(logical × scale)`; render a `gdk::MemoryTexture` of exactly that size (`R8g8b8a8Premultiplied` is a straight copy of a tiny-skia pixmap), and cache it by scale and layout revision.
3. Measure the offset: `p = widget.compute_point(&native, &Point::zero())`, `(sx, sy) = native.surface_transform()`, then for each axis `device = (p + s) × scale` and `shift = (device.round() − device) / scale`.
4. `snapshot.save()`, `snapshot.translate(shift)`, `snapshot.append_texture(&texture, &logical_bounds)`, `snapshot.restore()`.

A page whose logical size does not give a whole device size (an odd width at 1.5) rounds the texture up or down by one pixel: choose the widget's size from the device size rather than the other way round.

Cairo works the same way with `cr.translate(shift)` before painting an image surface with `set_device_scale`, but the texture path is simpler and faster and has no edge artefact. Do not use `append_scaled_texture` with `Nearest` for one-to-one drawing, and do not use `Picture` or `Image` for the page.

## Text

One line of text, 12 pt at 100 % (16 px at 96 dpi, so 24 device pixels at 1.5), is drawn into the same pixmap with a minimal skrifa outline and tiny-skia fill, unhinted, with advances in twips as the plan's mapping specifies (`pattern.rs`). It is part of the exactly-equal pixels above: the snapped texture shows it byte for byte, so no hinting, resampling or colour change happens between the pixmap and the screen.

The font crate on PR #25 (`quill-fonts`) was not used. It measures (`OutlineMeasure`) but has no glyph rasterizer yet, and it depends on the root workspace's `quill-core`, which would drag the spike's workspace into the root one. The spike reads Liberation Serif from `/usr/share/fonts` (or `QUILL_FONT`) instead; the rasterizer for G1 is the `quill-raster` work in P5.

## Input method

The probe (`--ime`) attaches a `GtkIMMulticontext` to a focusable area through an `EventControllerKey` (`set_im_context`) and an `EventControllerFocus`, and logs `commit`, `preedit-start`, `preedit-changed`, `preedit-end` and `delete-surrounding`. On this machine the context id is `wayland` (GTK's text-input-v3 module; `gtk-im-module` is empty).

- Typing through mutter's RemoteDesktop API (`tools/type-keys.py`) works: each character arrives as a `commit` with the matching `key-released`, and keys the IM context consumes never reach `key-pressed`, which only fires for keys it does not take (Shift). So shortcuts must run after the IM filter, as the plan has it.
- `wtype` fails on GNOME ("Compositor does not support the virtual keyboard protocol") and `xdotool` reaches only XWayland, so the RemoteDesktop route was the only injection that worked.
- **Not tested: dead keys, Compose and preedit.** The layout is plain `us` with no Compose key, only `xkb` IBus engines are installed (nothing that produces preedit text), and injected keysyms do not carry dead-key or Compose semantics even with `GTK_IM_MODULE=simple`, so the same injection shows nothing for either module. The probe is ready for a manual test: run `quill-gtk-spike --ime`, click the green strip and type with a Compose key or an input source that has dead keys or an IME, and read the log.

## File dialog

`gtk::FileDialog::open` (async, with a `gio::Cancellable`) called `org.freedesktop.portal.FileChooser.OpenFile` on the session bus even though the app is not sandboxed and `GTK_USE_PORTAL` is unset (seen with `dbus-monitor`), so on GNOME the dialog is the portal's. Cancelling through the `Cancellable` returns `Err` with the `gtk::DialogError::Cancelled` kind ("Cancelled by application"). What a chosen file returns was not exercised (no one to click).

## Dark mode

- `gtk-application-prefer-dark-theme` is false, and deprecated since GTK 4.20: it is not how GNOME expresses the preference.
- `gtk-interface-color-scheme` is `DARK`, and `adw::StyleManager::default()` reports `color_scheme()` = `Default`, `system_supports_color_schemes()` true and `is_dark()` true. Following the system needs only `adw::init()` and `StyleManager::is_dark()`, with `connect_dark_notify` for changes; the page's paper stays light and only the chrome follows.

## Recommendation

**Use the texture path**: `QuillPage` appends a device-size `gdk::MemoryTexture` with plain `append_texture` after the surface-aligned shift. It is exact at a scale of 1.5, it is what the plan assumed, and `quill-raster` can render straight into the texture's bytes. Keep the cairo path as a documented fallback only (it needs the same shift and has an edge artefact), and keep egui out of the picture: the plan's fallback chain is not needed.

## What the viewer needs

- The snap shift, recomputed on every snapshot (it is cheap), and a texture cache keyed by scale in 1/120ths so a move to another monitor re-renders.
- Widget sizes that give whole device pixels, or a page that renders to the rounded device size and lets the last logical pixel be partial.
- Tiles instead of one texture for a long page: each tile at a whole device offset, with the shift applied once to the whole page, not per tile.
- Scale from `gdk::Surface::scale()`, notified by `notify::scale` on the surface, never `scale_factor()`.
- The IM filter before shortcuts, and the caret rectangle given to `set_cursor_location` in logical pixels.
- `FileDialog` and `adw::StyleManager` as above; a test hook that bypasses the portal, as the Windows builds do for their common dialogs.
- Re-measuring the offset after the window is maximized, fullscreened or moved between monitors of different scales.

## Not verified

- **Other scales.** Only 1.5 was measured. A whole scale (1 or 2) should be exact without the shift but was not run; 1.25 and 1.75 give non-integer device sizes for odd logical sizes and were not run. The display settings were not changed.
- **A maximized or fullscreen window**, where the shadow is gone and the offset differs.
- **KDE and Wayland compositors other than mutter**, X11, and Flatpak. The ScreenCast frame is mutter's.
- **Dead keys, Compose, preedit and any real IME**, and the portal file dialog returning a chosen file.
- **`gtk::Picture` with a snapped paintable.** A paintable has no widget to measure the offset from, so it was not made exact; a wrapper widget would be needed.
- **The 1 % cairo edge difference** was not traced.
- **GTK versions other than 4.24.1.** `surface_transform` and the fractional-scale handling are version-dependent.

## Reproducing it

The spike program (`quill-gtk-spike`, with its patterns and probes) was replaced by the viewer in step G1 (see [linux.md](linux.md#the-gtk4-viewer-g1)); its source is in the history of `linux/quill-gtk` before that change, and `tools/capture.py`, `analyse.py`, `sheet.py` and `type-keys.py` stayed. To run the measurements again, build the spike from that history:

```sh
cd linux/quill-gtk
CARGO_TARGET_DIR=/path/outside/the/repo cargo build
quill-gtk-spike --out DIR --quit-after 9000 > DIR/log.txt &
python3 tools/capture.py DP-1 DIR/frame.png            # the monitor the window is on; gst-launch-1.0 and PyGObject
python3 -I tools/analyse.py DIR/frame.png DIR/log.txt DIR/source.png DIR/crop
python3 -I tools/sheet.py DIR/crop DIR/source.png sheet.png texture aligned nearest-aligned
```

Extra flags: `--ime` for the input-method probe, `--file-dialog` to open and cancel the file dialog, `--offsets` for a grid of textures at different logical margins. The window prints the scale, the monitor, the renderer and every panel's bounds on start. It needs `gtk4` and `libadwaita` with their development files; the root workspace never builds it. The viewer's own check, which needs no desktop session, is `linux/quill-gtk/tools/frames.sh`.
