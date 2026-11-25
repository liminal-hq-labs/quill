#!/usr/bin/env python3
"""Quillify: put the Quill emblem on the Afterglow dusk wallpaper and cut the 1024 x 768 BMP the ReactOS demo's desktop uses.

(c) Copyright 2026 Liminal HQ, Scott Morris
SPDX-License-Identifier: Apache-2.0 OR MIT

Usage: quillify.py AFTERGLOW_WALLPAPER.svg OUT.bmp [--emblem quill-emblem.svg] [--png FULL.png] [--dither N] [--seed N] [--rgb565]

The Afterglow wallpaper is 3840 x 2160 with its low sun at (2880, 1846), radius 104 for the gold disc. The emblem overlay has the same canvas
with the Q's ring centred on that sun. The composite is cropped to its centre 4:3 (x 480, width 2880, as Afterglow's own render script does for
ReactOS) and scaled to 1024 x 768 with Lanczos, as a 24-bit BMP. The Afterglow art is read from the fetched source; none of it is kept here.
Needs rsvg-convert (librsvg), ImageMagick 7 (magick) and numpy.

The smooth Afterglow wallpapers band even at 24 bits, so --dither N adds the seeded triangular noise of +-N levels (Afterglow's
scripts/render-wallpapers.mjs uses N = 1, seed 2026) to the 1024 x 768 pixels after the crop and scale, because scaling a dithered image averages
the noise away. The generator is the same mulberry32, keyed the same way, so the noise is the same as Afterglow's own BMP, and the result is
deterministic. --rgb565 then reduces the pixels to the 5-6-5 levels of the demo guest's 16-bit display with an 8 x 8 ordered dither and
writes them back as 24-bit, so the guest's own reduction loses nothing more and the gradient does not band on screen.
"""
import argparse
import os
import re
import subprocess
import sys
import tempfile

import numpy as np

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


DEFAULT_SEED = 2026
DITHER_KEY = 'afterglow-dusk-dark-smooth-bmp'   # Afterglow's key: `${id}-bmp`


def mulberry32_stream(seed, count):
	"""The first `count` draws in [0, 1) of mulberry32(seed), as Afterglow's script makes them. The state moves by a constant each draw, so it is a vector sum."""
	a = (np.uint64(seed & 0xFFFFFFFF) + np.arange(1, count + 1, dtype=np.uint64) * np.uint64(0x6D2B79F5)) & np.uint64(0xFFFFFFFF)
	a = a.astype(np.uint32)
	with np.errstate(over='ignore'):
		t = (a ^ (a >> np.uint32(15))) * (a | np.uint32(1))
		t = (t + (t ^ (t >> np.uint32(7))) * (t | np.uint32(61))) ^ t
		t = t ^ (t >> np.uint32(14))
	return t.astype(np.float64) / 4294967296.0


def fnv_hash(text):
	"""Afterglow's string hash: FNV-1a over the character codes, 32 bits."""
	h = 2166136261
	for ch in text:
		h = ((h ^ ord(ch)) * 16777619) & 0xFFFFFFFF
	return h


def triangular_dither(rgb, amount, key, seed=DEFAULT_SEED):
	"""rgb: uint8 array. Adds triangular noise (two uniform draws) of +-amount levels, rounded and clamped, as render-wallpapers.mjs dither() does."""
	flat = rgb.reshape(-1)
	draws = mulberry32_stream(fnv_hash(key) ^ seed, flat.size * 2)
	noise = (draws[0::2] + draws[1::2] - 1.0) * amount
	return np.clip(np.floor(flat + noise + 0.5), 0, 255).astype(np.uint8).reshape(rgb.shape)


def bayer8():
	m = np.array([[0]])
	for _ in range(3):
		m = np.block([[4 * m, 4 * m + 2], [4 * m + 3, 4 * m + 1]])
	return (m + 0.5) / 64.0


def to_rgb565(rgb):
	"""Reduce uint8 RGB (h, w, 3) to the levels of a 16-bit 5-6-5 display with an 8 x 8 ordered dither, expanded back to 8 bits the way a display
	driver widens them (top bits repeated), so converting again by truncation or rounding gives the same levels."""
	h, w, _ = rgb.shape
	thr = np.tile(bayer8(), (h // 8 + 1, w // 8 + 1))[:h, :w]
	out = np.empty_like(rgb)
	for c, bits in enumerate((5, 6, 5)):
		top = (1 << bits) - 1
		v = rgb[:, :, c].astype(np.float64) * top / 255.0
		q = np.clip(np.floor(v + thr), 0, top).astype(np.uint32)
		out[:, :, c] = ((q << (8 - bits)) | (q >> (2 * bits - 8))).astype(np.uint8)
	return out


def run(cmd):
	r = subprocess.run(cmd, capture_output=True)
	if r.returncode != 0:
		sys.exit('%s failed: %s' % (cmd[0], r.stderr.decode(errors='replace')))


def quillify(wallpaper, emblem, out, keep_png=None, dither=0, seed=DEFAULT_SEED, rgb565=False):
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
		crop = ['-crop', '%dx%d+%d+0' % (CROP_WIDTH, HEIGHT, CROP_X), '+repage', '-filter', 'Lanczos', '-resize', '%dx%d!' % OUT_SIZE]
		if not dither and not rgb565:
			run(['magick', full] + crop + ['-type', 'TrueColor', 'BMP3:' + out])
			return
		raw = os.path.join(tmp, 'small.rgb')
		run(['magick', full] + crop + ['-alpha', 'off', '-depth', '8', 'rgb:' + raw])
		w, h = OUT_SIZE
		px = np.fromfile(raw, dtype=np.uint8).reshape(h, w, 3)
		if dither:
			px = triangular_dither(px, dither, DITHER_KEY, seed)
		if rgb565:
			px = to_rgb565(px)
		px.tofile(raw)
		run(['magick', '-size', '%dx%d' % OUT_SIZE, '-depth', '8', 'rgb:' + raw, '-type', 'TrueColor', 'BMP3:' + out])


def main(argv):
	ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
	ap.add_argument('wallpaper')
	ap.add_argument('out')
	ap.add_argument('--emblem', default=os.path.join(HERE, 'quill-emblem.svg'))
	ap.add_argument('--png', help='also keep the full-size composite here')
	ap.add_argument('--dither', type=int, default=0, help='triangular dither of +-N levels after the crop (smooth wallpapers; Afterglow uses 1)')
	ap.add_argument('--seed', type=int, default=DEFAULT_SEED)
	ap.add_argument('--rgb565', action='store_true', help='reduce to the 16-bit display levels with an ordered dither')
	a = ap.parse_args(argv)
	quillify(a.wallpaper, a.emblem, a.out, a.png, a.dither, a.seed, a.rgb565)
	x, y, r = sun_in_bmp()
	print('quillify: %s (sun at %.1f, %.1f radius %.1f)' % (a.out, x, y, r))


if __name__ == '__main__':
	main(sys.argv[1:])
