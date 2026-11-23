#!/usr/bin/env python3
# Builds Quill's icon files from the SVG masters: the PNG set, QUILL.ICO (Windows 95 and later) and the Windows 3.x icons.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/make_icons.py [--out assets/icon] [--sheet contact-sheet.png]
#
# Needs Python 3 with Pillow and numpy, and `rsvg-convert` (librsvg). Without them, run it in Docker: tools/make-icons.sh.
# The output is reproducible: the same inputs give the same bytes.
#
# Inputs:   assets/icon/quill.svg         the master, drawn for 48 px and up
#           assets/icon/quill-small.svg   the small-size drawing (a bolder ring and a plain nib), used up to 32 px
#           tools/icon-src/quill-flat.svg the small drawing again in the sixteen standard Windows colours, no gradients
#
# Outputs:  assets/icon/quill-{16,24,32,48,64,128,256,512}.png, social-preview.png (1280 by 640)
#           assets/icon/QUILL.ICO     16, 32 and 48 px at 4, 8, 24 and 32 bits, and 256 px as a PNG
#           assets/icon/QUILL31.ICO   32 px, 16 colours: the Windows 3.x icon (D76)
#           assets/icon/QUILL31X.ICO  32 px, 256 colours: the optional Windows 3.x variant
#
# How each depth is made
#   4-bit   the flat drawing is rendered at eight times the size, every sample takes the nearest of the sixteen standard Windows
#           colours, and each pixel takes the most common sample in its cell (transparent if most samples are). No antialiasing
#           blends, so no pixel is a colour the palette only half has.
#   8-bit   the gradient drawing is rendered at the size, 236 colours are found by median cut over the opaque pixels, and the palette
#           is laid out as Windows expects: its 20 static colours at indices 0 to 9 and 246 to 255, the optimized ones between.
#           Pixels take the nearest of all 256. Transparency is the 1-bit AND mask (alpha under 128 is transparent).
#   24-bit  the gradient drawing with the same 1-bit mask.
#   32-bit  the gradient drawing with an alpha channel; the AND mask is set where alpha is 0, so Windows 98 and Me, which ignore the
#           alpha, still see the shape.
#   256 px  a 32-bit PNG, the only PNG-compressed image (Windows Vista and later; older Windows never reads past the 48 px images).
import argparse
import io
import struct
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parent.parent

# The sixteen standard Windows colours, in palette order, and the four extra static colours of the 256-colour system palette.
WIN16 = [(0, 0, 0), (128, 0, 0), (0, 128, 0), (128, 128, 0), (0, 0, 128), (128, 0, 128), (0, 128, 128), (192, 192, 192),
		 (128, 128, 128), (255, 0, 0), (0, 255, 0), (255, 255, 0), (0, 0, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255)]
WIN_EXTRA = [(192, 220, 192), (166, 202, 240), (255, 251, 240), (160, 160, 164)]
# Indices 0 to 9 and 246 to 255 of the system palette.
STATIC_LOW = WIN16[:8] + WIN_EXTRA[:2]
STATIC_HIGH = WIN_EXTRA[2:] + WIN16[8:]
STATIC20 = STATIC_LOW + STATIC_HIGH

# The 16 px, 16-colour image is drawn by hand: sampling a 1.75 px ring gives a lumpy one. t is transparent, n navy, y yellow, w white.
HAND16 = [
	"tttnnnnnnnnnnttt",
	"ttnnnnnnnnnnnntt",
	"tnnnyyyyyynnnnnt",
	"nnnyyyyyyyynnnnn",
	"nnyyynnnnyyynnnn",
	"nnyynnnnnnyynnnn",
	"nnyynnnnnnnyynnn",
	"nnyynnnnnnnnnnnn",
	"nnyynnnnnnwwnnnn",
	"nnyyynnnnwwwwnnn",
	"nnnyyyynwwwwwwnn",
	"nnnnyyyynwwwwwnn",
	"nnnnnnnnnnwwwwnn",
	"nnnnnnnnnnnwwwnn",
	"nnnnnnnnnnnnnwnn",
	"tnnnnnnnnnnnnnnt",
]

PNG_SIZES = [16, 24, 32, 48, 64, 128, 256, 512]
SMALL_UP_TO = 32
ICO_SIZES = [16, 32, 48]


class IconError(Exception):
	pass


def render(svg, size):
	"""The SVG at size by size pixels, as straight-alpha RGBA."""
	try:
		out = subprocess.run(["rsvg-convert", "-w", str(size), "-h", str(size), str(svg)], check=True, capture_output=True).stdout
	except FileNotFoundError as e:
		raise IconError("rsvg-convert is not installed (try tools/make-icons.sh, which runs this in Docker)") from e
	return Image.open(io.BytesIO(out)).convert("RGBA")


def art_for(size):
	return ROOT / "assets/icon" / ("quill-small.svg" if size <= SMALL_UP_TO else "quill.svg")


# --- Quantizing ------------------------------------------------------------------------------------------------------------------

def nearest(rgb, palette):
	"""Index of the nearest palette colour to each row of rgb (N by 3), by squared distance weighted for the eye."""
	pal = np.array(palette, dtype=np.int32)
	w = np.array([2, 4, 3], dtype=np.int32)
	d = ((rgb[:, None, :].astype(np.int32) - pal[None, :, :]) ** 2 * w).sum(axis=2)
	return d.argmin(axis=1)


def flat16(size):
	"""(indices into WIN16, opaque mask), both size by size, from the flat drawing (or by hand at 16 px)."""
	if size == 16:
		code = {"n": 4, "y": 11, "w": 15}
		return (np.array([[code.get(c, 0) for c in row] for row in HAND16], dtype=np.int32),
				np.array([[c != "t" for c in row] for row in HAND16]))
	ss = 8
	img = np.array(render(ROOT / "tools/icon-src/quill-flat.svg", size * ss))
	idx = nearest(img[:, :, :3].reshape(-1, 3), WIN16).reshape(size * ss, size * ss)
	solid = img[:, :, 3] >= 128
	out = np.zeros((size, size), dtype=np.int32)
	opaque = np.zeros((size, size), dtype=bool)
	for y in range(size):
		for x in range(size):
			cell = (slice(y * ss, (y + 1) * ss), slice(x * ss, (x + 1) * ss))
			if solid[cell].sum() * 2 <= ss * ss:
				continue
			opaque[y, x] = True
			out[y, x] = np.bincount(idx[cell][solid[cell]], minlength=16).argmax()
	return out, opaque


def optimized256(size):
	"""(indices into the 256-colour palette, opaque mask, palette) for the gradient drawing."""
	img = np.array(render(art_for(size), size))
	opaque = img[:, :, 3] >= 128
	rgb = img[:, :, :3]
	pixels = rgb[opaque]
	strip = Image.fromarray(pixels.reshape(1, -1, 3).astype(np.uint8), "RGB")
	q = strip.quantize(colors=236, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
	flat = q.getpalette()[: 3 * 236]
	optimized = [tuple(flat[i : i + 3]) for i in range(0, len(flat), 3)]
	optimized += [(0, 0, 0)] * (236 - len(optimized))
	palette = STATIC_LOW + optimized + STATIC_HIGH
	idx = np.zeros((size, size), dtype=np.int32)
	idx[opaque] = nearest(pixels, palette)
	return idx, opaque, palette


def truecolour(size):
	"""Straight-alpha RGBA (size by size by 4) of the gradient drawing."""
	return np.array(render(art_for(size), size))


# --- DIB and ICO writing ---------------------------------------------------------------------------------------------------------

def and_mask(transparent):
	"""The 1-bit AND mask, bottom row first, each row padded to four bytes. 1 means transparent."""
	h, w = transparent.shape
	stride = (w + 31) // 32 * 4
	out = bytearray()
	for y in range(h - 1, -1, -1):
		row = bytearray(stride)
		for x in range(w):
			if transparent[y, x]:
				row[x >> 3] |= 0x80 >> (x & 7)
		out += row
	return bytes(out)


def dib(width, height, bpp, xor, mask, palette=None):
	"""An icon image: BITMAPINFOHEADER with the doubled height, the palette, the XOR bits and the AND mask."""
	ncolours = len(palette) if palette else 0
	head = struct.pack("<IiiHHIIiiII", 40, width, height * 2, 1, bpp, 0, len(xor) + len(mask), 0, 0, ncolours, 0)
	pal = b"".join(bytes([b, g, r, 0]) for r, g, b in palette) if palette else b""
	return head + pal + xor + mask


def indexed_dib(idx, opaque, palette, bpp):
	h, w = idx.shape
	xor = bytearray()
	for y in range(h - 1, -1, -1):
		row = [int(idx[y, x]) if opaque[y, x] else 0 for x in range(w)]
		if bpp == 4:
			if w % 2:
				row.append(0)
			data = bytes((row[i] << 4) | row[i + 1] for i in range(0, len(row), 2))
		else:
			data = bytes(row)
		xor += data + bytes(-len(data) % 4)
	return dib(w, h, bpp, bytes(xor), and_mask(~opaque), palette)


def true_dib(rgba, bpp):
	h, w, _ = rgba.shape
	alpha = rgba[:, :, 3]
	if bpp == 32:
		transparent = alpha == 0
	else:
		transparent = alpha < 128
	xor = bytearray()
	for y in range(h - 1, -1, -1):
		row = bytearray()
		for x in range(w):
			r, g, b, a = (int(v) for v in rgba[y, x])
			if a == 0 or (bpp == 24 and transparent[y, x]):
				r = g = b = 0
			row += bytes([b, g, r, a]) if bpp == 32 else bytes([b, g, r])
		xor += row + bytes(-len(row) % 4)
	return dib(w, h, bpp, bytes(xor), and_mask(transparent))


def ico(images):
	"""images: list of (width, height, bpp, ncolours, data). One .ico file."""
	out = struct.pack("<HHH", 0, 1, len(images))
	offset = 6 + 16 * len(images)
	body = b""
	for w, h, bpp, ncol, data in images:
		out += struct.pack("<BBBBHHII", w % 256, h % 256, ncol if ncol < 256 else 0, 0, 1, bpp, len(data), offset + len(body))
		body += data
	return out + body


def image_entry(size, bpp):
	if bpp == 4:
		idx, opaque = flat16(size)
		return (size, size, 4, 16, indexed_dib(idx, opaque, WIN16, 4))
	if bpp == 8:
		idx, opaque, pal = optimized256(size)
		return (size, size, 8, 256, indexed_dib(idx, opaque, pal, 8))
	return (size, size, bpp, 0, true_dib(truecolour(size), bpp))


def png_bytes(img, optimize=True):
	buf = io.BytesIO()
	img.save(buf, "PNG", optimize=optimize)
	return buf.getvalue()


def build_quill_ico():
	images = []
	for size in ICO_SIZES:
		for bpp in (4, 8, 24, 32):
			images.append(image_entry(size, bpp))
	images.append((256, 256, 32, 0, png_bytes(render(ROOT / "assets/icon/quill.svg", 256))))
	return ico(images)


def build_win31_icos():
	sixteen = ico([image_entry(32, 4)])
	idx, opaque, pal = optimized256(32)
	# Windows 3.x draws an icon through the device palette, so the 256-colour variant keeps to the 20 static colours plus its own.
	return sixteen, ico([(32, 32, 8, 256, indexed_dib(idx, opaque, pal, 8))])


# --- PNGs and the social preview -------------------------------------------------------------------------------------------------

def social_preview():
	w, h = 1280, 640
	y, x = np.mgrid[0:h, 0:w]
	d = np.sqrt(((x - w * 0.5) / w) ** 2 + ((y - h * 0.5) / h) ** 2)
	t = np.clip(d / 0.62, 0, 1)
	top, bot = np.array([22, 39, 90]), np.array([7, 13, 36])
	bg = (top[None, None, :] * (1 - t[:, :, None]) + bot[None, None, :] * t[:, :, None]).astype(np.uint8)
	canvas = Image.fromarray(bg, "RGB").convert("RGBA")
	size = 400
	icon = render(ROOT / "assets/icon/quill.svg", size)
	glow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
	shadow = Image.new("RGBA", (size, size), (0, 0, 0, 160))
	shadow.putalpha(icon.getchannel("A").point(lambda a: a * 160 // 255))
	glow.paste(shadow, ((w - size) // 2, (h - size) // 2 + 24), shadow)
	canvas = Image.alpha_composite(canvas, glow.filter(ImageFilter.GaussianBlur(28)))
	canvas.alpha_composite(icon, ((w - size) // 2, (h - size) // 2))
	return canvas.convert("RGB")


# --- Reading back, for the checks and the contact sheet --------------------------------------------------------------------------

def read_ico(data):
	"""Parses an .ico with struct only. Returns a list of dicts."""
	reserved, kind, count = struct.unpack_from("<HHH", data, 0)
	if (reserved, kind) != (0, 1):
		raise IconError("not an icon file")
	out = []
	for i in range(count):
		w, h, ncol, res, planes, bpp, size, off = struct.unpack_from("<BBBBHHII", data, 6 + 16 * i)
		blob = data[off : off + size]
		if off + size > len(data):
			raise IconError(f"entry {i} runs past the end of the file")
		if blob[:8] == b"\x89PNG\r\n\x1a\n":
			iw, ih = struct.unpack(">II", blob[16:24])
			out.append({"w": w or 256, "h": h or 256, "bpp": bpp, "png": True, "pixels": iw, "ncol": ncol})
			continue
		hsize, bw, bh2, bplanes, bbpp, comp, _simg, _x, _y, used, _imp = struct.unpack_from("<IiiHHIIiiII", blob, 0)
		if (hsize, comp, bplanes) != (40, 0, 1) or bbpp != bpp or bw != (w or 256) or bh2 != 2 * (h or 256):
			raise IconError(f"entry {i}: header disagrees with directory")
		npal = used if bpp <= 8 else 0
		if bpp <= 8 and npal != (1 << bpp):
			raise IconError(f"entry {i}: {npal} palette entries for {bpp} bits")
		stride = (bw * bpp + 31) // 32 * 4
		mask_stride = (bw + 31) // 32 * 4
		want = 40 + 4 * npal + stride * (bh2 // 2) + mask_stride * (bh2 // 2)
		if len(blob) != want:
			raise IconError(f"entry {i}: {len(blob)} bytes, expected {want}")
		out.append({"w": bw, "h": bh2 // 2, "bpp": bpp, "png": False, "ncol": ncol, "palette": [tuple(blob[40 + 4 * k : 43 + 4 * k][::-1]) for k in range(npal)]})
	return out


def decode(data, index):
	"""Decodes entry `index` to RGBA with its own DIB parser (not Pillow's)."""
	count = struct.unpack_from("<H", data, 4)[0]
	if not 0 <= index < count:
		raise IconError("no such entry")
	w, h, _c, _r, _p, bpp, size, off = struct.unpack_from("<BBBBHHII", data, 6 + 16 * index)
	blob = data[off : off + size]
	if blob[:8] == b"\x89PNG\r\n\x1a\n":
		return Image.open(io.BytesIO(blob)).convert("RGBA")
	w, h = w or 256, h or 256
	npal = (1 << bpp) if bpp <= 8 else 0
	pal = [tuple(blob[40 + 4 * k : 43 + 4 * k][::-1]) for k in range(npal)]
	stride = (w * bpp + 31) // 32 * 4
	bits = blob[40 + 4 * npal :]
	mask = bits[stride * h :]
	mstride = (w + 31) // 32 * 4
	img = Image.new("RGBA", (w, h))
	px = img.load()
	for y in range(h):
		row = bits[stride * (h - 1 - y) : stride * (h - y)]
		mrow = mask[mstride * (h - 1 - y) : mstride * (h - y)]
		for x in range(w):
			if bpp == 4:
				v = (row[x >> 1] >> (4 if x % 2 == 0 else 0)) & 15
				c = pal[v] + (255,)
			elif bpp == 8:
				c = pal[row[x]] + (255,)
			elif bpp == 24:
				c = (row[3 * x + 2], row[3 * x + 1], row[3 * x], 255)
			else:
				c = (row[4 * x + 2], row[4 * x + 1], row[4 * x], row[4 * x + 3])
			if mrow[x >> 3] & (0x80 >> (x & 7)) and (bpp != 32 or c[3] == 0):
				c = (0, 0, 0, 0)
			px[x, y] = c
	return img


def contact_sheet(data, path, scale=4):
	"""Every image in the file on a checkerboard, at its own size and enlarged, in rows by size and columns by depth."""
	entries = read_ico(data)
	cell = 48 * scale
	cols = 5
	sheet = Image.new("RGBA", (cols * (cell + 8) + 8, ((len(entries) + cols - 1) // cols) * (cell + 8) + 8), (90, 90, 90, 255))
	for i, e in enumerate(entries):
		img = decode(data, i)
		s = max(1, cell // max(img.size))
		img = img.resize((img.width * s, img.height * s), Image.NEAREST) if img.width <= 48 else img.resize((cell, cell), Image.LANCZOS)
		tile = Image.new("RGBA", (cell, cell), (255, 0, 255, 255))
		tile.alpha_composite(img, ((cell - img.width) // 2, (cell - img.height) // 2))
		sheet.paste(tile, (8 + (i % cols) * (cell + 8), 8 + (i // cols) * (cell + 8)))
	sheet.convert("RGB").save(path)


# --- Main ------------------------------------------------------------------------------------------------------------------------

def main():
	ap = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
	ap.add_argument("--out", default=str(ROOT / "assets/icon"))
	ap.add_argument("--sheet", help="also write a contact sheet of every image in QUILL.ICO to this PNG")
	args = ap.parse_args()
	out = Path(args.out)
	out.mkdir(parents=True, exist_ok=True)

	for size in PNG_SIZES:
		(out / f"quill-{size}.png").write_bytes(png_bytes(render(art_for(size), size)))
	(out / "social-preview.png").write_bytes(png_bytes(social_preview()))

	quill = build_quill_ico()
	(out / "QUILL.ICO").write_bytes(quill)
	sixteen, x256 = build_win31_icos()
	(out / "QUILL31.ICO").write_bytes(sixteen)
	(out / "QUILL31X.ICO").write_bytes(x256)
	for name in ("QUILL.ICO", "QUILL31.ICO", "QUILL31X.ICO"):
		entries = read_ico((out / name).read_bytes())
		print(f"{name}: " + ", ".join(f"{e['w']}x{e['h']}@{e['bpp']}" + ("png" if e["png"] else "") for e in entries))
	if args.sheet:
		contact_sheet(quill, args.sheet)


if __name__ == "__main__":
	try:
		main()
	except IconError as e:
		sys.exit(f"make_icons: {e}")
