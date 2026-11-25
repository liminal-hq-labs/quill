#!/usr/bin/env bash
# Rebuild the shipped sample document from its RTF template: demo.rtf and demo.qld, plus the ReactOS demo's variants.
# The two icons are 4-bit BMP pictures (icon-64.bmp, icon-24.bmp), which every build draws; 8-bit ones are avoided on purpose.
# They are embedded as DIBs (a BMP without its 14-byte file header) in hex, shown at 96 dots per inch.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: samples/make-demo.sh [QUILL_CLI]   (default: target/release/quill, built with `cargo build --release -p quill-cli`)
#        samples/make-demo.sh --check [QUILL_CLI]   rebuilds into a scratch folder and fails if a committed file differs
set -euo pipefail
check=0
if [ "${1:-}" = "--check" ]; then
	check=1
	shift
fi
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
quill="${1:-$root/target/release/quill}"
out="$here"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
[ "$check" = 0 ] || out="$tmp"

pict() {
	local bmp="$1" px="$2"
	local twips=$((px * 15))
	printf '{\\pict\\dibitmap0\\picw%d\\pich%d\\picwgoal%d\\pichgoal%d\n' "$px" "$px" "$twips" "$twips"
	tail -c +15 "$bmp" | xxd -p | tr -d '\n' | fold -w 96
	printf '}'
}

big="$(pict "$here/icon-64.bmp" 64)"
small="$(pict "$here/icon-24.bmp" 24)"

build() {
	local name="$1" line="$2"
	BIG="$big" SMALL="$small" LINE="$line" awk 'BEGIN { big = ENVIRON["BIG"]; small = ENVIRON["SMALL"]; line = ENVIRON["LINE"] }
	{
		i = index($0, "@ICON64@"); if (i) $0 = substr($0, 1, i - 1) big substr($0, i + 8)
		i = index($0, "@ICON24@"); if (i) $0 = substr($0, 1, i - 1) small substr($0, i + 8)
		i = index($0, " @WHICH@"); if (i) $0 = substr($0, 1, i - 1) line substr($0, i + 8)
		print
	}' "$here/demo.rtf.in" > "$out/$name.rtf"
	"$quill" convert "$out/$name.rtf" "$out/$name.qld"
}

# The shipped sample is the same on every system. The ReactOS page in the browser says where it runs, so it has its own wording.
build demo ""
build demo-reactos " This copy is the Windows build, running on ReactOS (an open-source Windows) inside a PC emulator in your browser."
build demo-reactos16 " This copy is the Windows 3.x build, running through WineVDM on ReactOS (an open-source Windows) inside a PC emulator in your browser. That is emulation inside emulation, so it is slow."

if [ "$check" = 1 ]; then
	for f in demo demo-reactos demo-reactos16; do
		for e in rtf qld; do
			cmp "$here/$f.$e" "$tmp/$f.$e" || { echo "make-demo: $f.$e is out of date: run samples/make-demo.sh" >&2; exit 1; }
		done
	done
	echo "make-demo: committed samples match the template"
	exit 0
fi
ls -l "$here"/demo*.rtf "$here"/demo*.qld
