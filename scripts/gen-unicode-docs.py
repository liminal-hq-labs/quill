#!/usr/bin/env python3
# Writes the Unicode test document set in tests/docs/unicode/ (WP-UNI-2, CORE-225).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

"""Generates every file of the set that can be generated, and the reference files beside each document.

    python3 -I scripts/gen-unicode-docs.py          # write tests/docs/unicode/
    python3 -I scripts/gen-unicode-docs.py --check  # fail when a committed file differs from a regeneration

Per document `NAME.EXT` the script writes:

- `NAME.EXT.expect`: the code points of the body as the format defines them, one `U+XXXX` a line. A paragraph mark is U+000D.
- `NAME.EXT.today`: what today's readers give: `refused=N`, then the code points of the Windows-1252 bytes the document holds.
  A package that opens a door flips the file's `.today` to its `.expect` (and removes `flips` from its manifest entry).
- `NAME.EXT.clusters`: for each paragraph, the length in code points of each cluster (CORE-212).
- `NAME.EXT.breaks`: for each paragraph, the offsets in code points where a line may break (CORE-215).

`manifest.toml` lists every document with its source, what it checks and what each build is expected to show (CORE-213). The cluster and
break rules here are a reference written from the plan's rules with `unicodedata` (UCD 16.0.0); WP-UNI-1's tables are checked against the
UCD's own test files, and WP-UNI-3 compares them with these files.

The hand-written documents (`fr-ca.rtf`, `ru-word97-upr.rtf`) are kept as they are; the script holds their expected text.
"""

import os
import struct
import sys
import unicodedata

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
OUT = os.path.join(ROOT, "tests", "docs", "unicode")
HEADER = (
	"# {summary}\n#\n# (c) Copyright 2026 Liminal HQ, Scott Morris\n# SPDX-License-Identifier: Apache-2.0 OR MIT\n"
)

CR = 0x0D


# ---------------------------------------------------------------- code pages


def cp1252_char(b):
	"""The character of a Windows-1252 byte; the five undefined bytes keep their number, as `quill_text::cp1252` does."""
	try:
		return bytes([b]).decode("cp1252")
	except UnicodeDecodeError:
		return chr(b)


CP1252_CHARS = {cp1252_char(b): b for b in range(0x20, 0x100) if b != 0x7F}


def narrow(cps):
	"""What a reader that keeps only Windows-1252 makes of `cps`: the paragraph mark and tab stay, the rest is `?`.

	Returns the code points and the number refused.
	"""
	out, refused = [], 0
	for cp in cps:
		c = chr(cp)
		if cp in (CR, 9) or c in CP1252_CHARS:
			out.append(cp)
		else:
			out.append(ord("?"))
			refused += 1
	return out, refused


def cps_of(text):
	return [ord(c) for c in text]


# ---------------------------------------------------------------- expectations


def paragraphs(cps):
	"""The code points split after each paragraph mark; the last one always ends with it."""
	out, cur = [], []
	for cp in cps:
		cur.append(cp)
		if cp == CR:
			out.append(cur)
			cur = []
	if cur:
		out.append(cur)
	return out


EMOJI_RANGES = [
	(0x00A9, 0x00A9), (0x00AE, 0x00AE), (0x203C, 0x203C), (0x2049, 0x2049), (0x2122, 0x2122), (0x2139, 0x2139),
	(0x2194, 0x21AA), (0x231A, 0x23FF), (0x24C2, 0x24C2), (0x25AA, 0x25FE), (0x2600, 0x27BF), (0x2934, 0x2935),
	(0x2B05, 0x2B55), (0x3030, 0x3030), (0x303D, 0x303D), (0x3297, 0x3299), (0x1F000, 0x1FAFF),
]  # fmt: skip


def pictographic(cp):
	return any(lo <= cp <= hi for lo, hi in EMOJI_RANGES)


def regional(cp):
	return 0x1F1E6 <= cp <= 0x1F1FF


def extend(cp):
	"""Joins the character before it: marks, ZWNJ, variation selectors, emoji modifiers, tags (CORE-212)."""
	if cp in (CR, 0x200D):
		return False
	if 0xFE00 <= cp <= 0xFE0F or 0xE0100 <= cp <= 0xE01EF or 0x1F3FB <= cp <= 0x1F3FF or 0xE0020 <= cp <= 0xE007F:
		return True
	return cp == 0x200C or unicodedata.category(chr(cp)) in ("Mn", "Me", "Mc")


def clusters(para):
	"""The clusters of a paragraph as lists of code points."""
	out, i = [], 0
	while i < len(para):
		cp = para[i]
		cur = [cp]
		i += 1
		if cp != CR:
			if regional(cp) and i < len(para) and regional(para[i]):
				cur.append(para[i])
				i += 1
			while i < len(para):
				nxt = para[i]
				if extend(nxt):
					cur.append(nxt)
					i += 1
				elif nxt == 0x200D:
					cur.append(nxt)
					i += 1
					if i < len(para) and pictographic(para[i]) and pictographic(cur[0]):
						cur.append(para[i])
						i += 1
				else:
					break
		out.append(cur)
	return out


SPACES = {0x20, 0x1680, *range(0x2000, 0x2007), *range(0x2008, 0x200B), 0x205F, 0x3000}
NO_BREAK = {0x00A0, 0x202F, 0x2007, 0x2060, 0xFEFF}
HYPHENS = {0x2D, 0x2010, 0x00AD, 0x2012, 0x2013, 0x2014, 0x2E3A, 0x2E3B}
CLOSING = set(cps_of("、。，．・：；？！）」』】〕〉》")) | {0xFF09, 0xFF0C, 0xFF0E, 0xFF1A, 0xFF1B, 0xFF1F, 0xFF01, 0xFF61, 0xFF63, 0xFF64, 0xFF65}
OPENING = set(cps_of("（「『【〔〈《")) | {0xFF08, 0xFF62}


def ideographic(cp):
	return (
		0x3400 <= cp <= 0x4DBF or 0x4E00 <= cp <= 0x9FFF or 0xF900 <= cp <= 0xFAFF or 0x20000 <= cp <= 0x2FA1F
		or 0x3040 <= cp <= 0x30FF or 0x31F0 <= cp <= 0x31FF or 0xAC00 <= cp <= 0xD7AF
	)  # fmt: skip


def cjk(cp):
	return ideographic(cp) or cp in CLOSING or cp in OPENING or 0x3001 <= cp <= 0x303F or 0xFF00 <= cp <= 0xFFEF


def break_after(a, b):
	"""Whether a line may break between a cluster whose base is `a` and the next, whose base is `b` (CORE-215)."""
	if a in NO_BREAK or b in NO_BREAK or b == CR:
		return False
	if a in SPACES or a == 0x200B or a in HYPHENS:
		return b not in SPACES
	if (cjk(a) or cjk(b)) and b not in CLOSING and a not in OPENING:
		return True
	return False


def break_offsets(para):
	"""Offsets in code points, from the paragraph's start, where a line may break (never before the paragraph mark)."""
	out, at, cl = [], 0, clusters(para)
	for i, c in enumerate(cl[:-1]):
		at += len(c)
		if break_after(c[0], cl[i + 1][0]):
			out.append(at)
	return out


# ---------------------------------------------------------------- what each build shows

# CORE-226: right-to-left and complex scripts are placeholders on every build in v1.
SHAPING = [
	(0x0590, 0x08FF), (0x0900, 0x0DFF), (0x0E00, 0x0FFF), (0x1000, 0x109F), (0x1780, 0x17FF), (0x1800, 0x18AF),
	(0xFB1D, 0xFDFF), (0xFE70, 0xFEFE),
]  # fmt: skip
INVISIBLE = {0x200B, 0x200C, 0x200D, 0x200E, 0x200F, 0x2060, 0xFEFF, 0x061C, *range(0x202A, 0x202F), *range(0x2066, 0x206A)}
INVISIBLE |= set(range(0xFE00, 0xFE10)) | set(range(0xE0100, 0xE01F0))
BUILDS = ["dos_text", "dos_gfx", "win16", "win32", "tty", "gtk"]


def needs_shaping(cp):
	return any(lo <= cp <= hi for lo, hi in SHAPING)


def wide(cp):
	return unicodedata.east_asian_width(chr(cp)) in ("W", "F")


def show(build, cluster):
	"""`shown`, `composed`, `placeholder` or `invisible` for one cluster (CORE-213, CORE-214, CORE-226, CORE-227)."""
	if all(cp in INVISIBLE for cp in cluster):
		return "invisible"
	chars = "".join(chr(cp) for cp in cluster if cp not in INVISIBLE or len(cluster) == 1)
	single = len(chars) == 1
	if build == "dos_text":
		return "shown" if single and ord(chars) >= 0x20 and chars in CP437_CHARS else "placeholder"
	if build in ("dos_gfx", "win16", "win32"):
		if single:
			return "shown" if chars in CP1252_CHARS else "placeholder"
		composed = unicodedata.normalize("NFC", chars)
		return "composed" if len(composed) == 1 and composed in CP1252_CHARS else "placeholder"
	if any(needs_shaping(cp) for cp in cluster):
		return "placeholder"
	if build == "tty":
		single_cell = single and not wide(ord(chars)) and unicodedata.category(chars)[0] != "M" and not pictographic(ord(chars))
		return "shown" if single_cell and chars not in "\ufffc" else "placeholder"
	# GTK: a base with marks is drawn by the font; an emoji sequence is one box; a single emoji is drawn where a font has it.
	if len(cluster) > 1 and (pictographic(cluster[0]) or regional(cluster[0]) or cluster[0] in range(0x30, 0x3A)):
		return "placeholder"
	return "placeholder" if cluster[0] == 0xFFFC else "shown"


CP437_CHARS = {bytes([b]).decode("cp437"): b for b in range(0x20, 0x100) if b != 0x7F}


def display_counts(cps):
	out = {}
	for build in BUILDS:
		counts = {"shown": 0, "composed": 0, "placeholder": 0, "invisible": 0}
		for para in paragraphs(cps):
			for cl in clusters(para):
				if cl == [CR] or cl == [9]:
					continue
				counts[show(build, cl)] += 1
		out[build] = counts
	return out


# ---------------------------------------------------------------- the documents


def rtf_doc(text, cpg=1252, charset=0, fonts="Arial", body_mode="byte", uc=1):
	"""An RTF document of `text` (paragraphs end with a newline).

	`body_mode` is `byte` (characters of the code page as `\\'hh`, the rest as `\\uN?`) or `u` (everything above ASCII as `\\uN?`).
	"""
	codec = "cp%d" % cpg
	head = "{\\rtf1\\ansi\\ansicpg%d\\uc%d\\deff0{\\fonttbl{\\f0\\fnil\\fcharset%d %s;}}\n\\f0\\fs24 " % (cpg, uc, charset, fonts)
	body = []
	for ch in text:
		if ch == "\n":
			body.append("\\par\n")
		elif ch in "\\{}":
			body.append("\\" + ch)
		elif ord(ch) < 0x80:
			body.append(ch)
		else:
			try:
				if body_mode == "u":
					raise UnicodeError
				body.append("\\'%02x" % ch.encode(codec)[0])
			except (UnicodeError, IndexError):
				for unit in utf16_units(ch):
					signed = unit - 0x10000 if unit >= 0x8000 else unit
					body.append("\\u%d?" % signed)
	return (head + "".join(body) + "}\n").encode("ascii")


def utf16_units(ch):
	b = ch.encode("utf-16-be")
	return [(b[i] << 8) | b[i + 1] for i in range(0, len(b), 2)]


def text_cps(text):
	"""The code points the document holds for text read from a file: CRLF, LF, CR and form feed end a paragraph."""
	out, prev_cr = [], False
	for ch in text:
		cp = ord(ch)
		if cp == 0x0A and prev_cr:
			prev_cr = False
			continue
		prev_cr = cp == CR
		if cp in (0x0A, 0x0C, CR):
			out.append(CR)
		elif cp == 9 or cp >= 0x20:
			out.append(cp)
	if not out or out[-1] != CR:
		out.append(CR)
	return out


def today_text(data):
	"""What `text::read` makes of `data` today: every byte from 0x20 up is a Windows-1252 character, controls go."""
	out, prev_cr = [], False
	for b in data:
		if b == 0x0A and prev_cr:
			prev_cr = False
			continue
		if b in (0x0A, 0x0C, CR):
			out.append(CR)
			prev_cr = b == CR
			continue
		prev_cr = False
		if b == 9 or b >= 0x20:
			out.append(ord(cp1252_char(b)))
	if not out or out[-1] != CR:
		out.append(CR)
	return out, 0


# Source texts. Each paragraph is a line; the writers add the paragraph marks.
FR_CA = (
	"« Il était une fois… » — dit-elle, « c’est là qu’on s’est rencontrés. »\n"
	"Ça coûte 12 $ – ou 15 $ ; l’œuvre de Hélène est très prisée à Québec.\n"
)
PL = "Zażółć gęślą jaźń. Příliš žluťoučký kůň úpěl ďábelské ódy.\nŁódź, Wrocław, Śląsk.\n"
RU = "Привет, мир! Съешь же ещё этих мягких французских булок, да выпей чаю.\nЖук и ёж.\n"
EL = "Καλημέρα κόσμε. Ξεσκεπάζω την ψυχοφθόρα βδελυγμία.\nΑΒΓ αβγ ΩΏ ωώ.\n"
TR = "Pijamalı hasta yağız şoföre çabucak güvendi. İstanbul, ışık, ğüşiöç.\n"
LT = "Įlinkdama fechtuotojo špaga sublykčiojusi pragręžė apvalų arbatą.\nĄČĘĖĮŠŲŪŽ ąčęėįšųūž.\n"
HY_KA = (
	"Հայերեն. Բարեւ աշխարհ։ Գրիչ, թուղթ, մատիտ։\n"
	"ქართული. გამარჯობა მსოფლიო. ხვადია ქართული ენა.\n"
	"Ա Բ Գ ա բ գ ა ბ გ Ⴀ Ⴁ Ⴂ.\n"
)
VI_NFC = "Tiếng Việt: Chào bạn, đây là một đoạn văn. Người Việt Nam ở Hà Nội.\nƯu tiên, thương yêu, hỏi han.\n"
ZH_JA_KO = (
	"你好，世界。这是一段中文。\n"
	"こんにちは、世界。カタカナとひらがなと漢字。\n"
	"안녕하세요, 세계! 한글 음절입니다.\n"
	"「括弧」、『二重』（丸）【墨付き】。𠀀𠀁 Extension B.\n"
	"ＦＵＬＬＷＩＤＴＨ ｱｲｳ halfwidth.\n"
)
SYMBOLS = (
	"Arrows: ← ↑ → ↓ ↔ ⇒ ⇔ ↩\n"
	"Maths: ∀ ∃ ∅ ∈ ∑ √ ∞ ∫ ≠ ≈ ≤ ≥ ± × ÷ ∴\n"
	"Box drawing: ┌─┬─┐ │ │ │ ├─┼─┤ └─┴─┘ ╔═╗ ║ ║ ╚═╝\n"
	"Dingbats: ✓ ✔ ✗ ✘ ✂ ✈ ✉ ★ ☆ ♠ ♥ ♦ ♣\n"
	"Currency: $ ¢ £ ¥ € ₹ ₽ ₩ ₪ ₿\n"
)
HE = "שלום עולם. זוהי שורה בעברית.\n"
AR = "مرحبا بالعالم. هذا سطر بالعربية.\n"
HI = "नमस्ते दुनिया। यह हिन्दी की एक पंक्ति है।\n"
TH = "สวัสดีชาวโลก นี่คือบรรทัดภาษาไทย\n"
COMBINING = (
	"e\u0301 a\u0300\u0323 o\u0302\u0301 n\u0303 u\u0308\u0304 c\u0327 Zalgo: Z\u0351\u036b\u0343\u036a\u0302\u036b\u033d\u034f\u0334\u0319\u0324\u031e\u0349\u035a\u032f\u031e\u0320\u034d\n"
	"\u0301A mark at the start of a paragraph.\n"
	"x\u20dd enclosed, \u0e01\u0e34 Thai mark, \u0915\u093f Devanagari vowel sign.\n"
	"Precomposed and decomposed: \u00e9 / e\u0301, \u00f1 / n\u0303, \u00fc / u\u0308.\n"
)
EMOJI = (
	"Single: \U0001F600 \U0001F680 \u2764 \u2603\n"
	"ZWJ family: \U0001F468\u200D\U0001F469\u200D\U0001F467\u200D\U0001F466 and rainbow flag \U0001F3F3\uFE0F\u200D\U0001F308\n"
	"Skin tones: \U0001F44D\U0001F3FD \U0001F44B\U0001F3FF\n"
	"Flags: \U0001F1E8\U0001F1E6 \U0001F1EB\U0001F1F7 \U0001F1FA\U0001F1F8\U0001F1EC\U0001F1E7\n"
	"Keycaps: 1\uFE0F\u20E3 #\uFE0F\u20E3\n"
	"Text and emoji presentation: \u263A\uFE0E \u263A\uFE0F\n"
)
INVISIBLE_TEXT = (
	"ZWJ:a\u200Db ZWNJ:a\u200Cb ZWSP:a\u200Bb WJ:a\u2060b\n"
	"BOM inside text: a\uFEFFb\n"
	"Direction marks: a\u200Eb\u200Fc\n"
	"Object replacement: a\uFFFCb\n"
	"C0 from outside: a\u0001b\n"
)
HEXBOX = (
	"BMP four digits: \u0416 \u0434 \u4e2d \u05d0\n"
	"Astral five digits: \U0001F600 \U00020000 \U0001D11E\n"
	"Six digits: \U0010FFFD\n"
	"An emoji sequence is one box labelled with its first code point: \U0001F468\u200D\U0001F469\u200D\U0001F467 end.\n"
	"A letter and a mark with no Windows-1252 form: \u0436\u0301 end. With one: e\u0301 end.\n"
	"Boxes beside letters: abc\u0416\u0417\u0418def\n"
)
MIXED = (
	"Latin café, Greek καφές, Cyrillic кофе, Armenian սուրճ, Georgian ყავა, Chinese 咖啡, Japanese コーヒー, Korean 커피, Hebrew קפה, Arabic قهوة, Hindi कॉफ़ी.\n"
	"Numbers 1234 and ٣٤٥٦ and ४५६ and ３４５.\n"
)
LONG_WORD = "Supercalifragilisticexpialidocious" * 12
LONG_LINES = (
	"A long line of words that wraps many times: " + "the quick brown fox jumps over the lazy dog " * 12 + "\n"
	"One unbroken word that must break at a cluster boundary: " + LONG_WORD + "\n"
	"Une très longue ligne en français, avec des accents é è ê à ç, des guillemets « » et des tirets – — : " + "voilà l’été " * 12 + "\n"
	"A long CJK line that breaks between ideographs: " + "这是一段很长的中文文字没有空格所以可以在任何汉字之间换行。" * 6 + "\n"
	"A long run of combining marks that stays one cluster: a" + "\u0301\u0302\u0303\u0304" * 24 + "\n"
)
CP437_BYTES = (
	b"Box: \xc9\xcd\xcd\xcd\xbb \xba   \xba \xc8\xcd\xcd\xcd\xbc\r\n"
	b"Greek and maths: \xe0 \xe1 \xe2 \xe3 \xe4 \xe5 \xe6 \xe7 \xe8 \xe9 \xea \xeb \xec \xed \xee \xef\r\n"
	b"More: \xf0 \xf1 \xf2 \xf3 \xf4 \xf5 \xf6 \xf7 \xf8 \xf9 \xfa \xfb \xfc \xfd\r\n"
	b"Letters: \x80 \x81 \x82 \x84 \x86 \x94 \x99 \x9a \xa4 \xa5\r\n"
)
INVALID_UTF8 = b"ok \xc3( bad \xe2\x82 end \xf0\x9f\x98 \xfe\xff tail\r\nsecond line \xe9t\xe9\r\n"
CP1252_VALID_UTF8 = b"Caf\xc3\xa9 and \xc3\xa0 la carte\r\n"
FR_CA_1252_TXT = FR_CA


def fr_ca_1252_bytes():
	return FR_CA.replace("\n", "\r\n").encode("cp1252")


def utf16(text, order):
	data = text.replace("\n", "\r\n").encode("utf-16-" + order)
	return (b"\xff\xfe" if order == "le" else b"\xfe\xff") + data


def nfd(text):
	return unicodedata.normalize("NFD", text)


def sampler_text():
	parts = [
		("French (Canada)", FR_CA.splitlines()[0]),
		("Polish and Czech", PL.splitlines()[0]),
		("Russian", RU.splitlines()[0]),
		("Greek", EL.splitlines()[0]),
		("Turkish", TR.splitlines()[0]),
		("Lithuanian", LT.splitlines()[0]),
		("Armenian", HY_KA.splitlines()[0]),
		("Georgian", HY_KA.splitlines()[1]),
		("Vietnamese NFC", VI_NFC.splitlines()[0]),
		("Vietnamese NFD", nfd(VI_NFC).splitlines()[0]),
		("Chinese", ZH_JA_KO.splitlines()[0]),
		("Japanese", ZH_JA_KO.splitlines()[1]),
		("Korean", ZH_JA_KO.splitlines()[2]),
		("Symbols", SYMBOLS.splitlines()[1]),
		("Hebrew", HE.splitlines()[0]),
		("Arabic", AR.splitlines()[0]),
		("Hindi", HI.splitlines()[0]),
		("Thai", TH.splitlines()[0]),
		("Combining marks", COMBINING.splitlines()[0]),
		("Emoji", EMOJI.splitlines()[1]),
		("Hex boxes", HEXBOX.splitlines()[0]),
	]
	return "".join("%s: %s\n" % (name, line) for name, line in parts)


# ---------------------------------------------------------------- .QLD


def qld_chunk(ident, payload):
	data = ident + struct.pack("<I", len(payload)) + payload
	return data + (b"\0" if len(payload) % 2 else b"")


def qld_file(text_bytes, version=(1, 0), unic=None):
	"""A `.QLD` file for one body of Windows-1252 `text_bytes` (it must end with a paragraph mark), one font, one format and one
	paragraph format. `unic` is an optional list of `(position, utf8_bytes)` runs for a `UNIC` chunk of a 1.1 file."""
	paras = text_bytes.count(b"\r")
	docp = struct.pack("<8i", 12240, 15840, 1800, 1800, 1440, 1440, 720, 720) + struct.pack("<HHBB", 1, 0, 0, 0)
	font = struct.pack("<H", 1) + bytes([1, 0, 0, 15]) + b"Times New Roman"
	colr = struct.pack("<H", 1) + bytes([0, 0, 0])
	cfmt = struct.pack("<H", 1) + struct.pack("<HHBBH", 0, 24, 0, 0, 0)
	tabs = struct.pack("<H", 0)
	lists = struct.pack("<H", 0)
	pfmt = struct.pack("<H", 2) + PFMT_BODY
	story = b"STRY"
	story += qld_chunk(b"STHD", bytes([0]) + struct.pack("<I", len(text_bytes)))
	story += qld_chunk(b"TEXT", text_bytes)
	story += qld_chunk(b"RUNS", struct.pack("<I", 1) + struct.pack("<IH", len(text_bytes), 0))
	story += qld_chunk(b"PARA", b"\0\0" * paras)
	story += qld_chunk(b"OBJS", b"")
	if unic is not None:
		payload = b""
		for position, utf8 in unic:
			payload += struct.pack("<IHH", position, len(utf8.decode("utf-8")), len(utf8)) + utf8
		story += qld_chunk(b"UNIC", payload)
	body = b"QLD1" + qld_chunk(b"HEAD", struct.pack("<HHHH", version[0], version[1], 1, 0))
	for ident, payload in [
		(b"DOCP", docp), (b"FONT", font), (b"COLR", colr), (b"CFMT", cfmt), (b"TABS", tabs), (b"LIST", lists), (b"PFMT", pfmt),
	]:  # fmt: skip
		body += qld_chunk(ident, payload)
	body += qld_chunk(b"LIST", story)
	return b"RIFF" + struct.pack("<I", len(body)) + body


# The two paragraph formats every file Quill writes starts with (the default, and the first one a document adds), as `plain.qld` holds
# them: 29 bytes each (align, spacing, flags, list, level, tabset, then left, right, first, before and after).
PFMT_BODY = bytes.fromhex(
	"00000000000000ffff000000000000000000000000000000000000000001000000000000ffff00000000000000000000000000000000f0000000"
)

LATIN_QLD_TEXT = "Quill keeps Windows-1252: café, naïve, façade, “quotes”, – dashes — and € 5.\rA second paragraph.\r"
UNIC_QLD_TEXT = "Plain Latin, then Cyrillic: Привет, and Greek α.\r"


def unic_runs(text):
	"""`UNIC` runs for the characters of `text` Windows-1252 lacks, with `?` in their place in the `TEXT` bytes."""
	text_bytes, runs, run = bytearray(), [], None
	for i, ch in enumerate(text):
		if ch in CP1252_CHARS or ch == "\r":
			text_bytes += ch.encode("cp1252")
			if run:
				runs.append(run)
				run = None
		else:
			text_bytes += b"?"
			if run is None:
				run = [i, ""]
			run[1] += ch
	if run:
		runs.append(run)
	return bytes(text_bytes), [(p, s.encode("utf-8")) for p, s in runs]


# ---------------------------------------------------------------- the catalogue

# Each entry: (file, kind, source, checks, encoding, bytes, expected code points, today override or None, flips)
Doc = dict


def doc(name, kind, what, data, expect, source="generated", encoding="", today=None, note=""):
	return {
		"file": name, "kind": kind, "what": what, "data": data, "expect": expect, "source": source, "encoding": encoding,
		"today": today, "note": note,
	}  # fmt: skip


HAND = {
	"fr-ca.rtf": text_cps(FR_CA),
	"ru-word97-upr.rtf": text_cps("Привет, мир! Word 97 writes a Unicode copy in \\ud.\n"),
}


def catalogue():
	docs = []
	add = docs.append
	# Scripts, RTF.
	add(doc("pl-cs-1250.rtf", "rtf", "Polish and Czech through `\\fcharset238` and `\\ansicpg1250`", rtf_doc(PL, 1250, 238), text_cps(PL), encoding="windows-1250"))
	add(doc("ru-1251.rtf", "rtf", "Russian through `\\ansicpg1251` with `\\'hh` bytes", rtf_doc(RU, 1251, 204), text_cps(RU), encoding="windows-1251"))
	add(doc("ru-u.rtf", "rtf", "Russian as `\\uc1\\uN?`", rtf_doc(RU, 1252, 0, body_mode="u"), text_cps(RU), encoding="rtf-u"))
	add(doc("el-1253.rtf", "rtf", "Greek through `\\fcharset161` and `\\ansicpg1253`", rtf_doc(EL, 1253, 161), text_cps(EL), encoding="windows-1253"))
	add(doc("tr-1254.rtf", "rtf", "Turkish through `\\fcharset162` and `\\ansicpg1254`", rtf_doc(TR, 1254, 162), text_cps(TR), encoding="windows-1254"))
	add(doc("lt-1257.rtf", "rtf", "Lithuanian through `\\fcharset186` and `\\ansicpg1257`", rtf_doc(LT, 1257, 186), text_cps(LT), encoding="windows-1257"))
	# RTF edge cases.
	add(doc("uc0.rtf", "rtf", "`\\uc0`: no fallback character follows `\\uN`, so the text after it is kept",
		b"{\\rtf1\\ansi\\ansicpg1252\\uc0\\deff0{\\fonttbl{\\f0 Arial;}}\\f0 a\\u1055 b\\u1056 c\\u233 d\\par\n}\n",
		cps_of("a\u041fb\u0420c\u00e9d") + [CR], encoding="rtf-u"))  # fmt: skip
	add(doc("uc2.rtf", "rtf", "`\\uc2`: two fallback characters follow `\\uN` and are skipped",
		b"{\\rtf1\\ansi\\ansicpg1252\\uc2\\deff0{\\fonttbl{\\f0 Arial;}}\\f0 a\\u1055xxb\\u1056\\'3f\\'3fc\\u233\\'e9\\'e9d\\par\n}\n",
		cps_of("a\u041fb\u0420c\u00e9d") + [CR], encoding="rtf-u"))  # fmt: skip
	add(doc("u-negative.rtf", "rtf", "`\\uN` above 32767 is written as a negative number: U+AC00 as `\\u-21504` and U+FF21 as `\\u-223`",
		b"{\\rtf1\\ansi\\ansicpg1252\\uc1\\deff0{\\fonttbl{\\f0 Arial;}}\\f0 a\\u-21504?b\\u-223?c\\par\n}\n",
		cps_of("a\uac00b\uff21c") + [CR], encoding="rtf-u"))  # fmt: skip
	add(doc("lone-surrogate.rtf", "rtf", "A lone high surrogate and a lone low one are U+FFFD",
		b"{\\rtf1\\ansi\\ansicpg1252\\uc1\\deff0{\\fonttbl{\\f0 Arial;}}\\f0 a\\u-10179?b\\u-8704?c\\par\n}\n",
		cps_of("a\ufffdb\ufffdc") + [CR], encoding="rtf-u"))  # fmt: skip
	add(doc("astral-pair.rtf", "rtf", "Astral characters as surrogate pairs: U+1F600 and U+20000",
		b"{\\rtf1\\ansi\\ansicpg1252\\uc1\\deff0{\\fonttbl{\\f0 Arial;}}\\f0 a\\u-10179?\\u-8704?b\\u-10176?\\u-9216?c\\par\n}\n",
		cps_of("a\U0001F600b\U00020000c") + [CR], encoding="rtf-u"))  # fmt: skip
	add(doc("dbcs-932.rtf", "rtf", "A double-byte code page (932) beside `\\u`: the `\\u` value is used and the two fallback bytes are skipped",
		b"{\\rtf1\\ansi\\ansicpg932\\uc2\\deff0{\\fonttbl{\\f0\\fnil\\fcharset128 MS Gothic;}}\\f0 a\\u26085\\'93\\'fa\\u26412\\'96\\'7bb\\par\n}\n",
		cps_of("a\u65e5\u672cb") + [CR], encoding="windows-932"))  # fmt: skip
	# Plain text, scripts.
	def utext(name, text, what, nl="\r\n", note=""):
		add(doc(name, "text", what, text.replace("\n", nl).encode("utf-8"), text_cps(text), encoding="utf-8", note=note))

	utext("fr-ca.txt", FR_CA, "Canadian French in UTF-8 with curly quotes, guillemets, dashes and an ellipsis")
	add(doc("fr-ca-1252.txt", "text", "The same Canadian French as Windows-1252 bytes: invalid UTF-8, so read as the code page", fr_ca_1252_bytes(), text_cps(FR_CA), encoding="windows-1252"))
	utext("hy-ka.txt", HY_KA, "Armenian and Georgian (Mkhedruli and Asomtavruli)")
	utext("vi-nfc.txt", VI_NFC, "Vietnamese, precomposed (NFC)")
	utext("vi-nfd.txt", nfd(VI_NFC), "The same Vietnamese, decomposed (NFD): stored as it came, and equal in Find (CORE-214)")
	utext("zh-ja-ko.txt", ZH_JA_KO, "Ideographs, kana, Hangul, CJK punctuation, fullwidth and halfwidth forms and Extension B ideographs")
	utext("symbols.txt", SYMBOLS, "Arrows, maths, box drawing, dingbats and currency signs")
	utext("he.txt", HE, "Hebrew: kept in logical order, a placeholder in v1 (CORE-226)")
	utext("ar.txt", AR, "Arabic: kept, a placeholder in v1 (CORE-226)")
	utext("hi.txt", HI, "Devanagari: kept, a placeholder in v1 (CORE-226)")
	utext("th.txt", TH, "Thai: kept, a placeholder in v1 (CORE-226)")
	utext("mixed.txt", MIXED, "One paragraph with eleven scripts, and digits from four numeral systems")
	utext("combining.txt", COMBINING, "Stacked marks, a mark at a paragraph's start, enclosing marks, precomposed beside decomposed")
	utext("emoji.txt", EMOJI, "Single emoji, a ZWJ family, skin tones, flags, keycaps, VS15 and VS16")
	utext("invisible.txt", INVISIBLE_TEXT, "ZWJ, ZWNJ, ZWSP, WJ, a byte order mark inside text, LRM, RLM, U+FFFC and U+0001 from outside (dropped)")
	utext("hexbox.txt", HEXBOX, "Placeholder boxes of four, five and six hex digits, a sequence as one box, and boxes beside letters")
	utext("long-lines.txt", LONG_LINES, "Long lines: words, one unbroken word, French, CJK and a run of marks, for the line breaks")
	docs[-1]["note"] = "Each paragraph is a single line far wider than a page."
	utext("lf-only.txt", "First line\nSecond line\n\nAfter a blank line\n", "ASCII with LF line ends and a blank line: the same in every encoding", nl="\n")
	# Encodings.
	add(doc("utf8-bom.txt", "text", "UTF-8 with a byte order mark, which is not a character", b"\xef\xbb\xbf" + "Café ☕ and Привет\r\nSecond line\r\n".encode("utf-8"),
		text_cps("Café ☕ and Привет\nSecond line\n"), encoding="utf-8-bom"))  # fmt: skip
	u16 = "Café ☕ Привет 😀 𠀀\nSecond line\n"
	add(doc("utf16le-bom.txt", "text", "UTF-16 little endian with a byte order mark and astral characters", utf16(u16, "le"), text_cps(u16), encoding="utf-16le-bom"))
	add(doc("utf16be-bom.txt", "text", "UTF-16 big endian with a byte order mark and astral characters", utf16(u16, "be"), text_cps(u16), encoding="utf-16be-bom"))
	bad = INVALID_UTF8.decode("cp1252", errors="replace")
	bad_cps = text_cps("".join(cp1252_char(b) for b in INVALID_UTF8))
	add(doc("invalid-utf8.txt", "text", "Bad UTF-8 sequences: the file is not valid UTF-8, so every byte is read as Windows-1252 (C4)", INVALID_UTF8, bad_cps, encoding="invalid-utf8",
		note="Read as Windows-1252 on Windows and GTK, as CP437 on DOS; this expectation is the Windows reading."))  # fmt: skip
	del bad
	add(doc("cp1252-valid-utf8.txt", "text", "The C4 caveat: Windows-1252 text that is also valid UTF-8 reads as UTF-8", CP1252_VALID_UTF8, text_cps("Café and à la carte\n"), encoding="utf-8",
		note="The writer meant the bytes C3 A9 as the two letters Ã and a copyright sign; the C4 rule reads them as one é."))  # fmt: skip
	add(doc("cp437.txt", "text", "CP437 box drawing, Greek and maths bytes for the DOS reader", CP437_BYTES, text_cps(CP437_BYTES.decode("cp437").replace("\r\n", "\n")), encoding="cp437",
		note="This expectation is the DOS reading; a Windows reader sees the same bytes as Windows-1252."))  # fmt: skip
	add(doc("sampler.txt", "text", "The sampler's text: one line per script and case above; WP-UNI-4 adds `sampler.qld` from it", sampler_text().replace("\n", "\r\n").encode("utf-8"), text_cps(sampler_text()), encoding="utf-8"))
	# Hand-written RTF.
	for name, cps in HAND.items():
		what = {
			"fr-ca.rtf": "Canadian French as `\\'hh` bytes: Windows-1252 only, so it reads the same today",
			"ru-word97-upr.rtf": "Word 97's `\\upr` with a `\\ud` Unicode copy: the reader takes the `\\ud` group",
		}[name]
		path = os.path.join(OUT, name)
		with open(path, "rb") as f:
			data = f.read()
		# The `\\upr` copy holds literal question marks, so today's reader keeps them and refuses nothing.
		today = (narrow(cps)[0], 0) if name == "ru-word97-upr.rtf" else None
		add(doc(name, "rtf", what, data, cps, source="hand-written", encoding="windows-1252" if name == "fr-ca.rtf" else "rtf-u", today=today))
	# .QLD
	qtext, qruns = unic_runs(LATIN_QLD_TEXT)
	assert not qruns
	add(doc("latin-1-0.qld", "qld", "A 1.0 file that holds Windows-1252 only and must stay 1.0 on save", qld_file(qtext), text_cps(LATIN_QLD_TEXT.replace("\r", "\n")), encoding="windows-1252"))
	utext_bytes, uruns = unic_runs(UNIC_QLD_TEXT)
	add(doc("skip-unic-1-1.qld", "qld", "A synthetic 1.1 file with a `UNIC` chunk for Cyrillic and Greek: a 1.0 reader skips the chunk and shows `?`",
		qld_file(utext_bytes, (1, 1), uruns), text_cps(UNIC_QLD_TEXT.replace("\r", "\n")), encoding="qld-1.1",
		note="Made from the plan's `UNIC` layout (position u32, characters u16, bytes u16, UTF-8); WP-UNI-4 owns the real writer and replaces this with `mixed-1-1.qld`."))  # fmt: skip
	return docs


def today_for(d):
	if d["today"] is not None:
		return d["today"]
	if d["kind"] == "text":
		return today_text(d["data"])
	if d["kind"] == "qld":
		# A 1.0 reader keeps the `TEXT` bytes: Windows-1252 characters and `?` for the rest, nothing counted.
		out, _ = narrow(d["expect"])
		return out, 0
	return narrow(d["expect"])


# ---------------------------------------------------------------- output


def fmt_cps(cps):
	return "".join("U+%04X\n" % cp for cp in cps)


def toml_str(s):
	return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def render():
	"""Every file of the set as `{name: bytes}`."""
	files = {}
	docs = catalogue()
	manifest = HEADER.format(summary="The Unicode test document set (WP-UNI-2): every document, its source and what each build shows.")
	manifest += (
		"#\n# Generated by scripts/gen-unicode-docs.py; do not edit. `flips` names the package that makes today's reader give the `.expect`\n"
		"# code points; it is removed from the entry when that package merges and `.today` is made equal to `.expect`.\n"
		"# `dos_text`, `dos_gfx`, `win16`, `win32`, `tty` and `gtk` count clusters by what the build shows (CORE-213): shown as itself,\n"
		"# composed into a Windows-1252 letter, a placeholder box (a `▒` in a cell), or invisible.\n"
	)
	for d in docs:
		name = d["file"]
		expect = d["expect"]
		today, refused = today_for(d)
		paras = paragraphs(expect)
		cl = [[len(c) for c in clusters(p)] for p in paras]
		files[name] = d["data"]
		files[name + ".expect"] = fmt_cps(expect).encode()
		files[name + ".today"] = ("refused=%d\n" % refused + fmt_cps(today)).encode()
		files[name + ".clusters"] = "".join(" ".join(map(str, c)) + "\n" for c in cl).encode()
		files[name + ".breaks"] = "".join(" ".join(map(str, break_offsets(p))) + "\n" for p in paras).encode()
		counts = display_counts(expect)
		same = (today == expect) and refused == 0
		manifest += "\n[[doc]]\n"
		manifest += "file = %s\n" % toml_str(name)
		manifest += "kind = %s\n" % toml_str(d["kind"])
		manifest += "source = %s\n" % toml_str(d["source"] if d["source"] == "hand-written" else "scripts/gen-unicode-docs.py")
		manifest += "encoding = %s\n" % toml_str(d["encoding"])
		manifest += "what = %s\n" % toml_str(d["what"])
		if d["note"]:
			manifest += "note = %s\n" % toml_str(d["note"])
		manifest += "scalars = %d\n" % len(expect)
		manifest += "paragraphs = %d\n" % len(paras)
		manifest += "clusters = %d\n" % sum(len(c) for c in cl)
		manifest += "refused_today = %d\n" % refused
		if not same:
			manifest += 'flips = "WP-UNI-4"\n'
		for build in BUILDS:
			c = counts[build]
			manifest += "%s = %s\n" % (build, toml_str("shown=%d composed=%d placeholder=%d invisible=%d" % (c["shown"], c["composed"], c["placeholder"], c["invisible"])))
	files["manifest.toml"] = manifest.encode()
	return files, docs


def main(argv):
	check = "--check" in argv
	files, _ = render()
	bad = []
	os.makedirs(OUT, exist_ok=True)
	for name, data in sorted(files.items()):
		path = os.path.join(OUT, name)
		if check:
			try:
				with open(path, "rb") as f:
					have = f.read()
			except FileNotFoundError:
				have = None
			if have != data:
				bad.append(name)
		else:
			with open(path, "wb") as f:
				f.write(data)
	if check:
		on_disk = {n for n in os.listdir(OUT)}
		bad += ["%s (not generated)" % n for n in sorted(on_disk - set(files)) if not n.endswith("README.md")]
		if bad:
			print("gen-unicode-docs: stale or stray files in tests/docs/unicode:", file=sys.stderr)
			for n in bad:
				print("  " + n, file=sys.stderr)
			print("run: python3 -I scripts/gen-unicode-docs.py", file=sys.stderr)
			return 1
		print("gen-unicode-docs: ok (%d files)" % len(files))
	else:
		print("gen-unicode-docs: wrote %d files to tests/docs/unicode" % len(files))
	return 0


if __name__ == "__main__":
	sys.exit(main(sys.argv[1:]))
