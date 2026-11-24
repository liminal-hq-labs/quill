#!/usr/bin/env python3
# Builds the test fonts in ../fonts from Liberation 2.1.5: a Latin-1 and Windows-1252 subset, unhinted, renamed.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: make-test-fonts.py LIBERATION_DIR OUT_DIR   (needs fontTools: pip install fonttools)
#
# The Liberation fonts are under the SIL Open Font License 1.1 with the Reserved Font Names Liberation, Arimo, Tinos and Cousine.
# A subset is a Modified Version, so these files take the names Quill Test Serif, Quill Test Sans and Quill Test Mono, and the
# name table keeps the copyright and licence entries and drops the ones that name the reserved fonts.

import sys
from fontTools import subset
from fontTools.ttLib import TTFont

FAMILIES = {
	"LiberationSerif": ("Quill Test Serif", "QuillTestSerif"),
	"LiberationSans": ("Quill Test Sans", "QuillTestSans"),
	"LiberationMono": ("Quill Test Mono", "QuillTestMono"),
}
STYLES = ["Regular", "Bold", "Italic", "BoldItalic"]

# Windows-1252 from 0x20 to 0xFF, as Unicode.
HIGH = "€\x81‚ƒ„…†‡ˆ‰Š‹Œ\x8dŽ\x8f\x90‘’“”•–—˜™š›œ\x9džŸ"
CHARS = [chr(c) for c in range(0x20, 0x7F)] + [chr(0xA0 + i) for i in range(0x60)] + [c for c in HIGH if ord(c) > 0x9F]

KEEP_NAMES = {0, 1, 2, 4, 5, 6, 9, 13, 14, 16, 17}


def build(src, dst, family, ps_family, style):
	opts = subset.Options()
	opts.hinting = False
	opts.layout_features = []
	opts.name_IDs = list(KEEP_NAMES)
	opts.name_languages = [0x409]
	opts.notdef_outline = True
	opts.glyph_names = False
	opts.drop_tables += ["DSIG", "GDEF", "GPOS", "GSUB", "kern", "gasp", "FFTM", "meta"]
	font = subset.load_font(src, opts)
	sub = subset.Subsetter(opts)
	sub.populate(text="".join(CHARS))
	sub.subset(font)
	bold = "Bold" in style
	italic = "Italic" in style
	sub_family = {"Regular": "Regular", "Bold": "Bold", "Italic": "Italic", "BoldItalic": "Bold Italic"}[style]
	name = font["name"]
	name.names = [n for n in name.names if n.nameID in KEEP_NAMES and n.platformID == 3 and n.langID == 0x409]
	full = family if style == "Regular" else f"{family} {sub_family}"
	for nid, value in [(1, family), (2, sub_family), (4, full), (6, f"{ps_family}-{style}")]:
		name.setName(value, nid, 3, 1, 0x409)
	name.removeNames(nameID=16)
	name.removeNames(nameID=17)
	name.setName(f"Quill Test font, a renamed subset of Liberation 2.1.5 under the SIL Open Font License 1.1", 3, 3, 1, 0x409)
	font["OS/2"].usWeightClass = 700 if bold else 400
	font["head"].macStyle = (1 if bold else 0) | (2 if italic else 0)
	font.save(dst)


def main():
	src_dir, out_dir = sys.argv[1], sys.argv[2]
	for lib, (family, ps) in FAMILIES.items():
		for style in STYLES:
			build(f"{src_dir}/{lib}-{style}.ttf", f"{out_dir}/{ps}-{style}.ttf", family, ps, style)


main()
