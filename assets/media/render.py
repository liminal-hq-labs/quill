#!/usr/bin/env python3
# Lay Quill's print masters out on US Letter and A4 pages with crop marks, fold ticks and cut lines, and write the PDFs and previews.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
#   python3 assets/media/render.py OUT_DIR [MASTERS_DIR]     (MASTERS_DIR defaults to assets/media/masters)
#
# Writes to OUT_DIR:
#   Quill print set - complete.pdf     every page, Letter then A4, bookmarked by paper and piece with the print instruction for each
#   <piece>-Letter.pdf, <piece>-A4.pdf  each piece on its own: option-a-floppy-labels, option-b-floppy-labels, option-a-cd, option-b-cd,
#                                      stickers, booklet (imposed for folding), shortcut-card
#   booklet-reading.pdf                the eight booklet pages at their trim size, in reading order, for the screen
#   option-a-*.png, option-b-*.png     previews the owner's side-by-side viewer pairs (floppy, cd), and stickers.png, booklet.png,
#                                      shortcut-card.png
#   README.txt                         what each file is and how to print it
# Every page is vector: the masters' text is outlined, and only the screenshots are images. The page captions are outlined with
# gen_media.py's type. Needs rsvg-convert (librsvg) and qpdf. Headless: no display is used.
import base64
import os
import re
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gen_media as G  # noqa: E402

PAPERS = {"Letter": (215.9, 279.4), "A4": (210.0, 297.0)}
MARK = 5.0  # crop mark length
MARK_GAP = 1.0  # beyond the bleed
HAIRLINE = 0.25 * G.POINT
PREVIEW_DPI = 110
CUT = "#7a7a7a"


# ---- reading and placing masters ------------------------------------------------------------------------------------------------------
class Master:
	def __init__(self, path):
		self.path = path
		source = open(path, encoding="utf-8").read()
		tag = re.search(r"<svg [^>]*>", source).group(0)
		self.view_box = re.search(r'viewBox="([^"]+)"', tag).group(1)
		trim = [float(value) for value in re.search(r'data-trim="([^"]+)"', tag).group(1).split()]
		self.origin_x, self.origin_y, self.width, self.height = trim
		self.bleed = float(re.search(r'data-bleed="([^"]+)"', tag).group(1))
		body = source[source.index(tag) + len(tag): source.rindex("</svg>")]
		self.body = re.sub(r'(xlink:href|href)="([^"#][^"]*\.png)"', self._inline, body)

	def _inline(self, match):
		image = os.path.normpath(os.path.join(os.path.dirname(self.path), match.group(2)))
		with open(image, "rb") as handle:
			data = base64.b64encode(handle.read()).decode("ascii")
		return '%s="data:image/png;base64,%s"' % (match.group(1), data)

	def place(self, x, y, prefix):
		"""The master with its trim's top left at x, y on the page; ids get a prefix so two masters on one page never clash."""
		body = re.sub(r'id="([^"]+)"', lambda m: 'id="%s-%s"' % (prefix, m.group(1)), self.body)
		body = re.sub(r'url\(#([^)]+)\)', lambda m: "url(#%s-%s)" % (prefix, m.group(1)), body)
		body = re.sub(r'href="#([^"]+)"', lambda m: 'href="#%s-%s"' % (prefix, m.group(1)), body)
		full_width, full_height = self.width + 2 * self.bleed, self.height + 2 * self.bleed
		return '<svg x="%s" y="%s" width="%s" height="%s" viewBox="%s">%s</svg>' % (
			G.number(x - self.bleed), G.number(y - self.bleed), G.number(full_width), G.number(full_height), self.view_box, body)


def crop_marks(x, y, width, height, bleed):
	"""Hairline marks at each corner of the trim, starting MARK_GAP beyond the bleed so they never touch the art."""
	out = []
	reach = bleed + MARK_GAP
	for corner_x, sign_x in ((x, -1), (x + width, 1)):
		for corner_y, sign_y in ((y, -1), (y + height, 1)):
			out.append(G.line(corner_x + sign_x * reach, corner_y, corner_x + sign_x * (reach + MARK), corner_y, "#000000", HAIRLINE))
			out.append(G.line(corner_x, corner_y + sign_y * reach, corner_x, corner_y + sign_y * (reach + MARK), "#000000", HAIRLINE))
	return "".join(out)


def fold_ticks_vertical(x, y, height, bleed, folds):
	"""Dashed ticks above and below the trim at each x fold."""
	out = []
	reach = bleed + MARK_GAP
	for fold in folds:
		for start, end in ((y - reach - 3.0, y - reach), (y + height + reach, y + height + reach + 3.0)):
			out.append(G.line(x + fold, start, x + fold, end, "#000000", HAIRLINE, ' stroke-dasharray="0.8 0.5"'))
	return "".join(out)


def fold_ticks_horizontal(x, y, width, bleed, folds):
	out = []
	reach = bleed + MARK_GAP
	for fold in folds:
		for start, end in ((x - reach - 3.0, x - reach), (x + width + reach, x + width + reach + 3.0)):
			out.append(G.line(start, y + fold, end, y + fold, "#000000", HAIRLINE, ' stroke-dasharray="0.8 0.5"'))
	return "".join(out)


def page_svg(width, height, body):
	return ('<?xml version="1.0" encoding="UTF-8"?>\n<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
		'width="%smm" height="%smm" viewBox="0 0 %s %s"><rect width="%s" height="%s" fill="#ffffff"/>%s</svg>\n') % (
		G.number(width), G.number(height), G.number(width), G.number(height), G.number(width), G.number(height), body)


def caption(width, title, instruction, paper):
	"""The page's heading: what it is, and how to print it."""
	out = [G.text(title, 12.0, 11.0, 9.5, "sans-bold", G.INK_800)]
	out.append(G.text("Quill " + G.VERSION + " print set · " + paper, width - 12.0, 11.0, 6.6, "sans", G.MUTED, "end"))
	svg, _ = G.paragraph(instruction, 12.0, 15.6, width - 24.0, 6.6, "sans", G.MUTED, leading=1.25)
	out.append(svg)
	return "".join(out)


# ---- the pages ---------------------------------------------------------------------------------------------------------------------------
def floppy_page(masters_dir, option, paper):
	width, height = PAPERS[paper]
	names = ("dos", "w31", "w95", "all", "documents", "documents")
	pitch_x = G.LABEL + 2 * G.LABEL_BLEED + 16.0
	pitch_y = G.LABEL + 2 * G.LABEL_BLEED + 10.0
	left = (width - (2 * pitch_x - 16.0)) / 2 + G.LABEL_BLEED
	top = 30.0 + G.LABEL_BLEED + (height - 279.4) / 2
	out = []
	style = "A, era-accurate retro" if option == "option-a" else "B, Afterglow dusk"
	out.append(caption(width, "Floppy labels, option " + style, "Six 2.75 in (69.85 mm) square 3.5 in diskette labels, the Avery 5196 size: "
		"the DOS, Windows 3.x, Windows 95 and all-in-one disks and two documents disks. Print at 100 % (actual size) on full-sheet sticker "
		"paper and cut on the crop marks; the top 2 in goes on the front of the disk and the strip folds over the edge (ticks) onto the back. "
		"The positions are not matched to a die-cut sheet.", paper))
	for index, name in enumerate(names):
		master = Master(os.path.join(masters_dir, option, "floppy-%s.svg" % name))
		x = left + (index % 2) * pitch_x
		y = top + (index // 2) * pitch_y
		out.append(master.place(x, y, "l%d" % index))
		out.append(crop_marks(x, y, G.LABEL, G.LABEL, G.LABEL_BLEED))
		out.append(fold_ticks_horizontal(x, y, G.LABEL, G.LABEL_BLEED, (G.LABEL_FACE, G.LABEL_FACE + G.LABEL_EDGE)))
	return page_svg(width, height, "".join(out))


def single_piece_page(master, paper, title, instruction, prefix, extra=lambda x, y: ""):
	width, height = PAPERS[paper]
	x = (width - master.width) / 2
	y = max(34.0, (height - master.height) / 2)
	out = [caption(width, title, instruction, paper)]
	out.append(master.place(x, y, prefix))
	out.append(extra(x, y))
	return page_svg(width, height, "".join(out))


def cd_pages(masters_dir, option, paper):
	style = "option A, era-accurate retro" if option == "option-a" else "option B, Afterglow dusk"
	disc = Master(os.path.join(masters_dir, option, "disc.svg"))
	jewel = Master(os.path.join(masters_dir, option, "jewel-front.svg"))
	tray = Master(os.path.join(masters_dir, option, "tray-card.svg"))

	def disc_cuts(x, y):
		centre_x, centre_y = x + G.DISC_PRINT_OUTER, y + G.DISC_PRINT_OUTER
		dash = ' stroke-dasharray="1.2 0.8"'
		return (G.circle(centre_x, centre_y, G.DISC_PRINT_OUTER, "none", CUT, HAIRLINE * 1.5, dash)
			+ G.circle(centre_x, centre_y, G.DISC_PRINT_INNER, "none", CUT, HAIRLINE * 1.5, dash)
			+ G.line(centre_x - 3, centre_y, centre_x + 3, centre_y, "#000000", HAIRLINE)
			+ G.line(centre_x, centre_y - 3, centre_x, centre_y + 3, "#000000", HAIRLINE))

	def tray_marks(x, y):
		return (crop_marks(x, y, G.TRAY_WIDTH, G.TRAY_HEIGHT, G.CARD_BLEED)
			+ fold_ticks_vertical(x, y, G.TRAY_HEIGHT, G.CARD_BLEED, (G.SPINE, G.TRAY_WIDTH - G.SPINE)))

	return [
		single_piece_page(disc, paper, "CD face, " + style, "The standard 120 mm disc's print area: 118 mm outside, 41 mm inside, with 1 mm "
			"of bleed both ways. On a printable disc, print it through the printer's disc tray software; on a 117 mm full-face CD label, cut "
			"on the dashed circles (the label's own hole is 41 mm). Print at 100 %.", "disc", disc_cuts),
		single_piece_page(jewel, paper, "Jewel case front inlay, " + style, "120 x 120 mm, the standard jewel case front, with 3 mm of "
			"bleed. Print at 100 % on card or heavy paper (120 to 200 g/m²), cut on the crop marks and slide it under the lid's tabs. The "
			"booklet in this set is the same size, so it can go in its place.", "jewel",
			lambda x, y: crop_marks(x, y, G.JEWEL, G.JEWEL, G.CARD_BLEED)),
		single_piece_page(tray, paper, "Jewel case tray card, " + style, "150 x 118 mm with its two 6 mm spines inside (folds at 6 and "
			"144 mm, the dashed ticks), 3 mm of bleed. Print at 100 % on paper (90 to 120 g/m²), cut on the crop marks, score and fold the "
			"spines towards you, and set it under the clear tray.", "tray", tray_marks),
	]


def sticker_page(masters_dir, paper):
	width, height = PAPERS[paper]
	names = sorted(name for name in os.listdir(os.path.join(masters_dir, "stickers")) if name.endswith(".svg"))
	gap = 9.0
	pitch = G.STICKER + 2 * G.STICKER_BLEED + gap
	left = (width - (3 * pitch - gap)) / 2 + G.STICKER_BLEED
	top = 34.0 + G.STICKER_BLEED + (height - 279.4) / 2
	out = [caption(width, "Sticker sheet", "Twelve 2 in (50.8 mm) stickers, six round and six square, three across and four down. Print at "
		"100 % on full-sheet sticker paper and cut on the dashed lines (with scissors, a punch or a cutting machine; the dashed lines are the "
		"cut path and the art runs 1.5 mm past them). The positions are not matched to a die-cut sheet.", paper)]
	dash = ' stroke-dasharray="1.2 0.8"'
	for index, name in enumerate(names):
		master = Master(os.path.join(masters_dir, "stickers", name))
		x = left + (index % 3) * pitch
		y = top + (index // 3) * pitch
		shape = re.search(r'data-shape="(\w+)"', master.body).group(1)
		clip_id = "clip%d" % index
		if shape == "round":
			clip = G.circle(x + G.R, y + G.R, G.R + G.STICKER_BLEED, "#000")
			cut = G.circle(x + G.R, y + G.R, G.R, "none", CUT, HAIRLINE * 1.5, dash)
		else:
			clip = G.rect(x - G.STICKER_BLEED, y - G.STICKER_BLEED, G.STICKER + 2 * G.STICKER_BLEED, G.STICKER + 2 * G.STICKER_BLEED, "#000",
				radius=G.STICKER_CORNER + G.STICKER_BLEED)
			cut = G.rect(x, y, G.STICKER, G.STICKER, "none", radius=G.STICKER_CORNER, stroke=CUT, stroke_width=HAIRLINE * 1.5, extra=dash)
		out.append('<clipPath id="%s">%s</clipPath><g clip-path="url(#%s)">%s</g>%s' % (clip_id, clip, clip_id, master.place(x, y, "s%d" % index), cut))
	return page_svg(width, height, "".join(out))


BOOKLET_SHEETS = (  # (left page, right page, side) for two sheets folded and nested, saddle stitched
	(8, 1, "Sheet 1, outside"), (2, 7, "Sheet 1, inside"), (6, 3, "Sheet 2, outside"), (4, 5, "Sheet 2, inside"),
)


def booklet_pages(masters_dir, paper):
	portrait_width, portrait_height = PAPERS[paper]
	width, height = portrait_height, portrait_width  # landscape
	pages = []
	for left_page, right_page, side in BOOKLET_SHEETS:
		spread_width = 2 * G.PAGE
		x = (width - spread_width) / 2
		y = (height - G.PAGE) / 2 + 6.0
		out = [caption(width, "Booklet: " + side + " (pages %d and %d)" % (left_page, right_page), "Two 240 x 120 mm sheets, each folded "
			"once to 120 mm, the jewel case size; sheet 2 nests inside sheet 1. Print the four sides double-sided, flipping on the short "
			"edge, at 100 % on paper of 90 to 120 g/m². Cut on the crop marks, fold on the centre ticks, nest, and staple twice on the fold "
			"if you like.", paper)]
		for slot, page_number in enumerate((left_page, right_page)):
			master = Master(os.path.join(masters_dir, "booklet", "page-%d.svg" % page_number))
			page_x = x + slot * G.PAGE
			clip_id = "b%d" % slot
			clip_x = page_x - G.CARD_BLEED if slot == 0 else page_x
			out.append('<clipPath id="%s">%s</clipPath><g clip-path="url(#%s)">%s</g>' % (clip_id,
				G.rect(clip_x, y - G.CARD_BLEED, G.PAGE + G.CARD_BLEED, G.PAGE + 2 * G.CARD_BLEED, "#000"), clip_id,
				master.place(page_x, y, "p%d" % page_number)))
		out.append(crop_marks(x, y, spread_width, G.PAGE, G.CARD_BLEED))
		out.append(fold_ticks_vertical(x, y, G.PAGE, G.CARD_BLEED, (G.PAGE,)))
		pages.append(page_svg(width, height, "".join(out)))
	return pages


def booklet_reading(masters_dir):
	pages = []
	for number in range(1, 9):
		master = Master(os.path.join(masters_dir, "booklet", "page-%d.svg" % number))
		pages.append(page_svg(G.PAGE, G.PAGE, master.place(0, 0, "r")))
	return pages


def card_page(masters_dir, paper):
	width, height = PAPERS[paper]
	master = Master(os.path.join(masters_dir, "booklet", "shortcut-card.svg"))
	x = (width - G.CARD_WIDTH) / 2
	out = [caption(width, "Keyboard shortcut card", "Two 6 x 4 in (152.4 x 101.6 mm) index cards, with 3 mm of bleed. Print at 100 % on "
		"card, cut on the crop marks, and keep one by the keyboard. The same shortcuts are on page 7 of the booklet.", paper)]
	for index in range(2):
		y = 36.0 + index * (G.CARD_HEIGHT + 2 * G.CARD_BLEED + 14.0)
		out.append(master.place(x, y, "c%d" % index))
		out.append(crop_marks(x, y, G.CARD_WIDTH, G.CARD_HEIGHT, G.CARD_BLEED))
	return page_svg(width, height, "".join(out))


# ---- writing PDFs and previews -----------------------------------------------------------------------------------------------------------
def convert(svg_text, out_path, file_format, work, dpi=None):
	handle, svg_path = tempfile.mkstemp(suffix=".svg", dir=work)
	with os.fdopen(handle, "w", encoding="utf-8") as stream:
		stream.write(svg_text)
	command = ["rsvg-convert", "--format", file_format, "--output", out_path]
	if dpi:
		command += ["--dpi-x", str(dpi), "--dpi-y", str(dpi)]
	try:
		subprocess.run(command + [svg_path], check=True)
	except FileNotFoundError:
		sys.exit("render.py: rsvg-convert is required (the librsvg2-bin package)")
	os.remove(svg_path)


def qpdf(arguments):
	try:
		subprocess.run(["qpdf", "--deterministic-id", *arguments], check=True)
	except FileNotFoundError:
		sys.exit("render.py: qpdf is required")


def pdf_text(words):
	"""A PDF text string: UTF-16BE with its byte order mark, as hex."""
	return "<FEFF%s>" % words.encode("utf-16-be").hex().upper()


def add_bookmarks(pdf_path, title, subject, outline_nodes, page_labels):
	"""Give a PDF a bookmark tree, a page label for each page, a title and a subject (from liminal-hq/afterglow-themes
	desktop/cd-media/render.py). outline_nodes is a list of (title, page index, children); page_labels names every page. qpdf writes
	the file with a plain cross reference table, the new objects are appended as an incremental update, and qpdf writes it again."""
	plain = pdf_path + ".plain"
	updated = pdf_path + ".updated"
	qpdf(["--object-streams=disable", "--decode-level=none", pdf_path, plain])
	with open(plain, "rb") as handle:
		data = handle.read()
	source = data.decode("latin-1")
	trailer = source[source.rindex("trailer"):]
	root = int(re.search(r"/Root (\d+) 0 R", trailer).group(1))
	size = int(re.search(r"/Size (\d+)", trailer).group(1))
	previous = int(re.search(r"startxref\s+(\d+)", source[source.rindex("startxref"):]).group(1))
	pages_object = re.search(r"\d+ 0 obj\s*<<((?:(?!endobj).)*?/Type /Pages(?:(?!endobj).)*?)>>\s*endobj", source, re.S)
	kids = [int(value) for value in re.findall(r"(\d+) 0 R", re.search(r"/Kids \[([^\]]*)\]", pages_object.group(1)).group(1))]
	if len(kids) != len(page_labels):
		sys.exit("render.py: %s has %d pages, not %d" % (pdf_path, len(kids), len(page_labels)))
	catalog = re.search(r"(?<!\d)%d 0 obj\s*<<(.*?)>>\s*endobj" % root, source, re.S).group(1)
	objects = {}
	next_number = [size]

	def allocate():
		next_number[0] += 1
		return next_number[0] - 1

	outline_root = allocate()

	def write_level(nodes, parent):
		numbers = [allocate() for _node in nodes]
		count = len(nodes)
		for index, ((node_title, page_index, children), number_value) in enumerate(zip(nodes, numbers)):
			links = ""
			if index:
				links += " /Prev %d 0 R" % numbers[index - 1]
			if index + 1 < len(numbers):
				links += " /Next %d 0 R" % numbers[index + 1]
			child_part = ""
			if children:
				child_numbers, child_count = write_level(children, number_value)
				child_part = " /First %d 0 R /Last %d 0 R /Count %d" % (child_numbers[0], child_numbers[-1], child_count)
				count += child_count
			objects[number_value] = "<< /Title %s /Parent %d 0 R%s%s /Dest [ %d 0 R /Fit ] >>" % (
				pdf_text(node_title), parent, links, child_part, kids[page_index])
		return numbers, count

	top_numbers, total = write_level(outline_nodes, outline_root)
	info = allocate()
	labels = " ".join("%d << /P %s >>" % (index, pdf_text(label)) for index, label in enumerate(page_labels))
	objects[root] = "<<%s /Outlines %d 0 R /PageMode /UseOutlines /PageLabels << /Nums [ %s ] >> >>" % (catalog.rstrip(), outline_root, labels)
	objects[outline_root] = "<< /Type /Outlines /First %d 0 R /Last %d 0 R /Count %d >>" % (top_numbers[0], top_numbers[-1], total)
	objects[info] = "<< /Title %s /Subject %s /Author %s >>" % (pdf_text(title), pdf_text(subject), pdf_text("Liminal HQ Labs"))
	out = bytearray(data if data.endswith(b"\n") else data + b"\n")
	offsets = {}
	for object_number in sorted(objects):
		offsets[object_number] = len(out)
		out += ("%d 0 obj\n%s\nendobj\n" % (object_number, objects[object_number])).encode("latin-1")
	cross_reference = len(out)
	lines = ["xref"]
	for object_number in sorted(objects):
		lines.append("%d 1" % object_number)
		lines.append("%010d 00000 n " % offsets[object_number])
	identifier = re.search(r"/ID\s*\[[^\]]*\]", trailer)
	lines.append("trailer\n<< /Size %d /Root %d 0 R /Info %d 0 R /Prev %d %s >>" % (next_number[0], root, info, previous,
		identifier.group(0) if identifier else ""))
	lines.append("startxref\n%d\n%%%%EOF\n" % cross_reference)
	out += ("\n".join(lines)).encode("latin-1")
	with open(updated, "wb") as handle:
		handle.write(bytes(out))
	qpdf(["--object-streams=generate", updated, pdf_path])
	os.remove(plain)
	os.remove(updated)


def write_pdf(pages, out_path, work):
	"""pages: list of SVG texts; one PDF page each, merged with qpdf (pages may differ in size)."""
	parts = []
	for index, svg_text in enumerate(pages):
		part = os.path.join(work, "part-%03d.pdf" % index)
		convert(svg_text, part, "pdf", work)
		parts.append(part)
	qpdf(["--empty", "--pages", *parts, "--", out_path])
	for part in parts:
		os.remove(part)


def preview_board(pieces, width, height, background="#2b2f3a"):
	"""A board for a preview: pieces are (master, x, y) with x, y the trim's top left."""
	out = [G.rect(0, 0, width, height, background)]
	for index, (master, x, y) in enumerate(pieces):
		out.append(master.place(x, y, "v%d" % index))
	return page_svg(width, height, "".join(out))


def main():
	if len(sys.argv) < 2:
		sys.exit("usage: render.py OUT_DIR [MASTERS_DIR]")
	out_dir = sys.argv[1]
	masters_dir = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "masters")
	os.makedirs(out_dir, exist_ok=True)
	work = tempfile.mkdtemp(prefix="quill-print-")
	pieces = []  # (file stem, bookmark, [(page svg, page label, print instruction)])
	for paper in PAPERS:
		for option, style in (("option-a", "option A, era-accurate retro"), ("option-b", "option B, Afterglow dusk")):
			pieces.append((option + "-floppy-labels-" + paper, "Floppy labels, " + style, paper,
				[(floppy_page(masters_dir, option, paper), "Floppy labels " + option[-1].upper(), "print at 100 %, one side")]))
		for option, style in (("option-a", "option A, era-accurate retro"), ("option-b", "option B, Afterglow dusk")):
			disc, jewel, tray = cd_pages(masters_dir, option, paper)
			letter = option[-1].upper()
			pieces.append((option + "-cd-" + paper, "CD, " + style, paper, [
				(disc, "CD face " + letter, "disc tray or full-face label, 100 %"),
				(jewel, "Jewel front " + letter, "card, 100 %"),
				(tray, "Tray card " + letter, "paper, 100 %, fold the spines"),
			]))
		pieces.append(("stickers-" + paper, "Sticker sheet", paper, [(sticker_page(masters_dir, paper), "Stickers", "sticker paper, 100 %")]))
		booklet = booklet_pages(masters_dir, paper)
		pieces.append(("booklet-" + paper, "Booklet, for folding", paper, [
			(svg_text, "Booklet " + side.lower(), "double-sided, flip on the short edge")
			for svg_text, (_left, _right, side) in zip(booklet, BOOKLET_SHEETS)]))
		pieces.append(("shortcut-card-" + paper, "Keyboard shortcut card", paper,
			[(card_page(masters_dir, paper), "Shortcut card", "card, 100 %")]))
	reading = booklet_reading(masters_dir)
	pieces.append(("booklet-reading", "Booklet pages in reading order (for the screen)", None,
		[(svg_text, "Booklet page %d" % (index + 1), "for reading, not for folding") for index, svg_text in enumerate(reading)]))

	for stem, _bookmark, _paper, pages in pieces:
		write_pdf([svg_text for svg_text, _label, _instruction in pages], os.path.join(out_dir, stem + ".pdf"), work)
	# the complete PDF
	all_pages, outline_nodes, labels = [], [], []
	for paper in list(PAPERS) + [None]:
		children = []
		start = len(all_pages)
		for stem, bookmark, piece_paper, pages in pieces:
			if piece_paper != paper:
				continue
			children.append((bookmark, len(all_pages), [("%s — %s" % (label, instruction), len(all_pages) + index, [])
				for index, (_svg, label, instruction) in enumerate(pages)] if len(pages) > 1 else []))
			for svg_text, label, _instruction in pages:
				all_pages.append(svg_text)
				labels.append("%s · %s" % (paper or "Screen", label))
		outline_nodes.append(("US Letter" if paper == "Letter" else "A4" if paper == "A4" else "For the screen", start, children))
	complete = os.path.join(out_dir, "Quill print set - complete.pdf")
	write_pdf(all_pages, complete, work)
	add_bookmarks(complete, "Quill %s print set" % G.VERSION, "Floppy labels, CD, stickers, booklet and shortcut card; pick the pages to "
		"print", outline_nodes, labels)

	# previews the side-by-side viewer pairs by name
	for option in ("option-a", "option-b"):
		labels_board = []
		for index, name in enumerate(("dos", "w31", "w95", "all", "documents")):
			labels_board.append((Master(os.path.join(masters_dir, option, "floppy-%s.svg" % name)), 8 + index * 78.0, 8))
		convert(preview_board(labels_board, 8 + 5 * 78.0, 86.0), os.path.join(out_dir, option + "-floppy.png"), "png", work, PREVIEW_DPI)
		disc = Master(os.path.join(masters_dir, option, "disc.svg"))
		jewel = Master(os.path.join(masters_dir, option, "jewel-front.svg"))
		tray = Master(os.path.join(masters_dir, option, "tray-card.svg"))
		board = [(disc, 8, 8), (jewel, 136, 8), (tray, 266, 9)]
		convert(preview_board(board, 424.0, 136.0), os.path.join(out_dir, option + "-cd.png"), "png", work, PREVIEW_DPI)
	stickers = sorted(name for name in os.listdir(os.path.join(masters_dir, "stickers")) if name.endswith(".svg"))
	convert(sticker_page(masters_dir, "Letter"), os.path.join(out_dir, "stickers.png"), "png", work, PREVIEW_DPI)
	booklet_board = [(Master(os.path.join(masters_dir, "booklet", "page-%d.svg" % number)), 8 + ((number - 1) % 4) * 128.0,
		8 + ((number - 1) // 4) * 128.0) for number in range(1, 9)]
	convert(preview_board(booklet_board, 8 + 4 * 128.0, 264.0), os.path.join(out_dir, "booklet.png"), "png", work, PREVIEW_DPI)
	convert(card_page(masters_dir, "Letter"), os.path.join(out_dir, "shortcut-card.png"), "png", work, PREVIEW_DPI)
	os.rmdir(work)
	write_readme(out_dir, len(all_pages), len(stickers))
	print("render.py: wrote %d PDFs (the complete one has %d pages) and previews to %s" % (len(pieces) + 1, len(all_pages), out_dir))


README = """Quill {version} print set
{rule}

Print-ready floppy labels, CD face, jewel case inserts, stickers, a manual
booklet and a keyboard shortcut card for Quill {version} ({edition} release).
Generated from liminal-hq-labs/quill, assets/media (gen_media.py draws the
vector masters, render.py lays them out); do not edit these files.

Quill print set - complete.pdf
    EVERYTHING IN ONE PDF ({pages} pages), so you pick the pages to print.
    The bookmarks group the pages by paper (US Letter, then A4, then the
    booklet in reading order for the screen) and name each page with its
    print instruction; the page labels say the same. Print at 100 %.

The same pages, one piece per file, Letter and A4:
    option-a-floppy-labels   Floppy labels, option A: era-accurate retro
    option-b-floppy-labels   Floppy labels, option B: Afterglow dusk
                             2.75 in square 3.5 in diskette labels (the
                             Avery 5196 size), 2 in on the front, the rest
                             folds over the edge. DOS, Windows 3.x,
                             Windows 95, all-in-one, two documents disks.
    option-a-cd, option-b-cd The CD face (118 mm print area, 41 mm hole),
                             the 120 mm jewel case front inlay and the
                             150 x 118 mm tray card with its 6 mm spines.
    stickers                 {stickers} stickers, 2 in round and square.
    booklet                  The 8-page manual, two 240 x 120 mm sheets
                             folded to the jewel case size. Print its four
                             pages double-sided, flip on the short edge.
    shortcut-card            Two 6 x 4 in index cards.
    booklet-reading.pdf      The booklet's pages in order, for the screen.

Previews: option-a-floppy.png and option-b-floppy.png, option-a-cd.png and
option-b-cd.png (pick a direction for each), stickers.png, booklet.png and
shortcut-card.png.

Crop marks sit 1 mm outside the bleed; dashed ticks are folds; dashed lines
on the disc and the stickers are cut paths. None of the layouts is matched
to a die-cut label sheet: print on full-sheet sticker paper and cut.

The version on the media (Quill {version}, serial {serial}) is one setting
(VERSION in assets/media/gen_media.py) and changes with the release.

Not yet checked: no physical print has been made.
"""


def write_readme(out_dir, pages, stickers):
	title = "Quill %s print set" % G.VERSION
	text_out = README.format(version=G.VERSION, edition=G.EDITION.lower(), rule="=" * len(title), pages=pages, stickers=stickers,
		serial=G.SERIAL)
	with open(os.path.join(out_dir, "README.txt"), "w", encoding="utf-8") as handle:
		handle.write(text_out)


if __name__ == "__main__":
	main()
