#!/usr/bin/env python3
# Writes site/img/social-card.png, the site's link preview: the icon, the name and a one-line description on the ink ground.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/make_social_card.py [SERIF_BOLD.ttf SANS.ttf SANS_BOLD.ttf]   (needs Pillow)
#
# The fonts only draw pixels into the PNG; no font file is shipped. The defaults are the Noto faces most Linux systems carry.
# Colours are D70's: ink #0a1230 to #16275a, parchment #fff4d6, amber #f0a830.

import os
import sys

from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
NOTO = "/usr/share/fonts/noto"
fonts = sys.argv[1:4] or [os.path.join(NOTO, f) for f in ("NotoSerif-Bold.ttf", "NotoSans-Regular.ttf", "NotoSans-Bold.ttf")]

W, H = 1280, 640
INK_A, INK_B = (0x0a, 0x12, 0x30), (0x16, 0x27, 0x5a)
PARCHMENT, MUTED, AMBER = (0xFF, 0xF4, 0xD6), (0xE9, 0xDC, 0xB0), (0xF0, 0xA8, 0x30)

card = Image.new("RGB", (W, H))
px = card.load()
for y in range(H):
	for x in range(W):
		t = min(1.0, (x / W) * 0.6 + (1 - y / H) * 0.4)
		px[x, y] = tuple(round(a + (b - a) * t) for a, b in zip(INK_A, INK_B))

icon = Image.open(os.path.join(ROOT, "assets", "icon", "quill-512.png")).convert("RGBA").resize((340, 340), Image.LANCZOS)
card.paste(icon, (110, (H - 340) // 2), icon)

draw = ImageDraw.Draw(card)
title = ImageFont.truetype(fonts[0], 150)
body = ImageFont.truetype(fonts[1], 40)
chip = ImageFont.truetype(fonts[2], 28)
x = 520
draw.text((x, 120), "Quill", font=title, fill=PARCHMENT)
draw.text((x, 330), "A small word processor in Rust", font=body, fill=MUTED)
draw.text((x, 384), "for DOS and Windows 95", font=body, fill=MUTED)
label = "Experimental"
l, t, r, b = draw.textbbox((0, 0), label, font=chip)
cx, cy = x, 470
draw.rounded_rectangle((cx, cy, cx + (r - l) + 36, cy + (b - t) + 22), radius=24, fill=AMBER)
draw.text((cx + 18 - l, cy + 11 - t), label, font=chip, fill=INK_A)

out = os.path.join(ROOT, "site", "img", "social-card.png")
card.save(out, optimize=True)
print(f"make_social_card: wrote {os.path.relpath(out, ROOT)}")
