# Compares a compositor frame of the spike window with the pixels the spike drew, panel by panel.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
"""Usage: analyse.py FRAME.png LOG.txt SOURCE.png [OUTPREFIX]

The window's grid is painted #123456, so its bounding box in the frame gives the origin. Each `panel NAME x y w h` line of the
log (logical pixels, relative to the grid) then says where a panel sits. For each panel the script reports how many pixels
equal the source exactly, the largest channel difference, and how much of the one-pixel checkerboard survives. With OUTPREFIX
it also writes each panel crop as a PNG."""
import sys
import numpy as np
from PIL import Image

frame = np.asarray(Image.open(sys.argv[1]).convert("RGB")).astype(int)
log = open(sys.argv[2]).read().splitlines()
src = np.asarray(Image.open(sys.argv[3]).convert("RGB")).astype(int)
prefix = sys.argv[4] if len(sys.argv) > 4 else None

scale = float(next(l for l in log if l.startswith("layout scale")).split()[2])
mask = (frame[:, :, 0] == 0x12) & (frame[:, :, 1] == 0x34) & (frame[:, :, 2] == 0x56)
if not mask.any():
	sys.exit("window colour not found in the frame")


def longest_run(flags):
	"""The first and last index of the longest run of True values."""
	best, start = (0, 0, -1), None
	for i, f in enumerate(list(flags) + [False]):
		if f and start is None:
			start = i
		elif not f and start is not None:
			best = max(best, (i - start, start, i - 1))
			start = None
	return best[1], best[2]


# Stray pixels elsewhere on the desktop can share the colour, so take the longest dense run of columns, then of rows inside it.
x0, x1 = longest_run(mask.sum(axis=0) >= 8)
y0, y1 = longest_run(mask[:, x0 : x1 + 1].sum(axis=1) >= 8)
gx, gy = x0, y0
print(f"scale {scale}; grid origin in the frame ({gx}, {gy}), colour bbox {x1 - x0 + 1}x{y1 - y0 + 1} device px")

sh, sw, _ = src.shape
ch, cw = sh // 2 - 8, sw // 2  # the checkerboard region of the source


def purity(a):
	"""Share of checkerboard pixels that are pure black or white, and share whose four neighbours alternate."""
	g = a[:, :, 0]
	pure = np.isin(g, (0, 255)).mean()
	alt = ((np.abs(g[:, 1:] - g[:, :-1]) == 255).mean() + (np.abs(g[1:, :] - g[:-1, :]) == 255).mean()) / 2
	return pure, alt


print(f"{'panel':14} {'exact px':>9} {'maxdiff':>8} {'checker pure':>13} {'alternating':>12}  best shift (dx,dy) exact")
for l in log:
	if not l.startswith("panel "):
		continue
	_, name, x, y, w, h = l.split()
	x0 = gx + int(round(float(x) * scale))
	y0 = gy + int(round(float(y) * scale))
	exact = None
	best = None
	for dy in range(-2, 3):
		for dx in range(-2, 3):
			crop = frame[y0 + dy : y0 + dy + sh, x0 + dx : x0 + dx + sw]
			if crop.shape != src.shape:
				continue
			eq = (crop == src).all(axis=2).mean()
			if dx == 0 and dy == 0:
				exact = crop
			if best is None or eq > best[0]:
				best = (eq, dx, dy)
	crop = exact if exact is not None else frame[y0 : y0 + sh, x0 : x0 + sw]
	eq = (crop == src).all(axis=2).mean() if crop.shape == src.shape else float("nan")
	md = np.abs(crop - src).max() if crop.shape == src.shape else -1
	pure, alt = purity(crop[:ch, :cw])
	print(f"{name:14} {eq * 100:8.2f}% {md:8d} {pure * 100:12.2f}% {alt * 100:11.2f}%  ({best[1]},{best[2]}) {best[0] * 100:.2f}%")
	if prefix:
		Image.fromarray(crop.astype("uint8")).save(f"{prefix}-{name}.png")
