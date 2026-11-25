#!/usr/bin/env bash
# Build the demo documents the ReactOS page opens: demo.rtf and demo.qld (Windows 32-bit build) and demo16.* (Windows 3.x via WineVDM).
# The icons are rendered from the full-detail artwork and embedded as 4-bit BMP pictures: every build draws them, and 8-bit ones showed as a grey box in the 32-bit build on ReactOS.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: demo/make-demo.sh ICON_SVG QUILL_CLI   (run from site/try-reactos; QUILL_CLI is the built `quill` binary)
set -euo pipefail
svg="$1"
quill="$2"
here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# One picture as RTF: a DIB (a BMP without its 14-byte file header) in hex, shown at 96 dots per inch.
pict() {
	local px="$1" colours="$2"
	rsvg-convert -w "$px" -h "$px" "$svg" -o "$tmp/i$px.png"
	magick "$tmp/i$px.png" -background white -alpha remove -alpha off -colors "$colours" BMP3:"$tmp/i$px.bmp"
	local twips=$((px * 15))
	printf '{\\pict\\dibitmap0\\picw%d\\pich%d\\picwgoal%d\\pichgoal%d\n' "$px" "$px" "$twips" "$twips"
	tail -c +15 "$tmp/i$px.bmp" | xxd -p | tr -d '\n' | fold -w 96
	printf '}'
}

big="$(pict 64 16)"
small="$(pict 24 16)"

build() {
	local out="$1" which="$2"
	BIG="$big" SMALL="$small" WHICH="$which" awk 'BEGIN { big = ENVIRON["BIG"]; small = ENVIRON["SMALL"]; which = ENVIRON["WHICH"] }
	{
		i = index($0, "@ICON64@"); if (i) $0 = substr($0, 1, i - 1) big substr($0, i + 8)
		i = index($0, "@ICON24@"); if (i) $0 = substr($0, 1, i - 1) small substr($0, i + 8)
		i = index($0, "@WHICH@"); if (i) $0 = substr($0, 1, i - 1) which substr($0, i + 7)
		print
	}' "$here/demo.rtf.in" > "$here/$out.rtf"
	"$quill" convert "$here/$out.rtf" "$here/$out.qld"
}

build demo "This copy is the Windows build, running on ReactOS (an open-source Windows) inside a PC emulator in your browser."
build demo16 "This copy is the Windows 3.x build, running through WineVDM on ReactOS (an open-source Windows) inside a PC emulator in your browser. That is emulation inside emulation, so it is slow."
ls -l "$here"/demo*.rtf "$here"/demo*.qld
