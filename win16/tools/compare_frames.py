# Checks the frames e2e-scroll-w31.sh took: the vertical bar's thumb moves down with the arrow, the track and Page Down, the foot of the
# sheet (its shadow and the desk below it) comes into view at the end of the page, and Ctrl+Home brings the first frame back.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

import sys

from PIL import Image

STEPS = ["scroll-0-start", "scroll-1-arrow", "scroll-2-track", "scroll-3-pgdn", "scroll-4-end", "scroll-5-home"]

# Where the vertical bar is in the screenshots (find_vbar.py): its left edge, away from the pointer that sits on it, and the track.
BAR_X = 978
TRACK = (317, 730)
# The bottom of the page area, where the desk shows once the foot of the sheet is in view.
FOOT = (300, 900, 734, 744)


def thumb_top(img: Image.Image) -> int:
	"""The y of the thumb's top edge: the first dark row of the bar's left edge inside the track."""
	for y in range(*TRACK):
		if sum(img.getpixel((BAR_X, y))) < 60:
			return y
	return TRACK[1]


def desk_at_foot(img: Image.Image) -> bool:
	left, right, top, bottom = FOOT
	return any(
		img.getpixel((x, y)) != (255, 255, 255) for y in range(top, bottom) for x in range(left, right, 7)
	)


def main() -> int:
	work = sys.argv[1]
	frames = [Image.open(f"{work}/{name}.png").convert("RGB") for name in STEPS]
	thumb = [thumb_top(f) for f in frames]
	print("thumb top:", dict(zip(STEPS, thumb)))
	ok = True

	def need(cond: bool, why: str) -> None:
		nonlocal ok
		if not cond:
			print(why, file=sys.stderr)
			ok = False

	need(thumb[1] > thumb[0], "the arrow did not move the thumb down")
	need(thumb[2] > thumb[1], "the track did not move the thumb on")
	need(thumb[3] > thumb[2], "Page Down did not move the thumb on")
	need(thumb[4] >= thumb[3], "Ctrl+End moved the thumb back")
	need(thumb[5] == thumb[0], "Ctrl+Home did not bring the thumb back to the top")
	need(not desk_at_foot(frames[0]), "the foot of the sheet showed at the top of the page")
	need(desk_at_foot(frames[4]), "the foot of the sheet did not come into view at the end")
	need(not desk_at_foot(frames[5]), "Ctrl+Home left the foot of the sheet in view")
	return 0 if ok else 1


if __name__ == "__main__":
	sys.exit(main())
