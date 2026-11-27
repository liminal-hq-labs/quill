# Reads the frames of the Windows 3.11 print preview test: cuts the client area out of a screenshot, counts the sheets in it, and checks
# that the words of a sheet are as wide as at 100 percent times the preview's percentage.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
"""usage: check_preview.py client FRAME BX BY0 OUT        writes the window's client area (what the goldens hold)
       check_preview.py sheets CLIENT N                  the sheets of a client frame: N of them, white, on the desk
       check_preview.py words CLIENT PAGE100 [--max-pct] the words of CLIENT's one sheet against the 100 percent page area PAGE100

The window is maximised on DOSBox-X's 800 by 600 screen: its client area starts 78 pixels under the page area's top (BY0) and is 800 by 562
pixels, ending at the vertical bar's right edge (BX plus 10). Needs Pillow."""
import importlib.util
import os
import sys

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
BAR_H = 32
STATUS_H = 19
PAPER = (816, 1056)
DESK = (132, 130, 132)


def white(px) -> bool:
	return sum(px) > 740


def area_left(img: Image.Image, top: int) -> int:
	return next(x for x in range(150, 400) if sum(img.getpixel((x, top + 2))) >= 90)


def client(argv) -> int:
	bx, by0 = int(argv[1]), int(argv[2])
	img = Image.open(argv[0]).convert("RGB")
	left = area_left(img, by0)
	img.crop((left, by0 - 78, bx + 10, by0 + 484)).save(argv[3])
	return 0


def sheets(argv) -> int:
	img = Image.open(argv[0]).convert("RGB")
	want = int(argv[1])
	w, h = img.size
	# A row in the sheets' bottom margin, which has no text.
	y = h - STATUS_H - 28
	runs, start = [], None
	for x in range(w + 1):
		on = x < w and white(img.getpixel((x, y)))
		if on and start is None:
			start = x
		if not on and start is not None:
			if x - start > 20:
				runs.append((start, x))
			start = None
	if len(runs) != want:
		print(f"sheets: {len(runs)} sheets in the frame, wanted {want}", file=sys.stderr)
		return 1
	# The desk is on each side of a sheet: the pixel just outside its left edge.
	for left, _ in runs:
		if img.getpixel((left - 1, y)) != DESK:
			print(f"sheets: the sheet at {left} is not on the desk", file=sys.stderr)
			return 1
	print(f"sheets: ok ({len(runs)})")
	return 0


def words(argv) -> int:
	spec = importlib.util.spec_from_file_location("zoom_text", os.path.join(HERE, "..", "..", "tools", "zoom_text.py"))
	zt = importlib.util.module_from_spec(spec)
	spec.loader.exec_module(zt)
	pv = Image.open(argv[0]).convert("RGB")
	ref = Image.open(argv[1]).convert("L")
	w, h = pv.size
	area_h = h - BAR_H - STATUS_H
	pct = max(10, min(100, min((w - 32) * 100 // PAPER[0], (area_h - 32) * 100 // PAPER[1])))
	# The top of the sheet, where the text is: the sheet's shadow below it is not a line.
	sheet = pv.crop((0, BAR_H, w, BAR_H + area_h // 2)).convert("L")
	_, ref_bands = zt.lines_of(ref, (0, 0, ref.width, ref.height))
	if len(ref_bands) != len(zt.WORDS):
		print(f"words: {len(ref_bands)} lines at 100 percent, wanted {len(zt.WORDS)}")
		return 1
	refs = [zt.words_with(ref, band, zt.WORDS[i]) for i, band in enumerate(ref_bands)]
	_, bands = zt.lines_of(sheet, (0, 0, sheet.width, sheet.height))
	if len(bands) != len(ref_bands):
		print(f"words: {len(bands)} lines at {pct} percent, {len(ref_bands)} at 100 (lines merged or lost)")
		return 1
	bad = 0
	for i, band in enumerate(bands):
		# The line's ink, from its first to its last pixel: as wide as at 100 percent times the percentage, within the larger of 3 pixels
		# and 4 percent (a squashed or stretched line fails here).
		got, want = band[3] - band[2], (ref_bands[i][3] - ref_bands[i][2]) * pct / 100
		if abs(got - want) > max(3, 0.04 * want) + pct / 100:
			print(f"words: {pct} percent, line {i + 1}: {got} pixels wide, wanted {want:.1f}")
			bad += 1
		# Each word of the first four lines (the same words in four fonts), within the larger of 3 pixels and 15 percent: at this size the
		# system's fonts are a few pixels tall and each glyph a whole number of them, so a word may be a pixel off per letter. The
		# other lines run their words together at 45 percent, so only their extent is read.
		if i >= 4:
			continue
		got_words, _ = zt.words_near(sheet, band, refs[i][1], ref_bands[i][2], pct)
		for k, (a, b) in enumerate(zip(got_words, refs[i][0])):
			got, want = a[1] - a[0], (b[1] - b[0]) * pct / 100
			if abs(got - want) > max(3, 0.15 * want) + pct / 100:
				print(f"words: {pct} percent, line {i + 1}, word {k + 1}: {got} pixels wide, wanted {want:.1f}")
				bad += 1
	if bad:
		print(f"words: {bad} failures")
		return 1
	print(f"words: ok at {pct} percent")
	return 0


def main() -> int:
	modes = {"client": client, "sheets": sheets, "words": words}
	if len(sys.argv) < 3 or sys.argv[1] not in modes:
		print(__doc__)
		return 2
	return modes[sys.argv[1]](sys.argv[2:])


if __name__ == "__main__":
	sys.exit(main())
