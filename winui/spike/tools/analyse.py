# Compares a screenshot of the spike window with the frame the spike presented, pixel for pixel.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
"""Usage: analyse.py SCREENSHOT.png SOURCE.png [OUTPREFIX]

SOURCE is the `frame-*.png` the spike wrote next to its exe: the exact pixels it handed to the swap chain. Its surround is
painted #123456, so the largest solid run of that colour in the screenshot gives the panel's origin. The script then reports
how many pixels equal the source exactly, the largest channel difference, how much of the one-pixel checkerboard survives, and
the best match over a few pixels of shift, which separates a half-pixel offset from a blur. With OUTPREFIX it writes the crop
and a difference image."""
import sys
import numpy as np
from PIL import Image

shot = np.asarray(Image.open(sys.argv[1]).convert("RGB")).astype(int)
src = np.asarray(Image.open(sys.argv[2]).convert("RGB")).astype(int)
prefix = sys.argv[3] if len(sys.argv) > 3 else None
sh, sw, _ = src.shape

# The pattern constants must match src/pattern.rs.
MARGIN, CHECK_W, CHECK_H = 16, 256, 128

mask = (shot[:, :, 0] == 0x12) & (shot[:, :, 1] == 0x34) & (shot[:, :, 2] == 0x56)
if not mask.any():
	sys.exit("the surround colour #123456 is not in the screenshot")


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


# Pixels elsewhere on the desktop can share the colour, so take the longest dense run of columns, then of rows inside it.
x0, x1 = longest_run(mask.sum(axis=0) >= 8)
y0, y1 = longest_run(mask[:, x0 : x1 + 1].sum(axis=1) >= 8)
print(f"source {sw}x{sh}; surround bbox in the screenshot x {x0}..{x1}, y {y0}..{y1} ({x1 - x0 + 1}x{y1 - y0 + 1})")


def stats(crop, ref):
	eq = (crop == ref).all(axis=2).mean()
	return eq, np.abs(crop - ref).max()


def check_purity(a):
	g = a[:, :, 0]
	pure = np.isin(g, (0, 255)).mean()
	alt = ((np.abs(g[:, 1:] - g[:, :-1]) == 255).mean() + (np.abs(g[1:, :] - g[:-1, :]) == 255).mean()) / 2
	return pure, alt


# The first surround pixel is the panel's top left only when the pattern leaves the corner alone, which it does.
cx, cy = x0, y0
best = None
for dy in range(-2, 3):
	for dx in range(-2, 3):
		crop = shot[cy + dy : cy + dy + sh, cx + dx : cx + dx + sw]
		if crop.shape != src.shape:
			continue
		eq = (crop == src).all(axis=2).mean()
		if best is None or eq > best[0]:
			best = (eq, dx, dy)
crop = shot[cy : cy + sh, cx : cx + sw]
if crop.shape != src.shape:
	sys.exit(f"the crop {crop.shape} does not fit the source {src.shape}; the panel may be clipped")
eq, md = stats(crop, src)
box = (slice(MARGIN, MARGIN + CHECK_H), slice(MARGIN, MARGIN + CHECK_W))
ceq, cmd = stats(crop[box], src[box])
pure, alt = check_purity(crop[box])
print(f"whole panel      exact {eq * 100:7.3f}%  max channel difference {md}")
print(f"checkerboard     exact {ceq * 100:7.3f}%  max channel difference {cmd}  pure black/white {pure * 100:.2f}%  alternating {alt * 100:.2f}%")
print(f"best shift (dx, dy) = ({best[1]}, {best[2]}) gives {best[0] * 100:.3f}% exact")
print("RESULT", "EXACT" if ceq == 1.0 else "NOT EXACT")
if prefix:
	Image.fromarray(crop.astype("uint8")).save(f"{prefix}-crop.png")
	diff = (np.abs(crop - src).max(axis=2) > 0).astype("uint8") * 255
	Image.fromarray(diff).save(f"{prefix}-diff.png")
