#!/usr/bin/env python3
# Checks zoomed text frames of tests/docs/zoomtext.rtf: each word is as wide as at 100 % times the zoom, and no two words touch.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
"""usage: python3 -I tools/zoom_text.py frames DIR [--client L,T,R,B] [--zooms 50,75,...] [--words 6,6,...]
       python3 -I tools/zoom_text.py sizes FRAME [Z [L,T,R,B]]
       python3 -I tools/zoom_text.py crop FRAME OUT [--scale N]
       python3 -I tools/zoom_text.py preview DIR
       python3 -I tools/zoom_text.py crop-preview FRAME OUT [--scale N]

`frames` reads DIR/z050.png, z075.png, z100.png, z125.png, z150.png and z200.png (whole-screen frames of a window showing
tests/docs/zoomtext.rtf at those zooms), finds the words of each line by the columns with no ink, and checks each word's ink width
against the same word at 100 % times the zoom, within the larger of 2 pixels and 4 %, and that no two words have touched. It exits 1
and names the line, the word and the widths that fail. The same check serves the Win16 and Win32 painters: only CLIENT changes.

Every frame is also checked for the size of its glyphs: lines 1, 6 and 7 are Times New Roman 12, so their ink is as tall (within 10 %,
at least a pixel). A system that draws one line's words in another font, as ReactOS did the first line of the justified paragraph until
the painter asked the context about its font (W9X-82), passes the width check, because each word is fitted to its place, and fails this.
`sizes` runs only that check on one frame of the page view (Z the zoom, default 100; L,T,R,B the page area, default the Wine window's).

`crop` writes the text block of one frame (its ink with a margin of four pixels), at SCALE times (default 1, nearest neighbour).
`preview` reads DIR/ref.png (a frame of the page view at 100 %, its window at the screen's top left corner, 860 by 631) and the
print preview's frames DIR/pv-*.png (a window at the top left corner, cut to its own size). Each frame's sheets are found by their
white (the preview's scale is the sheet's width over the paper's: 816 pixels, a letter page at 96 dpi, unless the frame's name ends in
`-wNNN`, a paper NNN pixels wide), and each sheet's words are
checked against the reference's words times that scale, as `frames` does; `pv-two-*.png` has two sheets and `pv-*-actual.png` one at
100 %. It prints each frame's scale and exits 1 on a failure.

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


# The page area of a window at the screen's top left corner, for the preview's frames.
CLIENT_AT_ORIGIN = (2, 112, 841, 598)
# A letter page's width in pixels at 96 dpi.
SHEET_W = 816


def ink_box(im, client=CLIENT):
	"""The page's text block in a frame: left, top, right, bottom, with the page's own white found from its columns."""
	left, top, right, bottom = client
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


def lines_of(im, client=CLIENT, box=None):
	"""The text lines: (top, bottom, left, right) of each run of rows with ink; the words are found by `words_with`. `box` is the text
	block when it is known (a preview's sheet), otherwise it is found in `client`."""
	if box is None:
		box = ink_box(im, client)
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


def same_size(label, bands):
	"""Lines 1, 6 and 7 of the document are the same font at the same size, so their ink is as tall: within 10 % of the first line's
	height and at least a pixel. Prints each failure and returns how many there were."""
	if len(bands) < len(WORDS):
		print(f"{label}: {len(bands)} lines found, wanted {len(WORDS)} (words overlap so that lines merge)")
		return 1
	height = lambda band: band[1] - band[0]
	want = height(bands[0])
	allow = max(1, round(want * 0.1))
	bad = 0
	for k in (5, 6):
		got = height(bands[k])
		if abs(got - want) > allow:
			print(f"{label}: line {k + 1}: ink {got} pixels tall, as line 1's {want} should be (the words are in another font)")
			bad += 1
	return bad


def compare(label, im, bands, z, ref):
	"""Checks the lines `bands` of frame `im`, shown at `z` percent, against the reference lines `ref` (the 100 % bands, and for each the
	words and the centres of its gaps). Prints each failure and returns how many there were."""
	ref_bands, ref_words = ref
	if len(bands) != len(ref_bands):
		print(f"{label}: {len(bands)} lines found, {len(ref_bands)} at 100 % (lines merged or lost)")
		return 1
	bad = 0
	for i, band in enumerate(bands):
		words, found = words_near(im, band, ref_words[i][1], ref_bands[i][2], z)
		if not found:
			print(f"{label}: line {i + 1}: two words touch")
			bad += 1
			continue
		for k, (w, r) in enumerate(zip(words, ref_words[i][0])):
			got, want = w[1] - w[0], (r[1] - r[0]) * z / 100
			# The larger of 2 pixels and 4 %, and a pixel of the 100 % word's own rounding times the zoom.
			if abs(got - want) > max(2, 0.04 * want) + z / 100:
				print(f"{label}: line {i + 1}, word {k + 1}: {got} pixels wide, wanted {want:.1f}")
				bad += 1
	return bad


def reference(im, client=CLIENT, words=WORDS):
	"""The 100 % page's lines, and for each the words and the centres of its gaps."""
	_, ref_bands = lines_of(im, client)
	if len(ref_bands) != len(words):
		print(f"zoom_text: {len(ref_bands)} lines found at 100 %, wanted {len(words)}")
		return None
	return ref_bands, [words_with(im, band, words[i]) for i, band in enumerate(ref_bands)]


def check(folder, client=CLIENT, zooms=ZOOMS, words=WORDS):
	frames = {z: load(f"{folder}/z{z:03d}.png") for z in zooms}
	ref = reference(frames[100], client, words)
	if ref is None:
		return 1
	bad = 0
	for z in zooms:
		if z == 100:
			continue
		_, bands = lines_of(frames[z], client)
		bad += compare(f"zoom {z}%", frames[z], bands, z, ref)
		bad += same_size(f"zoom {z}%", bands)
	if bad:
		print(f"zoom_text: {bad} failures")
		return 1
	print("zoom_text: ok")
	return 0


def sheets_of(im):
	"""The white sheets of a print preview frame: (left, top, right, bottom) of each, found along a row through the middle of the page
	area (below the text of the first lines) and down and up its columns."""
	px = im.load()
	w, h = im.size
	ymid = h // 2
	runs, start = [], None
	for x in range(w + 1):
		white = x < w and px[x, ymid] >= 250
		if white and start is None:
			start = x
		if not white and start is not None:
			if x - start >= 60:
				runs.append((start, x))
			start = None
	out = []
	for l, r in runs:
		# A column near the right edge, where the right margin keeps the text away.
		xm = r - 8
		t = ymid
		while t > 0 and px[xm, t - 1] >= 250:
			t -= 1
		b = ymid
		while b < h - 1 and px[xm, b + 1] >= 250:
			b += 1
		out.append((l, t, r, b + 1))
	return out


def preview(folder):
	import glob

	ref = reference(load(f"{folder}/ref.png"), CLIENT_AT_ORIGIN)
	if ref is None:
		return 1
	bad = 0
	names = sorted(glob.glob(f"{folder}/pv-*.png"))
	if not names:
		print("zoom_text: no preview frames")
		return 1
	for name in names:
		im = load(name)
		label = name.rsplit("/", 1)[-1][:-4]
		found = sheets_of(im)
		tail = label.rsplit("-w", 1)
		paper_w = int(tail[1]) if len(tail) == 2 and tail[1].isdigit() else SHEET_W
		want = 2 if "-two-" in label else 1
		if len(found) != want:
			print(f"{label}: {len(found)} sheets found, wanted {want}")
			bad += 1
			continue
		for k, (l, t, r, b) in enumerate(found):
			z = (r - l) * 100 / paper_w
			# Inside the sheet, clear of its edge and of the part a window may cut off.
			box = (l + 2, t + 2, r - 2, b - 2)
			text = ink_box_in(im, box)
			if text is None:
				print(f"{label}: sheet {k + 1}: no text")
				bad += 1
				continue
			_, bands = lines_of(im, box=text)
			print(f"zoom_text: {label}: sheet {k + 1} at {z:.1f} %")
			bad += compare(f"{label}: sheet {k + 1} ({z:.0f}%)", im, bands, z, ref)
	if bad:
		print(f"zoom_text: {bad} failures")
		return 1
	print("zoom_text: ok")
	return 0


def ink_box_in(im, rect):
	"""The inked area inside `rect` (left, top, right, bottom): left, top, right, bottom, or None."""
	left, top, right, bottom = rect
	px = im.load()
	rows, xs = [], []
	for y in range(top, bottom):
		hit = [x for x in range(left, right) if px[x, y] < INK]
		if hit:
			rows.append(y)
			xs += [hit[0], hit[-1]]
	if not rows:
		return None
	return min(xs), min(rows), max(xs) + 1, max(rows) + 1


def crop_preview(frame, out, scale):
	"""Writes the text of every sheet of a preview frame: the union of the sheets' ink with a margin of four pixels."""
	im = Image.open(frame).convert("RGB")
	grey = im.convert("L")
	boxes = []
	for l, t, r, b in sheets_of(grey):
		box = ink_box_in(grey, (l + 2, t + 2, r - 2, b - 2))
		if box is not None:
			boxes.append(box)
	if not boxes:
		print("zoom_text: no text in the frame")
		return 1
	l, t = min(b[0] for b in boxes), min(b[1] for b in boxes)
	r, b = max(b[2] for b in boxes), max(b[3] for b in boxes)
	c = im.crop((max(l - 4, 0), max(t - 4, 0), min(r + 4, im.width), min(b + 4, im.height)))
	if scale != 1:
		c = c.resize((c.width * scale, c.height * scale), Image.NEAREST)
	c.save(out)
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


def sizes(frame, zoom, client):
	im = load(frame)
	_, bands = lines_of(im, client)
	print(f"zoom_text: {len(bands)} lines, ink heights {[b - t for t, b, _, _ in bands]} at {zoom} %")
	bad = same_size(f"zoom {zoom}%", bands)
	print("zoom_text: ok" if not bad else f"zoom_text: {bad} failures")
	return 1 if bad else 0


def main(argv):
	if len(argv) >= 3 and argv[1] == "frames":
		# `--client L,T,R,B` is where the page area is (a Win16 page crop is the whole image: 0,0,W,H); `--zooms 25,50,...` the frames.
		client, zooms, words = CLIENT, ZOOMS, WORDS
		for flag, value in zip(argv[3::2], argv[4::2]):
			if flag == "--client":
				client = tuple(int(v) for v in value.split(","))
			elif flag == "--words":
				words = tuple(int(v) for v in value.split(","))
			elif flag == "--zooms":
				zooms = tuple(int(v) for v in value.split(","))
		return check(argv[2], client, zooms, words)
	if 3 <= len(argv) <= 5 and argv[1] == "sizes":
		zoom = int(argv[3]) if len(argv) > 3 else 100
		client = tuple(int(v) for v in argv[4].split(",")) if len(argv) > 4 else CLIENT
		return sizes(argv[2], zoom, client)
	if len(argv) == 3 and argv[1] == "preview":
		return preview(argv[2])
	if len(argv) >= 4 and argv[1] in ("crop", "crop-preview"):
		scale = int(argv[5]) if len(argv) == 6 and argv[4] == "--scale" else 1
		return (crop if argv[1] == "crop" else crop_preview)(argv[2], argv[3], scale)
	print(__doc__)
	return 2


if __name__ == "__main__":
	sys.exit(main(sys.argv))
