#!/usr/bin/env bash
# Checks that RTF written by Quill opens in LibreOffice with the same text and formats as the original it was read from.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: scripts/check-rtf-libreoffice.sh [DIR/FILE.rtf ...]   (default: tests/docs/letter.rtf)
#
# Each file is read and written again by quill-convert, both are converted to HTML by LibreOffice headless, and the text and the
# counts of bold, italic, underline, strike, superscript, subscript, alignment and font mentions must match. Exits 0 with a note
# when soffice is not installed. Set CARGO_TARGET_DIR to keep the build output outside the tree.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v soffice > /dev/null; then
	echo "check-rtf-libreoffice: soffice not found; skipping"
	exit 0
fi
files=("$@")
[ ${#files[@]} -gt 0 ] || files=(tests/docs/letter.rtf)
cargo build -q -p quill-host --bin quill-convert
bin="${CARGO_TARGET_DIR:-target}/debug/quill-convert"
work="$(mktemp -d "${TMPDIR:-/tmp}/quill-lo.XXXXXX")"
trap 'rm -rf "$work"' EXIT

for f in "${files[@]}"; do
	name="$(basename "$f" .rtf)"
	cp "$f" "$work/$name-orig.rtf"
	"$bin" "$f" "$work/$name-quill.rtf"
	(cd "$work" && soffice --headless -env:UserInstallation="file://$work/profile" --convert-to html "$name-orig.rtf" "$name-quill.rtf" > /dev/null 2>&1)
	python3 -I - "$work/$name-orig.html" "$work/$name-quill.html" << 'PY'
import html
import re
import sys


def summary(path):
    t = open(path, encoding="latin-1").read()
    body = t[t.index("<body"):]
    counts = {k: len(re.findall(p, body)) for k, p in [
        ("bold", r"<b>"), ("italic", r"<i>"), ("underline", r"<u>"), ("strike", r"<(strike|s)>"),
        ("super", r"<sup"), ("sub", r"<sub"), ("img", r"<img")]}
    low = body.lower()
    for k in ("center", "right", "justify", "ff0000", "arial", "courier"):
        counts[k] = low.count(k)
    text = re.sub(r"\s+", " ", html.unescape(re.sub(r"<[^>]+>", "", body))).strip()
    return text, counts


a, b = summary(sys.argv[1]), summary(sys.argv[2])
if a[0] != b[0]:
    sys.exit("text differs:\n" + a[0] + "\n" + b[0])
if a[1] != b[1]:
    sys.exit("formats differ:\n" + repr(a[1]) + "\n" + repr(b[1]))
print("check-rtf-libreoffice: ok:", sys.argv[1].rsplit("/", 1)[-1].replace("-orig.html", ""), a[1])
PY
done
