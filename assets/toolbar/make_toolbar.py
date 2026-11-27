#!/usr/bin/env python3
# Draws Quill's toolbar glyphs: a strip of 16 by 15 pixel pictures in the 16 standard colours, as a 4-bit Windows bitmap.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: assets/toolbar/make_toolbar.py [--sheet OUT.png]   (writes assets/toolbar/toolbar.bmp next to this file)
#
# The glyph order is the order the painters use (see `GLYPH_*` in win9x/quill-win9x/src/chrome.rs). Colour 0 is the button face, so the
# strip can be drawn straight onto a button with no mask. The palette is the 16 standard Windows colours, in the order they are
# numbered in a 16-colour display.
import os
import struct
import sys

W, H = 16, 15
FACE, BLACK, WHITE, GREY, NAVY, RED, YELLOW, OLIVE, BLUE, GREEN, TEAL, SILVER = 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11
PALETTE = [
    (0xC0, 0xC0, 0xC0), (0, 0, 0), (0xFF, 0xFF, 0xFF), (0x80, 0x80, 0x80), (0, 0, 0x80), (0xFF, 0, 0), (0xFF, 0xFF, 0),
    (0x80, 0x80, 0), (0, 0, 0xFF), (0, 0x80, 0), (0, 0x80, 0x80), (0xDF, 0xDF, 0xDF), (0x80, 0, 0), (0x80, 0, 0x80), (0, 0xFF, 0xFF), (0, 0xFF, 0),
]


class Canvas:
    def __init__(self):
        self.p = [[FACE] * W for _ in range(H)]

    def px(self, x, y, c):
        if 0 <= x < W and 0 <= y < H:
            self.p[y][x] = c

    def rect(self, x0, y0, x1, y1, c):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                self.px(x, y, c)

    def frame(self, x0, y0, x1, y1, c):
        for x in range(x0, x1 + 1):
            self.px(x, y0, c)
            self.px(x, y1, c)
        for y in range(y0, y1 + 1):
            self.px(x0, y, c)
            self.px(x1, y, c)

    def line(self, x0, y0, x1, y1, c):
        n = max(abs(x1 - x0), abs(y1 - y0), 1)
        for i in range(n + 1):
            self.px(round(x0 + (x1 - x0) * i / n), round(y0 + (y1 - y0) * i / n), c)

    def art(self, x, y, rows, key):
        for j, row in enumerate(rows):
            for i, ch in enumerate(row):
                if ch in key:
                    self.px(x + i, y + j, key[ch])


def page(c, x0=3, y0=1, x1=11, y1=13):
    c.rect(x0, y0, x1, y1, WHITE)
    c.frame(x0, y0, x1, y1, BLACK)


def glyph_new(c):
    page(c, 3, 1, 12, 13)
    c.rect(9, 1, 12, 4, FACE)
    c.line(9, 1, 12, 4, BLACK)
    c.line(9, 1, 9, 4, BLACK)
    c.line(9, 4, 12, 4, BLACK)
    c.rect(10, 2, 11, 3, WHITE)


# The open folder, hinted by hand from its master, open-folder.svg (drawn on this grid): the back sheet with a rounded tab sloping
# down to its top edge, a lit yellow rim round the olive inside, a sheet of paper standing in it, and the front flap leaning
# forward as a parallelogram, its top edge lower and further right than the back's and its bottom edge on the back's, with a
# white lit edge on the top and left. Nothing crosses the front of the folder but the flap itself.
OPEN_FOLDER = [
    "................",
    "................",
    "..KKKK..........",
    ".KYYYYKKKKKK....",
    ".KYOOOYYYYYYK...",
    ".KYOWWWWWWWOK...",
    ".KYOWGGGGGWOK...",
    ".KYOKKKKKKKKKKKK",
    ".KYOKWWWWWWWWWWK",
    ".KYKWYYYYYYYYYK.",
    ".KYKWYYYYYYYYYK.",
    ".KKWYYYYYYYYYK..",
    ".KKWYYYYYYYYYK..",
    ".KKKKKKKKKKKK...",
    "................",
]


def glyph_open(c):
    c.art(0, 0, OPEN_FOLDER, {"K": BLACK, "Y": YELLOW, "O": OLIVE, "W": WHITE, "G": GREY})


def glyph_save(c):
    c.rect(2, 2, 13, 13, NAVY)
    c.frame(2, 2, 13, 13, BLACK)
    c.rect(4, 2, 11, 6, SILVER)
    c.rect(9, 3, 10, 5, GREY)
    c.rect(4, 9, 11, 13, WHITE)
    c.line(5, 10, 10, 10, GREY)
    c.line(5, 12, 10, 12, GREY)


def glyph_print(c):
    c.rect(4, 1, 11, 5, WHITE)
    c.frame(4, 1, 11, 5, BLACK)
    c.rect(1, 6, 14, 11, GREY)
    c.frame(1, 6, 14, 11, BLACK)
    c.rect(2, 7, 13, 8, SILVER)
    c.rect(4, 10, 11, 14, WHITE)
    c.frame(4, 10, 11, 14, BLACK)
    c.px(12, 8, GREEN)


def glyph_preview(c):
    page(c, 1, 1, 9, 13)
    c.line(3, 4, 7, 4, GREY)
    c.line(3, 6, 7, 6, GREY)
    c.frame(8, 7, 13, 12, BLACK)
    c.rect(9, 8, 12, 11, TEAL + 4)  # light cyan
    c.line(8, 12, 6, 14, BLACK)


def glyph_cut(c):
    c.line(4, 1, 9, 9, BLACK)
    c.line(11, 1, 6, 9, BLACK)
    c.frame(3, 10, 6, 13, BLUE)
    c.frame(9, 10, 12, 13, BLUE)
    c.px(7, 8, BLACK)
    c.px(8, 8, BLACK)


def glyph_copy(c):
    page(c, 1, 1, 8, 10)
    page(c, 6, 5, 13, 14)
    c.line(8, 8, 11, 8, GREY)
    c.line(8, 10, 11, 10, GREY)
    c.line(8, 12, 11, 12, GREY)


def glyph_paste(c):
    c.rect(2, 2, 12, 14, OLIVE)
    c.frame(2, 2, 12, 14, BLACK)
    c.rect(5, 1, 9, 3, GREY)
    c.frame(5, 1, 9, 3, BLACK)
    page(c, 6, 6, 14, 14)
    c.line(8, 9, 12, 9, GREY)
    c.line(8, 11, 12, 11, GREY)


def arrow(c, left):
    # A curved arrow: a hook that turns back.
    for x, y in [(4, 5), (5, 4), (6, 4), (7, 4), (8, 4), (9, 4), (10, 5), (11, 6), (11, 7), (11, 8), (10, 9), (9, 10), (8, 10)]:
        c.px(x if left else W - 1 - x, y, BLACK)
        c.px(x if left else W - 1 - x, y + 1, BLACK)
    for dx, h in [(0, 0), (1, 1), (2, 2), (3, 3)]:
        for k in range(-h, h + 1):
            c.px((1 + dx) if left else (W - 2 - dx), 5 + k, NAVY)
    c.px(0 if left else W - 1, 5, NAVY)


GLYPH_B = [
    "XXXX.",
    "X...X",
    "X...X",
    "XXXX.",
    "X...X",
    "X...X",
    "XXXX.",
]
GLYPH_I = [
    ".XXX.",
    "..X..",
    "..X..",
    "..X..",
    "..X..",
    "..X..",
    ".XXX.",
]
GLYPH_U = [
    "X...X",
    "X...X",
    "X...X",
    "X...X",
    "X...X",
    "X...X",
    ".XXX.",
]
GLYPH_A = [
    "..X..",
    ".X.X.",
    ".X.X.",
    "XXXXX",
    "X...X",
    "X...X",
    "X...X",
]


def scale2(c, rows, x, y, colour):
    for j, row in enumerate(rows):
        for i, ch in enumerate(row):
            if ch == "X":
                c.rect(x + i * 2 - (i > 0) * 0, y + j, x + i * 2 + 1 - 0, y + j, colour)


def glyph_bold(c):
    c.art(3, 3, GLYPH_B, {"X": BLACK})
    c.art(4, 3, GLYPH_B, {"X": BLACK})
    c.art(3, 4, [r for r in GLYPH_B][0:1] and [""], {})


GLYPH_ITALIC = [
    "....XXXX",
    ".....XX.",
    "....XX..",
    "....XX..",
    "...XX...",
    "...XX...",
    "..XX....",
    "..XX....",
    ".XXXX...",
]


def glyph_italic(c):
    # A slanted I on the rows of the underline glyph: a two-pixel stem that moves one pixel left every two rows, and a four-pixel serif
    # centred on each end of it.
    c.art(4, 2, GLYPH_ITALIC, {"X": BLACK})


def glyph_underline(c):
    c.art(5, 2, GLYPH_U, {"X": BLACK})
    c.line(3, 11, 12, 11, BLACK)


def glyph_colour(c):
    c.art(5, 1, GLYPH_A, {"X": BLACK})
    c.rect(2, 11, 13, 13, RED)


def aligned(c, kind):
    widths = [11, 7, 11, 5, 11, 8]
    for i, w in enumerate(widths):
        y = 2 + i * 2
        if kind == 0:
            x0 = 2
        elif kind == 1:
            x0 = 2 + (11 - w) // 2
        elif kind == 2:
            x0 = 2 + 11 - w
        else:
            x0, w = 2, 11
        c.line(x0, y, x0 + w - 1, y, BLACK)


def glyph_bullets(c):
    for i in range(3):
        y = 2 + i * 4
        c.rect(2, y, 4, y + 2, BLACK)
        c.line(7, y + 1, 13, y + 1, BLACK)


def glyph_numbering(c):
    digits = [[".X.", "XX.", ".X.", ".X.", "XXX"], ["XX.", "..X", ".X.", "X..", "XXX"], ["XX.", "..X", ".X.", "..X", "XX."]]
    for i, d in enumerate(digits):
        y = 1 + i * 4
        for j, row in enumerate(d[:3] if i < 2 else d[:3]):
            for k, ch in enumerate(row):
                if ch == "X":
                    c.px(2 + k, y + j, BLACK)
        c.line(7, y + 1, 13, y + 1, BLACK)


def glyph_spelling(c):
    c.art(1, 1, GLYPH_A, {"X": BLACK})
    c.art(7, 1, ["XXXX.", "X...X", "XXXX.", "X...X", "XXXX.", ".....", "....."], {"X": BLACK})
    c.line(3, 10, 5, 13, GREEN)
    c.line(5, 13, 12, 6, GREEN)
    c.line(3, 11, 5, 14, GREEN)
    c.line(5, 14, 12, 7, GREEN)


def glyph_arrow(c):
    for i in range(5):
        c.line(5 + i, 6 + i // 1 if False else 6 + 0, 5 + i, 6, BLACK)
    c.line(4, 6, 10, 6, BLACK)
    c.line(5, 7, 9, 7, BLACK)
    c.line(6, 8, 8, 8, BLACK)
    c.px(7, 9, BLACK)


GLYPHS = [
    glyph_new, glyph_open, glyph_save, glyph_print, glyph_preview, glyph_cut, glyph_copy, glyph_paste,
    lambda c: arrow(c, True), lambda c: arrow(c, False), glyph_bold, glyph_italic, glyph_underline, glyph_colour,
    lambda c: aligned(c, 0), lambda c: aligned(c, 1), lambda c: aligned(c, 2), lambda c: aligned(c, 3),
    glyph_bullets, glyph_numbering, glyph_spelling, glyph_arrow,
]


def build():
    canvases = []
    for g in GLYPHS:
        c = Canvas()
        g(c)
        canvases.append(c)
    return canvases


def bmp(canvases):
    w = W * len(canvases)
    stride = ((w * 4 + 31) // 32) * 4
    rows = []
    for y in range(H - 1, -1, -1):
        row = bytearray(stride)
        for i, c in enumerate(canvases):
            for x in range(W):
                v = c.p[y][x]
                px = i * W + x
                row[px // 2] |= (v << 4) if px % 2 == 0 else v
        rows.append(bytes(row))
    pixels = b"".join(rows)
    pal = b"".join(bytes([b, g, r, 0]) for r, g, b in PALETTE)
    off = 14 + 40 + len(pal)
    head = b"BM" + struct.pack("<IHHI", off + len(pixels), 0, 0, off)
    info = struct.pack("<IiiHHIIiiII", 40, w, H, 1, 4, 0, len(pixels), 2835, 2835, 16, 0)
    return head + info + pal + pixels


def sheet(canvases, path):
    import zlib
    scale = 6
    w = (W + 1) * len(canvases) * scale
    h = (H + 1) * scale
    raw = bytearray()
    for y in range(h):
        raw.append(0)
        for x in range(w):
            gi, gx = divmod(x // scale, W + 1)
            gy = y // scale
            if gx >= W or gy >= H:
                raw += bytes((255, 0, 255))
            else:
                raw += bytes(PALETTE[canvases[gi].p[gy][gx]])

    def chunk(k, d):
        c = struct.pack(">I", len(d)) + k + d
        return c + struct.pack(">I", zlib.crc32(k + d))

    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(bytes(raw))) + chunk(b"IEND", b"")
    open(path, "wb").write(png)


if __name__ == "__main__":
    cs = build()
    here = os.path.dirname(os.path.abspath(__file__))
    open(os.path.join(here, "toolbar.bmp"), "wb").write(bmp(cs))
    if "--sheet" in sys.argv:
        sheet(cs, sys.argv[sys.argv.index("--sheet") + 1])
    print(f"toolbar.bmp: {len(cs)} glyphs of {W} by {H}")
