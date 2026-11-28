# Reads the frames e2e-zoom-w31.sh took of Quill on Windows 3.11 and checks that the page was drawn at the zoom the menu chose.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: check_zoom.py click FRAME BX BY0           prints where to click: the fifth line of text, a little way along it
#        check_zoom.py foot FRAME BX BY0            exits 0 when the foot of the last sheet is in the frame
#        check_zoom.py check WORKDIR BX BY0         the text session's comparisons; exits 1 and says why when one fails
#        check_zoom.py picture-at FRAME BX BY0      prints the middle of the first grey picture box
#        check_zoom.py handle FRAME BX BY0          prints the middle of the selected picture's bottom right handle
#        check_zoom.py pictures WORKDIR BX BY0      the picture session's comparisons
#        check_zoom.py crop FRAME BX BY0 BY1 OUT    writes the page area of a frame (what the goldens hold)
#        check_zoom.py sheets FRAME BX BY0 BY1 N    exits 0 when the page area shows N sheets, or fewer where the window ends first, and
#                                                   the desk (and nothing else) below the last one
#        check_zoom.py pagetop FRAME BX BY0         exits 0 when the top of the sheet is in the page area's first rows, under a strip of desk
#
# BX is the x of the vertical scroll bar and BY0 the y of the top of the page area (find_vbar.py). Everything is measured from what the
# layout fixes and Windows' fonts cannot change: the sheet's edges, and the caret, which is a two-pixel bar as tall as its line. The
# sample (tests/docs/scroll.rtf) has margins of 1.25 inches at the left, so at zoom Z the text's left edge is 120 * Z / 100 pixels from
# the sheet's left edge and a letter-size sheet is 816 * Z / 100 pixels wide.
import sys

from PIL import Image

DESK = (132, 130, 132)
# The page area is this many pixels wide: from the window's left edge to the vertical bar.
BAR_LEFT = 7
# The rows of the page area the measures look at (the horizontal bar and the status line are below them).
ROWS = 420
# How far down the page area the foot of a sheet is looked for: its last rows, above the horizontal bar.
FOOT_ROWS = 440
# The sheet's width at 100 percent, the margin, and the gaps the layout leaves between a sheet and the window's edge.
PAPER = 816
MARGIN = 120
PAD = 8


# The grey of a picture Windows 3.x does not draw, and the selection's navy, as the 64K-colour display shows them.
FACE = (198, 195, 198)
HANDLE = 7


def face(px) -> bool:
	return all(abs(a - b) <= 6 for a, b in zip(px, FACE))


def navy(px) -> bool:
	return px[0] < 40 and px[1] < 40 and 100 <= px[2] <= 160


def dark(px) -> bool:
	return sum(px) < 90


def white(px) -> bool:
	return sum(px) > 740


def load(path: str) -> Image.Image:
	return Image.open(path).convert("RGB")


def area_left(img: Image.Image, top: int) -> int:
	"""The first pixel of the page area: the window's left edge is black, the desk or the sheet starts after it."""
	return next(x for x in range(150, 400) if not dark(img.getpixel((x, top + 2))))


class Frame:
	def __init__(self, path: str, bx: int, by0: int):
		self.img = load(path)
		self.px = self.img.load()
		self.top = by0
		self.left = area_left(self.img, by0)
		self.right = bx - BAR_LEFT
		# The sheet's left edge and width, from a row inside the sheet's top margin. A sheet wider than the window runs to its edge.
		# The page area opens on the desk's gap above the sheet (CORE-233), so the row is two below the first one with white in it.
		row = next((y for y in range(by0 + 2, by0 + 80) if any(white(self.img.getpixel((x, y))) for x in range(self.left, self.right, 4))), by0) + 2
		self.page_left = next((x for x in range(self.left, self.right) if white(self.img.getpixel((x, row)))), None)
		x = self.page_left
		while x is not None and x < self.right and white(self.img.getpixel((x, row))):
			x += 1
		self.clipped = x is not None and x >= self.right
		self.page_width = None if x is None else x - self.page_left

	def caret(self):
		"""The caret: the leftmost of the columns with the longest run of black pixels inside the sheet, as (x, height, top y)."""
		if self.page_left is None:
			return None
		hi = min(self.right, self.page_left + (self.page_width or 0)) - 3
		best = (0, 0, 0)
		for x in range(self.page_left + 2, hi):
			run = start = 0
			for y in range(self.top, self.top + ROWS):
				if dark(self.px[x, y]):
					if run == 0:
						start = y
					run += 1
					if run > best[1]:
						best = (x, run, start)
				else:
					run = 0
		return best if best[1] >= 6 else None

	def face_box(self):
		"""The bounding box (left, top, right, bottom, exclusive) of the first grey picture box in the page area, or None."""
		start = next(
			(
				(x, y)
				for y in range(self.top, self.top + ROWS)
				for x in range(self.left, self.right)
				if face(self.px[x, y]) and face(self.px[x + 3, y]) and face(self.px[x, y + 3])
			),
			None,
		)
		if start is None:
			return None
		seen, todo = {start}, [start]
		x0, y0, x1, y1 = start[0], start[1], start[0], start[1]
		while todo:
			x, y = todo.pop()
			x0, y0, x1, y1 = min(x0, x), min(y0, y), max(x1, x), max(y1, y)
			for n in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
				if n not in seen and self.left <= n[0] < self.right and self.top <= n[1] < self.top + ROWS and face(self.px[n]):
					seen.add(n)
					todo.append(n)
		return x0, y0, x1 + 1, y1 + 1

	def handles(self, box):
		"""The centres and sizes of the solid black squares around `box` (a picture's handles): [(cx, cy, size)]."""
		x0, y0, x1, y1 = box
		found = []
		for y in range(max(self.top, y0 - 14), min(self.top + ROWS, y1 + 14)):
			for x in range(max(self.left, x0 - 14), min(self.right, x1 + 14)):
				if all(dark(self.px[x + i, y + j]) for i in range(HANDLE) for j in range(HANDLE)):
					found.append((x, y))
		# Each handle is one block of such windows; windows of one block touch.
		blocks = []
		for x, y in found:
			for b in blocks:
				if any(abs(x - bx) <= 2 and abs(y - by) <= 2 for bx, by in b):
					b.append((x, y))
					break
			else:
				blocks.append([(x, y)])
		out = []
		for b in blocks:
			xs, ys = [p[0] for p in b], [p[1] for p in b]
			size = max(max(xs) - min(xs), max(ys) - min(ys)) + HANDLE
			out.append(((min(xs) + max(xs) + HANDLE) // 2, (min(ys) + max(ys) + HANDLE) // 2, size))
		return out

	def ruler_marks(self):
		"""The x of every black pixel of the ruler's scale (the row a half-inch tick is drawn in), left of the vertical bar."""
		y = self.top - 11
		return [x for x in range(self.left + 24, self.right) if dark(self.px[x, y])]

	def selection(self):
		"""The bounding box of the navy selection in the page area, and whether its top rows have navy in every column of it."""
		pts = [(x, y) for y in range(self.top, self.top + ROWS) for x in range(self.left, self.right) if navy(self.px[x, y])]
		if not pts:
			return None
		x0, x1 = min(p[0] for p in pts), max(p[0] for p in pts) + 1
		y0, y1 = min(p[1] for p in pts), max(p[1] for p in pts) + 1
		solid = all(any(navy(self.px[x, y]) for y in range(y0, y0 + 3)) for x in range(x0, x1))
		return x0, y0, x1, y1, solid

	def foot(self) -> bool:
		"""True when desk shows under the sheet in the middle of the page area (the sheet has ended above the window's bottom)."""
		if self.page_left is None:
			return False
		x = min(self.page_left + 300, self.right - 10)
		return any(self.img.getpixel((x, y)) == DESK for y in range(self.top + 300, self.top + FOOT_ROWS))


def click(argv: list[str]) -> int:
	f = Frame(argv[0], int(argv[1]), int(argv[2]))
	c = f.caret()
	if c is None:
		print("no caret in the first frame", file=sys.stderr)
		return 1
	x, h, y = c
	print(x + 300, y + 4 * (h + 2) + h // 2)
	return 0


def picture_at(argv: list[str]) -> int:
	box = Frame(argv[0], int(argv[1]), int(argv[2])).face_box()
	if box is None:
		print("no grey picture box in the frame", file=sys.stderr)
		return 1
	print((box[0] + box[2]) // 2, (box[1] + box[3]) // 2)
	return 0


def handle(argv: list[str]) -> int:
	f = Frame(argv[0], int(argv[1]), int(argv[2]))
	box = f.face_box()
	hs = f.handles(box) if box else []
	if len(hs) != 8:
		print(f"{len(hs)} handles in the frame, not 8", file=sys.stderr)
		return 1
	x, y, _ = max(hs, key=lambda h: h[0] + h[1])
	print(x, y)
	return 0


def pictures(argv: list[str]) -> int:
	work, bx, by0 = argv[0], int(argv[1]), int(argv[2])
	ok = True

	def need(cond: bool, why: str) -> None:
		nonlocal ok
		if not cond:
			print("FAIL:", why, file=sys.stderr)
			ok = False

	sizes = {}
	for name, pct in [("200-select", 200), ("150", 150), ("100", 100), ("50", 50)]:
		f = Frame(f"{work}/zoompic-{name}.png", bx, by0)
		box = f.face_box()
		if box is None:
			need(False, f"{name}: no grey picture box")
			continue
		w, h = box[2] - box[0], box[3] - box[1]
		sizes[name] = (w, h)
		# The box is the picture's size at the zoom, less the one-pixel edge each side.
		need(abs(w - (64 * pct // 100 - 2)) <= 2 and abs(h - (48 * pct // 100 - 2)) <= 2, f"{name}: the picture is {w} by {h}, at {pct} percent it should be about {64 * pct // 100 - 2} by {48 * pct // 100 - 2}")
		hs = f.handles(box)
		need(len(hs) == 8, f"{name}: {len(hs)} handles around the picture, not 8")
		need(all(abs(size - HANDLE) <= 1 for _, _, size in hs), f"{name}: the handles are {[size for _, _, size in hs]} pixels, not {HANDLE} at any zoom")
		# Each handle is on the frame: at a corner or the middle of a side.
		x0, y0, x1, y1 = box
		anchors = [(x, y) for x in (x0, (x0 + x1) // 2, x1) for y in (y0, (y0 + y1) // 2, y1) if (x, y) != ((x0 + x1) // 2, (y0 + y1) // 2)]
		need(all(min(abs(cx - ax) + abs(cy - ay) for ax, ay in anchors) <= 14 for cx, cy, _ in hs), f"{name}: a handle is not on the frame's corners or sides")
	after = Frame(f"{work}/zoompic-100-dragged.png", bx, by0).face_box()
	before = sizes.get("100")
	need(after is not None and before is not None, "100: no picture box after the drag")
	if after is not None and before is not None:
		w, h = after[2] - after[0], after[3] - after[1]
		need(w >= before[0] + 20, f"the drag of 32 pixels left the picture {w} wide, it was {before[0]}")
		need(abs(h * before[0] - w * before[1]) <= 0.12 * w * before[1], f"the drag changed the picture's proportions: {before[0]} by {before[1]} to {w} by {h}")
	print("check_zoom:", "ok" if ok else "failed", f"(picture boxes {sizes}, dragged to {after})")
	return 0 if ok else 1


def foot(argv: list[str]) -> int:
	return 0 if Frame(argv[0], int(argv[1]), int(argv[2])).foot() else 1


def check(argv: list[str]) -> int:
	work, bx, by0 = argv[0], int(argv[1]), int(argv[2])
	ok = True

	def need(cond: bool, why: str) -> None:
		nonlocal ok
		if not cond:
			print("FAIL:", why, file=sys.stderr)
			ok = False

	def near(got, want, slack: int) -> bool:
		return got is not None and abs(got - want) <= slack

	top = Frame(f"{work}/zoom-200-top.png", bx, by0)
	click_f = Frame(f"{work}/zoom-200-click.png", bx, by0)
	c0, c1 = top.caret(), click_f.caret()
	need(c0 is not None and c1 is not None, "the caret is not in the 200 percent frames")
	if c0 is None or c1 is None:
		return 1
	# The first frame: a sheet at twice its size, its left margin and its caret too.
	need(top.clipped and near(top.page_left, top.left + PAD, 1), f"200: the sheet should run past the window from {PAD} pixels in, got {top.page_left} (window at {top.left})")
	need(near(c0[0] - top.page_left, 2 * MARGIN, 4), f"200: the text starts {c0[0] - top.page_left} pixels in, not {2 * MARGIN}")
	line200 = c0[1]
	need(line200 >= 30, f"200: the caret is {line200} pixels tall, a 12-point line is about 36")
	# The click put the caret where the pointer was: in the fifth line and within a letter of the pointer.
	cx, cy = click_f_target(work, bx, by0)
	need(near(c1[0], cx, 14), f"200: the click was at x={cx} and the caret is at x={c1[0]}")
	need(c1[2] - 4 <= cy <= c1[2] + c1[1] + 4, f"200: the click was at y={cy} and the caret is at y={c1[2]} to {c1[2] + c1[1]}")
	rel200 = c1[0] - click_f.page_left
	need(rel200 > 2 * MARGIN + 100, f"200: the caret is only {rel200} pixels from the sheet's edge after a click 300 pixels into the text")

	# The same caret, the same character, at every other zoom: its distance from the sheet's edge and its height follow the zoom.
	for name, pct, fits in [("150", 150, False), ("100", 100, False), ("75", 75, True), ("50", 50, True), ("width", 0, True)]:
		f = Frame(f"{work}/zoom-{name}.png", bx, by0)
		c = f.caret()
		if c is None or f.page_left is None:
			need(False, f"{name}: no sheet or no caret in the frame")
			continue
		if pct == 0:
			# Page Width: the largest zoom up to 100 at which the sheet fits with eight pixels either side.
			pct = (f.right - f.left - 2 * PAD) * 100 // PAPER
		want_rel = rel200 * pct // 200
		need(near(c[0] - f.page_left, want_rel, 4), f"{name}: the caret is {c[0] - f.page_left} pixels from the sheet's edge, at {pct} percent it should be {want_rel}")
		need(near(c[1], line200 * pct // 200, 3), f"{name}: the caret is {c[1]} pixels tall, at {pct} percent it should be {line200 * pct // 200}")
		need(f.clipped == (not fits), f"{name}: the sheet {'runs past' if f.clipped else 'fits in'} the window; at {pct} percent it should {'fit in' if fits else 'run past'} it")
		if fits:
			need(near(f.page_width, PAPER * pct // 100, 4), f"{name}: the sheet is {f.page_width} pixels wide, at {pct} percent it should be {PAPER * pct // 100}")
		else:
			need(near(f.page_left, f.left + PAD, 1), f"{name}: the sheet starts {f.page_left - f.left} pixels in, it should start {PAD}")

	# The ruler's scale starts where the text does (one pixel before it) and a half inch is 48 page units; where the sheet fits, the scale
	# ends where the column does. The same frames, so the page and the ruler agree at every zoom.
	for name, pct, fits in [("200-top", 200, False), ("150", 150, False), ("100", 100, False), ("75", 75, True), ("50", 50, True), ("width", 0, True)]:
		f = top if name == "200-top" else Frame(f"{work}/zoom-{name}.png", bx, by0)
		if pct == 0:
			pct = (f.right - f.left - 2 * PAD) * 100 // PAPER
		marks = f.ruler_marks()
		text_left = f.page_left + MARGIN * pct // 100
		need(any(abs(m - (text_left - 1)) <= 1 for m in marks), f"{name}: the ruler's scale does not start at the text's edge, {text_left - 1} (black at {marks[:6]})")
		half = text_left + 48 * pct // 100
		need(any(abs(m - half) <= 1 for m in marks), f"{name}: the ruler has no half-inch tick at {half} (black at {marks[:8]})")
		if fits:
			end = text_left + 576 * pct // 100
			need(any(abs(m - end) <= 1 for m in marks), f"{name}: the ruler's scale does not end with the column, at {end} (black at {marks[-4:]})")
	# The selection is the cells from the caret back to the start of the line, one cell for each character with none missing between.
	sel = Frame(f"{work}/zoom-200-select.png", bx, by0).selection()
	need(sel is not None, "200: Shift+Home selected nothing")
	if sel is not None:
		x0, y0, x1, y1, solid = sel
		need(near(y1 - y0, c1[1], 3), f"200: the selection is {y1 - y0} pixels tall, the line is {c1[1]}")
		need(near(x0 - click_f.page_left, 2 * MARGIN, 3), f"200: the selection starts {x0 - click_f.page_left} pixels from the sheet's edge, at the text's {2 * MARGIN}")
		need(near(x1, c1[0], 3), f"200: the selection ends at x={x1}, at the caret's {c1[0]}")
		need(solid, "200: there are gaps between the selection's cells")

	# The scroll range: the larger the zoom, the longer the page, so the more clicks on the track it takes to reach the foot.
	n200, n50 = (int(open(f"{work}/clicks-{z}").read()) for z in ("200", "50"))
	need(n200 >= n50 + 2, f"the foot took {n200} clicks at 200 percent and {n50} at 50: the range did not grow with the zoom")
	need(n200 < 14, "the foot of the sheet never came into view at 200 percent")
	need(Frame(f"{work}/zoom-200-foot.png", bx, by0).foot(), "200: the foot frame has no foot")
	end = Frame(f"{work}/zoom-200-end.png", bx, by0)
	need(not end.foot(), "200: Ctrl+End already shows the foot of the sheet, so it is not the end of the text")
	print("check_zoom:", "ok" if ok else "failed", f"(200 percent: margin {c0[0] - top.page_left}, caret {line200} tall, foot after {n200} clicks; 50 percent: {n50})")
	return 0 if ok else 1


def click_f_target(work: str, bx: int, by0: int) -> tuple[int, int]:
	"""Where `click` sent the pointer, worked out again from the first frame."""
	top = Frame(f"{work}/zoom-200-top.png", bx, by0)
	x, h, y = top.caret()
	return x + 300, y + 4 * (h + 2) + h // 2


def crop(argv: list[str]) -> int:
	bx, by0, by1 = int(argv[1]), int(argv[2]), int(argv[3])
	img = load(argv[0])
	left = area_left(img, by0)
	img.crop((left, by0, bx, by1)).save(argv[4])
	return 0


def sheets(argv: list[str]) -> int:
	"""The sheets the page area shows, from the white of a column just inside the sheet's left edge: at most `n` of them, and below the
	last one's shadow only the desk."""
	f = Frame(argv[0], int(argv[1]), int(argv[2]))
	by1, n = int(argv[3]), int(argv[4])
	if f.page_left is None:
		print("sheets: no sheet in the frame", file=sys.stderr)
		return 1
	x = f.page_left + 4
	# Runs of white down the column: a sheet each. A sheet may start above the area (scrolled) or run past its bottom.
	runs: list[list[int]] = []
	for y in range(f.top, by1):
		if white(f.img.getpixel((x, y))):
			if runs and runs[-1][1] == y:
				runs[-1][1] = y + 1
			else:
				runs.append([y, y + 1])
	if not runs or len(runs) > n:
		print(f"sheets: {len(runs)} sheets in the frame, the document has {n}", file=sys.stderr)
		return 1
	last = runs[-1][1]
	if last >= by1 - 1:
		# The last sheet runs past the window's bottom, so no desk shows under it.
		print(f"sheets: ok ({len(runs)} of {n}; the window ends inside the last)")
		return 0
	# The shadow below the sheet is two rows and the right one runs three rows lower: the desk is everything after.
	stray = [(xx, yy) for yy in range(last + 4, by1) for xx in range(f.left, f.right) if f.img.getpixel((xx, yy)) != DESK]
	if stray:
		print(f"sheets: {len(stray)} pixels under the last sheet are not the desk, the first at {stray[0]}", file=sys.stderr)
		return 1
	print(f"sheets: ok ({len(runs)} of {n}, the desk below)")
	return 0


def pagetop(argv: list[str]) -> int:
	f = Frame(argv[0], int(argv[1]), int(argv[2]))
	if f.page_left is None or f.page_width is None:
		print("pagetop: no sheet in the frame", file=sys.stderr)
		return 1
	xs = range(f.page_left + 10, f.page_left + f.page_width - 10)
	# The sheet's top is in the page area's first rows, under a strip of desk (CORE-233).
	first = next((y for y in range(f.top, f.top + ROWS) if any(white(f.px[x, y]) for x in xs)), None)
	if first is None or not 1 <= first - f.top <= 12:
		print(f"pagetop: the sheet's top is {None if first is None else first - f.top} rows down the page area, not in 1 to 12", file=sys.stderr)
		return 1
	print(f"pagetop: ok (the sheet's top {first - f.top} rows down)")
	return 0


def main() -> int:
	modes = {"pagetop": pagetop, "crop": crop, "sheets": sheets, "click": click, "foot": foot, "check": check, "picture-at": picture_at, "handle": handle, "pictures": pictures}
	if len(sys.argv) < 5 or sys.argv[1] not in modes:
		print(__doc__, file=sys.stderr)
		return 2
	return modes[sys.argv[1]](sys.argv[2:])


if __name__ == "__main__":
	sys.exit(main())
