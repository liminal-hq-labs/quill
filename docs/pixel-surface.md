# The pixel surface

`quill_paint::pixel` is the `Surface` of the page painter over a 32-bit RGBA buffer, so the GTK4 client, the WinUI 3 client and a browser (WebAssembly and a canvas) draw the page with one engine. It is behind the `pixel` feature of `quill-paint` and needs `alloc`; the DOS build does not enable it. Decisions D210 to D214 are in [decisions.md](decisions.md).

## Table of contents

- [Using it](#using-it)
- [Pixel format](#pixel-format)
- [Scale](#scale)
- [Text](#text)
- [Lines, pictures and clipping](#lines-pictures-and-clipping)
- [Checks](#checks)
- [Not done](#not-done)

## Using it

```rust
let measure = OutlineMeasure::new(FontSet::system()).with_resolution(96);
let glyphs = OutlineMeasure::new(FontSet::system()).with_resolution(96);

let scale = Scale::from_percent(175);
let mut buf = PixelBuffer::new(scale.dev(logical_w) as u32, scale.dev(logical_h) as u32);
let mut sf = buf.surface(ByteOrder::Rgba, scale, MeasureGlyphs::new(&glyphs));
paint_sheet(&mut sf, &app, &mut measure, &store, 8, 8, 1, pages, area, &mut window);
```

`PixelSurface::new` takes a borrowed `&mut [u8]`, a width and height in device pixels and a stride in bytes (rows may be padded, as a GTK or Direct2D mapping is), and returns `None` when the slice is too small. `PixelBuffer` owns the bytes when the client has none to lend. The painter's coordinates, `area` and the layout are in logical pixels at 96 to the inch; the surface does the rest.

## Pixel format

Four bytes a pixel, **premultiplied**. `ByteOrder::Rgba` is red, green, blue, alpha in memory (canvas `ImageData`, `gdk::MemoryFormat::R8g8b8a8Premultiplied`); `ByteOrder::Bgra` is blue, green, red, alpha (Direct2D, WriteableBitmap, Cairo `ARgb32` on a little-endian machine). A new buffer is transparent. Everything the painter draws is opaque, so on the page the premultiplied and straight forms are the same bytes; premultiplied matters for the antialiased edge of a glyph blended over what is there and for a margin left transparent, and every compositor the clients hand the bitmap to expects it. `invert_rect` keeps alpha and turns each colour `c` into `alpha - c`.

## Scale

`Scale` is a fixed-point number in 1/256, built with `from_percent(175)`, `from_ratio(7, 4)` or `from_raw(448)`. The supported range is 1.0 to 3.0; 0.25 to 8.0 is accepted so a browser at an odd zoom still draws. A logical length `v` is `round(v × scale)` device pixels, and every edge is rounded alone, so two rectangles that share an edge share it at every scale. A one-pixel line is `max(1, round(scale))` device pixels thick: 1, 1, 2, 2, 2 and 3 at 1.0, 1.25, 1.5, 1.75, 2.0 and 3.0. `Surface::size` is the device size over the scale, in logical pixels. No float is used anywhere in `quill-paint`.

## Text

`Surface::font` resolves a request through a `GlyphSource`; `quill-fonts` provides `MeasureGlyphs` over an `OutlineMeasure`'s faces, so the glyphs are the font the widths came from. `Surface::advance` is the measure's own rounding at 96 to the inch, so what the painter lays out is what layout measured. Glyphs are unhinted and filled from their outlines at the device size by exact-area coverage in 16.16 fixed point (`pixel::Rasteriser`), with the non-zero rule, and blended with the text colour. The pen sits at the fractional device position the logical `x` gives, and the baseline at a whole device row. A synthetic bold draws the outline twice an `em / 64` apart; a synthetic italic leans it a quarter. `PixelSurface::underline` and `strikethrough` draw the lines the painter draws, at the baseline plus one and at three eighths of the ascent above it.

## Lines, pictures and clipping

`clip` maps its rectangle to the device and every draw call is limited to it and to the buffer. `rgb_row` and `bitmap4` repeat rows and columns of what the painter decodes at logical size (nearest neighbour). The `smooth` flag of `glyph_run` is ignored: text is always antialiased.

## Checks

- `crates/quill-fonts/tests/pixel.rs` draws one fixed page at 1.0, 1.25, 1.5, 1.75, 2.0 and 3.0 through `paint_sheet` and compares each with `tests/golden/pixel/page.<scale>.png` and `page.sums`; `QUILL_UPDATE_GOLDEN=1` rewrites them and the diff is reviewed like code. It also checks clipping and edges at fractional scales, byte order and stride, line thickness, underline and strikethrough, antialiasing and advances, picture scaling, inversion and the `Scale` arithmetic.
- `make wasm` (part of `make validate`, and a CI job) runs `cargo check --target wasm32-unknown-unknown -p quill-paint --features pixel`.

## Not done

A glyph cache, subpixel (LCD) text, bilinear picture scaling and a `u32` buffer type. The painter has no `Rect`-based dirty tracking, so a client redraws the whole surface.
