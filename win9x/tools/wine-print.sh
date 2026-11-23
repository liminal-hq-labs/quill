#!/usr/bin/env bash
# Prints documents with QUILL32.EXE under Wine to PostScript files (`/PO`), renders them with Ghostscript and checks what is on the pages.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-print.sh   (needs wine, xvfb-run and gs; exits 0 with a note when they are missing)
#
# print3.rtf has three pages with a header (Page n of m), a footer and a picture on page 2: the job must have three pages, every page
# must have ink in the header and the footer bands, page 2 must have coloured pixels where the picture goes and the others none.
# letter.rtf must print as one page with red text. The PNG renderings are left in $PRINT_OUT (default: a scratch directory) to look at.
set -euo pipefail
cd "$(dirname "$0")/../.."

command -v wine > /dev/null && command -v xvfb-run > /dev/null && command -v gs > /dev/null || { echo "wine-print: wine, xvfb-run or gs not found; skipping"; exit 0; }
exe=win9x/dist/QUILL32.EXE
[ -f "$exe" ] || { echo "wine-print: build $exe first (win9x/build.sh)"; exit 1; }
export WINEPREFIX="${WINEPREFIX:-${TMPDIR:-/tmp}/quill-wineprefix}" WINEDEBUG=-all
unset WINEARCH
work="${PRINT_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/quill-print.XXXXXX")}"
mkdir -p "$work"
cp "$exe" "$work/QUILL32.EXE"
fail() { echo "wine-print: FAIL: $*" >&2; exit 1; }

for doc in print3 letter; do
	cp "tests/docs/$doc.rtf" "$work/$doc.rtf"
	(cd "$work" && timeout 180 xvfb-run -a wine QUILL32.EXE "$doc.rtf" /PO "$doc.ps" > /dev/null 2>&1) || fail "$doc: QUILL32.EXE /PO did not finish"
	[ -s "$work/$doc.ps" ] || fail "$doc: no PostScript was written"
	gs -q -dNOPAUSE -dBATCH -sDEVICE=ppmraw -r50 -sOutputFile="$work/$doc-%d.ppm" "$work/$doc.ps"
	for f in "$work/$doc"-*.ppm; do
		convert "$f" "${f%.ppm}.png" 2> /dev/null || magick "$f" "${f%.ppm}.png"
	done
done

python3 -I - "$work" << 'PY'
import sys, glob

work = sys.argv[1]

def load(path):
    import re
    d = open(path, "rb").read()
    m = re.match(rb"P6\s+(?:#[^\n]*\n\s*)*(\d+)\s+(\d+)\s+255\s", d)
    assert m, path
    w, h = int(m.group(1)), int(m.group(2))
    return w, h, d[-w * h * 3:]

def ink(page, y0, y1, colour=False):
    w, h, px = page
    n = 0
    for y in range(int(y0 * h), int(y1 * h)):
        row = px[y * w * 3:(y + 1) * w * 3]
        for x in range(0, len(row), 3):
            r, g, b = row[x], row[x + 1], row[x + 2]
            if colour:
                if max(r, g, b) - min(r, g, b) > 60:
                    n += 1
            elif r < 160 and g < 160 and b < 160:
                n += 1
    return n

pages = sorted(glob.glob(work + "/print3-*.ppm"))
assert len(pages) == 3, f"print3 has {len(pages)} pages, wanted 3"
for i, p in enumerate(pages, 1):
    pg = load(p)
    assert ink(pg, 0.03, 0.08) > 10, f"print3 page {i}: no header ink"
    assert ink(pg, 0.92, 0.98) > 10, f"print3 page {i}: no footer ink"
    assert ink(pg, 0.09, 0.30) > 10, f"print3 page {i}: no body ink"
    coloured = ink(pg, 0.09, 0.45, colour=True)
    if i == 2:
        assert coloured > 400, f"print3 page 2: the picture is missing ({coloured} coloured pixels)"
    else:
        assert coloured == 0, f"print3 page {i}: unexpected colour"
letters = sorted(glob.glob(work + "/letter-*.ppm"))
assert len(letters) == 1, f"letter has {len(letters)} pages, wanted 1"
assert ink(load(letters[0]), 0.05, 0.45, colour=True) > 20, "letter: the red word is missing"
print("wine-print: ok: print3 prints 3 pages with header, footer and the picture on page 2; letter prints 1 page with colour")
PY
echo "wine-print: renderings in $work"
