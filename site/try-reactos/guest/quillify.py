#!/usr/bin/env python3
"""Quillify: put the Quill emblem on the Afterglow dusk wallpaper and cut the 1024 x 768 BMP the ReactOS demo's desktop uses.

(c) Copyright 2026 Liminal HQ, Scott Morris
SPDX-License-Identifier: Apache-2.0 OR MIT

Usage: quillify.py AFTERGLOW_WALLPAPER.svg OUT.bmp [--emblem quill-emblem.svg] [--png FULL.png]

The Afterglow wallpaper is 3840 x 2160 with its low sun at (2880, 1846), radius 104 for the gold disc. The emblem overlay has the same canvas
with the Q's ring centred on that sun. The composite is cropped to its centre 4:3 (x 480, width 2880, as Afterglow's own render script does for
ReactOS) and scaled to 1024 x 768 with Lanczos, as a 24-bit BMP. The Afterglow art is read from the fetched source; none of it is kept here.
Needs rsvg-convert (librsvg) and ImageMagick 7 (magick).
"""
import argparse
import os
import re
import subprocess
import sys
import tempfile

WIDTH, HEIGHT = 3840, 2160
SUN = (2880, 1846)
SUN_RADIUS = 104
OUT_SIZE = (1024, 768)
CROP_WIDTH = HEIGHT * 4 // 3
CROP_X = (WIDTH - CROP_WIDTH) // 2
HERE = os.path.dirname(os.path.abspath(__file__))


def crop_scale():
	"""The scale from the 3840 x 2160 canvas to the BMP: 4:3 centre crop, then to 1024 x 768."""
	return OUT_SIZE[0] / CROP_WIDTH, OUT_SIZE[1] / HEIGHT


def sun_in_bmp():
	"""Where the sun's centre and radius land in the BMP, in pixels."""
	sx, sy = crop_scale()
	return ((SUN[0] - CROP_X) * sx, SUN[1] * sy, SUN_RADIUS * sx)


def find_sun(svg_text):
	"""The gold sun disc of the Afterglow wallpaper: the circle at the sun with the smallest radius. Returns (cx, cy, r) or None."""
	best = None
	for m in re.finditer(r'<circle\s+cx="([\d.]+)"\s+cy="([\d.]+)"\s+r="([\d.]+)"', svg_text):
		cx, cy, r = (float(v) for v in m.groups())
		if (cx, cy) == (float(SUN[0]), float(SUN[1])) and (best is None or r < best[2]):
			best = (cx, cy, r)
	return best


def run(cmd):
	r = subprocess.run(cmd, capture_output=True)
	if r.returncode != 0:
		sys.exit('%s failed: %s' % (cmd[0], r.stderr.decode(errors='replace')))


def quillify(wallpaper, emblem, out, keep_png=None):
	with open(wallpaper, encoding='utf-8') as f:
		text = f.read()
	if 'viewBox="0 0 %d %d"' % (WIDTH, HEIGHT) not in text:
		sys.exit('quillify: %s is not a %d x %d wallpaper' % (wallpaper, WIDTH, HEIGHT))
	sun = find_sun(text)
	if sun is None or sun[2] != SUN_RADIUS:
		sys.exit('quillify: the sun is not at %s with radius %d in %s (Afterglow moved it: update SUN here and in quill-emblem.svg)' % (SUN, SUN_RADIUS, wallpaper))
	with tempfile.TemporaryDirectory() as tmp:
		base, over, full = (os.path.join(tmp, n) for n in ('base.png', 'over.png', 'full.png'))
		run(['rsvg-convert', '-w', str(WIDTH), '-h', str(HEIGHT), '-o', base, wallpaper])
		run(['rsvg-convert', '-w', str(WIDTH), '-h', str(HEIGHT), '-o', over, emblem])
		run(['magick', base, over, '-compose', 'over', '-composite', '-alpha', 'off', full])
		if keep_png:
			run(['magick', full, keep_png])
		run(['magick', full, '-crop', '%dx%d+%d+0' % (CROP_WIDTH, HEIGHT, CROP_X), '+repage', '-filter', 'Lanczos',
			'-resize', '%dx%d!' % OUT_SIZE, '-type', 'TrueColor', 'BMP3:' + out])


def main(argv):
	ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
	ap.add_argument('wallpaper')
	ap.add_argument('out')
	ap.add_argument('--emblem', default=os.path.join(HERE, 'quill-emblem.svg'))
	ap.add_argument('--png', help='also keep the full-size composite here')
	a = ap.parse_args(argv)
	quillify(a.wallpaper, a.emblem, a.out, a.png)
	x, y, r = sun_in_bmp()
	print('quillify: %s (sun at %.1f, %.1f radius %.1f)' % (a.out, x, y, r))


if __name__ == '__main__':
	main(sys.argv[1:])
