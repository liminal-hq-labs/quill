#!/usr/bin/env python3
# Generates quill-text's Unicode tables from the Unicode Character Database 16.0.0: the Rust tables, UNITAB.BIN and the goldens the tests compare with.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

"""Run as `python3 -I scripts/gen-unicode.py` to rewrite the outputs, `--check` to fail when one is out of date, `--sizes` to print the size of each table.

The UCD files are read from ~/.cache/quill-ucd/16.0.0/ (or the folder in QUILL_UCD), after their SHA-256 is checked against
assets/unicode/ucd-16.0.0.sha256. `--record-hashes` writes that list from the folder's files (done once, when the version is chosen).
Download them from https://www.unicode.org/Public/16.0.0/ucd/ (the `auxiliary/` and `emoji/` files by name, the rest by path): see
assets/unicode/README.txt for the list.

Outputs:
  crates/quill-text/src/{cluster,breaks,word,fold,compose,width,script,blocks}.rs  the text between `// BEGIN GENERATED key` and `// END GENERATED key`
  assets/unicode/UNITAB.BIN                  every table in one blob (layout in crates/quill-text/src/unitab.rs), for a build that serves it from a resource
  crates/quill-text/tests/data/unicode.tsv   the goldens: every table once more, computed code point by code point from the UCD and Python's unicodedata
  crates/quill-text/tests/ucd/*.txt          GraphemeBreakTest.txt and LineBreakTest.txt, trimmed to the lines the tables answer exactly

Without the UCD files `--check` still checks that UNITAB.BIN holds exactly what the committed Rust tables hold, which is what CI runs.
"""

import hashlib
import os
import re
import struct
import sys
import unicodedata

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
VERSION = "16.0.0"
UCD = os.environ.get("QUILL_UCD") or os.path.expanduser("~/.cache/quill-ucd/" + VERSION)
HASHES = os.path.join(ROOT, "assets/unicode/ucd-16.0.0.sha256")
FILES = [
	"Blocks.txt",
	"CaseFolding.txt",
	"DerivedGeneralCategory.txt",
	"EastAsianWidth.txt",
	"GraphemeBreakProperty.txt",
	"GraphemeBreakTest.txt",
	"LineBreak.txt",
	"LineBreakTest.txt",
	"Scripts.txt",
	"UnicodeData.txt",
	"emoji-data.txt",
]
LAST = 0x10FFFF
TOP = LAST + 1

# Cluster classes (the values of `cluster::Class`).
C_OTHER, C_EXTEND, C_ZWJ, C_REGIONAL, C_PICT, C_CONTROL = range(6)
# Line-break classes (the values of `breaks::Brk`).
B_OTHER, B_SPACE, B_AFTER, B_IDEO, B_CLOSE, B_OPEN, B_GLUE = range(7)
# Word classes (the values of `word::Class`).
W_PUNCT, W_LETTER, W_SPACE, W_IDEO = range(4)
# Script flags.
S_SHAPING, S_INVISIBLE = 1, 2

LB_MAP = {"SP": B_SPACE, "ZW": B_SPACE}
for name in ("BA", "HY", "B2"):
	LB_MAP[name] = B_AFTER
for name in ("ID", "H2", "H3", "JL", "JV", "JT", "CJ"):
	LB_MAP[name] = B_IDEO
for name in ("CL", "CP", "NS", "EX", "IS"):
	LB_MAP[name] = B_CLOSE
LB_MAP["OP"] = B_OPEN
for name in ("GL", "WJ", "ZWJ"):
	LB_MAP[name] = B_GLUE

# The blocks CORE-226 sends to placeholders, by their Blocks.txt names, and the ranges Blocks.txt has no block for.
SHAPING_BLOCKS = [
	"Hebrew", "Arabic", "Syriac", "Arabic Supplement", "Thaana", "NKo", "Samaritan", "Mandaic", "Syriac Supplement",
	"Arabic Extended-B", "Arabic Extended-A", "Devanagari", "Bengali", "Gurmukhi", "Gujarati", "Oriya", "Tamil", "Telugu",
	"Kannada", "Malayalam", "Sinhala", "Thai", "Lao", "Tibetan", "Myanmar", "Khmer", "Mongolian", "Vedic Extensions",
	"Devanagari Extended", "Myanmar Extended-A", "Myanmar Extended-B", "Khmer Symbols", "Mongolian Supplement",
	"Arabic Presentation Forms-A", "Arabic Presentation Forms-B",
]
SHAPING_RANGES = [(0xFB1D, 0xFB4F)]  # Hebrew presentation forms, inside Alphabetic Presentation Forms with the Latin ligatures
BIDI_CONTROLS = [(0x061C, 0x061C), (0x200E, 0x200F), (0x202A, 0x202E), (0x2066, 0x2069)]
INVISIBLE = BIDI_CONTROLS + [
	(0x00AD, 0x00AD), (0x034F, 0x034F), (0x180E, 0x180E), (0x200B, 0x200D), (0x2060, 0x2064), (0x206A, 0x206F),
	(0xFE00, 0xFE0F), (0xFEFF, 0xFEFF), (0xE0001, 0xE0001), (0xE0020, 0xE007F), (0xE0100, 0xE01EF),
]

# The blocks a document is likely to hold: (Blocks.txt name, label). The label is the Blocks.txt name unless one is given.
BLOCKS = [
	("Latin-1 Supplement", None), ("Latin Extended-A", None), ("Latin Extended-B", None), ("IPA Extensions", None),
	("Spacing Modifier Letters", "Modifier Letters"), ("Combining Diacritical Marks", "Combining Marks"),
	("Greek and Coptic", "Greek"), ("Cyrillic", None), ("Cyrillic Supplement", None), ("Armenian", None), ("Hebrew", None),
	("Arabic", None), ("Syriac", None), ("Thaana", None), ("NKo", None), ("Devanagari", None), ("Bengali", None),
	("Gurmukhi", None), ("Gujarati", None), ("Oriya", None), ("Tamil", None), ("Telugu", None), ("Kannada", None),
	("Malayalam", None), ("Sinhala", None), ("Thai", None), ("Lao", None), ("Tibetan", None), ("Myanmar", None),
	("Georgian", None), ("Hangul Jamo", None), ("Ethiopic", None), ("Cherokee", None),
	("Unified Canadian Aboriginal Syllabics", "Canadian Syllabics"), ("Khmer", None), ("Mongolian", None),
	("Phonetic Extensions", None), ("Latin Extended Additional", None), ("Greek Extended", None),
	("General Punctuation", "Punctuation"), ("Superscripts and Subscripts", "Super and Subscripts"),
	("Currency Symbols", None), ("Letterlike Symbols", None), ("Number Forms", None), ("Arrows", None),
	("Mathematical Operators", "Mathematics"), ("Miscellaneous Technical", "Technical"), ("Enclosed Alphanumerics", None),
	("Box Drawing", None), ("Block Elements", None), ("Geometric Shapes", None), ("Miscellaneous Symbols", "Symbols"),
	("Dingbats", None), ("CJK Symbols and Punctuation", "CJK Punctuation"), ("Hiragana", None), ("Katakana", None),
	("Bopomofo", None), ("Hangul Compatibility Jamo", "Hangul Jamo (compat.)"),
	("CJK Unified Ideographs Extension A", "CJK Ideographs Ext. A"), ("CJK Unified Ideographs", "CJK Ideographs"),
	("Hangul Syllables", None), ("CJK Compatibility Ideographs", "CJK Compat. Ideographs"),
	("Alphabetic Presentation Forms", "Presentation Forms"), ("Arabic Presentation Forms-A", None),
	("Variation Selectors", None), ("Arabic Presentation Forms-B", None), ("Halfwidth and Fullwidth Forms", "Fullwidth Forms"),
	("Mahjong Tiles", None), ("Playing Cards", None), ("Enclosed Alphanumeric Supplement", "Enclosed Alphanumerics 2"),
	("Miscellaneous Symbols and Pictographs", "Pictographs"), ("Emoticons", None), ("Transport and Map Symbols", "Transport"),
	("Supplemental Symbols and Pictographs", "Pictographs 2"), ("Symbols and Pictographs Extended-A", "Pictographs 3"),
	("CJK Unified Ideographs Extension B", "CJK Ideographs Ext. B"), ("Tags", None),
]

# The 12 short names of invisible characters (CORE-228). Each is at most four letters.
SHORT_NAMES = [
	(0x00AD, "SHY"), (0x034F, "CGJ"), (0x061C, "ALM"), (0x200B, "ZWSP"), (0x200C, "ZWNJ"), (0x200D, "ZWJ"),
	(0x200E, "LRM"), (0x200F, "RLM"), (0x2060, "WJ"), (0xFE0E, "VS15"), (0xFE0F, "VS16"), (0xFEFF, "BOM"),
]

FOLD_SCRIPTS = {"Latin", "Greek", "Cyrillic", "Armenian", "Georgian", "Cherokee"}
MARKS = [0x0300, 0x0301, 0x0302, 0x0303, 0x0308, 0x030A, 0x030C, 0x0327]

# The lines of LineBreakTest.txt the break rule answers exactly are the lines made only of these classes.
LB_TEST_CLASSES = {"AL", "SP", "ZW", "BA", "HY", "ID", "GL", "H2", "H3"}


def fail(message):
	print("gen-unicode.py: " + message, file=sys.stderr)
	sys.exit(1)


def read_ucd(name):
	with open(os.path.join(UCD, name), encoding="utf-8") as f:
		return f.read()


def check_hashes():
	"""Checks the UCD files against the committed SHA-256 list."""
	expected = {}
	with open(HASHES, encoding="utf-8") as f:
		for line in f:
			if line.strip() and not line.startswith("#"):
				digest, name = line.split()
				expected[name] = digest
	for name in FILES:
		path = os.path.join(UCD, name)
		if not os.path.exists(path):
			fail("%s is missing from %s (see assets/unicode/README.txt)" % (name, UCD))
		with open(path, "rb") as f:
			actual = hashlib.sha256(f.read()).hexdigest()
		if expected.get(name) != actual:
			fail("%s does not match assets/unicode/ucd-16.0.0.sha256" % name)


def record_hashes():
	lines = ["# SHA-256 of the Unicode Character Database %s files that scripts/gen-unicode.py reads. Written by `gen-unicode.py --record-hashes`." % VERSION]
	for name in FILES:
		with open(os.path.join(UCD, name), "rb") as f:
			lines.append("%s  %s" % (hashlib.sha256(f.read()).hexdigest(), name))
	with open(HASHES, "w", encoding="utf-8") as f:
		f.write("\n".join(lines) + "\n")


def ranges_of(text, fields=2):
	"""The data lines of a UCD file as (first, last, [fields after the code points])."""
	out = []
	for line in text.splitlines():
		line = line.split("#")[0].strip()
		if not line:
			continue
		parts = [p.strip() for p in line.split(";")]
		first, _, last = parts[0].partition("..")
		out.append((int(first, 16), int(last or first, 16), parts[1:]))
	return out


def fill(table, text, convert):
	for first, last, parts in ranges_of(text):
		value = convert(parts)
		for cp in range(first, last + 1):
			table[cp] = value


def load_ucd():
	"""Reads every UCD property the tables use into per-code-point arrays."""
	data = {}
	general_category = ["Cn"] * TOP
	fill(general_category, read_ucd("DerivedGeneralCategory.txt"), lambda p: p[0])
	data["gc"] = general_category

	scripts = ["Unknown"] * TOP
	fill(scripts, read_ucd("Scripts.txt"), lambda p: p[0])
	data["script"] = scripts

	grapheme = ["Other"] * TOP
	fill(grapheme, read_ucd("GraphemeBreakProperty.txt"), lambda p: p[0])
	data["gbp"] = grapheme

	pictographic = [False] * TOP
	for first, last, parts in ranges_of(read_ucd("emoji-data.txt")):
		if parts[0] == "Extended_Pictographic":
			for cp in range(first, last + 1):
				pictographic[cp] = True
	data["pict"] = pictographic

	line_break = ["XX"] * TOP
	fill(line_break, read_ucd("LineBreak.txt"), lambda p: p[0])
	data["lb"] = line_break

	east_asian = ["N"] * TOP
	fill(east_asian, read_ucd("EastAsianWidth.txt"), lambda p: p[0])
	data["eaw"] = east_asian

	blocks = []
	for first, last, parts in ranges_of(read_ucd("Blocks.txt")):
		blocks.append((first, last, parts[0]))
	data["blocks"] = blocks

	folding = {}
	for line in read_ucd("CaseFolding.txt").splitlines():
		line = line.split("#")[0].strip()
		if not line:
			continue
		code, status, mapping = [p.strip() for p in line.split(";")[:3]]
		if status in ("C", "S"):
			folding[int(code, 16)] = int(mapping, 16)
	data["fold"] = folding

	decompositions = {}
	for line in read_ucd("UnicodeData.txt").splitlines():
		fields = line.split(";")
		decomposition = fields[5]
		if decomposition and not decomposition.startswith("<"):
			decompositions[int(fields[0], 16)] = [int(x, 16) for x in decomposition.split()]
	data["decomp"] = decompositions
	return data


def in_ranges(cp, ranges):
	return any(first <= cp <= last for first, last in ranges)


def shaping_ranges(data):
	out = list(SHAPING_RANGES) + BIDI_CONTROLS
	names = {name: (first, last) for first, last, name in data["blocks"]}
	for name in SHAPING_BLOCKS:
		if name not in names:
			fail("no block named %s in Blocks.txt" % name)
		out.append(names[name])
	return out


def runs(values):
	"""Merges per-code-point values into [(start, value)]. A value of None is a wildcard: it joins whichever run it is in."""
	out = []
	for cp, value in enumerate(values):
		if value is None:
			continue
		if not out or out[-1][1] != value:
			out.append((cp if out else 0, value))
	return out


def raw_runs(values):
	"""The same, as (first, last, value) with wildcards kept as their own runs, for the goldens."""
	out = []
	for cp, value in enumerate(values):
		if out and out[-1][2] == value:
			out[-1][1] = cp
		else:
			out.append([cp, cp, value])
	return out


def build_values(data):
	"""Per-code-point value arrays for every range table, computed straight from the UCD."""
	shaping = shaping_ranges(data)
	is_shaping = [False] * TOP
	for first, last in shaping:
		for cp in range(first, last + 1):
			is_shaping[cp] = True

	cluster = [C_OTHER] * TOP
	for cp in range(TOP):
		prop = data["gbp"][cp]
		if prop in ("Control", "CR", "LF"):
			cluster[cp] = C_CONTROL
		elif prop == "Extend":
			cluster[cp] = C_EXTEND
		elif prop == "ZWJ":
			cluster[cp] = C_ZWJ
		elif prop == "Regional_Indicator":
			cluster[cp] = C_REGIONAL
		elif prop == "SpacingMark" and not is_shaping[cp]:
			cluster[cp] = C_EXTEND
		elif data["pict"][cp]:
			cluster[cp] = C_PICT

	breaks = [LB_MAP.get(data["lb"][cp], B_OTHER) for cp in range(TOP)]

	word = [None] * TOP
	ideo_scripts = {"Han", "Hiragana", "Katakana"}
	for cp in range(TOP):
		category = data["gc"][cp]
		script = data["script"][cp]
		if category in ("Cn", "Co", "Cs"):
			word[cp] = None
		elif category in ("Zs", "Zl", "Zp") or cp in (0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x85):
			word[cp] = W_SPACE
		elif (script in ideo_scripts and (category[0] == "L" or category == "Nl")) or cp in (0x30FC, 0xFF70) or 0xAC00 <= cp <= 0xD7A3:
			word[cp] = W_IDEO
		elif category[0] in "LM" or category == "Nd" or cp in (0x200C, 0x200D):
			word[cp] = W_LETTER
		else:
			word[cp] = W_PUNCT

	width = [1 if data["eaw"][cp] in ("W", "F") else 0 for cp in range(TOP)]

	script = [0] * TOP
	for cp in range(TOP):
		if is_shaping[cp]:
			script[cp] |= S_SHAPING
	for first, last in INVISIBLE:
		for cp in range(first, last + 1):
			if data["gc"][cp] not in ("Cf", "Mn"):
				fail("U+%04X is not a format character or a variation selector" % cp)
			script[cp] |= S_INVISIBLE

	block = [0] * TOP
	block_ids = {}
	by_name = {name: (first, last) for first, last, name in data["blocks"]}
	for index, (name, _) in enumerate(BLOCKS):
		if name not in by_name:
			fail("no block named %s in Blocks.txt" % name)
		first, last = by_name[name]
		for cp in range(first, last + 1):
			block[cp] = index + 1
		block_ids[name] = index + 1
	return {"cluster": cluster, "breaks": breaks, "word": word, "width": width, "script": script, "blocks": block}, is_shaping


def pack(start, value):
	return (start << 8) | value


def range_words(values):
	return [pack(start, value) for start, value in runs(values)]


def fold_entries(data):
	"""Simple case folding for the scripts in scope, as (start, length, kind, delta) entries. Kind 0 adds the delta to every code point of the run; kind 1 does so to every other one, starting with the first."""
	mapping = {}
	for source, target in data["fold"].items():
		if data["script"][source] in FOLD_SCRIPTS or data["script"][target] in FOLD_SCRIPTS:
			mapping[source] = target
	entries = []
	cp = min(mapping)
	last_source = max(mapping)
	while cp <= last_source:
		if cp not in mapping:
			cp += 1
			continue
		delta = mapping[cp] - cp
		add_run = 1
		while add_run < 256 and mapping.get(cp + add_run) == cp + add_run + delta:
			add_run += 1
		alt_run = 1
		while alt_run < 128 and mapping.get(cp + 2 * alt_run) == cp + 2 * alt_run + delta and (cp + 2 * alt_run - 1) not in mapping:
			alt_run += 1
		if alt_run > add_run and alt_run > 1:
			entries.append((cp, 2 * alt_run - 1, 1, delta))
			cp += 2 * alt_run - 1
		else:
			entries.append((cp, add_run, 0, delta))
			cp += add_run
	return entries, mapping


def fold_words(entries):
	words = []
	for start, length, kind, delta in entries:
		words.append((start << 8) | (length - 1))
		words.append((kind << 24) | (delta & 0xFFFFFF))
	return words


def apply_fold(entries, cp):
	for start, length, kind, delta in entries:
		if start <= cp < start + length:
			if kind == 0 or (cp - start) % 2 == 0:
				return cp + delta
			return cp
	return cp


def golden_fold(data, mapping):
	"""The independent view of folding: Python's casefold, and the S line of CaseFolding.txt where the full folding is longer than one character."""
	golden = {}
	for cp in range(TOP):
		if 0xD800 <= cp <= 0xDFFF:
			continue
		if data["script"][cp] in FOLD_SCRIPTS or cp in mapping:
			folded = chr(cp).casefold()
			if len(folded) == 1:
				target = ord(folded)
			else:
				target = data["fold"].get(cp, cp)
			if target != cp and (data["script"][cp] in FOLD_SCRIPTS or cp in mapping):
				golden[cp] = target
	return golden


def compose_entries(data):
	"""The letters Windows-1252 has that are one base and one combining mark, from UnicodeData.txt's canonical decompositions."""
	entries = []
	for byte in range(0x80, 0x100):
		try:
			letter = ord(bytes([byte]).decode("cp1252"))
		except UnicodeDecodeError:
			continue
		parts = data["decomp"].get(letter)
		if parts and len(parts) == 2 and unicodedata.category(chr(letter))[0] == "L":
			base, mark = parts
			if base >= 0x80 or mark not in MARKS:
				fail("U+%04X decomposes to something compose.rs does not hold" % letter)
			entries.append((base, mark, letter))
	entries.sort()
	return entries


def compose_words(entries):
	return [(base << 24) | ((mark - 0x300) << 16) | letter for base, mark, letter in entries]


def golden_compose():
	"""Python's NFC of every ASCII base and combining mark of the table that is one Windows-1252 letter."""
	golden = []
	for base in range(0x80):
		for mark in MARKS:
			composed = unicodedata.normalize("NFC", chr(base) + chr(mark))
			if len(composed) == 1:
				try:
					composed.encode("cp1252")
				except UnicodeEncodeError:
					continue
				if composed.isalpha():
					golden.append((base, mark, ord(composed)))
	return golden


def names_pool():
	pool = bytearray()
	for _, label in BLOCKS_LABELS:
		encoded = label.encode("ascii")
		pool.append(len(encoded))
		pool += encoded
	return bytes(pool)


BLOCKS_LABELS = [(name, label or name) for name, label in BLOCKS]


def short_words():
	words = []
	for cp, name in SHORT_NAMES:
		words.append(cp)
		words.append(int.from_bytes(name.encode("ascii").ljust(4, b"\0"), "little"))
	return words


def build_tables(data=None):
	"""Every table as a list of 32-bit words (names as bytes), with the data the goldens need."""
	values, is_shaping = build_values(data)
	entries, mapping = fold_entries(data)
	for cp, target in mapping.items():
		if apply_fold(entries, cp) != target:
			fail("fold table disagrees with CaseFolding.txt at U+%04X" % cp)
	compose = compose_entries(data)
	tables = {
		"cluster": range_words(values["cluster"]),
		"breaks": range_words(values["breaks"]),
		"word": range_words(values["word"]),
		"fold": fold_words(entries),
		"compose": compose_words(compose),
		"width": range_words(values["width"]),
		"script": range_words(values["script"]),
		"blocks": range_words(values["blocks"]),
		"names": names_pool(),
		"short": short_words(),
	}
	return tables, {"values": values, "is_shaping": is_shaping, "fold_entries": entries, "fold_map": mapping, "compose": compose}


# ----- the python cross-checks, run whenever the UCD is present -----

def cross_check(data, values):
	"""Python's own unicodedata (also 16.0.0) must agree with the UCD files the tables are built from."""
	if unicodedata.unidata_version != VERSION:
		fail("this Python has Unicode %s data; %s is needed to cross-check (the tables themselves are built from the UCD files)" % (unicodedata.unidata_version, VERSION))
	for cp in range(TOP):
		category = unicodedata.category(chr(cp))
		if category != data["gc"][cp]:
			fail("U+%04X: unicodedata says %s, DerivedGeneralCategory.txt %s" % (cp, category, data["gc"][cp]))
		wide = unicodedata.east_asian_width(chr(cp)) in ("W", "F")
		if wide != bool(values["width"][cp]):
			fail("U+%04X: East Asian Width disagrees with unicodedata" % cp)
		if category in ("Mn", "Me") and values["cluster"][cp] != C_EXTEND:
			fail("U+%04X: a nonspacing mark that is not an extender" % cp)
		expected = values["word"][cp]
		if expected is not None and expected == W_LETTER and category[0] not in "LM" and category != "Nd" and cp not in (0x200C, 0x200D):
			fail("U+%04X: a letter that is not L, M or Nd" % cp)


# ----- the rule the tests check against the UCD's own test files -----

def can_break(a, b):
	"""The python twin of breaks::break_between."""
	if b in (B_SPACE, B_CLOSE):
		return False
	if b == B_AFTER:
		return a == B_SPACE
	if b == B_GLUE:
		return a in (B_SPACE, B_AFTER)
	if a in (B_GLUE, B_OPEN):
		return False
	if a in (B_SPACE, B_AFTER):
		return True
	return a == B_IDEO or b == B_IDEO


def joins(prev, nxt, regional_odd):
	"""The python twin of cluster::joins."""
	if prev == C_CONTROL or nxt == C_CONTROL:
		return False
	if nxt in (C_EXTEND, C_ZWJ):
		return True
	if prev == C_ZWJ and nxt == C_PICT:
		return True
	if prev == C_REGIONAL and nxt == C_REGIONAL:
		return regional_odd
	return False


def test_file_header(name):
	"""The comment lines that open a UCD test file, which carry its copyright and licence."""
	out = []
	for line in read_ucd(name).splitlines():
		if not line.startswith("#"):
			break
		out.append(line)
	return out


def test_lines(name):
	"""(code points, expected breaks) of each line, where the break before the first and after the last are dropped."""
	out = []
	for line in read_ucd(name).splitlines():
		body = line.split("#")[0].split()
		if not body:
			continue
		cps = [int(x, 16) for x in body if x not in ("×", "÷")]
		marks = [x == "÷" for x in body if x in ("×", "÷")]
		out.append((cps, marks[1:-1]))
	return out


def trimmed_grapheme_test(data, values, is_shaping):
	reasons = {}
	kept = []

	def drop(reason):
		reasons[reason] = reasons.get(reason, 0) + 1

	lines = test_lines("GraphemeBreakTest.txt")
	for cps, expected in lines:
		props = [data["gbp"][cp] for cp in cps]
		if any(p in ("L", "V", "T", "LV", "LVT") for p in props):
			drop("Hangul jamo and syllable classes (L, V, T, LV, LVT)")
			continue
		if any(p == "Prepend" for p in props):
			drop("Prepend")
			continue
		if any(is_shaping[cp] for cp in cps):
			drop("a character of a script that needs shaping (Indic conjuncts, Thai and Lao vowels, Khmer)")
			continue
		if any(a == 0x0D and b == 0x0A for a, b in zip(cps, cps[1:])):
			drop("CR followed by LF (one cluster in UAX #29; Quill keeps the paragraph mark one character)")
			continue
		classes = [values["cluster"][cp] for cp in cps]
		actual = []
		regional = 0
		for index in range(1, len(cps)):
			regional = regional + 1 if classes[index - 1] == C_REGIONAL else 0
			actual.append(not joins(classes[index - 1], classes[index], regional % 2 == 1))
		if actual != expected:
			if any(classes[index - 1] == C_ZWJ and classes[index] == C_PICT for index in range(1, len(cps))):
				drop("ZWJ before a pictographic that is not after a pictographic base (UAX #29 rule GB11 looks behind the ZWJ; the pair rule does not)")
				continue
			fail("cluster rule disagrees with GraphemeBreakTest.txt on %s" % " ".join("%04X" % cp for cp in cps))
		kept.append(cps)
	return lines, kept, reasons


def trimmed_line_break_test(data, values):
	reasons = {}
	kept = []

	def drop(reason):
		reasons[reason] = reasons.get(reason, 0) + 1

	lines = test_lines("LineBreakTest.txt")
	for cps, expected in lines:
		classes = [data["lb"][cp] for cp in cps]
		outside = [c for c in classes if c not in LB_TEST_CLASSES]
		if outside:
			drop("the line's first class outside the subset is %s, which the pair rule does not answer exactly" % outside[0])
			continue
		ours = [values["breaks"][cp] for cp in cps]
		actual = [can_break(ours[index - 1], ours[index]) for index in range(1, len(cps))]
		if actual != expected:
			hyphen = [index for index in range(len(cps) - 1) if classes[index] in ("BA", "HY") and classes[index + 1] == "AL" and (index == 0 or classes[index - 1] in ("SP", "ZW", "GL"))]
			if hyphen:
				drop("LB20a: no break after a word-initial hyphen")
				continue
			fail("break rule disagrees with LineBreakTest.txt on %s" % " ".join("%04X" % cp for cp in cps))
		kept.append((cps, expected))
	return lines, kept, reasons


# ----- rendering -----

def spell(words, per_line, maker):
	lines = []
	for index in range(0, len(words), per_line):
		lines.append("\t" + " ".join(maker(w) + "," for w in words[index : index + per_line]))
	return "\n".join(lines)


def render_ranges(words, doc):
	body = spell(words, 5, lambda w: "r(0x%04X, %d)" % (w >> 8, w & 0xFF))
	return "/// %s\n#[rustfmt::skip]\npub(crate) const RANGES: [u32; %d] = [\n%s\n];\n" % (doc, len(words), body)


def render_fold(words):
	entries = []
	for index in range(0, len(words), 2):
		w0, w1 = words[index], words[index + 1]
		delta = w1 & 0xFFFFFF
		if delta >= 0x800000:
			delta -= 0x1000000
		entries.append((w0 >> 8, (w0 & 0xFF) + 1, w1 >> 24, delta))
	body = spell(entries, 2, lambda e: "r(0x%04X, %d), d(%d, %d)" % (e[0], e[1] - 1, e[2], e[3]))
	return "/// Runs of simple case folding, two words each: where it starts, how long it is, its kind and the delta.\n#[rustfmt::skip]\npub(crate) const RUNS: [u32; %d] = [\n%s\n];\n" % (len(words), body)


def render_compose(words):
	body = spell(words, 4, lambda w: "m(0x%02X, 0x%04X, 0x%04X)" % (w >> 24, 0x300 + ((w >> 16) & 0xFF), w & 0xFFFF))
	return "/// The letters, sorted by base then mark: one word each.\n#[rustfmt::skip]\npub(crate) const LETTERS: [u32; %d] = [\n%s\n];\n" % (len(words), body)


def render_names(pool):
	text = []
	index = 0
	while index < len(pool):
		length = pool[index]
		text.append('\t"\\x%02x%s",' % (length, pool[index + 1 : index + 1 + length].decode("ascii")))
		index += 1 + length
	return "/// Each label after the first is a length byte and its letters; the first block is id 1.\npub(crate) const NAMES: &[u8] = concat!(\n%s\n)\n.as_bytes();\n" % "\n".join(text)


def render_short(words):
	lines = []
	for index in range(0, len(words), 2):
		name = words[index + 1].to_bytes(4, "little").rstrip(b"\0").decode("ascii")
		lines.append("\t0x%04X, 0x%08X, // %s" % (words[index], words[index + 1], name))
	return "/// The short names, sorted by code point: two words each, the code point and four letters.\n#[rustfmt::skip]\npub(crate) const SHORT: [u32; %d] = [\n%s\n];\n" % (len(words), "\n".join(lines))


def renderings(tables):
	"""(file, key) to the Rust text of the table."""
	return {
		("cluster", "RANGES"): render_ranges(tables["cluster"], "Start of each run of code points with one cluster class, and the class (`Class`)."),
		("breaks", "RANGES"): render_ranges(tables["breaks"], "Start of each run of code points with one line-break class, and the class (`Brk`)."),
		("word", "RANGES"): render_ranges(tables["word"], "Start of each run of code points with one word class, and the class (`Class`). Unassigned, private-use and surrogate code points take the run they are in."),
		("fold", "RUNS"): render_fold(tables["fold"]),
		("compose", "LETTERS"): render_compose(tables["compose"]),
		("width", "RANGES"): render_ranges(tables["width"], "Start of each run of code points that are wide (1) or not (0)."),
		("script", "RANGES"): render_ranges(tables["script"], "Start of each run of code points with one set of flags: bit 0 needs shaping or bidi, bit 1 is invisible."),
		("blocks", "RANGES"): render_ranges(tables["blocks"], "Start of each run of code points in one block, and the block's id (0 for any other)."),
		("blocks", "NAMES"): render_names(tables["names"]),
		("blocks", "SHORT"): render_short(tables["short"]),
	}


def splice(text, key, body):
	begin = "// BEGIN GENERATED %s (scripts/gen-unicode.py)\n" % key
	end = "// END GENERATED %s\n" % key
	start = text.find(begin)
	stop = text.find(end)
	if start < 0 or stop < 0:
		fail("a BEGIN or END marker for %s is missing" % key)
	return text[: start + len(begin)] + body + text[stop:]


def region(text, key):
	begin = "// BEGIN GENERATED %s (scripts/gen-unicode.py)\n" % key
	end = "// END GENERATED %s\n" % key
	start = text.find(begin)
	stop = text.find(end)
	if start < 0 or stop < 0:
		fail("a BEGIN or END marker for %s is missing" % key)
	return text[start + len(begin) : stop]


# ----- reading the committed Rust back (the check CI can run without the UCD) -----

def parse_rust(tables_dir):
	"""The tables as the committed Rust source holds them."""
	def source(name):
		with open(os.path.join(ROOT, "crates/quill-text/src", name + ".rs"), encoding="utf-8") as f:
			return f.read()

	tables = {}
	for name in ("cluster", "breaks", "word", "width", "script", "blocks"):
		body = region(source(name), "RANGES")
		tables[name] = [(int(a, 16) << 8) | int(b) for a, b in re.findall(r"r\(0x([0-9A-F]+), (\d+)\)", body)]
	words = []
	for a, b, c, d in re.findall(r"r\(0x([0-9A-F]+), (\d+)\), d\((\d+), (-?\d+)\)", region(source("fold"), "RUNS")):
		words.append((int(a, 16) << 8) | int(b))
		words.append((int(c) << 24) | (int(d) & 0xFFFFFF))
	tables["fold"] = words
	tables["compose"] = [(int(a, 16) << 24) | ((int(b, 16) - 0x300) << 16) | int(c, 16) for a, b, c in re.findall(r"m\(0x([0-9A-F]+), 0x([0-9A-F]+), 0x([0-9A-F]+)\)", region(source("compose"), "LETTERS"))]
	pool = bytearray()
	for literal in re.findall(r'"((?:\\x[0-9a-f]{2}|[^"\\])*)"', region(source("blocks"), "NAMES")):
		index = 0
		while index < len(literal):
			if literal.startswith("\\x", index):
				pool.append(int(literal[index + 2 : index + 4], 16))
				index += 4
			else:
				pool += literal[index].encode("ascii")
				index += 1
	tables["names"] = bytes(pool)
	short = []
	for cp, word in re.findall(r"0x([0-9A-F]+), 0x([0-9A-F]{8}), // [A-Z0-9]+", region(source("blocks"), "SHORT")):
		short.append(int(cp, 16))
		short.append(int(word, 16))
	tables["short"] = short
	return tables


# ----- the blob -----

ORDER = ["cluster", "breaks", "word", "fold", "compose", "width", "script", "blocks", "names", "short"]


def blob(tables):
	sections = []
	for name in ORDER:
		value = tables[name]
		sections.append(value if isinstance(value, bytes) else b"".join(struct.pack("<I", w) for w in value))
	header_size = 8 + 4 * len(ORDER)
	out = bytearray(b"LIUT" + struct.pack("<HH", 1, len(ORDER)))
	offset = header_size
	for section in sections:
		out += struct.pack("<HH", offset, len(section))
		offset += (len(section) + 3) & ~3
	for section in sections:
		out += section
		out += b"\0" * (-len(section) % 4)
	if len(out) > 0xFFFF:
		fail("UNITAB.BIN does not fit its 16-bit offsets")
	return bytes(out)


def golden(data, tables, extra):
	values = extra["values"]
	lines = ["# Goldens for crates/quill-text/tests/unicode.rs, computed code point by code point from the UCD %s files and Python's unicodedata. Generated by scripts/gen-unicode.py." % VERSION,
		"# [name] then `first<TAB>last<TAB>value` (hex code points; a value of x is a code point the table may answer anything for), or for [fold] `code point<TAB>folded` and for [compose] `base<TAB>mark<TAB>letter`."]
	for name in ("cluster", "breaks", "word", "width", "script", "blocks"):
		lines.append("[%s]" % name)
		for first, last, value in raw_runs(values[name]):
			lines.append("%04X\t%04X\t%s" % (first, last, "x" if value is None else value))
	lines.append("[fold]")
	for cp, target in sorted(golden_fold(data, extra["fold_map"]).items()):
		lines.append("%04X\t%04X" % (cp, target))
	lines.append("[compose]")
	for base, mark, letter in golden_compose():
		lines.append("%02X\t%04X\t%04X" % (base, mark, letter))
	return ("\n".join(lines) + "\n").encode()


def outputs_from_ucd():
	check_hashes()
	data = load_ucd()
	tables, extra = build_tables(data)
	cross_check(data, extra["values"])
	files = {}
	for (module, key), body in renderings(tables).items():
		path = os.path.join(ROOT, "crates/quill-text/src", module + ".rs")
		files.setdefault(path, open(path, encoding="utf-8").read())
		files[path] = splice(files[path], key, body)
	byte_files = {}
	for path, text in files.items():
		byte_files[path] = text.encode()
	byte_files[os.path.join(ROOT, "assets/unicode/UNITAB.BIN")] = blob(tables)
	byte_files[os.path.join(ROOT, "crates/quill-text/tests/data/unicode.tsv")] = golden(data, tables, extra)
	for name, grapheme in (("GraphemeBreakTest.txt", True), ("LineBreakTest.txt", False)):
		if grapheme:
			lines, kept, reasons = trimmed_grapheme_test(data, extra["values"], extra["is_shaping"])
		else:
			lines, kept, reasons = trimmed_line_break_test(data, extra["values"])
		# The trimmed file keeps the original line form: `× 0020 ÷ 0308 ×`.
		out = list(test_file_header(name))
		out.append("#")
		out.append("# Trimmed by scripts/gen-unicode.py to the %d of %d lines whose answer the quill-text table gives exactly; the comments are dropped." % (len(kept), len(lines)))
		for reason in sorted(reasons, key=lambda r: (-reasons[r], r)):
			out.append("# excluded %d: %s" % (reasons[reason], reason))
		originals = original_lines(name)
		for cps in [k if grapheme else k[0] for k in kept]:
			out.append(originals[tuple(cps)])
		byte_files[os.path.join(ROOT, "crates/quill-text/tests/ucd", name)] = ("\n".join(out) + "\n").encode()
	return byte_files, tables


def original_lines(name):
	"""Each test line's data (before the comment) by its code points. Lines with the same code points but other breaks are not in the files."""
	out = {}
	for line in read_ucd(name).splitlines():
		body = line.split("#")[0].strip()
		if not body:
			continue
		cps = tuple(int(x, 16) for x in body.split() if x not in ("×", "÷"))
		out.setdefault(cps, body)
	return out


def sizes(tables):
	print("table      bytes")
	total = 0
	for name in ORDER:
		value = tables[name]
		size = len(value) if isinstance(value, bytes) else 4 * len(value)
		total += size
		print("%-9s %6d" % (name, size))
	print("%-9s %6d" % ("tables", total))
	print("%-9s %6d" % ("UNITAB.BIN", len(blob(tables))))


def main():
	args = sys.argv[1:]
	if "--record-hashes" in args:
		record_hashes()
		return
	checking = "--check" in args
	cache_present = os.path.isdir(UCD) and all(os.path.exists(os.path.join(UCD, name)) for name in FILES)
	if not cache_present:
		if not checking:
			fail("the UCD files are not in %s (see assets/unicode/README.txt)" % UCD)
		tables = parse_rust(None)
		path = os.path.join(ROOT, "assets/unicode/UNITAB.BIN")
		if not os.path.exists(path) or open(path, "rb").read() != blob(tables):
			print("out of date: assets/unicode/UNITAB.BIN does not hold the committed tables")
			sys.exit(1)
		print("gen-unicode.py: UCD files not in %s; checked UNITAB.BIN against the committed tables only" % UCD)
		return
	files, tables = outputs_from_ucd()
	if "--sizes" in args:
		sizes(tables)
		return
	bad = False
	for path, data in files.items():
		if checking:
			if not os.path.exists(path) or open(path, "rb").read() != data:
				print("out of date:", os.path.relpath(path, ROOT))
				bad = True
		else:
			os.makedirs(os.path.dirname(path), exist_ok=True)
			with open(path, "wb") as f:
				f.write(data)
	if bad:
		sys.exit(1)


if __name__ == "__main__":
	main()
