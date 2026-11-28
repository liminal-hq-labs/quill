#!/usr/bin/env python3
# Draw Quill's printable media set as vector masters: floppy labels, the CD face, jewel case front and tray card, stickers, a booklet
# and a shortcut card.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
#   python3 assets/media/gen_media.py [OUT_DIR]     (default: assets/media/masters)
#
# Writes one SVG per piece, every length in millimetres (the viewBox is the piece at its trim size, plus its bleed on every side):
#   option-a/  era-accurate retro: each disk in its own period (DOS blue screen and block characters, the Windows 3.1 grey bevel and
#              Program Manager group, the Windows 95 teal desktop and Start menu)
#   option-b/  modern Afterglow dusk: Quill's ink, parchment and gold, with the stepped dusk bands of the Afterglow palette
#              Each option has floppy-dos, floppy-w31, floppy-w95, floppy-all, floppy-documents, disc, jewel-front and tray-card.
#   stickers/  twelve 2 in stickers, round and square
#   booklet/   the eight 120 mm pages of the manual, and shortcut-card.svg
# render.py lays these out on Letter and A4 pages with crop marks and cut lines and writes the PDFs and previews.
#
# Every word is outlined from the Desk faces in assets/fonts/desk (SIL Open Font License 1.1, which allows embedding the outlines), so
# no font is needed to render or print a master; each text path carries data-text and data-pt. The pixel icons come from the Windows
# 3.1 icons in assets/icon, the gold ring and nib from assets/icon/quill.svg, and the screenshots are linked from docs/media by a
# relative path (render.py inlines them). Deterministic: no randomness, no clock.
import math
import os
import re
import struct
import sys

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
import outline  # noqa: E402

# ---- the words: change the version here and run the generator and the renderer --------------------------------------------------------------
VERSION = "0.1.0"  # the release the media is worded for
EDITION = "Demo"  # what that release is
YEAR = "2026"
REPOSITORY = "github.com/liminal-hq-labs/quill"
SITE = "labs.liminalhq.ca/quill"
LICENCE = "Apache-2.0 OR MIT"
COPYRIGHT = "© " + YEAR + " Liminal HQ, Scott Morris"
LABS = "A Liminal HQ Labs experiment"
HONEST = "Experimental: every build has run in emulators, virtual machines and Wine only, never on real hardware."
HONEST_SHORT = "Experimental · tried in emulators and Wine only"
TRADEMARKS = "MS-DOS and Windows are trademarks of Microsoft; ReactOS is a trademark of the ReactOS project. Quill is not made, endorsed or supported by either."
SERIAL_PREFIX = "QD05"  # the catalogue prefix of the printed serial (INS-25)
TAGLINE = "A small word processor for DOS, Windows 3.x and Windows 95, written in Rust."


def serial(version=VERSION):
	"""The printed serial, the owner's catalogue number (INS-25): SERIAL_PREFIX, then the major version as one hex digit, the minor as
	two and the patch as one (the Fizz scheme of the release media plan, version in the serial), so 0.1.0 is QD05-0010."""
	major, minor, patch = (int(part) for part in version.split("."))
	if major > 15 or minor > 255 or patch > 15:
		sys.exit("gen_media.py: the version does not fit the serial (major and patch under 16, minor under 256)")
	return "%s-%X%02X%X" % (SERIAL_PREFIX, major, minor, patch)


VERSION_LINE = "Version " + VERSION + " · " + EDITION
SERIAL = serial()

# The disk set: one disk per system, each complete with its installer (docs/roadmap.md, Release disks), and the all-in-one disk of the
# release media plan. The volume label is what DIR and VOL print.
DISKS = {
	"dos": {
		"short": "DOS", "long": "DOS", "volume": "QUILL DOS", "image": "QUILL-DOS.IMG", "program": "QUILLDOS.EXE",
		"run": "A:\\SETUP", "needs": "386 or later · 4 MB · VGA or text mode", "edge": "QUILL · DOS",
	},
	"w31": {
		"short": "WIN 3.X", "long": "Windows 3.1 and 3.11", "volume": "QUILL W31", "image": "QUILL-W31.IMG", "program": "QUILL.EXE",
		"run": "File, Run: A:\\SETUP", "needs": "Windows 3.1 or 3.11", "edge": "QUILL · WINDOWS 3.X",
	},
	"w95": {
		"short": "WIN 95", "long": "Windows 95, 98 and Me", "volume": "QUILL W95", "image": "QUILL-W95.IMG", "program": "QUILL32.EXE",
		"run": "Start, Run: A:\\SETUP", "needs": "Pentium · Windows 95, 98 or Me", "edge": "QUILL · WINDOWS 95",
	},
	"all": {
		"short": "ALL", "long": "DOS, Windows 3.x and Windows 95", "volume": "QUILL", "image": "QUILL-" + VERSION + ".IMG",
		"program": "QUILLDOS.EXE · QUILL.EXE · QUILL32.EXE", "run": "A:\\SETUP · A:\\WIN31\\SETUP · A:\\WIN95\\SETUP",
		"needs": "One disk for all three systems", "edge": "QUILL · ALL SYSTEMS",
	},
}

FEATURES = (
	"Fonts, sizes, bold, italic, underline and colour",
	"Tab stops with dot leaders, and lists",
	"Headers and footers with page numbers",
	"Find and replace, undo and redo",
	"Spelling in Canadian, US or UK English",
	"Its own .QLD files, RTF and plain text",
)

SCREENSHOTS = (  # (file in docs/media, pixel size, caption)
	("m2-letter-dos-colour.png", (720, 400), "DOS text mode, in DOSBox"),
	("install-w31-quill-running.png", (800, 584), "Windows 3.11, in DOSBox-X"),
	("m3-letter-win95.png", (640, 480), "Windows 95, in DOSBox-X"),
)

# ---- the physical sizes, in mm ---------------------------------------------------------------------------------------------------------------
POINT = 25.4 / 72  # one point
MIN_POINTS = 6.0  # the smallest type anywhere
LABEL = 2.75 * 25.4  # 69.85: the 2.75 in square 3.5 in diskette label (Avery 5196 size)
LABEL_FACE = 2.0 * 25.4  # 50.8: the part on the front of the disk
LABEL_EDGE = 3.3  # the disk's edge, which the label folds over
LABEL_BACK = LABEL - LABEL_FACE - LABEL_EDGE  # 15.75: the part on the back, upside down so it reads when the disk is turned over
LABEL_BLEED = 1.5
DISC_RADIUS = 60.0  # a 120 mm disc
DISC_PRINT_OUTER = 59.0  # 118 mm: the printable outer diameter
DISC_PRINT_INNER = 20.5  # 41 mm: the printable inner diameter (and a label's centre hole)
DISC_BLEED = 1.0
JEWEL = 120.0  # the jewel case front inlay, square
TRAY_WIDTH, TRAY_HEIGHT, SPINE = 150.0, 118.0, 6.0  # the tray card, with its two 6 mm spines inside the 150 mm
CARD_BLEED = 3.0
STICKER = 2.0 * 25.4  # 50.8: 2 in round and square stickers
STICKER_BLEED = 1.5
STICKER_CORNER = 3.0
PAGE = 120.0  # a booklet page, the jewel case's front: the booklet doubles as the CD insert
CARD_WIDTH, CARD_HEIGHT = 6 * 25.4, 4 * 25.4  # the shortcut card: a 6 x 4 in index card

# ---- the palette -----------------------------------------------------------------------------------------------------------------------------
# Quill's own (D70; site/css/site.css and assets/hero.svg)
INK_900, INK_800, INK_700, INK_600 = "#070d24", "#0a1230", "#16275a", "#2b3b72"
PARCHMENT_100, PARCHMENT_200, PARCHMENT_300 = "#fffaf0", "#fff4d6", "#e9dcb0"
AMBER, AMBER_DEEP, GOLD_LOW, GOLD_HIGH = "#f0a830", "#8a5600", "#b8893a", "#f6dca0"
MUTED = "#3d4670"
RULE = "#d9cba0"
# The 16 VGA text colours that matter here
VGA_BLACK, VGA_BLUE, VGA_CYAN, VGA_RED = "#000000", "#0000aa", "#00aaaa", "#aa0000"
VGA_GREY, VGA_DARK, VGA_LIGHT_CYAN, VGA_YELLOW, VGA_WHITE = "#aaaaaa", "#555555", "#55ffff", "#ffff55", "#ffffff"
# Windows 3.1 and 95 system colours
WIN_FACE, WIN_SHADOW, WIN_LIGHT, WIN_DARK = "#c0c0c0", "#808080", "#ffffff", "#000000"
WIN_NAVY, WIN_TEAL, WIN_SKY = "#000080", "#008080", "#a6caf0"  # WIN_SKY: the 3.11 active title bar Quill's screenshots show
WIN_YELLOW = "#ffff00"
# Afterglow dusk: the boot palette of liminal-hq/afterglow-themes desktop/boot/palette.json, night to gold
DUSK = ("#1f2554", "#2a3164", "#35366a", "#42396c", "#523d6c", "#643f68", "#7a4562", "#b8893a", "#e89930", "#ffaa40", "#ffd08e")
RUST = "#a4441c"

FONT_DIR = os.path.join(ROOT, "assets", "fonts", "desk")
ICON_SVG = os.path.join(ROOT, "assets", "icon", "quill.svg")
ICON_31 = os.path.join(ROOT, "assets", "icon", "QUILL31.ICO")
ICON_ALL = os.path.join(ROOT, "assets", "icon", "QUILL.ICO")
MEDIA_DIR = os.path.join(ROOT, "docs", "media")

FACES = {
	"serif": "desk-serif.ttf", "serif-bold": "desk-serif-bold.ttf", "serif-italic": "desk-serif-italic.ttf",
	"sans": "desk-sans.ttf", "sans-bold": "desk-sans-bold.ttf", "sans-italic": "desk-sans-italic.ttf",
	"mono": "desk-mono.ttf", "mono-bold": "desk-mono-bold.ttf",
}
_fonts = {}


def font(face):
	if face not in _fonts:
		_fonts[face] = outline.Font(os.path.join(FONT_DIR, FACES[face]))
	return _fonts[face]


def number(value):
	text = ("%.3f" % value).rstrip("0").rstrip(".")
	return "0" if text in ("-0", "") else text


def attribute(words):
	return words.replace("&", "&amp;").replace('"', "&quot;").replace("<", "&lt;").replace(">", "&gt;")


# ---- drawing ---------------------------------------------------------------------------------------------------------------------------------
def rect(x, y, width, height, fill, radius=0, stroke=None, stroke_width=0.25, extra=""):
	parts = ['<rect x="%s" y="%s" width="%s" height="%s"' % (number(x), number(y), number(width), number(height))]
	if radius:
		parts.append(' rx="%s"' % number(radius))
	parts.append(' fill="%s"' % fill)
	if stroke:
		parts.append(' stroke="%s" stroke-width="%s"' % (stroke, number(stroke_width)))
	return "".join(parts) + extra + "/>"


def circle(cx, cy, radius, fill, stroke=None, stroke_width=0.25, extra=""):
	text = '<circle cx="%s" cy="%s" r="%s" fill="%s"' % (number(cx), number(cy), number(radius), fill)
	if stroke:
		text += ' stroke="%s" stroke-width="%s"' % (stroke, number(stroke_width))
	return text + extra + "/>"


def line(x1, y1, x2, y2, stroke, width=0.25, extra=""):
	return '<line x1="%s" y1="%s" x2="%s" y2="%s" stroke="%s" stroke-width="%s"%s/>' % (
		number(x1), number(y1), number(x2), number(y2), stroke, number(width), extra)


def polygon(points, fill):
	return '<polygon points="%s" fill="%s"/>' % (" ".join("%s,%s" % (number(x), number(y)) for x, y in points), fill)


def text_width(words, face, points, tracking=0.0):
	return outline.text_width(font(face), words, points * POINT, tracking)


def text(words, x, y, points, face="sans", fill=INK_800, anchor="start", tracking=0.0, transform=None):
	"""One line of outlined text with its baseline at y."""
	if points < MIN_POINTS:
		sys.exit("gen_media.py: %r is set at %s pt, under the %s pt minimum" % (words, points, MIN_POINTS))
	width = text_width(words, face, points, tracking)
	if anchor == "middle":
		x -= width / 2
	elif anchor == "end":
		x -= width
	data, _ = outline.text_path(font(face), words, points * POINT, x, y, tracking)
	extra = ' transform="%s"' % transform if transform else ""
	return '<path data-text="%s" data-pt="%s" fill="%s"%s d="%s"/>' % (attribute(words), number(points), fill, extra, data)


def runs(pieces, x, y, points, anchor="start"):
	"""Several runs on one baseline: pieces are (words, face, fill)."""
	total = sum(text_width(words, face, points) for words, face, _fill in pieces)
	if anchor == "middle":
		x -= total / 2
	elif anchor == "end":
		x -= total
	out = []
	for words, face, fill in pieces:
		out.append(text(words, x, y, points, face, fill))
		x += text_width(words, face, points)
	return "".join(out)


def wrap(words, face, points, width):
	lines, current = [], ""
	for word in words.split(" "):
		trial = current + " " + word if current else word
		if not current or text_width(trial, face, points) <= width:
			current = trial
		else:
			lines.append(current)
			current = word
	if current:
		lines.append(current)
	return lines


def paragraph(words, x, y, width, points, face="sans", fill=INK_800, leading=1.32, anchor="start"):
	"""Wrapped text from the first baseline at y; returns the SVG and the baseline after the last line."""
	out = []
	for one in wrap(words, face, points, width):
		out.append(text(one, x, y, points, face, fill, anchor))
		y += points * POINT * leading
	return "".join(out), y


def arc_text(words, cx, cy, radius, points, face, fill, top=True, tracking=0.0, centre=None):
	"""Text round a circle, centred at the top (reading clockwise, glyphs pointing out, baseline on the radius) or at the bottom
	(reading left to right, glyphs pointing in, baseline on the radius)."""
	size = points * POINT
	total = text_width(words, face, points, tracking)
	out = []
	for index, character in enumerate(words):
		if character == " ":
			continue
		before = text_width(words[:index], face, points, tracking) + (tracking if index else 0)
		advance = text_width(character, face, points)
		middle = before + advance / 2 - total / 2
		if centre is None:
			centre = -90 if top else 90
		if top:
			angle = math.radians(centre) + middle / radius
			turn = math.degrees(angle) + 90
		else:
			angle = math.radians(centre) - middle / radius
			turn = math.degrees(angle) - 90
		x = cx + radius * math.cos(angle)
		y = cy + radius * math.sin(angle)
		data, _ = outline.text_path(font(face), character, size, -advance / 2, 0)
		out.append('<path fill="%s" transform="translate(%s %s) rotate(%s)" d="%s"/>' % (fill, number(x), number(y), number(turn), data))
	return '<g data-text="%s" data-pt="%s">%s</g>' % (attribute(words), number(points), "".join(out))


def cell_text(words, x, y, cell_width, points, fill, face="mono"):
	"""Monospaced text whose every character sits on a cell of cell_width (the advance of Desk Mono is 0.6 em)."""
	return text(words, x, y, points, face, fill, tracking=cell_width - 0.6 * points * POINT)


def mono_points(cell_width):
	return cell_width / 0.6 / POINT


def bevel(x, y, width, height, raised=True, face=WIN_FACE, depth=0.3, outer=True):
	"""A Windows 3.1 and 95 bevel: light top and left, shadow bottom and right, and a black outer edge on a raised one."""
	light, dark = (WIN_LIGHT, WIN_SHADOW) if raised else (WIN_SHADOW, WIN_LIGHT)
	out = [rect(x, y, width, height, face)]
	if outer and raised:
		out.append(rect(x, y + height - depth, width, depth, WIN_DARK))
		out.append(rect(x + width - depth, y, depth, height, WIN_DARK))
		width -= depth
		height -= depth
	out.append(rect(x, y, width, depth, light))
	out.append(rect(x, y, depth, height, light))
	out.append(rect(x, y + height - depth, width, depth, dark))
	out.append(rect(x + width - depth, y, depth, height, dark))
	return "".join(out)


def triangle(cx, cy, size, pointing, fill=WIN_DARK):
	half = size / 2
	if pointing == "down":
		points = ((cx - half, cy - half / 2), (cx + half, cy - half / 2), (cx, cy + half / 2))
	elif pointing == "up":
		points = ((cx - half, cy + half / 2), (cx + half, cy + half / 2), (cx, cy - half / 2))
	else:  # right
		points = ((cx - half / 2, cy - half), (cx - half / 2, cy + half), (cx + half / 2, cy))
	return polygon(points, fill)


# ---- the icons ---------------------------------------------------------------------------------------------------------------------------
def ico_pixels(path, size):
	"""The 16-colour image of `size` pixels in a Windows icon, as rows of colours (None where the mask is transparent)."""
	data = open(path, "rb").read()
	count = struct.unpack_from("<H", data, 4)[0]
	for index in range(count):
		width, height, colours, _reserved, _planes, bits, _length, offset = struct.unpack_from("<BBBBHHII", data, 6 + 16 * index)
		if width == size and bits == 4:
			header = struct.unpack_from("<I", data, offset)[0]
			palette = [data[offset + header + 4 * i: offset + header + 4 * i + 3] for i in range(16)]
			palette = ["#%02x%02x%02x" % (blue_green_red[2], blue_green_red[1], blue_green_red[0]) for blue_green_red in palette]
			pixels_at = offset + header + 64
			stride = ((size * 4 + 31) // 32) * 4
			mask_at = pixels_at + stride * size
			mask_stride = ((size + 31) // 32) * 4
			rows = []
			for row in range(size):
				source = size - 1 - row  # bottom-up
				out = []
				for column in range(size):
					byte = data[pixels_at + source * stride + column // 2]
					value = byte >> 4 if column % 2 == 0 else byte & 15
					masked = data[mask_at + source * mask_stride + column // 8] >> (7 - column % 8) & 1
					out.append(None if masked else palette[value])
				rows.append(out)
			return rows
	sys.exit("gen_media.py: no %d-pixel 16-colour image in %s" % (size, path))


def pixel_art(rows, x, y, cell):
	"""Pixels as rectangles, one per run of a colour along a row, each overlapping the next a little so no seam shows in print."""
	out = []
	for row_index, row in enumerate(rows):
		column = 0
		while column < len(row):
			colour = row[column]
			start = column
			while column < len(row) and row[column] == colour:
				column += 1
			if colour is not None:
				out.append(rect(x + start * cell, y + row_index * cell, (column - start) * cell + 0.02, cell + 0.02, colour))
	return '<g shape-rendering="crispEdges">%s</g>' % "".join(out)


_icon_cache = {}


def pixel_icon(x, y, size_mm, pixels=32):
	key = pixels
	if key not in _icon_cache:
		_icon_cache[key] = ico_pixels(ICON_31 if pixels == 32 else ICON_ALL, pixels)
	return pixel_art(_icon_cache[key], x, y, size_mm / pixels)


def emblem(x, y, size, prefix, tile=True):
	"""Quill's icon from assets/icon/quill.svg: the gold ring forming a Q with the nib, on its navy tile or (tile=False) alone."""
	source = open(ICON_SVG).read()
	body = source[source.index(">", source.index("<svg")) + 1: source.rindex("</svg>")]
	body = re.sub(r"<title>.*?</title>", "", body)
	body = body.replace("q-q-nib-", prefix + "-")
	if not tile:
		body = re.sub(r'<rect width="1024" height="1024"[^>]*/>', "", body)
		body = re.sub(r"<ellipse[^>]*/>", "", body)
	return '<svg x="%s" y="%s" width="%s" height="%s" viewBox="0 0 1024 1024">%s</svg>' % (
		number(x), number(y), number(size), number(size), body)


# The ring's centre and radius inside the icon's 1024 box, for placing the emblem round something
EMBLEM_RING = (470, 466, 272)

BLOCK_GLYPHS = {  # a 5 x 7 block face for the DOS banner, in the spirit of the screen fonts of the time
	"Q": (".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"),
	"U": ("#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."),
	"I": ("###", ".#.", ".#.", ".#.", ".#.", ".#.", "###"),
	"L": ("#....", "#....", "#....", "#....", "#....", "#....", "#####"),
	"D": ("####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."),
	"O": (".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."),
	"S": (".####", "#....", "#....", ".###.", "....#", "....#", "####."),
}


def block_width(word):
	return sum(len(BLOCK_GLYPHS[letter][0]) for letter in word) + len(word) - 1


def block_banner(word, x, y, pixel_width, pixel_height, fill, shadow=None):
	"""A word in block characters: each pixel one text cell wide and half a cell high, as the half-block characters draw it."""
	out = []
	for layer, colour, dx, dy in ((0, shadow, pixel_width, pixel_height), (1, fill, 0, 0)):
		if colour is None:
			continue
		column = 0
		for letter in word:
			glyph = BLOCK_GLYPHS[letter]
			for row, bits in enumerate(glyph):
				for index, bit in enumerate(bits):
					if bit == "#":
						out.append(rect(x + dx + (column + index) * pixel_width, y + dy + row * pixel_height, pixel_width + 0.02, pixel_height + 0.02, colour))
			column += len(glyph[0]) + 1
	return '<g shape-rendering="crispEdges">%s</g>' % "".join(out)


def dusk_bands(x, y, width, height, colours=DUSK):
	"""The Afterglow dusk as flat bands, the stepped look of its boot screen."""
	step = height / len(colours)
	return "".join(rect(x, y + i * step, width, step + 0.02, colour) for i, colour in enumerate(colours))


def ink_gradient(identifier, x1=0, y1=0, x2=1, y2=1):
	return ('<linearGradient id="%s" x1="%s" y1="%s" x2="%s" y2="%s"><stop offset="0" stop-color="%s"/><stop offset=".6" stop-color="%s"/>'
		'<stop offset="1" stop-color="%s"/></linearGradient>') % (identifier, x1, y1, x2, y2, INK_700, INK_800, INK_900)


def gold_gradient(identifier):
	return ('<linearGradient id="%s" x1="0" y1="1" x2="1" y2="0"><stop offset="0" stop-color="%s"/><stop offset=".55" stop-color="%s"/>'
		'<stop offset="1" stop-color="%s"/></linearGradient>') % (identifier, GOLD_LOW, AMBER, PARCHMENT_300)


def floppy_shape(x, y, size, body, shutter, label):
	"""A generic 3.5 in diskette, drawn plainly."""
	unit = size / 90.0
	out = [rect(x, y, size, size, body, radius=2.5 * unit)]
	out.append(polygon(((x + size - 6 * unit, y), (x + size, y + 6 * unit), (x + size, y)), "none"))
	out.append(rect(x + 22 * unit, y, 46 * unit, 30 * unit, shutter, radius=1 * unit))
	out.append(rect(x + 52 * unit, y + 4 * unit, 9 * unit, 22 * unit, body))
	out.append(rect(x + 12 * unit, y + 44 * unit, 66 * unit, 46 * unit, label, radius=1.5 * unit))
	return "".join(out)


# ---- writing a master ---------------------------------------------------------------------------------------------------------------------
HEAD = """<?xml version="1.0" encoding="UTF-8"?>
<!--
	%s

	(c) Copyright 2026 Liminal HQ, Scott Morris
	SPDX-License-Identifier: Apache-2.0 OR MIT

	Generated by assets/media/gen_media.py for Quill %s; edit the generator, not this file. Lengths are in millimetres: the trim is
	%s mm, the viewBox adds %s mm of bleed on every side.
-->
"""


def master(path, summary, width, height, bleed, defs, body, origin=(0.0, 0.0)):
	left, top = origin[0] - bleed, origin[1] - bleed
	full_width, full_height = width + 2 * bleed, height + 2 * bleed
	svg = HEAD % (summary, VERSION, "%s x %s" % (number(width), number(height)), number(bleed))
	svg += '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="%smm" height="%smm" viewBox="%s %s %s %s"' % (
		number(full_width), number(full_height), number(left), number(top), number(full_width), number(full_height))
	svg += ' data-trim="%s %s %s %s" data-bleed="%s">\n' % (number(origin[0]), number(origin[1]), number(width), number(height), number(bleed))
	if defs:
		svg += "<defs>%s</defs>\n" % defs
	svg += body + "\n</svg>\n"
	os.makedirs(os.path.dirname(path), exist_ok=True)
	with open(path, "w", encoding="utf-8") as handle:
		handle.write(svg)


def screenshot(name, x, y, width, height, master_dir, fit="meet"):
	"""A screenshot from docs/media, linked by a path relative to the master (render.py inlines it)."""
	relative = os.path.relpath(os.path.join(MEDIA_DIR, name), master_dir)
	return '<image x="%s" y="%s" width="%s" height="%s" preserveAspectRatio="xMidYMid %s" xlink:href="%s" href="%s"/>' % (
		number(x), number(y), number(width), number(height), fit, relative, relative)


# ---- floppy labels -----------------------------------------------------------------------------------------------------------------------
def label_strip(background, lines, edge_words, edge_fill, accent, mono=False):
	"""The part that folds over the edge: the background, then the back's lines and the edge's line, turned upside down so the back
	reads when the disk is turned over and the edge reads in a disk box."""
	out = [rect(-LABEL_BLEED, LABEL_FACE, LABEL + 2 * LABEL_BLEED, LABEL - LABEL_FACE + LABEL_BLEED, background)]
	inner = []
	face = "mono" if mono else "sans"
	baseline = LABEL_FACE + 4.3
	for words, fill, bold in lines:
		inner.append(text(words, 3.0, baseline, 6.5, face + ("-bold" if bold else ""), fill) if isinstance(words, str) else
			runs([(piece, face + ("-bold" if bold else ""), fill) for piece in words], 3.0, baseline, 6.5))
		baseline += 3.4
	edge_y = LABEL - LABEL_EDGE
	inner.append(rect(-LABEL_BLEED, edge_y, LABEL + 2 * LABEL_BLEED, LABEL_EDGE, accent))  # not past LABEL: that is the face once turned
	inner.append(text(edge_words, LABEL / 2, edge_y + 2.45, 6.0, "sans-bold", edge_fill, "middle", tracking=0.25))
	centre = (LABEL_FACE + LABEL) / 2
	out.append('<g transform="rotate(180 %s %s)">%s</g>' % (number(LABEL / 2), number(centre), "".join(inner)))
	return "".join(out)


def strip_lines(disk, fill, dim, bold_first=True, mono=False):
	gap = " · " if mono else "  ·  "
	return (
		((disk["volume"] + gap + "Version " + VERSION), fill, bold_first),
		(("Serial " + SERIAL + gap + "1.44 MB" + gap + disk["image"]), dim, False),
		(((("© " + YEAR + " Liminal HQ") if mono else COPYRIGHT) + gap + LICENCE), dim, False),
	)


# Option A: era-accurate retro ----------------------------------------------------------------------------------------------------------------
def floppy_a_dos(disk):
	columns, rows = 32, 11
	cell_width, cell_height = LABEL / columns, LABEL_FACE / rows
	points = mono_points(cell_width)
	baseline = cell_height * 0.74

	def at(column, row):
		return column * cell_width, row * cell_height

	out = [rect(-LABEL_BLEED, -LABEL_BLEED, LABEL + 2 * LABEL_BLEED, LABEL_FACE + LABEL_BLEED, VGA_BLUE)]
	out.append(rect(-LABEL_BLEED, -LABEL_BLEED, LABEL + 2 * LABEL_BLEED, cell_height + LABEL_BLEED, VGA_GREY))
	x = cell_width
	for word in ("File", "Edit", "Insert", "Find", "Help"):
		out.append(cell_text(word[0], x, baseline, cell_width, points, VGA_RED))
		out.append(cell_text(word[1:], x + cell_width, baseline, cell_width, points, VGA_BLACK))
		x += (len(word) + 1) * cell_width
	# the dialog box, its shadow and its double border
	box_left, box_top, box_right, box_bottom = 1, 1.5, 30, 8.5
	shadow_x, shadow_y = at(box_left + 2, box_top + 1)
	box_x, box_y = at(box_left, box_top)
	box_w, box_h = (box_right - box_left + 1) * cell_width, (box_bottom - box_top + 1) * cell_height
	out.append(rect(shadow_x, shadow_y, box_w, box_h, VGA_BLACK))
	out.append(rect(box_x, box_y, box_w, box_h, VGA_BLUE))
	inset_x, inset_y = cell_width / 2, cell_height / 2
	frame = '<rect x="%s" y="%s" width="%s" height="%s" fill="none" stroke="%s" stroke-width="%s"/>'
	for offset in (0.0, 0.7):
		out.append(frame % (number(box_x + inset_x - offset), number(box_y + inset_y - offset), number(box_w - 2 * inset_x + 2 * offset),
			number(box_h - 2 * inset_y + 2 * offset), VGA_WHITE, number(0.32)))
	title = " Quill " + VERSION + " "
	title_width = len(title) * cell_width
	title_x = box_x + (box_w - title_width) / 2
	out.append(rect(title_x, box_y + inset_y - 1.6, title_width, 3.2, VGA_BLUE))
	out.append(cell_text(title, title_x, box_y + inset_y + 1.05, cell_width, points, VGA_YELLOW))
	# the banner
	pixel_width, pixel_height = cell_width * 0.8, cell_height * 0.38
	banner_width = block_width("QUILL") * pixel_width
	out.append(block_banner("QUILL", (LABEL - banner_width) / 2 - pixel_width / 2, box_y + cell_height * 0.95, pixel_width, pixel_height,
		VGA_YELLOW, shadow=VGA_BLACK))
	line_y = box_y + cell_height * 5.2
	out.append(text("Word processor for DOS", LABEL / 2, line_y, points, "mono", VGA_LIGHT_CYAN, "middle"))
	out.append(text(VERSION_LINE, LABEL / 2, line_y + cell_height, points * 0.86, "mono", VGA_WHITE, "middle"))
	out.append(text("386 · 4 MB · VGA or text mode", LABEL / 2, line_y + cell_height * 1.9, points * 0.8, "mono", VGA_GREY, "middle"))
	# the status line
	status_y = (rows - 1) * cell_height
	out.append(rect(-LABEL_BLEED, status_y, LABEL + 2 * LABEL_BLEED, cell_height + 0.02, VGA_GREY))
	out.append(cell_text(" A:\\SETUP to install", 0, status_y + baseline, cell_width, points, VGA_BLACK))
	out.append(text(SERIAL + " ", LABEL, status_y + baseline, points * 0.86, "mono", VGA_BLUE, "end"))
	out.append(label_strip(VGA_BLACK, strip_lines(disk, VGA_WHITE, VGA_GREY, mono=True), disk["edge"] + " · " + VERSION, VGA_BLACK, VGA_LIGHT_CYAN, mono=True))
	return "", "".join(out)


def win31_window(x, y, width, height, title, title_points=7.0, client=WIN_LIGHT):
	"""A Windows 3.1 window: frame, the control-menu box, a centred title on the active colour, minimise and maximise."""
	bar = 4.4
	out = [rect(x, y, width, height, WIN_FACE, stroke=WIN_DARK, stroke_width=0.2)]
	out.append(rect(x + 0.7, y + 0.7, width - 1.4, height - 1.4, client, stroke=WIN_DARK, stroke_width=0.2))
	out.append(rect(x + 0.7, y + 0.7, width - 1.4, bar, WIN_SKY, stroke=WIN_DARK, stroke_width=0.2))
	out.append(bevel(x + 0.8, y + 0.8, bar - 0.2, bar - 0.2, raised=True, outer=False, depth=0.25))
	out.append(rect(x + 1.55, y + 0.7 + bar / 2 - 0.3, bar - 1.7, 0.6, WIN_LIGHT, stroke=WIN_DARK, stroke_width=0.12))
	for index, pointing in enumerate(("up", "down")):
		button_x = x + width - 0.7 - (index + 1) * bar
		out.append(bevel(button_x, y + 0.8, bar - 0.1, bar - 0.2, depth=0.3))
		out.append(triangle(button_x + bar / 2 - 0.1, y + 0.7 + bar / 2, 1.8, pointing))
	out.append(text(title, x + (width - 2 * bar) / 2 + bar / 2, y + 0.7 + bar * 0.73, title_points, "sans-bold", WIN_DARK, "middle"))
	return "".join(out), y + 0.7 + bar


def help_icon(x, y, size):
	out = [text("?", x + size * 0.55, y + size * 0.92, size / POINT * 1.05, "sans-bold", WIN_DARK, "middle")]
	out.append(text("?", x + size * 0.47, y + size * 0.86, size / POINT * 1.05, "sans-bold", WIN_YELLOW, "middle"))
	return "".join(out)


def window_icon(x, y, size):
	return (rect(x + size * 0.1, y + size * 0.2, size * 0.8, size * 0.6, WIN_FACE, stroke=WIN_DARK, stroke_width=0.2)
		+ rect(x + size * 0.18, y + size * 0.32, size * 0.64, size * 0.4, WIN_LIGHT, stroke=WIN_SHADOW, stroke_width=0.15))


def floppy_a_w31(disk):
	out = [rect(-LABEL_BLEED, -LABEL_BLEED, LABEL + 2 * LABEL_BLEED, LABEL_FACE + LABEL_BLEED, WIN_FACE)]
	window, client_top = win31_window(2.2, 2.2, LABEL - 4.4, 29.0, "Quill")
	out.append(window)
	icon = 9.0
	slots = ((7.0, "Quill", "quill"), (25.5, "Quill Help", "help"), (46.5, "Uninstall Quill", "window"))
	for x, caption, kind in slots:
		icon_x, icon_y = x + 3.5 - icon / 2 + 3.5, client_top + 2.4
		if kind == "quill":
			out.append(pixel_icon(icon_x, icon_y, icon))
		elif kind == "help":
			out.append(help_icon(icon_x, icon_y, icon))
		else:
			out.append(window_icon(icon_x, icon_y, icon))
		caption_x = icon_x + icon / 2
		caption_y = icon_y + icon + 3.0
		if kind == "quill":
			caption_width = text_width(caption, "sans", 6.5) + 1.2
			out.append(rect(caption_x - caption_width / 2, caption_y - 2.3, caption_width, 3.0, WIN_NAVY))
			out.append(text(caption, caption_x, caption_y, 6.5, "sans", WIN_LIGHT, "middle"))
		else:
			out.append(text(caption, caption_x, caption_y, 6.5, "sans", WIN_DARK, "middle"))
	out.append(text("Quill for Windows 3.1 and 3.11", 3.0, 37.4, 8.6, "sans-bold", WIN_DARK))
	out.append(text(VERSION_LINE, 3.0, 41.6, 6.8, "sans", WIN_DARK))
	button_x, button_y, button_w, button_h = LABEL - 25.0, 38.4, 22.0, 7.6
	out.append(rect(button_x - 0.35, button_y - 0.35, button_w + 0.7, button_h + 0.7, WIN_DARK, radius=0.6))
	out.append(bevel(button_x, button_y, button_w, button_h, depth=0.4))
	out.append(text("A:\\SETUP", button_x + button_w / 2, button_y + 5.0, 7.2, "sans-bold", WIN_DARK, "middle"))
	out.append(text("Program Manager: File, Run", 3.0, 46.6, 6.2, "sans", "#404040"))
	out.append(label_strip(WIN_SKY, strip_lines(disk, WIN_DARK, "#202040"), disk["edge"] + " · " + VERSION, WIN_LIGHT, WIN_NAVY))
	return "", "".join(out)


def menu_panel(x, y, width, items, highlight, row=4.2, sidebar=None):
	"""A Windows 95 menu: a raised panel, items with an arrow where they open another, one item lit."""
	height = row * len(items) + 1.2
	out = [bevel(x, y, width, height, depth=0.3)]
	left = x + 0.9
	if sidebar:
		out.append(rect(x + 0.6, y + 0.6, 4.0, height - 1.2, WIN_SHADOW))
		out.append(text(sidebar, 0, 0, 6.5, "sans-bold", WIN_LIGHT, transform="translate(%s %s) rotate(-90)" % (number(x + 3.55), number(y + height - 1.6))))
		left = x + 4.9
	for index, (words, has_arrow, icon) in enumerate(items):
		top = y + 0.6 + index * row
		lit = index == highlight
		fill = WIN_LIGHT if lit else WIN_DARK
		if lit:
			out.append(rect(left - 0.3, top, x + width - 0.6 - left + 0.3, row, WIN_NAVY))
		text_x = left + 0.4
		if icon == "quill":
			out.append(pixel_icon(left + 0.2, top + 0.5, row - 1.0, pixels=16))
			text_x = left + row + 0.3
		elif icon == "folder":
			out.append(rect(left + 0.3, top + 1.0, row - 1.6, row - 2.0, "#ffff80" if not lit else "#e0e080", stroke=WIN_DARK, stroke_width=0.12))
			text_x = left + row + 0.3
		elif icon == "help":
			out.append(rect(left + 0.4, top + 0.8, row - 1.8, row - 1.6, "#800080", stroke=WIN_DARK, stroke_width=0.12))
			text_x = left + row + 0.3
		out.append(text(words, text_x, top + row * 0.72, 6.3, "sans", fill))
		if has_arrow:
			out.append(triangle(x + width - 2.0, top + row / 2, 1.5, "right", fill))
	return "".join(out), height


def start_button(x, y, width, height, pressed=False):
	"""The classic grey Windows 95 Start button, with the small 16-colour Quill icon where the flag would be and the word Start."""
	out = [bevel(x, y, width, height, raised=not pressed, depth=0.3)]
	icon = height - 1.6
	out.append(pixel_icon(x + 0.9, y + 0.8, icon, pixels=16))
	out.append(text("Start", x + 1.6 + icon, y + height * 0.7, 7.0, "sans-bold", WIN_DARK))
	return "".join(out)


def floppy_a_w95(disk):
	out = [rect(-LABEL_BLEED, -LABEL_BLEED, LABEL + 2 * LABEL_BLEED, LABEL_FACE + LABEL_BLEED, WIN_TEAL)]
	out.append(text("Quill", 3.0, 11.0, 22, "sans-bold", WIN_DARK, tracking=0.1))
	out.append(text("Quill", 2.7, 10.7, 22, "sans-bold", WIN_LIGHT, tracking=0.1))
	out.append(text("for Windows 95, 98 and Me", 3.0, 15.6, 7.6, "sans-bold", WIN_LIGHT))
	out.append(text(VERSION_LINE + " · Start, Run: A:\\SETUP", 3.0, 19.6, 6.4, "sans", WIN_LIGHT))
	taskbar_y = LABEL_FACE - 5.6
	menu, menu_h = menu_panel(1.0, taskbar_y - 4.2 * 4 - 1.2 - 0.2, 22.0,
		(("Programs", True, "folder"), ("Documents", True, None), ("Help", False, None), ("Run...", False, None)), 0, sidebar="Quill")
	out.append(menu)
	programs_y = taskbar_y - 4.2 * 4 - 1.2 - 0.2 - 0.6
	sub, _ = menu_panel(22.4, programs_y, 22.6, (("Accessories", True, "folder"), ("Quill", True, "folder"), ("StartUp", True, "folder")), 1)
	out.append(sub)
	group, _ = menu_panel(44.4, programs_y + 4.2, 23.6, (("Quill", False, "quill"), ("Quill Help", False, "help")), 0)
	out.append(group)
	out.append(rect(-LABEL_BLEED, taskbar_y, LABEL + 2 * LABEL_BLEED, LABEL_FACE - taskbar_y + 0.02, WIN_FACE))
	out.append(rect(-LABEL_BLEED, taskbar_y + 0.25, LABEL + 2 * LABEL_BLEED, 0.3, WIN_LIGHT))
	out.append(start_button(1.0, taskbar_y + 1.0, 13.5, 4.2, pressed=True))
	out.append(bevel(LABEL - 15.5, taskbar_y + 1.0, 14.5, 4.2, raised=False, depth=0.25, outer=False))
	out.append(text(VERSION, LABEL - 8.25, taskbar_y + 4.0, 6.5, "sans", WIN_DARK, "middle"))
	out.append(label_strip(WIN_FACE, strip_lines(disk, WIN_DARK, "#303030"), disk["edge"] + " · " + VERSION, WIN_LIGHT, WIN_TEAL))
	return "", "".join(out)


def floppy_a_all(disk):
	out = []
	third = LABEL / 3
	top_h = 28.0
	out.append(rect(-LABEL_BLEED, -LABEL_BLEED, third + LABEL_BLEED, top_h + LABEL_BLEED, VGA_BLUE))
	out.append(rect(third, -LABEL_BLEED, third, top_h + LABEL_BLEED, WIN_FACE))
	out.append(rect(2 * third, -LABEL_BLEED, third + LABEL_BLEED, top_h + LABEL_BLEED, WIN_TEAL))
	# DOS
	out.append(rect(-LABEL_BLEED, -LABEL_BLEED, third + LABEL_BLEED, 4.0 + LABEL_BLEED, VGA_GREY))
	out.append(text("QUILLDOS", third / 2, 3.0, 6.8, "mono-bold", VGA_BLACK, "middle"))
	pixel = 0.62
	out.append(block_banner("DOS", (third - block_width("DOS") * pixel) / 2, 7.5, pixel, pixel * 1.1, VGA_YELLOW, VGA_BLACK))
	out.append(text("C:\\>A:\\SETUP", third / 2, 21.0, 6.3, "mono", VGA_WHITE, "middle"))
	out.append(text("386 · 4 MB", third / 2, 25.0, 6.0, "mono", VGA_GREY, "middle"))
	# Windows 3.x
	window, client_top = win31_window(third + 1.2, 1.2, third - 2.4, 19.5, "Quill", title_points=6.0)
	out.append(window)
	out.append(pixel_icon(third + third / 2 - 3.6, client_top + 2.0, 7.2))
	out.append(text("A:\\WIN31\\SETUP", third + third / 2, 25.0, 6.0, "sans-bold", WIN_DARK, "middle"))
	# Windows 95
	out.append(text("Windows 95", 2 * third + third / 2, 6.0, 7.2, "sans-bold", WIN_LIGHT, "middle"))
	out.append(text("98 and Me", 2 * third + third / 2, 9.6, 6.2, "sans", WIN_LIGHT, "middle"))
	out.append(text("A:\\WIN95\\SETUP", 2 * third + third / 2, 15.0, 6.0, "sans-bold", WIN_LIGHT, "middle"))
	out.append(rect(2 * third, top_h - 5.4, third + LABEL_BLEED, 5.4, WIN_FACE))
	out.append(rect(2 * third, top_h - 5.15, third + LABEL_BLEED, 0.3, WIN_LIGHT))
	out.append(start_button(2 * third + 1.0, top_h - 4.6, 13.0, 3.9, pressed=False))
	# the title band
	out.append(rect(-LABEL_BLEED, top_h, LABEL + 2 * LABEL_BLEED, LABEL_FACE - top_h + 0.02, PARCHMENT_100))
	out.append(rect(-LABEL_BLEED, top_h, LABEL + 2 * LABEL_BLEED, 0.5, INK_800))
	out.append(emblem(2.6, top_h + 3.0, 12.0, "a-all-emblem", tile=True))
	out.append(text("Quill", 17.0, top_h + 11.2, 22, "serif-bold", INK_800))
	out.append(text("One disk for all three", 43.5, top_h + 6.8, 6.6, "sans-bold", INK_700))
	out.append(text(VERSION_LINE, 43.5, top_h + 10.6, 6.3, "sans", MUTED))
	out.append(text("DOS · Windows 3.x · Windows 95, 98 and Me", 3.0, top_h + 19.0, 6.6, "sans", INK_800))
	out.append(label_strip(INK_800, strip_lines(disk, PARCHMENT_200, PARCHMENT_300), disk["edge"] + " · " + VERSION, INK_900, AMBER))
	return "", "".join(out)


def write_in_lines(x, y, width, labels, ink, rule, points=6.5, face="sans"):
	out = []
	for index, words in enumerate(labels):
		baseline = y + index * 6.0
		out.append(text(words, x, baseline, points, face, ink))
		start = x + text_width(words, face, points) + 1.5
		out.append(line(start, baseline + 0.4, x + width, baseline + 0.4, rule, 0.2))
	return "".join(out)


def tick_boxes(x, y, labels, ink, box, points=6.5, face="sans"):
	out = []
	for words in labels:
		out.append(rect(x, y - 2.3, 2.6, 2.6, box, stroke=ink, stroke_width=0.2))
		out.append(text(words, x + 3.6, y, points, face, ink))
		x += 3.6 + text_width(words, face, points) + 3.6
	return "".join(out)


def floppy_a_documents(_disk):
	out = [rect(-LABEL_BLEED, -LABEL_BLEED, LABEL + 2 * LABEL_BLEED, LABEL_FACE + LABEL_BLEED, VGA_WHITE)]
	out.append(rect(-LABEL_BLEED, -LABEL_BLEED, LABEL + 2 * LABEL_BLEED, 12.5 + LABEL_BLEED, VGA_BLUE))
	out.append(text("QUILL DOCUMENTS", 3.0, 6.4, 9.0, "mono-bold", VGA_YELLOW))
	out.append(text("Disk ___ of ___", 3.0, 10.6, 6.6, "mono", VGA_WHITE))
	out.append(text(".QLD  .RTF  .TXT", LABEL - 3.0, 10.6, 6.6, "mono", VGA_LIGHT_CYAN, "end"))
	out.append(write_in_lines(3.0, 19.0, LABEL - 6.0, ("Name", "Holds", "", "Date"), VGA_BLACK, VGA_GREY, face="mono"))
	out.append(tick_boxes(3.0, 46.2, ("DOS", "Win 3.x", "Win 95"), VGA_BLACK, VGA_WHITE, face="mono"))
	lines = ((("QUILL DOCS  ·  keep a copy"), VGA_WHITE, True), ("Quill " + VERSION + " opens .QLD, .RTF and .TXT", VGA_GREY, False),
		(SITE, VGA_GREY, False))
	out.append(label_strip(VGA_BLACK, lines, "QUILL · DOCUMENTS", VGA_BLACK, VGA_YELLOW, mono=True))
	return "", "".join(out)


# Option B: modern Afterglow dusk -------------------------------------------------------------------------------------------------------------
def platform_glyph(kind, x, y, colour):
	"""A small line drawing for each system: a prompt, a window, a Start button, or all three."""
	if kind == "dos":
		return text("C:\\>_", x, y + 3.4, 7.0, "mono-bold", colour)
	if kind == "w31":
		return (rect(x, y, 10.0, 7.0, "none", stroke=colour, stroke_width=0.35) + rect(x, y, 10.0, 1.8, colour)
			+ rect(x + 1.2, y + 3.0, 2.6, 2.6, colour))
	if kind == "w95":
		return (rect(x, y + 1.6, 11.0, 4.2, "none", radius=0.6, stroke=colour, stroke_width=0.35)
			+ text("Start", x + 5.5, y + 4.8, 6.0, "sans-bold", colour, "middle"))
	return text("A:\\", x, y + 3.4, 7.0, "mono-bold", colour)


def floppy_b(disk_key):
	disk = DISKS[disk_key]
	prefix = "b-" + disk_key
	defs = ink_gradient(prefix + "-ground")
	out = [rect(-LABEL_BLEED, -LABEL_BLEED, LABEL + 2 * LABEL_BLEED, LABEL_FACE + LABEL_BLEED, "url(#%s-ground)" % prefix)]
	out.append(dusk_bands(-LABEL_BLEED, 41.0, LABEL + 2 * LABEL_BLEED, LABEL_FACE - 41.0 + 0.02))
	out.append(emblem(1.0, 4.0, 33.0, prefix + "-emblem", tile=False))
	out.append(text("Quill", 34.0, 17.0, 24, "serif-bold", PARCHMENT_200))
	words = {"dos": "for DOS", "w31": "for Windows 3.x", "w95": "for Windows 95", "all": "for all three"}[disk_key]
	out.append(text(words, 34.4, 22.8, 8.0, "serif-italic", AMBER))
	detail = {"dos": "386 · 4 MB · VGA or text", "w31": "Windows 3.1 and 3.11", "w95": "Windows 95, 98 and Me",
		"all": "DOS · Windows 3.x · 95"}[disk_key]
	out.append(text(detail, 34.4, 27.6, 6.4, "sans", PARCHMENT_300))
	out.append(text(VERSION_LINE, 34.4, 31.6, 6.4, "sans", PARCHMENT_300))
	run = disk["run"] if disk_key != "all" else "A:\\SETUP \\WIN31 \\WIN95"
	out.append(text(run.replace("File, Run: ", "").replace("Start, Run: ", ""), 34.4, 36.6, 6.4, "mono-bold", PARCHMENT_200))
	chip = disk["short"]
	chip_width = text_width(chip, "sans-bold", 6.5, 0.2) + 3.4
	out.append(rect(LABEL - 2.5 - chip_width, 2.6, chip_width, 4.2, AMBER, radius=2.1))
	out.append(text(chip, LABEL - 2.5 - chip_width / 2, 5.75, 6.5, "sans-bold", INK_800, "middle", tracking=0.2))
	out.append(label_strip(INK_900, strip_lines(disk, PARCHMENT_200, PARCHMENT_300), disk["edge"] + " · " + VERSION, INK_900, AMBER))
	return defs, "".join(out)


def floppy_b_documents(_disk):
	out = [rect(-LABEL_BLEED, -LABEL_BLEED, LABEL + 2 * LABEL_BLEED, LABEL_FACE + LABEL_BLEED, PARCHMENT_100)]
	out.append(dusk_bands(-LABEL_BLEED, -LABEL_BLEED, LABEL + 2 * LABEL_BLEED, 3.0 + LABEL_BLEED))
	out.append(emblem(2.0, 4.6, 11.0, "b-documents-emblem", tile=True))
	out.append(text("Quill documents", 15.0, 11.4, 13, "serif-bold", INK_800))
	out.append(text("Disk ___ of ___", LABEL - 3.0, 16.6, 6.4, "sans", MUTED, "end"))
	out.append(write_in_lines(3.0, 22.0, LABEL - 6.0, ("Name", "Holds", "Date"), INK_800, RULE))
	out.append(tick_boxes(3.0, 46.0, (".QLD", ".RTF", ".TXT"), INK_800, PARCHMENT_200))
	lines = (("QUILL DOCS  ·  keep a copy", PARCHMENT_200, True), ("Quill " + VERSION + " opens .QLD, .RTF and .TXT", PARCHMENT_300, False),
		(SITE, PARCHMENT_300, False))
	out.append(label_strip(INK_900, lines, "QUILL · DOCUMENTS", INK_900, AMBER))
	return "", "".join(out)


# ---- the CD ---------------------------------------------------------------------------------------------------------------------------------
def disc_clip(identifier):
	"""The printable ring, with the bleed: 1 mm past the 118 mm edge and 1 mm into the 41 mm hole."""
	outer, inner = DISC_PRINT_OUTER + DISC_BLEED, DISC_PRINT_INNER - DISC_BLEED
	path = "M%s 0A%s %s 0 1 0 %s 0A%s %s 0 1 0 %s 0ZM%s 0A%s %s 0 1 1 %s 0A%s %s 0 1 1 %s 0Z" % (
		number(-outer), number(outer), number(outer), number(outer), number(outer), number(outer), number(-outer),
		number(-inner), number(inner), number(inner), number(inner), number(inner), number(inner), number(-inner))
	return '<clipPath id="%s"><path clip-rule="evenodd" d="%s"/></clipPath>' % (identifier, path)


def disc_a(master_dir):
	defs = disc_clip("a-disc-clip")
	out = [circle(0, 0, DISC_RADIUS + 1, WIN_TEAL)]
	out.append(circle(0, 0, 57.5, "none", stroke=WIN_NAVY, stroke_width=7.0))
	out.append(arc_text("QUILL · A WORD PROCESSOR FOR DOS, WINDOWS 3.X AND WINDOWS 95", 0, 0, 55.6, 6.6, "mono-bold", WIN_LIGHT, top=True))
	out.append(arc_text("VERSION " + VERSION + " · " + EDITION.upper() + " · " + SERIAL + " · " + LICENCE, 0, 0, 56.8, 6.4, "mono", WIN_LIGHT,
		top=False))
	# the setup window above the hole
	window_x, window_y, window_w, window_h = -31.0, -44.0, 62.0, 19.5
	out.append(bevel(window_x, window_y, window_w, window_h, depth=0.4))
	out.append(rect(window_x + 0.8, window_y + 0.8, window_w - 1.6, 4.0, WIN_NAVY))
	out.append(pixel_icon(window_x + 1.2, window_y + 1.15, 3.3, pixels=16))
	out.append(text("Quill Setup", window_x + 5.2, window_y + 3.85, 7.0, "sans-bold", WIN_LIGHT))
	for index, glyph in enumerate(("×",)):
		button_x = window_x + window_w - 4.4
		out.append(bevel(button_x, window_y + 1.2, 3.4, 3.0, depth=0.25))
		out.append(text(glyph, button_x + 1.7, window_y + 3.75, 7.0, "sans-bold", WIN_DARK, "middle"))
	out.append(text("Quill", window_x + 4.0, window_y + 14.8, 22, "sans-bold", WIN_DARK))
	out.append(text("Word processor", window_x + 35.0, window_y + 10.2, 7.0, "sans-bold", WIN_DARK))
	out.append(text(VERSION_LINE, window_x + 35.0, window_y + 13.8, 6.4, "sans", WIN_DARK))
	out.append(text("Written in Rust", window_x + 35.0, window_y + 17.2, 6.4, "sans", WIN_DARK))
	# beside the hole
	out.append(emblem(-46.5, -8.5, 17.0, "a-disc-emblem", tile=True))
	out.append(rect(26.0, -4.0, 20.0, 8.0, VGA_YELLOW))
	out.append(text("CD-ROM", 36.0, 1.6, 8.4, "mono-bold", VGA_BLACK, "middle"))
	# the three systems below the hole
	labels = (("DOS", VGA_BLUE, VGA_WHITE), ("Windows 3.x", WIN_FACE, WIN_DARK), ("Windows 95", WIN_FACE, WIN_DARK))
	x = -36.0
	for words, face, ink in labels:
		width = 18.0 if words == "DOS" else 26.0
		if words == "DOS":
			out.append(rect(x, 25.0, width, 7.0, face, stroke=WIN_LIGHT, stroke_width=0.4))
		else:
			out.append(bevel(x, 25.0, width, 7.0, depth=0.4))
		out.append(text(words, x + width / 2, 30.0, 7.2, "mono-bold" if words == "DOS" else "sans-bold", ink, "middle"))
		x += width + 1.0
	out.append(text("A Liminal HQ Labs experiment · " + SITE, 0, 39.5, 6.4, "sans", WIN_LIGHT, "middle"))
	out.append(text(HONEST_SHORT, 0, 43.4, 6.2, "sans", "#cfe8e8", "middle"))
	body = '<g clip-path="url(#a-disc-clip)">%s</g>' % "".join(out)
	return defs, body


def disc_b(master_dir):
	defs = disc_clip("b-disc-clip") + gold_gradient("b-disc-gold")
	defs += ('<radialGradient id="b-disc-ground" cx="0" cy="0" r="60" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="%s"/>'
		'<stop offset=".7" stop-color="%s"/><stop offset="1" stop-color="%s"/></radialGradient>') % (INK_700, INK_800, INK_900)
	out = [circle(0, 0, DISC_RADIUS + 1, "url(#b-disc-ground)")]
	# the dusk at the rim, stepped
	for index, colour in enumerate(DUSK):
		radius = 61.0 - index * 0.85
		out.append(circle(0, 0, radius, colour))
	out.append(circle(0, 0, 61.0 - len(DUSK) * 0.85, "url(#b-disc-ground)"))
	# the ring of the Q round the hole, and the nib, from the icon's geometry
	ring_x, ring_y, ring_r = EMBLEM_RING
	scale = 31.0 / ring_r
	out.append(circle(0, 0, 31.0, "none", stroke="url(#b-disc-gold)", stroke_width=76 * scale))
	nib = open(ICON_SVG).read()
	nib = nib[nib.index('<g transform="translate(830 826)'): nib.rindex("</g>") + 4]
	nib = nib.replace("q-q-nib-", "b-disc-nib-")
	out.append('<g transform="scale(%s) translate(%s %s)">%s</g>' % (number(scale), -ring_x, -ring_y, nib))
	nib_gradient = ('<linearGradient id="b-disc-nib-nib" x1="0" y1="1" x2="1" y2="0"><stop offset="0" stop-color="%s"/>'
		'<stop offset=".5" stop-color="%s"/><stop offset="1" stop-color="%s"/></linearGradient>') % (GOLD_LOW, AMBER, GOLD_HIGH)
	defs += nib_gradient
	out.append(arc_text("QUILL", 0, 0, 38.6, 13, "serif-bold", PARCHMENT_200, top=True, tracking=1.6))
	out.append(arc_text("DOS · WINDOWS 3.X · WINDOWS 95, 98 AND ME", 0, 0, 41.5, 6.6, "sans-bold", AMBER, top=False, tracking=0.25,
		centre=128))
	out.append(arc_text("A small word processor written in Rust · " + VERSION_LINE + " · " + SERIAL, 0, 0, 47.0, 6.3, "sans",
		PARCHMENT_300, top=True))
	out.append(arc_text(LABS + " · " + SITE + " · " + LICENCE, 0, 0, 49.6, 6.3, "sans", PARCHMENT_300, top=False, centre=126))
	body = '<g clip-path="url(#b-disc-clip)">%s</g>' % "".join(out)
	return defs, body


# The jewel case front inlay -------------------------------------------------------------------------------------------------------------------
def dos_screen_lines(x, y, width, rows, cell_height):
	"""A few lines of the test letter as the DOS text screen shows them."""
	words = (
		(("Dear reader,", VGA_WHITE),),
		(("This letter has ", VGA_WHITE), ("bold", VGA_WHITE + "|b"), (", ", VGA_WHITE), ("italic", VGA_LIGHT_CYAN), (" and", VGA_WHITE)),
		(("Item", VGA_WHITE + "|b"), ("          Qty", VGA_WHITE + "|b"), ("......Price", VGA_WHITE + "|b")),
		(("Quills", VGA_WHITE), ("        12", VGA_WHITE), ("........3.50", VGA_WHITE)),
		(("Ink bottles", VGA_WHITE), ("   3", VGA_WHITE), ("..........8.25", VGA_WHITE)),
		(("   Yours truly,", VGA_WHITE),),
	)
	cell_width = width / 40
	points = mono_points(cell_width) * 0.98
	out = []
	for row, pieces in enumerate(words[:rows]):
		cursor = x
		for piece, colour in pieces:
			bold = colour.endswith("|b")
			colour = colour.split("|")[0]
			out.append(cell_text(piece, cursor, y + row * cell_height, cell_width, points, colour, "mono-bold" if bold else "mono"))
			cursor += len(piece) * cell_width
	return "".join(out)


def jewel_a(master_dir):
	out = [rect(-CARD_BLEED, -CARD_BLEED, JEWEL + 2 * CARD_BLEED, JEWEL + 2 * CARD_BLEED, VGA_BLUE)]
	# the DOS screen behind
	out.append(rect(-CARD_BLEED, -CARD_BLEED, JEWEL + 2 * CARD_BLEED, 6.2 + CARD_BLEED, VGA_GREY))
	cell = JEWEL / 48
	points = mono_points(cell)
	x = cell
	for word in ("File", "Edit", "Insert", "Find", "Character", "Paragraph", "Help"):
		out.append(cell_text(word[0], x, 4.4, cell, points, VGA_RED))
		out.append(cell_text(word[1:], x + cell, 4.4, cell, points, VGA_BLACK))
		x += (len(word) + 1) * cell
	out.append(dos_screen_lines(3.0, 12.0, 96.0, 3, 4.6))
	out.append(rect(JEWEL - 24.0, 8.5, 21.0, 7.0, VGA_YELLOW))
	out.append(text("CD-ROM", JEWEL - 13.5, 13.6, 9.0, "mono-bold", VGA_BLACK, "middle"))
	# the Windows 3.1 window
	window, client_top = win31_window(6.0, 25.0, 70.0, 42.0, "Quill - LETTER.QLD", title_points=6.8)
	out.append(window)
	out.append(rect(6.7, client_top, 68.6, 5.0, WIN_FACE))
	for index in range(7):
		out.append(bevel(7.6 + index * 5.0, client_top + 0.6, 4.4, 3.8, depth=0.3))
	letter_x = 12.0
	out.append(text("Quill Letter", 41.0, client_top + 11.5, 9.0, "serif-bold", WIN_DARK, "middle"))
	body = ("Dear reader, this letter has bold, italic, underlined and struck text, a coloured word and some raised and lowered "
		"text, typed once and opened on three systems.")
	para, _ = paragraph(body, letter_x, client_top + 17.0, 58.0, 6.6, "serif", WIN_DARK, leading=1.25)
	out.append(para)
	# the Windows 95 window in front
	wx, wy, ww, wh = 58.0, 44.0, 58.0, 34.0
	out.append(bevel(wx, wy, ww, wh, depth=0.4))
	out.append(rect(wx + 0.8, wy + 0.8, ww - 1.6, 4.2, WIN_NAVY))
	out.append(pixel_icon(wx + 1.2, wy + 1.2, 3.4, pixels=16))
	out.append(text("Quill - letter.rtf", wx + 5.4, wy + 4.0, 6.6, "sans-bold", WIN_LIGHT))
	for index, glyph in enumerate(("×", "□", "_")):
		button_x = wx + ww - 4.4 - index * 3.7
		out.append(bevel(button_x, wy + 1.3, 3.3, 3.0, depth=0.25))
		out.append(text(glyph, button_x + 1.65, wy + 3.8, 6.5, "sans-bold", WIN_DARK, "middle"))
	out.append(rect(wx + 0.8, wy + 5.6, ww - 1.6, wh - 6.4, WIN_SHADOW))
	out.append(rect(wx + 4.0, wy + 7.0, ww - 8.0, wh - 7.8, WIN_LIGHT))
	out.append(text("Quill Letter", wx + ww / 2, wy + 13.0, 8.0, "sans-bold", WIN_DARK, "middle"))
	para, _ = paragraph("Dear reader, the same letter on Windows 95 has its fonts again.", wx + 7.0, wy + 18.0, ww - 14.0, 6.2, "serif",
		WIN_DARK, leading=1.25)
	out.append(para)
	# the title band
	band_y = 80.0
	out.append(rect(-CARD_BLEED, band_y, JEWEL + 2 * CARD_BLEED, JEWEL - band_y + CARD_BLEED, PARCHMENT_100))
	out.append(rect(-CARD_BLEED, band_y, JEWEL + 2 * CARD_BLEED, 0.8, INK_800))
	out.append(emblem(6.0, band_y + 6.0, 20.0, "a-jewel-emblem", tile=True))
	out.append(text("Quill", 30.0, band_y + 22.0, 44, "serif-bold", INK_800))
	out.append(text("The word processor for DOS, Windows 3.1 and Windows 95", 6.0, band_y + 31.0, 8.6, "sans-bold", INK_700))
	out.append(text(VERSION_LINE + " · " + LABS, 6.0, band_y + 36.0, 7.0, "sans", MUTED))
	# the starburst
	cx, cy, outer, inner = 98.0, 84.0, 17.0, 12.5
	points_list = []
	for index in range(32):
		radius = outer if index % 2 == 0 else inner
		angle = math.pi * 2 * index / 32 - math.pi / 2
		points_list.append((cx + radius * math.cos(angle), cy + radius * math.sin(angle)))
	out.append(polygon(points_list, VGA_RED))
	out.append(circle(cx, cy, inner - 1.2, VGA_YELLOW))
	out.append(text("Fits on", cx, cy - 2.6, 7.4, "sans-bold", VGA_BLACK, "middle"))
	out.append(text("a floppy!", cx, cy + 1.4, 7.4, "sans-bold", VGA_BLACK, "middle"))
	out.append(text("1.44 MB", cx, cy + 5.4, 6.4, "sans", VGA_BLACK, "middle"))
	return "", "".join(out)


def jewel_b(master_dir):
	defs = ink_gradient("b-jewel-ground")
	out = [rect(-CARD_BLEED, -CARD_BLEED, JEWEL + 2 * CARD_BLEED, JEWEL + 2 * CARD_BLEED, "url(#b-jewel-ground)")]
	out.append(dusk_bands(-CARD_BLEED, 98.0, JEWEL + 2 * CARD_BLEED, JEWEL - 98.0 + CARD_BLEED))
	out.append(emblem(28.0, 8.0, 64.0, "b-jewel-emblem", tile=False))
	out.append(text("Quill", JEWEL / 2, 80.0, 40, "serif-bold", PARCHMENT_200, "middle"))
	out.append(text("A small word processor for DOS, Windows 3.x and Windows 95", JEWEL / 2, 87.5, 8.2, "serif-italic", AMBER, "middle"))
	out.append(text(VERSION_LINE + "  ·  written in Rust", JEWEL / 2, 93.0, 6.8, "sans", PARCHMENT_300, "middle"))
	out.append(text(LABS, JEWEL / 2, 113.0, 6.8, "sans-bold", INK_900, "middle"))
	chip = "CD-ROM"
	out.append(rect(JEWEL - 24.0, 6.0, 18.0, 5.4, AMBER, radius=2.7))
	out.append(text(chip, JEWEL - 15.0, 9.9, 6.8, "sans-bold", INK_800, "middle", tracking=0.2))
	return defs, "".join(out)


# The tray card ------------------------------------------------------------------------------------------------------------------------------
def tray_contents(style, master_dir):
	"""The back of the case, between the spines; style holds the colours and faces of an option."""
	left, width = SPINE + 7.0, TRAY_WIDTH - 2 * SPINE - 14.0
	out = []
	out.append(text("Quill " + VERSION, left, 13.0, 16, style["display"], style["heading"]))
	out.append(text(EDITION + " release · " + LABS, left + width, 13.0, 6.8, style["body"], style["muted"], "end"))
	para, y = paragraph("One word processor for DOS, Windows 3.x and Windows 95, written in Rust. Type a letter in blue-screen DOS, where "
		"bold is bright white and italic is cyan. Save it, open it in the Windows 95 build, and it has its fonts again. Quill's RTF opens "
		"in LibreOffice too.", left, 19.5, width, 7.2, style["body"], style["ink"], leading=1.3)
	out.append(para)
	# the screenshots
	shot_y = y + 0.8
	gap = 4.0
	shot_w = (width - 2 * gap) / 3
	shot_h = shot_w * 0.66
	for index, (name, _size, caption) in enumerate(SCREENSHOTS):
		x = left + index * (shot_w + gap)
		out.append(style["frame"](x, shot_y, shot_w, shot_h))
		out.append(screenshot(name, x + 0.8, shot_y + 0.8, shot_w - 1.6, shot_h - 1.6, master_dir))
		out.append(text(caption, x, shot_y + shot_h + 3.2, 6.2, style["body"], style["muted"]))
	# the features
	feature_y = shot_y + shot_h + 9.0
	column = width / 2
	for index, feature in enumerate(FEATURES):
		x = left + (index % 2) * column
		baseline = feature_y + (index // 2) * 4.1
		out.append(rect(x, baseline - 1.9, 1.6, 1.6, style["bullet"]))
		out.append(text(feature, x + 3.0, baseline, 6.8, style["body"], style["ink"]))
	# the systems and the honest note
	box_y = feature_y + 10.6
	box_h = 17.0
	systems_w = width * 0.64
	out.append(style["panel"](left, box_y, systems_w, box_h))
	out.append(text("Runs on", left + 2.5, box_y + 4.2, 7.0, style["body"] + "-bold" if not style["body"].endswith("bold") else style["body"],
		style["panel_heading"]))
	systems = (("DOS", "QUILLDOS.EXE", "386 · 4 MB · VGA or text"), ("Windows 3.x", "QUILL.EXE", "Windows 3.1 or 3.11"),
		("Windows 95", "QUILL32.EXE", "95, 98 or Me · Pentium"))
	for index, (name, program, needs) in enumerate(systems):
		x = left + 2.5 + index * (systems_w - 5.0) / 3
		out.append(text(name, x, box_y + 8.8, 6.8, style["body"] + "-bold", style["panel_ink"]))
		out.append(text(program, x, box_y + 12.0, 6.0, "mono", style["panel_ink"]))
		out.append(text(needs, x, box_y + 15.0, 6.0, style["body"], style["panel_ink"]))
	note_x = left + systems_w + 3.0
	note_w = width - systems_w - 3.0
	out.append(style["note"](note_x, box_y, note_w, box_h))
	out.append(text("Experimental", note_x + 2.5, box_y + 4.2, 7.0, style["body"] + "-bold", style["note_heading"]))
	para, _ = paragraph("Run in emulators, virtual machines and Wine only, never on real hardware.", note_x + 2.5, box_y + 8.2, note_w - 5.0,
		6.2, style["body"], style["note_ink"], leading=1.25)
	out.append(para)
	# the small print
	small_y = box_y + box_h + 4.0
	out.append(text(COPYRIGHT + " · " + LICENCE + " · " + REPOSITORY, left, small_y, 6.0, style["body"], style["muted"]))
	out.append(text(SITE, left + width, small_y, 6.0, style["body"] + "-bold", style["muted"], "end"))
	para, _ = paragraph("CD volume QUILL, serial " + SERIAL + ". " + TRADEMARKS, left, small_y + 3.0, width, 6.0, style["body"], style["muted"], leading=1.2)
	out.append(para)
	return "".join(out)


def spine_text(x, words_left, words_middle, words_right, fill, points=7.0, reverse=False):
	"""A spine's line, rotated to read bottom to top (left spine) or top to bottom (right spine)."""
	length = TRAY_HEIGHT - 10.0
	turn = 90 if reverse else -90
	centre_x = x + SPINE / 2
	origin_y = TRAY_HEIGHT / 2
	group = []
	baseline = points * POINT * 0.36
	group.append(text(words_left, -length / 2, baseline, points, "sans-bold", fill))
	group.append(text(words_middle, 0, baseline, points, "sans", fill, "middle"))
	group.append(text(words_right, length / 2, baseline, points, "sans-bold", fill, "end"))
	return '<g transform="translate(%s %s) rotate(%s)">%s</g>' % (number(centre_x), number(origin_y), turn, "".join(group))


def tray_a(master_dir):
	style = {
		"display": "sans-bold", "body": "sans", "heading": WIN_NAVY, "ink": WIN_DARK, "muted": "#303030", "bullet": WIN_NAVY,
		"panel_heading": WIN_NAVY, "panel_ink": WIN_DARK, "note_heading": WIN_DARK, "note_ink": WIN_DARK,
		"frame": lambda x, y, w, h: bevel(x, y, w, h, raised=False, depth=0.5, outer=False) + rect(x + 0.8, y + 0.8, w - 1.6, h - 1.6, VGA_BLACK),
		"panel": lambda x, y, w, h: bevel(x, y, w, h, raised=False, face=WIN_LIGHT, depth=0.4, outer=False),
		"note": lambda x, y, w, h: bevel(x, y, w, h, raised=True, face="#ffffe1", depth=0.4),
	}
	out = [rect(-CARD_BLEED, -CARD_BLEED, TRAY_WIDTH + 2 * CARD_BLEED, TRAY_HEIGHT + 2 * CARD_BLEED, WIN_FACE)]
	out.append(rect(-CARD_BLEED, -CARD_BLEED, SPINE + CARD_BLEED, TRAY_HEIGHT + 2 * CARD_BLEED, WIN_NAVY))
	out.append(rect(TRAY_WIDTH - SPINE, -CARD_BLEED, SPINE + CARD_BLEED, TRAY_HEIGHT + 2 * CARD_BLEED, WIN_NAVY))
	for x, reverse in ((0.0, False), (TRAY_WIDTH - SPINE, True)):
		out.append(spine_text(x, "QUILL", "Word processor for DOS, Windows 3.x and Windows 95", VERSION, WIN_LIGHT, reverse=reverse))
	out.append(tray_contents(style, master_dir))
	return "", "".join(out)


def tray_b(master_dir):
	defs = ink_gradient("b-tray-ground")
	frame = lambda x, y, w, h: rect(x, y, w, h, INK_900, stroke=GOLD_LOW, stroke_width=0.35)
	style = {
		"display": "serif-bold", "body": "sans", "heading": PARCHMENT_200, "ink": PARCHMENT_200, "muted": PARCHMENT_300, "bullet": AMBER,
		"panel_heading": AMBER, "panel_ink": PARCHMENT_200, "note_heading": INK_900, "note_ink": INK_900,
		"frame": frame,
		"panel": lambda x, y, w, h: rect(x, y, w, h, INK_800, radius=1.2, stroke=INK_600, stroke_width=0.3),
		"note": lambda x, y, w, h: rect(x, y, w, h, AMBER, radius=1.2),
	}
	out = [rect(-CARD_BLEED, -CARD_BLEED, TRAY_WIDTH + 2 * CARD_BLEED, TRAY_HEIGHT + 2 * CARD_BLEED, "url(#b-tray-ground)")]
	for x in (-CARD_BLEED, TRAY_WIDTH - SPINE):
		out.append(rect(x, -CARD_BLEED, SPINE + CARD_BLEED, TRAY_HEIGHT + 2 * CARD_BLEED, INK_900))
	out.append(dusk_bands(-CARD_BLEED, TRAY_HEIGHT - 2.0, TRAY_WIDTH + 2 * CARD_BLEED, 2.0 + CARD_BLEED))
	for x, reverse in ((0.0, False), (TRAY_WIDTH - SPINE, True)):
		out.append(spine_text(x, "QUILL", "A small word processor for DOS, Windows 3.x and Windows 95", VERSION, PARCHMENT_200, reverse=reverse))
	out.append(tray_contents(style, master_dir))
	return defs, "".join(out)


# ---- stickers -------------------------------------------------------------------------------------------------------------------------------
R = STICKER / 2


def round_ground(fill):
	return circle(R, R, R + STICKER_BLEED, fill)


def square_ground(fill):
	return rect(-STICKER_BLEED, -STICKER_BLEED, STICKER + 2 * STICKER_BLEED, STICKER + 2 * STICKER_BLEED, fill,
		radius=STICKER_CORNER + STICKER_BLEED)


def sticker_emblem():
	out = [round_ground(INK_800), circle(R, R, R - 2.2, "none", stroke=GOLD_LOW, stroke_width=0.4)]
	out.append(emblem(R - 15.5, R - 14.5, 31.0, "s-emblem", tile=False))
	out.append(arc_text("QUILL", R, R, R - 6.8, 9.5, "serif-bold", PARCHMENT_200, top=True, tracking=1.0))
	out.append(arc_text("WORD PROCESSOR", R, R, R - 4.0, 6.2, "sans-bold", AMBER, top=False, tracking=0.4))
	return "round", "".join(out)


def sticker_dos():
	out = [round_ground(VGA_BLUE)]
	out.append(rect(-STICKER_BLEED, 9.0, STICKER + 2 * STICKER_BLEED, 4.6, VGA_GREY))
	out.append(text("QUILLDOS.EXE", R, 12.5, 7.0, "mono-bold", VGA_BLACK, "middle"))
	pixel = 1.9
	out.append(block_banner("DOS", R - block_width("DOS") * pixel / 2 - pixel / 2, 16.5, pixel, pixel * 1.05, VGA_YELLOW, VGA_BLACK))
	out.append(text("C:\\QUILL>QUILLDOS_", R, 34.5, 6.8, "mono", VGA_WHITE, "middle"))
	out.append(text("BUILD", R, 41.0, 7.0, "mono-bold", VGA_LIGHT_CYAN, "middle", tracking=0.8))
	return "round", "".join(out)


def sticker_w31():
	out = [round_ground(WIN_FACE)]
	window, client_top = win31_window(8.0, 9.0, STICKER - 16.0, 25.0, "Quill", title_points=7.0)
	out.append(window)
	out.append(pixel_icon(R - 5.0, client_top + 2.6, 10.0))
	caption_width = text_width("Quill", "sans", 6.5) + 1.2
	out.append(rect(R - caption_width / 2, client_top + 13.6, caption_width, 3.0, WIN_NAVY))
	out.append(text("Quill", R, client_top + 15.9, 6.5, "sans", WIN_LIGHT, "middle"))
	out.append(text("Windows 3.x", R, 40.0, 9.0, "sans-bold", WIN_DARK, "middle"))
	out.append(text("build", R, 44.2, 6.8, "sans", WIN_DARK, "middle"))
	return "round", "".join(out)


def sticker_w95():
	out = [round_ground(WIN_TEAL)]
	out.append(emblem(R - 7.0, 7.5, 14.0, "s-w95-emblem", tile=True))
	out.append(text("Windows 95", R, 28.5, 10.0, "sans-bold", WIN_LIGHT, "middle"))
	out.append(text("98 and Me build", R, 33.0, 6.8, "sans", WIN_LIGHT, "middle"))
	out.append(rect(-STICKER_BLEED, 36.5, STICKER + 2 * STICKER_BLEED, 20.0, WIN_FACE))
	out.append(rect(-STICKER_BLEED, 36.8, STICKER + 2 * STICKER_BLEED, 0.35, WIN_LIGHT))
	out.append(start_button(R - 8.0, 38.3, 16.0, 5.2))
	return "round", "".join(out)


def sticker_reactos():
	defs = ink_gradient("s-reactos-ground", 0, 0, 0, 1)
	out = [round_ground("url(#s-reactos-ground)")]
	out.append(dusk_bands(-STICKER_BLEED, 31.0, STICKER + 2 * STICKER_BLEED, 20.0 + STICKER_BLEED))
	out.append(circle(R, 31.0, 9.0, AMBER))
	out.append(dusk_bands(-STICKER_BLEED, 31.0, STICKER + 2 * STICKER_BLEED, 20.0 + STICKER_BLEED, DUSK[3:]))
	out.append(text("Has run on", R, 13.0, 7.2, "sans", PARCHMENT_300, "middle"))
	out.append(text("ReactOS", R, 21.5, 14, "serif-bold", PARCHMENT_200, "middle"))
	out.append(text("QUILL32.EXE", R, 26.5, 6.4, "mono-bold", AMBER, "middle"))
	out.append(text("in the browser, on v86", R, 44.0, 6.2, "sans-bold", INK_900, "middle"))
	return "round", "".join(out), defs


def sticker_rust():
	out = [round_ground(RUST)]
	teeth = []
	for index in range(24):
		angle = math.pi * 2 * index / 24
		teeth.append('<rect x="%s" y="%s" width="2.4" height="3.2" fill="%s" transform="rotate(%s %s %s)"/>' % (
			number(R - 1.2), number(1.6), PARCHMENT_200, number(math.degrees(angle)), number(R), number(R)))
	out.append("".join(teeth))
	out.append(circle(R, R, R - 4.3, PARCHMENT_200))
	out.append(circle(R, R, R - 5.6, RUST))
	out.append(text("MADE IN", R, 21.5, 8.0, "sans-bold", PARCHMENT_200, "middle", tracking=0.6))
	out.append(text("RUST", R, 33.0, 20, "serif-bold", PARCHMENT_200, "middle", tracking=0.8))
	return "round", "".join(out)


def sticker_experimental():
	defs = ('<pattern id="s-hazard" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">'
		'<rect width="3" height="6" fill="%s"/><rect x="3" width="3" height="6" fill="%s"/></pattern>') % (AMBER, INK_800)
	out = [square_ground("url(#s-hazard)")]
	out.append(rect(4.0, 11.0, STICKER - 8.0, 28.8, PARCHMENT_100, radius=1.0))
	out.append(text("EXPERIMENTAL", R, 20.5, 10.5, "sans-bold", INK_800, "middle", tracking=0.3))
	out.append(text("Tried in emulators,", R, 27.4, 7.0, "sans", INK_800, "middle"))
	out.append(text("virtual machines and Wine.", R, 31.0, 7.0, "sans", INK_800, "middle"))
	out.append(text("Not yet on real hardware.", R, 35.6, 7.0, "sans-bold", INK_800, "middle"))
	return "square", "".join(out), defs


def sticker_labs():
	out = [square_ground(PARCHMENT_100)]
	out.append(rect(3.0, 3.0, STICKER - 6.0, STICKER - 6.0, "none", radius=1.5, stroke=INK_800, stroke_width=0.5))
	out.append(emblem(R - 9.0, 7.0, 18.0, "s-labs-emblem", tile=True))
	out.append(text("A Liminal HQ", R, 32.0, 9.4, "serif-bold", INK_800, "middle"))
	out.append(text("Labs experiment", R, 36.6, 9.4, "serif-bold", INK_800, "middle"))
	out.append(text("early · rough · open", R, 42.4, 6.6, "sans", AMBER_DEEP, "middle"))
	return "square", "".join(out)


def sticker_floppy():
	out = [square_ground(INK_800)]
	out.append(floppy_shape(R - 13.0, 5.0, 26.0, "#2a2f3a", "#b8bcc6", PARCHMENT_200))
	out.append(text("Quill", R, 25.0, 7.0, "serif-bold", INK_800, "middle"))
	out.append(text("Fits on a", R, 38.0, 8.2, "sans", PARCHMENT_200, "middle"))
	out.append(text("1.44 MB floppy", R, 43.4, 10.0, "sans-bold", AMBER, "middle"))
	return "square", "".join(out)


def sticker_series():
	defs = ink_gradient("s-series-ground", 0, 0, 0, 1)
	out = [square_ground("url(#s-series-ground)")]
	out.append(dusk_bands(-STICKER_BLEED, 33.0, STICKER + 2 * STICKER_BLEED, STICKER - 33.0 + STICKER_BLEED))
	out.append(text("Cross-compiling", R, 13.0, 10.0, "serif-bold", PARCHMENT_200, "middle"))
	out.append(text("to the past", R, 19.5, 10.0, "serif-italic", AMBER, "middle"))
	out.append(text("the blog series Quill", R, 25.6, 6.4, "sans", PARCHMENT_300, "middle"))
	out.append(text("is the companion to", R, 29.0, 6.4, "sans", PARCHMENT_300, "middle"))
	out.append(text("liminalhq.ca", R, 45.0, 7.0, "sans-bold", INK_900, "middle"))
	return "square", "".join(out), defs


def sticker_version():
	out = [square_ground(VGA_BLACK)]
	out.append(rect(3.0, 3.0, STICKER - 6.0, STICKER - 6.0, "none", stroke=VGA_LIGHT_CYAN, stroke_width=0.45))
	out.append(rect(4.0, 4.0, STICKER - 8.0, STICKER - 8.0, "none", stroke=VGA_LIGHT_CYAN, stroke_width=0.45))
	out.append(text(EDITION.upper() + " RELEASE", R, 12.5, 7.4, "mono-bold", VGA_YELLOW, "middle"))
	out.append(text(VERSION, R, 27.0, 26, "mono-bold", VGA_WHITE, "middle"))
	out.append(text("Serial " + SERIAL, R, 35.0, 7.0, "mono", VGA_LIGHT_CYAN, "middle"))
	out.append(text("Volume QUILL", R, 40.0, 7.0, "mono", VGA_GREY, "middle"))
	return "square", "".join(out)


def sticker_document():
	out = [square_ground(PARCHMENT_200)]
	page_x, page_y, page_w, page_h, fold = R - 11.0, 5.5, 22.0, 28.0, 6.0
	out.append(polygon(((page_x, page_y), (page_x + page_w - fold, page_y), (page_x + page_w, page_y + fold), (page_x + page_w, page_y + page_h),
		(page_x, page_y + page_h)), WIN_LIGHT))
	out.append(polygon(((page_x + page_w - fold, page_y), (page_x + page_w - fold, page_y + fold), (page_x + page_w, page_y + fold)), PARCHMENT_300))
	out.append('<path d="M%s %sH%sL%s %sV%sH%sZ" fill="none" stroke="%s" stroke-width="0.45" stroke-linejoin="round"/>' % (
		number(page_x), number(page_y), number(page_x + page_w - fold), number(page_x + page_w), number(page_y + fold),
		number(page_y + page_h), number(page_x), INK_800))
	for index in range(5):
		out.append(rect(page_x + 3.0, page_y + 9.0 + index * 3.4, page_w - 6.0 - (5.0 if index == 4 else 0), 1.0, INK_600 if index else INK_800))
	out.append(emblem(page_x + page_w - 10.0, page_y + page_h - 9.0, 12.0, "s-document-emblem", tile=True))
	out.append(text(".QLD", R, 41.0, 11, "mono-bold", INK_800, "middle"))
	out.append(text("Quill document", R, 46.0, 6.8, "sans", MUTED, "middle"))
	return "square", "".join(out)


STICKERS = (
	("01-emblem", sticker_emblem), ("02-dos", sticker_dos), ("03-windows-3x", sticker_w31), ("04-windows-95", sticker_w95),
	("05-reactos", sticker_reactos), ("06-made-in-rust", sticker_rust), ("07-experimental", sticker_experimental),
	("08-labs", sticker_labs), ("09-floppy", sticker_floppy), ("10-series", sticker_series), ("11-version", sticker_version),
	("12-document", sticker_document),
)


# ---- the booklet ------------------------------------------------------------------------------------------------------------------------------
MARGIN = 9.0
BODY_POINTS = 7.6
PAGE_WIDTH = PAGE - 2 * MARGIN


def page_ground(fill=PARCHMENT_100):
	return rect(-CARD_BLEED, -CARD_BLEED, PAGE + 2 * CARD_BLEED, PAGE + 2 * CARD_BLEED, fill)


def page_head(title, kicker, folio):
	out = [text(kicker.upper(), MARGIN, MARGIN + 3.0, 6.4, "sans-bold", AMBER_DEEP, tracking=0.5)]
	out.append(text(title, MARGIN, MARGIN + 11.5, 16, "serif-bold", INK_700))
	out.append(rect(MARGIN, MARGIN + 14.5, 18.0, 0.7, AMBER))
	out.append(text(str(folio), PAGE - MARGIN, PAGE - 5.0, 6.4, "sans", MUTED, "end"))
	out.append(text("Quill " + VERSION, MARGIN, PAGE - 5.0, 6.4, "sans", MUTED))
	return "".join(out), MARGIN + 21.0


def body(words, y, width=PAGE_WIDTH, x=MARGIN, points=BODY_POINTS, face="sans", fill=INK_800, gap=2.0):
	svg, after = paragraph(words, x, y, width, points, face, fill)
	return svg, after + gap - points * POINT * 0.32


def system_chip(x, y, words, kind, width=None):
	"""A heading chip in the colours of a system's own period."""
	fill, ink, face = {"dos": (VGA_BLUE, VGA_YELLOW, "mono-bold"), "w31": (WIN_SKY, WIN_DARK, "sans-bold"),
		"w95": (WIN_TEAL, WIN_LIGHT, "sans-bold"), "linux": (INK_700, PARCHMENT_200, "sans-bold")}[kind]
	width = width or text_width(words, face, 7.6) + 5.0
	return rect(x, y, width, 5.4, fill, radius=0.8) + text(words, x + 2.5, y + 3.9, 7.6, face, ink), y + 5.4


def code(words, x, y, points=7.0):
	width = text_width(words, "mono", points) + 2.4
	return rect(x, y - points * POINT * 0.85, width, points * POINT * 1.25, "#f3ead0", radius=0.6) + text(words, x + 1.2, y, points, "mono", INK_800)


def booklet_cover():
	defs = ink_gradient("p1-ground")
	out = [rect(-CARD_BLEED, -CARD_BLEED, PAGE + 2 * CARD_BLEED, PAGE + 2 * CARD_BLEED, "url(#p1-ground)")]
	out.append(dusk_bands(-CARD_BLEED, 100.0, PAGE + 2 * CARD_BLEED, PAGE - 100.0 + CARD_BLEED))
	out.append(emblem(PAGE / 2 - 22.0, 12.0, 44.0, "p1-emblem", tile=True))
	out.append(text("Quill", PAGE / 2, 74.0, 34, "serif-bold", PARCHMENT_200, "middle"))
	out.append(text("The manual", PAGE / 2, 82.0, 10, "serif-italic", AMBER, "middle"))
	out.append(text(VERSION_LINE + " · DOS · Windows 3.x · Windows 95, 98 and Me · Linux", PAGE / 2, 90.0, 6.6, "sans", PARCHMENT_300, "middle"))
	out.append(text(LABS, PAGE / 2, 112.5, 7.0, "sans-bold", INK_900, "middle"))
	return defs, "".join(out)


def booklet_what():
	out = [page_ground()]
	head, y = page_head("What Quill is", "Welcome", 2)
	out.append(head)
	for words in (
		"Quill is a small rich-text word processor for the PCs of 1992 to 2000: fonts, lists, tab stops, headers and footers, pictures, "
		"a spelling checker and a real help file, in a program that fits on a floppy and runs in 4 MB.",
		"It comes as separate programs for separate machines. QUILLDOS.EXE runs on DOS, QUILL.EXE on Windows 3.1 and 3.11, and "
		"QUILL32.EXE on Windows 95, 98 and Me. One Rust core drives all three, so a letter typed in blue-screen DOS opens with its "
		"fonts on Windows 3.1, and in LibreOffice today.",
		"Quill is the companion to Cross-compiling to the past, a blog series on Liminal HQ about the toolchains it is built with.",
	):
		svg, y = body(words, y)
		out.append(svg)
	box_y = y + 1.0
	out.append(rect(MARGIN, box_y, PAGE_WIDTH, 19.0, PARCHMENT_200, radius=1.2, stroke=AMBER, stroke_width=0.35))
	out.append(text("Experimental", MARGIN + 3.0, box_y + 5.0, 7.6, "sans-bold", AMBER_DEEP))
	svg, _ = paragraph("Every build has run in emulators, virtual machines and Wine only, never on real hardware. Windows 98 has not been tried, and anything may change from one release to the next.", MARGIN + 3.0,
		box_y + 9.2, PAGE_WIDTH - 6.0, 6.8, "sans", INK_800, leading=1.28)
	out.append(svg)
	return "", "".join(out)


def booklet_platforms():
	out = [page_ground()]
	head, y = page_head("The platforms", "What runs where", 3)
	out.append(head)
	rows = (
		("dos", "DOS", "QUILLDOS.EXE", "A 386 or later with 4 MB, VGA or text mode. Text screens, and graphics with real fonts and a mouse.",
			"DOSBox Staging, js-dos"),
		("w31", "Windows 3.x", "QUILL.EXE", "Windows 3.1 or 3.11. Menus, toolbars, a ruler, the common dialogs and WinHelp.",
			"Windows for Workgroups 3.11 in DOSBox-X"),
		("w95", "Windows 95, 98, Me", "QUILL32.EXE", "Windows 95, 98 or Me on a Pentium. Print, Print Preview and Print to PDF.",
			"Windows 95 OSR2.5 in DOSBox-X, Me in VirtualBox, Wine"),
		("linux", "Linux", "quill-tty, quill", "The DOS text screen in a terminal, and a command that converts and inspects documents.",
			"Built and tested on the host"),
	)
	for kind, name, program, needs, tried in rows:
		chip, _ = system_chip(MARGIN, y, name, kind, width=31.0)
		out.append(chip)
		out.append(text(program, MARGIN + 34.0, y + 3.9, 6.8, "mono-bold", INK_700))
		svg, after = paragraph(needs, MARGIN + 34.0, y + 8.6, PAGE_WIDTH - 34.0, 6.8, "sans", INK_800, leading=1.25)
		out.append(svg)
		out.append(text("Tried in: " + tried, MARGIN + 34.0, after + 0.2, 6.2, "sans-italic", MUTED))
		y = after + 5.4
	return "", "".join(out)


def install_block(kind, name, steps, y, screenshot_name=None, master_dir=None):
	out = []
	chip, y = system_chip(MARGIN, y, name, kind)
	out.append(chip)
	y += 4.2
	text_width_available = PAGE_WIDTH - (36.0 if screenshot_name else 0)
	top = y
	for index, step in enumerate(steps):
		out.append(text("%d" % (index + 1), MARGIN + 0.5, y, 7.0, "sans-bold", AMBER_DEEP))
		svg, y = paragraph(step, MARGIN + 4.0, y, text_width_available - 4.0, 7.0, "sans", INK_800, leading=1.25)
		out.append(svg)
		y += 0.6
	if screenshot_name:
		shot_w, shot_h = 33.0, 24.75
		out.append(rect(PAGE - MARGIN - shot_w, top - 3.2, shot_w, shot_h, INK_800))
		out.append(screenshot(screenshot_name, PAGE - MARGIN - shot_w + 0.5, top - 2.7, shot_w - 1.0, shot_h - 1.0, master_dir))
		y = max(y, top - 3.2 + shot_h + 3.0)
	return "".join(out), y + 1.5


def booklet_install_one(master_dir):
	out = [page_ground()]
	head, y = page_head("Installing", "DOS and Windows 3.x", 4)
	out.append(head)
	svg, y = install_block("dos", "DOS", (
		"Put the DOS disk in drive A, or the CD in its drive.",
		"Type A:\\SETUP and press Enter. Quill goes in C:\\QUILL.",
		"Start it with QUILLDOS. To remove it, run SETUP /U from the disk.",
	), y, "install-dos-welcome.png", master_dir)
	out.append(svg)
	svg, y = install_block("w31", "Windows 3.1 and 3.11", (
		"In Program Manager choose File, Run, and type A:\\SETUP.",
		"Quill goes in C:\\QUILL with its own Program Manager group.",
		"To remove it, use Uninstall Quill in that group.",
	), y, "install-w31-wizard-welcome.png", master_dir)
	out.append(svg)
	return "", "".join(out)


def booklet_install_two(master_dir):
	out = [page_ground()]
	head, y = page_head("Installing", "Windows 95, 98, Me and Linux", 5)
	out.append(head)
	svg, y = install_block("w95", "Windows 95, 98 and Me", (
		"Choose Start, Run, and type A:\\SETUP.",
		"Quill goes in C:\\Program Files\\Quill and in the Start menu.",
		"To remove it, use Add/Remove Programs.",
	), y, "install-w95-wizard-welcome.png", master_dir)
	out.append(svg)
	svg, y = install_block("linux", "Linux", (
		"Linux has no disk yet: build from the repository with Rust.",
		"cargo install --path linux/quill-tty gives the terminal editor; cargo install --path crates/quill-cli gives quill.",
		"The Windows builds also run under Wine.",
	), y)
	out.append(svg)
	svg, y = body("Removing Quill deletes only the files Setup installed. It leaves QUILL.INI, your dictionary and your documents.", y + 0.5,
		points=6.8, fill=MUTED)
	out.append(svg)
	return "", "".join(out)


def booklet_disks():
	out = [page_ground()]
	head, y = page_head("The disk set", "What is on each disk", 6)
	out.append(head)
	header = ("Disk", "Volume", "Holds")
	columns = (MARGIN, MARGIN + 22.0, MARGIN + 45.0)
	for x, words in zip(columns, header):
		out.append(text(words, x, y, 6.6, "sans-bold", AMBER_DEEP))
	y += 1.6
	out.append(rect(MARGIN, y, PAGE_WIDTH, 0.3, INK_700))
	y += 4.4
	rows = (
		("DOS", "QUILL DOS", "SETUP.EXE, QUILLDOS.EXE, the help, the fonts and three dictionaries"),
		("Windows 3.x", "QUILL W31", "SETUP.EXE, QUILL.EXE, QUILL.HLP and three dictionaries"),
		("Windows 95", "QUILL W95", "SETUP.EXE, QUILL32.EXE, QUILL.HLP and three dictionaries"),
		("All-in-one, planned", "QUILL", "All three, sharing one copy of the common files, with the Canadian dictionary"),
		("CD-ROM, planned", "QUILL", "Everything, all three dictionaries; Windows 95 starts Setup by itself"),
	)
	for name, volume, holds in rows:
		out.append(text(name, columns[0], y, 6.8, "sans-bold", INK_800))
		out.append(text(volume, columns[1], y, 6.6, "mono", INK_800))
		svg, after = paragraph(holds, columns[2], y, PAGE - MARGIN - columns[2], 6.6, "sans", INK_800, leading=1.22)
		out.append(svg)
		y = after + 1.6
	y += 1.0
	for words in (
		"Every disk carries README.TXT and LICENCE.TXT, and is 1.44 MB. On the all-in-one disk and the CD, Windows 3.x Setup is "
		"A:\\WIN31\\SETUP and Windows 95 Setup is A:\\WIN95\\SETUP; A:\\SETUP is the DOS one.",
		"The serial is " + SERIAL + ": " + SERIAL_PREFIX + ", then the version " + VERSION + " in hexadecimal digits. DIR and VOL show the volume label.",
	):
		svg, y = body(words, y, points=6.8)
		out.append(svg)
	return "", "".join(out)


SHORTCUTS = (
	("File", (("New", "Ctrl+N"), ("Open", "Ctrl+O"), ("Save", "Ctrl+S"), ("Print", "Ctrl+P"), ("Exit", "Alt+F4"))),
	("Edit", (("Undo", "Ctrl+Z"), ("Redo", "Ctrl+Y"), ("Cut", "Ctrl+X"), ("Copy", "Ctrl+C"), ("Paste", "Ctrl+V"), ("Select all", "Ctrl+A"))),
	("Find", (("Find", "Ctrl+F"), ("Find next", "F3"), ("Replace", "Ctrl+H"), ("Spelling", "F7"))),
	("Character", (("Bold", "Ctrl+B"), ("Italic", "Ctrl+I"), ("Underline", "Ctrl+U"), ("Regular", "F5"), ("Superscript", "Ctrl+Shift+="),
		("Subscript", "Ctrl+="))),
	("Paragraph", (("Left", "Ctrl+L"), ("Centred", "Ctrl+E"), ("Right", "Ctrl+R"), ("Justified", "Ctrl+J"), ("Single, 1½, double", "Ctrl+1, 5, 2"),
		("Level up, down", "Alt+Shift+→ ←"))),
	("View", (("Zoom in", "Ctrl+numpad +"), ("Zoom out", "Ctrl+- or numpad -"), ("Page Width", "Ctrl+0 or numpad 0"),
		("Zoom", "Ctrl+wheel"))),
	("Moving", (("Word", "Ctrl+← →"), ("Paragraph", "Ctrl+↑ ↓"), ("Document ends", "Ctrl+Home, End"), ("Page break", "Ctrl+Enter"),
		("Help", "F1"))),
)


def shortcut_columns(x, y, width, columns, points, gap_after_group=2.4, leading=1.32, key_fill=INK_700, name_fill=INK_800,
		head_fill=AMBER_DEEP):
	out = []
	per_column = math.ceil(len(SHORTCUTS) / columns)
	column_width = width / columns
	for column in range(columns):
		cursor = y
		for group, entries in SHORTCUTS[column * per_column:(column + 1) * per_column]:
			out.append(text(group.upper(), x + column * column_width, cursor, max(MIN_POINTS, points * 0.86), "sans-bold", head_fill, tracking=0.3))
			cursor += points * POINT * leading
			for name, keys in entries:
				out.append(text(name, x + column * column_width, cursor, points, "sans", name_fill))
				out.append(text(keys, x + (column + 1) * column_width - 2.5, cursor, points, "mono-bold", key_fill, "end"))
				cursor += points * POINT * leading
			cursor += gap_after_group
	return "".join(out)


def booklet_keys():
	out = [page_ground()]
	head, y = page_head("Keyboard shortcuts", "On every build", 7)
	out.append(head)
	out.append(shortcut_columns(MARGIN, y, PAGE_WIDTH + 2.5, 2, 6.4, gap_after_group=1.6, leading=1.24))
	out.append(text("On DOS, Alt or F10 opens the menus.", MARGIN, PAGE - 10.5, 6.4, "sans-italic", MUTED))
	return "", "".join(out)


def booklet_credits():
	defs = ink_gradient("p8-ground")
	out = [rect(-CARD_BLEED, -CARD_BLEED, PAGE + 2 * CARD_BLEED, PAGE + 2 * CARD_BLEED, "url(#p8-ground)")]
	y = MARGIN + 5.0
	out.append(text("Credits and licences", MARGIN, y + 6.0, 14, "serif-bold", PARCHMENT_200))
	out.append(rect(MARGIN, y + 9.0, 18.0, 0.7, AMBER))
	y += 16.0
	for heading, words in (
		("Quill", COPYRIGHT + ". Licensed under the Apache License 2.0 or the MIT licence, at your option. Source: " + REPOSITORY + "."),
		("Desk fonts", "Desk Serif, Sans and Mono, © 2026 Liminal HQ, Scott Morris, under the SIL Open Font License 1.1."),
		("Dictionaries", "Built from SCOWL, © 2000–2018 Kevin Atkinson and others; its COPYRIGHT file travels inside every dictionary."),
		("DOS extender", "QUILLDOS.EXE carries CWSDPMI's stub, © Charles W. Sandmann, under its own licence."),
		("Installer", "Setup is the Liminal Installer, © 2026 Liminal HQ, Scott Morris, Apache-2.0 OR MIT."),
	):
		out.append(text(heading, MARGIN, y, 7.0, "sans-bold", AMBER))
		svg, y = paragraph(words, MARGIN, y + 3.4, PAGE_WIDTH, 6.6, "sans", PARCHMENT_200, leading=1.24)
		out.append(svg)
		y += 1.4
	svg, y = paragraph(TRADEMARKS, MARGIN, y + 0.6, PAGE_WIDTH, 6.2, "sans-italic", PARCHMENT_300, leading=1.22)
	out.append(svg)
	out.append(emblem(MARGIN, PAGE - 21.0, 12.0, "p8-emblem", tile=True))
	out.append(text(LABS, MARGIN + 15.0, PAGE - 15.6, 7.4, "sans-bold", PARCHMENT_200))
	out.append(text(SITE, MARGIN + 15.0, PAGE - 11.6, 6.8, "sans", AMBER))
	return defs, "".join(out)


BOOKLET_PAGES = (
	("page-1", "Cover", lambda master_dir: booklet_cover()),
	("page-2", "What Quill is", lambda master_dir: booklet_what()),
	("page-3", "The platforms", lambda master_dir: booklet_platforms()),
	("page-4", "Installing on DOS and Windows 3.x", booklet_install_one),
	("page-5", "Installing on Windows 95, 98, Me and Linux", booklet_install_two),
	("page-6", "The disk set", lambda master_dir: booklet_disks()),
	("page-7", "Keyboard shortcuts", lambda master_dir: booklet_keys()),
	("page-8", "Credits and licences (back cover)", lambda master_dir: booklet_credits()),
)


def shortcut_card():
	defs = ink_gradient("card-ground", 0, 0, 1, 0)
	out = [rect(-CARD_BLEED, -CARD_BLEED, CARD_WIDTH + 2 * CARD_BLEED, CARD_HEIGHT + 2 * CARD_BLEED, PARCHMENT_100)]
	out.append(rect(-CARD_BLEED, -CARD_BLEED, CARD_WIDTH + 2 * CARD_BLEED, 15.0 + CARD_BLEED, "url(#card-ground)"))
	out.append(dusk_bands(-CARD_BLEED, 15.0, CARD_WIDTH + 2 * CARD_BLEED, 1.6))
	out.append(emblem(5.0, 2.0, 11.0, "card-emblem", tile=True))
	out.append(text("Quill keyboard shortcuts", 19.0, 10.3, 13, "serif-bold", PARCHMENT_200))
	out.append(text("DOS · Windows 3.x · Windows 95", CARD_WIDTH - 6.0, 10.0, 6.8, "sans", PARCHMENT_300, "end"))
	out.append(shortcut_columns(6.0, 23.5, CARD_WIDTH - 9.5, 3, 7.0, gap_after_group=2.2, leading=1.28))
	out.append(rect(0, CARD_HEIGHT - 7.5, CARD_WIDTH, 0.3, RULE))
	out.append(text("On DOS, Alt or F10 opens the menus; Ctrl+I is Italic and Ctrl+H is Replace there too.", 6.0, CARD_HEIGHT - 4.6, 6.2,
		"sans", MUTED))
	out.append(text("On DOS the zoom keys are Alt+numpad +, Alt+- and Alt+0; Windows 3.x has no wheel.", 6.0, CARD_HEIGHT - 1.7, 6.2,
		"sans", MUTED))
	out.append(text("Quill " + VERSION + " · " + SITE, CARD_WIDTH - 6.0, CARD_HEIGHT - 3.4, 6.2, "sans-bold", INK_700, "end"))
	return defs, "".join(out)


# ---- writing them all -------------------------------------------------------------------------------------------------------------------------
FLOPPY_A = {"dos": floppy_a_dos, "w31": floppy_a_w31, "w95": floppy_a_w95, "all": floppy_a_all, "documents": floppy_a_documents}
FLOPPY_KEYS = ("dos", "w31", "w95", "all", "documents")


def main():
	out_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "masters")
	written = 0

	def write(relative, summary, width, height, bleed, drawn, origin=(0.0, 0.0)):
		nonlocal written
		defs, body_text = drawn[0], drawn[1]
		master(os.path.join(out_dir, relative), summary, width, height, bleed, defs, body_text, origin)
		written += 1

	for key in FLOPPY_KEYS:
		disk = DISKS.get(key, {})
		name = disk.get("long", "a documents disk")
		write("option-a/floppy-%s.svg" % key, "Quill floppy label, option A (era-accurate retro): %s." % name, LABEL, LABEL, LABEL_BLEED,
			FLOPPY_A[key](disk))
		drawn = floppy_b_documents(disk) if key == "documents" else floppy_b(key)
		write("option-b/floppy-%s.svg" % key, "Quill floppy label, option B (Afterglow dusk): %s." % name, LABEL, LABEL, LABEL_BLEED, drawn)
	for option, disc, jewel, tray in (("option-a", disc_a, jewel_a, tray_a), ("option-b", disc_b, jewel_b, tray_b)):
		folder = os.path.join(HERE, "masters", option)  # screenshot links are relative to the committed masters
		style = "option A (era-accurate retro)" if option == "option-a" else "option B (Afterglow dusk)"
		write(option + "/disc.svg", "Quill CD face, %s: the 118 mm print area with the 41 mm hole, centred on 0 0." % style,
			2 * DISC_PRINT_OUTER, 2 * DISC_PRINT_OUTER, DISC_BLEED, disc(folder), origin=(-DISC_PRINT_OUTER, -DISC_PRINT_OUTER))
		write(option + "/jewel-front.svg", "Quill jewel case front inlay, %s." % style, JEWEL, JEWEL, CARD_BLEED, jewel(folder))
		write(option + "/tray-card.svg", "Quill jewel case tray card with its two 6 mm spines, %s." % style, TRAY_WIDTH, TRAY_HEIGHT,
			CARD_BLEED, tray(folder))
	for name, draw in STICKERS:
		result = draw()
		shape, body_text = result[0], result[1]
		defs = result[2] if len(result) > 2 else ""
		write("stickers/sticker-%s.svg" % name, "Quill sticker, %s, 2 in %s." % (name[3:].replace("-", " "), shape), STICKER, STICKER,
			STICKER_BLEED, (defs, '<g data-shape="%s">%s</g>' % (shape, body_text)))
	booklet_dir = os.path.join(HERE, "masters", "booklet")
	for name, title, draw in BOOKLET_PAGES:
		write("booklet/%s.svg" % name, "Quill manual, %s: %s." % (name.replace("-", " "), title), PAGE, PAGE, CARD_BLEED, draw(booklet_dir))
	write("booklet/shortcut-card.svg", "Quill keyboard shortcut card, a 6 x 4 in index card.", CARD_WIDTH, CARD_HEIGHT, CARD_BLEED,
		shortcut_card())
	print("gen_media.py: wrote %d masters to %s for Quill %s (serial %s)" % (written, out_dir, VERSION, SERIAL))


if __name__ == "__main__":
	main()
