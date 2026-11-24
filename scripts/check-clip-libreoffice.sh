#!/usr/bin/env bash
# Checks that the RTF the clipboard carries (the golden selection copy) opens in LibreOffice with the selection's text and formats.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: scripts/check-clip-libreoffice.sh
#
# `tests/golden/files/clip-selection.rtf` is what `quill_clip::copy` writes for "bold italic and underlined " taken out of a
# paragraph with bold, italic and underlined runs. LibreOffice converts it to HTML headless, and the text and the bold, italic and
# underline marks must match. Exits 0 with a note when soffice is not installed.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v soffice > /dev/null; then
	echo "check-clip-libreoffice: soffice not found; skipping"
	exit 0
fi
work="$(mktemp -d "${TMPDIR:-/tmp}/quill-clip-lo.XXXXXX")"
trap 'rm -rf "$work"' EXIT
cp tests/golden/files/clip-selection.rtf "$work/clip.rtf"
(cd "$work" && soffice --headless -env:UserInstallation="file://$work/profile" --convert-to html clip.rtf > /dev/null 2>&1)
python3 -I - "$work/clip.html" << 'PY'
import html
import re
import sys

t = open(sys.argv[1], encoding="latin-1").read()
body = t[t.index("<body"):]
text = re.sub(r"\s+", " ", html.unescape(re.sub(r"<[^>]+>", "", body))).strip()
if text != "bold italic and underlined":
    sys.exit("text differs: " + repr(text))
counts = {k: len(re.findall(p, body)) for k, p in [("bold", r"<b>"), ("italic", r"<i>"), ("underline", r"<u>")]}
if counts != {"bold": 1, "italic": 1, "underline": 1}:
    sys.exit("formats differ: " + repr(counts))
print("check-clip-libreoffice: ok:", counts)
PY
