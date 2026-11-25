# Finds Quill's window in a Windows 3.11 screenshot and prints where its vertical scroll bar is: the bar's x, the y of its top and the
# y of the bottom of its lower arrow. DOSBox-X centres the 800 by 600 screen in the 1024 by 768 virtual display, and Windows opens
# Quill's window at the same place every time, so the bar is found from the window's frame and not hunted for in the picture.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

import sys

from PIL import Image


def dark(px) -> bool:
	return sum(px) < 30


def main() -> int:
	img = Image.open(sys.argv[1]).convert("RGB")
	w, h = img.size
	# The window's right edge: scanning left from the right edge of the picture along a row inside the page, the first pixel that is not
	# the black of the unused display.
	row = 500
	right = next((x for x in range(w - 1, 0, -1) if not dark(img.getpixel((x, row)))), None)
	# Its top: scanning down a column in the middle of the window, the first pixel that is not black (the title bar).
	col = right - 400 if right else 0
	top = next((y for y in range(h) if not dark(img.getpixel((col, y)))), None)
	if right is None or top is None:
		print("Quill's window was not found", file=sys.stderr)
		return 1
	# The title bar is 19 pixels, the menu 20, the toolbars 28 + 28 and the ruler 22 high: the page and its bar start 116 below the top
	# of the window; the lower arrow ends 561 below it with the horizontal bar showing (the default size).
	print(right - 8, top + 116, top + 561)
	return 0


if __name__ == "__main__":
	sys.exit(main())
