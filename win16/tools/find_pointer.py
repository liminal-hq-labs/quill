# Finds the mouse pointer (Windows' arrow) in a screenshot of the Windows 3.11 guest and prints the x and y of its tip. The guest's
# pointer moves by what the emulator is told, not to where the host's does, so the scroll test steers it by looking at where it is.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

import sys

from PIL import Image

# The arrow's outline, as the standard pointer draws it: a column down the left, a diagonal back from the tip and the tail.
ARROW = [
	"##..........",
	"#.#.........",
	"#..#........",
	"#...#.......",
	"#....#......",
	"#.....#.....",
	"#......#....",
	"#.......#...",
	"#........#..",
	"#.........#.",
	"#......#####",
	"#...#..#....",
	"#..##..#....",
	"#.#.##..#...",
	"##...#..#...",
	"#.....#..#..",
	"......#..#..",
	".......#..#.",
	".......#..#.",
	"........##..",
]


def main() -> int:
	img = Image.open(sys.argv[1]).convert("L")
	w, h = img.size
	px = img.load()
	dark = [[px[x, y] < 30 for x in range(w)] for y in range(h)]
	cells = [(x, y) for y, row in enumerate(ARROW) for x, c in enumerate(row) if c == "#"]
	# The inside of the arrow's head is white: the black of the unused display would otherwise match everywhere.
	inside = [(x, y) for y in range(2, 9) for x in range(1, y - 1) if ARROW[y][x] == "."]
	for y in range(h - len(ARROW)):
		row = dark[y]
		for x in range(w - 12):
			if not (row[x] and row[x + 1]):
				continue
			if all(dark[y + dy][x + dx] for dx, dy in cells) and not any(dark[y + dy][x + dx] for dx, dy in inside):
				print(x, y)
				return 0
	print("no pointer found", file=sys.stderr)
	return 1


if __name__ == "__main__":
	sys.exit(main())
