#!/usr/bin/env python3
# Checks zoomed text frames of tests/docs/zoomtext.rtf: each word is as wide as at 100 % times the zoom, and no two words touch.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
"""usage: python3 -I tools/zoom_text.py frames DIR
       python3 -I tools/zoom_text.py crop FRAME OUT [--scale N]

`frames` reads DIR/z050.png, z075.png, z100.png, z125.png, z150.png and z200.png (whole-screen frames of a window showing
tests/docs/zoomtext.rtf at those zooms), finds the words of each line by the columns with no ink, and checks each word's ink width
against the same word at 100 % times the zoom, within the larger of 2 pixels and 4 %, and that no two words have touched. It exits 1
and names the line, the word and the widths that fail. The same check serves the Win16 and Win32 painters: only CLIENT changes.

`crop` writes the text block of one frame (its ink with a margin of four pixels), at SCALE times (default 1, nearest neighbour).
Needs Pillow."""
import sys

from PIL import Image

# The page area of the 860 by 630 window on a 900 by 700 screen: left, top, right, bottom in pixels. It stops short of the scroll bar.
CLIENT = (22, 146, 861, 632)
ZOOMS = (50, 75, 100, 125, 150, 200)
# A column or row is blank when nothing in it is darker than this (the faint pixels of a small font count as ink).
INK = 240
# The words on each line of the document, in order: the four fonts' one line, the bold and italic line, and the two justified lines.
WORDS = (6, 6, 6, 6, 7, 10, 11)


def load(path):
	return Image.open(path).convert("L")


def ink_box(im):
	"""The page's text block in a frame: left, top, right, bottom, with the page's own white found from its columns."""
	left, top, right, bottom = CLIENT
	px = im.load()
	white = [sum(1 for y in range(top, bottom) if px[x, y] >= 250) for x in range(left, right)]
	need = (bottom - top) * 8 // 10
	cols = [left + i for i, n in enumerate(white) if n >= need]
	if not cols:
		return None
	p0, p1 = cols[0] + 4, cols[-1] - 3
	mid = (p0 + p1) // 2
	# The paper's top edge is a dark line; the text is below the first white row.
	first = next((y for y in range(top, bottom) if px[mid, y] >= 250), top) + 2
	rows, xs = [], []
	for y in range(first, bottom):
		hit = [x for x in range(p0, p1) if px[x, y] < INK]
		if hit:
			rows.append(y)
			xs += [hit[0], hit[-1]]
	if not rows:
		return None
	return min(xs), min(rows), max(xs) + 1, max(rows) + 1


def lines_of(im):
	"""The text lines: (top, bottom) of each run of rows with ink, left to right words are found by `words_of`."""
	box = ink_box(im)
	if box is None:
		return None, []
	left, top, right, bottom = box
	px = im.load()
	bands, start = [], None
	for y in range(top, bottom + 1):
		busy = y < bottom and any(px[x, y] < INK for x in range(left, right))
		if busy and start is None:
			start = y
		if not busy and start is not None:
			bands.append((start, y))
			start = None
	# The caret sits alone on a last empty line, at most a few pixels wide.
	keep = []
	for t, b in bands:
		xs = [x for x in range(left, right) if any(px[x, y] < INK for y in range(t, b))]
		if xs and xs[-1] - xs[0] >= 4:
			keep.append((t, b, xs[0], xs[-1] + 1))
	return box, keep


def gaps_of(im, band):
	"""The blank gaps between the inked columns of one line: (width, left, right) of each, in order."""
	t, b, x0, x1 = band
	px = im.load()
	inked = [any(px[x, y] < INK for y in range(t, b)) for x in range(x0, x1)]
	gaps, start = [], None
	for i, on in enumerate(inked):
		if not on and start is None:
			start = i
		if on and start is not None:
			gaps.append((i - start, x0 + start, x0 + i))
			start = None
	return gaps


def words_with(im, band, count):
	"""Splits a line at its `count - 1` widest gaps: the words as (left, right) in pixels, and the centres of the gaps."""
	t, b, x0, x1 = band
	widest = sorted(gaps_of(im, band), key=lambda g: (-g[0], g[1]))[: count - 1]
	widest.sort(key=lambda g: g[1])
	edges = [x0] + [v for g in widest for v in (g[1], g[2])] + [x1]
	words = [(edges[i], edges[i + 1]) for i in range(0, len(edges), 2)]
	return words, [(g[1] + g[2]) / 2 for g in widest]


def words_near(im, band, centres, ref_left, z):
	"""Splits a line at the gaps nearest where the 100 % line's gaps would be at zoom `z`: the words as (left, right), and whether
	every gap was found (when none is blank near a place, two words have touched)."""
	t, b, x0, x1 = band
	gaps = gaps_of(im, band)
	cuts, found = [], True
	for c in centres:
		at = x0 + (c - ref_left) * z / 100
		near = [g for g in gaps if abs((g[1] + g[2]) / 2 - at) <= 2 + z / 100]
		if not near:
			found = False
			continue
		cuts.append(max(near, key=lambda g: (g[0], -abs((g[1] + g[2]) / 2 - at))))
	cuts.sort(key=lambda g: g[1])
	edges = [x0] + [v for g in cuts for v in (g[1], g[2])] + [x1]
	return [(edges[i], edges[i + 1]) for i in range(0, len(edges), 2)], found


def check(folder):
	frames = {z: load(f"{folder}/z{z:03d}.png") for z in ZOOMS}
	_, ref_bands = lines_of(frames[100])
	if len(ref_bands) != len(WORDS):
		print(f"zoom_text: {len(ref_bands)} lines found at 100 %, wanted {len(WORDS)}")
		return 1
	counts = WORDS
	ref = [words_with(frames[100], band, counts[i]) for i, band in enumerate(ref_bands)]
	bad = 0
	for z in ZOOMS:
		if z == 100:
			continue
		_, bands = lines_of(frames[z])
		if len(bands) != len(ref_bands):
			print(f"zoom {z}%: {len(bands)} lines found, {len(ref_bands)} at 100 % (lines merged or lost)")
			bad += 1
			continue
		for i, band in enumerate(bands):
			words, found = words_near(frames[z], band, ref[i][1], ref_bands[i][2], z)
			if not found:
				print(f"zoom {z}%: line {i + 1}: two words touch")
				bad += 1
				continue
			for k, (w, r) in enumerate(zip(words, ref[i][0])):
				got, want = w[1] - w[0], (r[1] - r[0]) * z / 100
				# The larger of 2 pixels and 4 %, and a pixel of the 100 % word's own rounding times the zoom.
				if abs(got - want) > max(2, 0.04 * want) + z / 100:
					print(f"zoom {z}%: line {i + 1}, word {k + 1}: {got} pixels wide, wanted {want:.1f}")
					bad += 1
	if bad:
		print(f"zoom_text: {bad} failures")
		return 1
	print("zoom_text: ok")
	return 0


def crop(frame, out, scale):
	im = Image.open(frame).convert("RGB")
	box, _ = lines_of(im.convert("L"))
	if box is None:
		print("zoom_text: no text in the frame")
		return 1
	l, t, r, b = box
	c = im.crop((max(l - 4, 0), max(t - 4, 0), min(r + 4, im.width), min(b + 4, im.height)))
	if scale != 1:
		c = c.resize((c.width * scale, c.height * scale), Image.NEAREST)
	c.save(out)
	return 0


def main(argv):
	if len(argv) == 3 and argv[1] == "frames":
		return check(argv[2])
	if len(argv) >= 4 and argv[1] == "crop":
		scale = int(argv[5]) if len(argv) == 6 and argv[4] == "--scale" else 1
		return crop(argv[2], argv[3], scale)
	print(__doc__)
	return 2


if __name__ == "__main__":
	sys.exit(main(sys.argv))
