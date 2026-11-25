# Afterglow original: palette, a small pixel canvas and a BMP writer. Every bitmap in the style is drawn from these primitives.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import numpy as np


def rgb(h):
	h = h.lstrip("#")
	return np.array([int(h[i : i + 2], 16) for i in (0, 2, 4)], dtype=float)


def hexstr(c):
	c = np.clip(np.round(np.asarray(c, dtype=float)), 0, 255).astype(int)
	return "#%02x%02x%02x" % tuple(c)


def ini_colour(c):
	c = np.clip(np.round(np.asarray(c, dtype=float)), 0, 255).astype(int)
	return "%d %d %d" % tuple(c)


def mix(a, b, t):
	"""Linear mix of two colours: t = 0 gives a, t = 1 gives b."""
	return np.asarray(a, dtype=float) * (1 - t) + np.asarray(b, dtype=float) * t


# ---- palette --------------------------------------------------------------------------------------------------------------------------
# Liminal HQ Afterglow tokens (surfaces, text, brand) and the Quill dusk wallpaper's glow steps.
P = {
	"night": rgb("#050507"),
	"panel": rgb("#0c0c10"),
	"panel2": rgb("#111116"),
	"raised": rgb("#14161d"),
	"raised2": rgb("#1b1e28"),
	"raised3": rgb("#262a37"),
	"tile": rgb("#0a1230"),  # icon tile navy, used for outlines
	"horizon": rgb("#4a4580"),  # front horizon edge in the wallpaper
	"text": rgb("#e0e0e0"),
	"text2": rgb("#b4bfce"),
	"muted": rgb("#7c8796"),
	"white": rgb("#ffffff"),
	"gold": rgb("#ffaa40"),
	"gold_down": rgb("#e89930"),
	"gold_light": rgb("#ffd08e"),
	"gold_pale": rgb("#ffe7c4"),
	"gold_deep": rgb("#b8893a"),
	"ink": rgb("#f0a830"),
	"rose": rgb("#f43f5e"),
	"rose_mid": rgb("#c9354e"),
	"rose_light": rgb("#f995a6"),
	"rose_deep": rgb("#8f2638"),
	# Classic client colours (ReactOS classic scheme): controls on dialogs sit on these.
	"cream": rgb("#d4d0c8"),
	"cream_hi": rgb("#ece9e2"),
	"cream_lo": rgb("#c2bdb2"),
	"shadow": rgb("#808080"),
	"edge": rgb("#7f849c"),  # control outline: a grey with a little dusk in it
	"edge_dim": rgb("#a7a59f"),
	"ink_text": rgb("#0a0c1a"),
}
# Active caption: the wallpaper's seven inner glow steps, navy on the left to rose on the right.
DUSK = [rgb(h) for h in ("#1f2554", "#2a3164", "#35366a", "#42396c", "#523d6c", "#643f68", "#7a4562")]
KEY = np.array([255.0, 0.0, 255.0])  # transparent key colour


def dusk_at(t):
	"""Smooth colour along the seven dusk stops, t in 0..1."""
	t = min(max(t, 0.0), 1.0) * (len(DUSK) - 1)
	i = min(int(t), len(DUSK) - 2)
	return mix(DUSK[i], DUSK[i + 1], t - i)


# ---- canvas ---------------------------------------------------------------------------------------------------------------------------


def canvas(w, h, c=KEY):
	a = np.zeros((h, w, 3), dtype=float)
	a[:, :] = c
	return a


def round_mask(w, h, r, corners="tl tr bl br"):
	"""Boolean mask of a w x h rectangle with pixel-art rounded corners of radius r (sampled at pixel centres)."""
	m = np.ones((h, w), dtype=bool)
	if r <= 0:
		return m
	cs = corners.split()
	for y in range(h):
		for x in range(w):
			for name, cx, cy in (("tl", r, r), ("tr", w - r, r), ("bl", r, h - r), ("br", w - r, h - r)):
				if name not in cs:
					continue
				px, py = x + 0.5, y + 0.5
				inx = px < cx if name in ("tl", "bl") else px > cx
				iny = py < cy if name in ("tl", "tr") else py > cy
				if inx and iny and (px - cx) ** 2 + (py - cy) ** 2 > r * r + 0.3:
					m[y, x] = False
	return m


def inner_edge(m):
	"""Pixels of mask m that touch the outside (4-neighbourhood): the 1 px outline of a shape."""
	p = np.pad(m, 1, constant_values=False)
	inside = p[1:-1, 1:-1] & p[:-2, 1:-1] & p[2:, 1:-1] & p[1:-1, :-2] & p[1:-1, 2:]
	return m & ~inside


def shape(w, h, r, fill, border=None, corners="tl tr bl br", bg=KEY, rows=None, cols=None):
	"""A rounded rectangle: fill may be a colour, or rows/cols give per-row or per-column fills. Outside the shape is bg."""
	a = canvas(w, h, bg)
	m = round_mask(w, h, r, corners)
	body = np.zeros((h, w, 3))
	if fill is not None:
		body[:, :] = fill
	if rows is not None:
		for y, c in enumerate(rows):
			body[y, :] = c
	if cols is not None:
		for x, c in enumerate(cols):
			body[:, x] = c
	a[m] = body[m]
	if border is not None:
		e = inner_edge(m)
		a[e] = border
	return a, m


def stack(frames, vertical=True):
	return np.concatenate(frames, axis=0 if vertical else 1)


def gloss_rows(h, top, bottom, split=0.5):
	"""Two-band 'glass' fill: the top part a flat lighter colour, then a short ramp to the base. Few colours, no banding."""
	rows = []
	k = max(1, int(round(h * split)))
	for y in range(h):
		if y < k:
			rows.append(top)
		else:
			t = (y - k) / max(1, h - k - 1)
			rows.append(mix(mix(top, bottom, 0.55), bottom, t))
	return rows


# ---- BMP output -----------------------------------------------------------------------------------------------------------------------


def _bmp(w, h, bpp, pal, rows_bytes):
	stride = (len(rows_bytes[0]) + 3) & ~3
	data = b"".join(r + bytes(stride - len(r)) for r in reversed(rows_bytes))  # bottom-up
	palbytes = b"".join(bytes((int(c[2]), int(c[1]), int(c[0]), 0)) for c in pal)
	off = 14 + 40 + len(palbytes)
	hdr = b"BM" + (off + len(data)).to_bytes(4, "little") + bytes(4) + off.to_bytes(4, "little")
	info = (40).to_bytes(4, "little") + w.to_bytes(4, "little", signed=True) + h.to_bytes(4, "little", signed=True)
	info += (1).to_bytes(2, "little") + bpp.to_bytes(2, "little") + bytes(4) + len(data).to_bytes(4, "little")
	info += (2835).to_bytes(4, "little") * 2 + len(pal).to_bytes(4, "little") + bytes(4)
	return hdr + info + palbytes + data


def encode_bmp(a):
	"""Smallest uncompressed BMP that holds the image exactly: 4-bit or 8-bit palette, or 24-bit."""
	a = np.clip(np.round(a), 0, 255).astype(np.uint8)
	h, w = a.shape[:2]
	cols, inv = np.unique(a.reshape(-1, 3), axis=0, return_inverse=True)
	idx = inv.reshape(h, w)
	if len(cols) <= 16:
		rows = []
		for y in range(h):
			r = list(idx[y]) + ([0] if w & 1 else [])
			rows.append(bytes((r[i] << 4) | r[i + 1] for i in range(0, len(r), 2)))
		return _bmp(w, h, 4, cols, rows)
	if len(cols) <= 256:
		return _bmp(w, h, 8, cols, [bytes(idx[y].astype(np.uint8)) for y in range(h)])
	rows = [bytes(a[y, :, ::-1].reshape(-1)) for y in range(h)]
	return _bmp(w, h, 24, [], rows)
