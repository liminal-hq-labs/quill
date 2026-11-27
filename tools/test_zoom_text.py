#!/usr/bin/env python3
# Checks the glyph-size check of zoom_text.py on the heights ReactOS gave the justified paragraph's first line (W9X-82) and on good ones.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/test_zoom_text.py        (needs Pillow, which zoom_text.py imports)
import contextlib
import io
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import zoom_text


def bands(heights):
	"""Line bands (top, bottom, left, right) with the given ink heights, one under the other."""
	out, top = [], 0
	for h in heights:
		out.append((top, top + h, 0, 100))
		top += h + 4
	return out


def run(heights):
	with contextlib.redirect_stdout(io.StringIO()) as log:
		bad = zoom_text.same_size("zoom", bands(heights))
	return bad, log.getvalue()


class SameSize(unittest.TestCase):
	def test_equal_lines_pass(self):
		self.assertEqual(run([19, 20, 17, 15, 15, 19, 19])[0], 0)

	def test_a_pixel_of_rounding_passes(self):
		self.assertEqual(run([8, 8, 7, 7, 6, 9, 7])[0], 0)

	def test_a_line_in_a_smaller_font_fails(self):
		# ReactOS at 125 %: the first line of the justified paragraph, 11 pixels tall among 19s.
		bad, text = run([19, 20, 17, 15, 15, 11, 19])
		self.assertEqual(bad, 1)
		self.assertIn("line 6", text)

	def test_a_line_in_a_larger_font_fails(self):
		self.assertEqual(run([8, 8, 7, 7, 6, 13, 8])[0], 1)

	def test_merged_lines_fail(self):
		# ReactOS at 50 %: the overlapped words merged the last two lines into one.
		self.assertEqual(run([8, 8, 7, 7, 6, 19])[0], 1)


if __name__ == "__main__":
	unittest.main()
