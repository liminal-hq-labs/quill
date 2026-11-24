# Builds the zoomed checkerboard comparison image from the panel crops that analyse.py writes.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
"""Usage: sheet.py CROP_PREFIX SOURCE.png OUT.png NAME..."""
import sys
from PIL import Image, ImageDraw

prefix, source, out, names = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
W, H, Z = 36, 18, 8
cells = [("source", Image.open(source).convert("RGB"))] + [(n, Image.open(f"{prefix}-{n}.png").convert("RGB")) for n in names]
cols = 3
rows = (len(cells) + cols - 1) // cols
sheet = Image.new("RGB", (cols * (W * Z + 8) + 8, rows * (H * Z + 24) + 8), (30, 30, 36))
draw = ImageDraw.Draw(sheet)
for i, (name, im) in enumerate(cells):
	x, y = 8 + (i % cols) * (W * Z + 8), 8 + (i // cols) * (H * Z + 24)
	draw.text((x, y), name, fill=(235, 235, 235))
	sheet.paste(im.crop((0, 0, W, H)).resize((W * Z, H * Z), Image.NEAREST), (x, y + 14))
sheet.save(out, optimize=True)
