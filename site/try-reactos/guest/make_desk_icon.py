#!/usr/bin/env python3
"""Build QUILLDESK.ICO, the icon of the ReactOS demo's desktop shortcuts, from the repository's icon sources at build time.

(c) Copyright 2026 Liminal HQ, Scott Morris
SPDX-License-Identifier: Apache-2.0 OR MIT

Usage: make_desk_icon.py OUT.ICO

Quill's own QUILL.ICO draws 16 to 32 px from the small-size art (a bolder ring and a plain nib) and only 48 px and up from the detailed master.
The demo desktop shows its shortcuts at 48 px (demo-settings.reg), which has room for the detail, so this icon takes the master for the 32 px,
48 px and 256 px frames and keeps the small-size art for 16 px (the Start menu and the taskbar). The frames are made by tools/make_icons.py's own
functions with only the choice of art changed, so they are quantized and written exactly as QUILL.ICO's are. The program's embedded icons are
not touched. Needs what tools/make_icons.py needs: Pillow, numpy and rsvg-convert. The output is reproducible.
"""
import importlib.util
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
MAKE_ICONS = os.path.join(HERE, '..', '..', '..', 'tools', 'make_icons.py')

# (size, bit depths). 16 px keeps the 4-bit image the 16-colour displays use; the larger frames are 8 (256-colour displays), 24 and 32 bits.
FRAMES = [(16, (4, 8, 24, 32)), (32, (8, 24, 32)), (48, (8, 24, 32))]


def load_make_icons():
	spec = importlib.util.spec_from_file_location('quill_make_icons', MAKE_ICONS)
	mod = importlib.util.module_from_spec(spec)
	spec.loader.exec_module(mod)
	return mod


def build():
	mi = load_make_icons()
	master = mi.ROOT / 'assets/icon/quill.svg'
	small = mi.ROOT / 'assets/icon/quill-small.svg'
	mi.art_for = lambda size: small if size <= 16 else master
	images = [mi.image_entry(size, bpp) for size, depths in FRAMES for bpp in depths]
	images.append((256, 256, 32, 0, mi.png_bytes(mi.render(master, 256))))
	return mi.ico(images)


def main(argv):
	if len(argv) != 1:
		sys.exit(__doc__.split('\n')[0] + '\nusage: make_desk_icon.py OUT.ICO')
	data = build()
	with open(argv[0], 'wb') as f:
		f.write(data)
	print('make-desk-icon: %s (%d bytes)' % (argv[0], len(data)))


if __name__ == '__main__':
	main(sys.argv[1:])
