#!/usr/bin/env bash
# Writes documents to PDF with QUILL32.EXE under Wine (`/PDF`) and checks the files with qpdf, pdfinfo, pdftotext, pdfimages and pdftoppm.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-pdf.sh   (needs wine and poppler-utils with qpdf; uses $DISPLAY when set, else xvfb-run; exits 0 with a note when
#        a tool is missing)
#
# letter.qld must be one letter page whose words come back out in order, in the fonts its faces map to. print3.rtf must be three pages
# with the header (Page n of 3) and the footer on each, and the picture on page 2 only. The PDFs are left in $PDF_OUT (default: a scratch
# directory), and each is also written at `[Print] PdfDpi=600` to check that the same pages come out of a printer-like layout.
set -euo pipefail
cd "$(dirname "$0")/../.."

for tool in wine qpdf pdfinfo pdftotext pdfimages pdftoppm; do
	command -v "$tool" > /dev/null || { echo "wine-pdf: $tool not found; skipping"; exit 0; }
done
[ -n "${DISPLAY:-}" ] || command -v xvfb-run > /dev/null || { echo "wine-pdf: no DISPLAY and no xvfb-run; skipping"; exit 0; }
exe=win9x/dist/QUILL32.EXE
[ -f "$exe" ] || { echo "wine-pdf: build $exe first (win9x/build.sh)"; exit 1; }
export WINEPREFIX="${WINEPREFIX:-${TMPDIR:-/tmp}/quill-wineprefix}" WINEDEBUG=-all
unset WINEARCH WAYLAND_DISPLAY
work="${PDF_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/quill-pdf.XXXXXX")}"
mkdir -p "$work"
cp "$exe" "$work/QUILL32.EXE"
fail() { echo "wine-pdf: FAIL: $*" >&2; exit 1; }
display_prefix=()
[ -n "${DISPLAY:-}" ] || display_prefix=(xvfb-run -a)

ini="$WINEPREFIX/drive_c/windows/QUILL.INI"
mkdir -p "$(dirname "$ini")"

make_pdf() { # doc out [dpi]
	if [ -n "${3:-}" ]; then printf '[Print]\r\nPdfDpi=%s\r\n' "$3" > "$ini"; else rm -f "$ini"; fi
	(cd "$work" && timeout 180 "${display_prefix[@]}" wine QUILL32.EXE "$1" /PDF "$2" > /dev/null 2>&1) || fail "$1: QUILL32.EXE /PDF did not finish"
	rm -f "$ini"
	[ -s "$work/$2" ] || fail "$1: no PDF was written"
	qpdf --check "$work/$2" > "$work/$2.check" 2>&1 || fail "$1: qpdf --check failed (see $work/$2.check)"
}

cp tests/docs/letter.qld tests/docs/print3.rtf samples/demo-reactos.qld "$work/"
for dpi in "" 600; do
	suffix="${dpi:+-${dpi}dpi}"
	make_pdf letter.qld "letter$suffix.pdf" "$dpi"
	make_pdf print3.rtf "print3$suffix.pdf" "$dpi"
	make_pdf demo-reactos.qld "demo-reactos$suffix.pdf" "$dpi"

	pages() { pdfinfo "$work/$1" | awk '/^Pages:/ {print $2}'; }
	[ "$(pages "letter$suffix.pdf")" = 1 ] || fail "letter$suffix: not one page"
	[ "$(pages "print3$suffix.pdf")" = 3 ] || fail "print3$suffix: not three pages"
	[ "$(pages "demo-reactos$suffix.pdf")" = 1 ] || fail "demo-reactos$suffix: not one page"
	pdfinfo "$work/letter$suffix.pdf" | grep -q '612 x 792 pts (letter)' || fail "letter$suffix: not a letter page"

	letter="$(pdftotext -layout "$work/letter$suffix.pdf" - | tr -s ' ')"
	for want in "Quill Letter" "Dear reader," "bold, italic, underlined and struck text" "Paper (500 sheets)" "Yours truly," "The Quill team"; do
		grep -qF "$want" <<< "$letter" || fail "letter$suffix: the text lacks '$want'"
	done
	grep -qF $'\xe2\x80\x9ccurly quotes\xe2\x80\x9d' <<< "$(pdftotext "$work/letter$suffix.pdf" - | tr '\n' ' ' | tr -s ' ')" || fail "letter$suffix: curly quotes did not survive"

	for page in 1 2 3; do
		text="$(pdftotext -f "$page" -l "$page" -layout "$work/print3$suffix.pdf" - | tr -s ' \n' ' ')"
		grep -qF "Page $page of 3" <<< "$text" || fail "print3$suffix page $page: no 'Page $page of 3' in the header"
		grep -qF "Quill print test" <<< "$text" || fail "print3$suffix page $page: no footer"
	done
	[ "$(pdfimages -list -f 2 -l 2 "$work/print3$suffix.pdf" | tail -n +3 | wc -l)" -ge 1 ] || fail "print3$suffix: page 2 has no picture"
	[ "$(pdfimages -list -f 1 -l 1 "$work/print3$suffix.pdf" | tail -n +3 | wc -l)" = 0 ] || fail "print3$suffix: page 1 has a picture"
	[ "$(pdfimages -list -f 3 -l 3 "$work/print3$suffix.pdf" | tail -n +3 | wc -l)" = 0 ] || fail "print3$suffix: page 3 has a picture"
	pdftoppm -r 50 -png "$work/print3$suffix.pdf" "$work/print3$suffix-page"
	pdftoppm -r 100 -png "$work/demo-reactos$suffix.pdf" "$work/demo-reactos$suffix-page"

	for f in letter print3 demo-reactos; do
		size="$(wc -c < "$work/$f$suffix.pdf")"
		[ "$size" -lt 500000 ] || fail "$f$suffix: $size bytes is over the 0.5 MB a page budget"
		echo "wine-pdf: $f$suffix.pdf $size bytes"
	done
done
echo "wine-pdf: ok: letter and the demo are one letter page, print3 is three with header, footer and the picture on page 2; text extracts in order"
echo "wine-pdf: files in $work"
