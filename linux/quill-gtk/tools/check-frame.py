# Compares a frame (a compositor or X capture, or GTK's own render of the window) with the page texture the client drew.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
"""Usage: check-frame.py FRAME.png TEXTURE.png X Y [SEARCH [W H]]

X and Y are where `quill --dump` says the texture sits, in device pixels. The texture is cut out of the frame at that place (and,
with SEARCH, at the best place within that many pixels, as a frame may carry a border the client does not know about) and compared
pixel for pixel, over W by H pixels when given (what the widget shows of the texture): the share exactly equal, the largest channel difference, and the share of pure black text pixels that match."""
import sys
import numpy as np
from PIL import Image

frame = np.asarray(Image.open(sys.argv[1]).convert("RGB")).astype(int)
tex = np.asarray(Image.open(sys.argv[2]).convert("RGB")).astype(int)
x0, y0 = round(float(sys.argv[3])), round(float(sys.argv[4]))
search = int(sys.argv[5]) if len(sys.argv) > 5 else 0
th, tw = tex.shape[:2]
if len(sys.argv) > 7:
	tw, th = min(tw, int(sys.argv[6])), min(th, int(sys.argv[7]))
	tex = tex[:th, :tw]
probe = tex[: min(th, 160), : min(tw, 160)]


def crop(x, y, h, w):
	return frame[y : y + h, x : x + w]


best = (None, -1.0)
for dy in range(-search, search + 1):
	for dx in range(-search, search + 1):
		x, y = x0 + dx, y0 + dy
		if x < 0 or y < 0:
			continue
		c = crop(x, y, *probe.shape[:2])
		if c.shape != probe.shape:
			continue
		score = float((c == probe).all(axis=2).mean())
		if score > best[1]:
			best = ((x, y), score)
(x, y), _ = best
c = crop(x, y, th, tw)
h, w = min(c.shape[0], th), min(c.shape[1], tw)
c, t = c[:h, :w], tex[:h, :w]
same = (c == t).all(axis=2)
ink = (t.sum(axis=2) < 3 * 40)
print(f"found at {x} {y} (asked {x0} {y0}), compared {w}x{h}")
print(f"pixels equal {100 * same.mean():.3f} %  max channel difference {int(np.abs(c - t).max())}")
print(f"dark text pixels {int(ink.sum())}, equal {100 * (same & ink).sum() / max(1, ink.sum()):.2f} %")
sys.exit(0 if same.mean() > 0.999 else 1)
