#!/usr/bin/env bash
# Drives QUILL32.EXE under Wine on a 640 by 480 Xvfb screen and checks the editing behaviour a hand test found missing.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-editing.sh [--shots DIR]     (build first with win9x/build.sh)
#
# Checks, by pixels: the page is centred with no horizontal scroll bar (page-width zoom); Ctrl+I switches Italic on without typing a
# tab; a drag over text highlights it; the mouse wheel scrolls; a tooltip shows over a preview button; and print preview leaves the
# window exactly as it was after Esc. Needs wine, Xvfb, xdotool and ImageMagick; exits 0 with a note when they are missing. Uses its
# own display (QUILL_EDITING_DISPLAY, default :174) and a WINEPREFIX of its own under the cache directory. Local only: Wine paces it.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
SHOTS=""
if [ "${1:-}" = "--shots" ]; then SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"; fi
IM=convert
command -v magick > /dev/null && IM="magick"
for t in wine Xvfb xdotool import compare; do
	command -v "$t" > /dev/null || { echo "wine-editing: $t not found; skipping"; exit 0; }
done
[ -f "$HERE/dist/QUILL32.EXE" ] || { echo "wine-editing: build win9x/dist/QUILL32.EXE first" >&2; exit 1; }
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-editing.XXXXXX")"
export DISPLAY="${QUILL_EDITING_DISPLAY:-:174}" WINEDEBUG=-all
export WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-editing}"
unset WINEARCH
XPID=""
WPID=""
cleanup() {
	[ -z "$WPID" ] || kill "$WPID" 2> /dev/null || true
	wineserver -k 2> /dev/null || true
	[ -z "$XPID" ] || kill "$XPID" 2> /dev/null || true
	rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "wine-editing: FAIL: $*" >&2; exit 1; }
shot() {
	import -window root "$WORK/$1.png"
	if [ -n "$SHOTS" ]; then cp "$WORK/$1.png" "$SHOTS/editing-$1.png"; fi
}
# The colour of one pixel of a screenshot, as hex.
px() { $IM "$WORK/$1.png" -format "%[hex:p{$2,$3}]" info:; }
# How many pixels of one colour a rectangle of a screenshot holds: SHOT COLOUR X Y W H.
count() { $IM "$WORK/$1.png" -crop "$5x$6+$3+$4" +repage -fill '#FF00FE' -opaque "#$2" -fill black +opaque '#FF00FE' -fill white -opaque '#FF00FE' -format '%[fx:round(mean*w*h)]' info:; }
# How many pixels differ between two screenshots.
differ() { compare -metric AE "$WORK/$1.png" "$WORK/$2.png" null: 2>&1 | cut -d' ' -f1 | cut -d. -f1 || true; }

[ -d "$WINEPREFIX/drive_c" ] || WINEDEBUG=-all wineboot --init > /dev/null 2>&1 || true
Xvfb "$DISPLAY" -screen 0 640x480x24 > /dev/null 2>&1 &
XPID=$!
sleep 2
cp "$HERE/dist/QUILL32.EXE" "$WORK/QUILL32.EXE"
(cd "$WORK" && wine QUILL32.EXE > /dev/null 2>&1) &
WPID=$!
for _ in $(seq 1 40); do
	sleep 2
	if xdotool search --name Quill > /dev/null 2>&1; then break; fi
done
sleep 5
xdotool mousemove 300 250 click 1
sleep 1

# 1. Page width: the white page is centred in the client area left of the scroll bar.
shot start
left=0
right=0
for x in $(seq 0 630); do
	if [ "$(px start "$x" 300)" = "FFFFFF" ]; then
		if [ "$left" -eq 0 ]; then left=$x; fi
		right=$x
	fi
done
echo "wine-editing: the page runs from x=$left to x=$right on a 640 pixel screen"
[ "$left" -ge 4 ] && [ "$left" -le 30 ] || fail "the page does not start near the left edge ($left)"
d=$((left - (622 - right)))
[ "${d#-}" -le 6 ] || fail "the page is not centred: $left pixels on the left, $((622 - right)) on the right"

# 1b. The caret is where the layout puts the end of the text, which is where the glyphs end: a zoomed font is hinted to whole pixels and
# drifts from the layout's widths unless each character is drawn at its layout position. With the caret at the start, the text's
# right-most ink is at R; with it at the end, the caret (the right-most ink) is at C, and C - R is the side bearing of the last letter.
xdotool type --delay 40 "This is  Quill, a simple Rust based rich text editor"
xdotool key Home
sleep 1
shot caret_home
xdotool key End
sleep 1
shot caret_end
edge() { for x in $(seq 600 -1 120); do if [ "$(count "$1" 000000 "$x" 195 1 30)" -gt 0 ]; then echo "$x"; return; fi; done; echo 0; }
r=$(edge caret_home)
c=$(edge caret_end)
echo "wine-editing: the text ends at x=$r and the caret is at x=$c"
[ "$r" -gt 120 ] && [ "$((c - r))" -ge 0 ] && [ "$((c - r))" -le 8 ] || fail "the caret is not at the end of the text (text $r, caret $c)"
xdotool key ctrl+a BackSpace

# 2. Ctrl+I: Italic switches on (its button shows pressed, light grey) and no tab is typed.
xdotool type --delay 80 "abc"
xdotool key ctrl+a ctrl+i
sleep 1
shot italic
[ "$(px italic 343 98)" = "DFDFDF" ] || fail "Ctrl+I did not switch Italic on (button pixel $(px italic 343 98))"

# 3. A drag over text highlights it.
xdotool mousemove 103 211 mousedown 1 mousemove 118 211 mouseup 1
sleep 1
shot drag
sel=$(count drag 000080 90 195 90 30)
[ "$sel" -gt 60 ] || fail "dragging over the text selected too little ($sel navy pixels)"

# 4. Wheel: with the text long enough, three notches move the page.
xdotool key ctrl+End
for _ in $(seq 1 60); do xdotool key Return; xdotool type --delay 5 "line of text"; done
xdotool key ctrl+Home
sleep 1
shot top
for _ in 1 2 3; do xdotool click 5; done
sleep 1
shot wheeled
[ "$(differ top wheeled)" -gt 500 ] || fail "the mouse wheel did not scroll the page"

# 5. Print preview and back with Esc: the window is as it was; a tooltip shows over the preview's Next Page button.
xdotool mousemove 330 300
sleep 1
shot before
xdotool mousemove 115 62 click 1
sleep 2
shot preview
[ "$(differ before preview)" -gt 5000 ] || fail "print preview did not show"
xdotool mousemove 130 64
sleep 0.3
xdotool mousemove 131 65
sleep 2
shot tip
xdotool mousemove 330 300
sleep 1.5
shot notip
[ "$(differ tip notip)" -gt 300 ] || fail "no tooltip over the preview's Next Page button"
xdotool key Escape
xdotool mousemove 330 300
sleep 2
shot after
n=$(differ before after)
[ "$n" -le 40 ] || fail "closing the preview changed the window ($n pixels differ)"
echo "wine-editing: ok"
