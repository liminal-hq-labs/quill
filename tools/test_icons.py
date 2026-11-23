#!/usr/bin/env python3
# Checks the committed icon files: structure, depths, palettes and masks, read back with a parser that shares nothing with the writer.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/test_icons.py        (needs Pillow and numpy, not librsvg)
import io
import struct
import sys
import unittest
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import make_icons as m  # noqa: E402

ICONS = Path(__file__).resolve().parent.parent / "assets" / "icon"


def load(name):
	return (ICONS / name).read_bytes()


class QuillIco(unittest.TestCase):
	def test_images_are_the_planned_set(self):
		got = [(e["w"], e["bpp"], e["png"]) for e in m.read_ico(load("QUILL.ICO"))]
		want = [(s, bpp, False) for s in (16, 32, 48) for bpp in (4, 8, 24, 32)] + [(256, 32, True)]
		self.assertEqual(got, want)

	def test_directory_matches_the_images(self):
		data = load("QUILL.ICO")
		count = struct.unpack_from("<H", data, 4)[0]
		for i in range(count):
			w, h, colours, res, planes, bpp, size, off = struct.unpack_from("<BBBBHHII", data, 6 + 16 * i)
			self.assertEqual((res, planes), (0, 1))
			self.assertEqual(colours, {4: 16}.get(bpp, 0))
			self.assertEqual(w, h)
			if bpp == 32 and w == 0:
				self.assertEqual(data[off : off + 8], b"\x89PNG\r\n\x1a\n")
				self.assertEqual(struct.unpack(">II", data[off + 16 : off + 24]), (256, 256))
			else:
				self.assertEqual(struct.unpack_from("<I", data, off)[0], 40)
			self.assertLessEqual(off + size, len(data))

	def test_four_bit_images_use_only_the_standard_sixteen(self):
		data = load("QUILL.ICO")
		for i, e in enumerate(m.read_ico(data)):
			if e["bpp"] == 4:
				self.assertEqual(e["palette"], m.WIN16)
				img = m.decode(data, i)
				px = np.array(img).reshape(-1, 4)
				colours = {tuple(int(v) for v in p[:3]) for p in px if p[3]}
				self.assertTrue(colours <= set(m.WIN16), colours - set(m.WIN16))

	def test_eight_bit_palettes_carry_the_twenty_static_colours(self):
		for e in m.read_ico(load("QUILL.ICO")):
			if e["bpp"] == 8:
				pal = e["palette"]
				self.assertEqual(len(pal), 256)
				self.assertEqual(pal[:10] + pal[246:], m.STATIC20)

	def test_masks_agree_with_the_pixels(self):
		data = load("QUILL.ICO")
		for i, e in enumerate(m.read_ico(data)):
			if e["png"]:
				continue
			img = m.decode(data, i)
			a = [int(v) for v in np.array(img)[:, :, 3].reshape(-1)]
			self.assertIn(0, a, "the rounded corners are transparent")
			self.assertEqual(a[img.width // 2 + img.width * (img.height // 2)], 255)
			if e["bpp"] != 32:
				self.assertTrue(set(a) <= {0, 255})

	def test_pillow_reads_every_image(self):
		data = load("QUILL.ICO")
		ico = Image.open(io.BytesIO(data))
		self.assertEqual(sorted(ico.ico.sizes()), sorted({(16, 16), (32, 32), (48, 48), (256, 256)}))
		for e in m.read_ico(data):
			img = ico.ico.getimage((e["w"], e["h"]), e["bpp"]) if e["bpp"] in (24, 32) or e["png"] else None
			if img is not None:
				self.assertEqual(img.size, (e["w"], e["h"]))

	def test_the_256_image_is_the_only_png(self):
		self.assertEqual([e["w"] for e in m.read_ico(load("QUILL.ICO")) if e["png"]], [256])


class Windows3(unittest.TestCase):
	def check(self, name, bpp, colours):
		data = load(name)
		entries = m.read_ico(data)
		self.assertEqual([(e["w"], e["h"], e["bpp"], e["png"]) for e in entries], [(32, 32, bpp, False)])
		count, = struct.unpack_from("<H", data, 4)
		self.assertEqual(count, 1)
		w, h, ncol, res, planes, bits, size, off = struct.unpack_from("<BBBBHHII", data, 6)
		self.assertEqual((w, h, ncol, planes, bits), (32, 32, colours if colours < 256 else 0, 1, bpp))
		# A Windows 3.x RT_ICON is this image as it stands: a 40-byte header, the palette, the XOR bits, the AND mask.
		self.assertEqual(size, 40 + 4 * (1 << bpp) + 32 * 32 * bpp // 8 + 32 * 4)
		return entries[0]

	def test_sixteen_colours(self):
		e = self.check("QUILL31.ICO", 4, 16)
		self.assertEqual(e["palette"], m.WIN16)

	def test_two_hundred_fifty_six_colours(self):
		e = self.check("QUILL31X.ICO", 8, 256)
		self.assertEqual(e["palette"][:10] + e["palette"][246:], m.STATIC20)


if __name__ == "__main__":
	unittest.main()
