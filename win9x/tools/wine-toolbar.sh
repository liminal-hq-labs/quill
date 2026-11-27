#!/usr/bin/env bash
# Drives QUILL32.EXE under Wine on a 640 by 480 Xvfb screen and checks that a toolbar glyph is drawn on its button's own base colour (W9X-83).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-toolbar.sh [--shots DIR]     (build first with win9x/build.sh)
#
# A switched-on toggle sits on the scheme's light colour and every other button on its face colour. The script switches Bold on with a
# click and checks that the 16 by 15 cell of its glyph holds the light colour and no pixel of the face colour; then, with the pointer
# held down on Underline, which is not on, that the cell of its glyph holds the face colour and none of the light colour; then that the
# cell of Italic, which is off and flat, holds the face colour. Needs wine, Xvfb, xdotool and ImageMagick; exits 0 with a note when
# they are missing. Uses its own display (QUILL_TOOLBAR_DISPLAY, default :197) and a WINEPREFIX of its own under the cache directory.
# Local only: Wine paces it.
set -euo pipefail
# The caret stays solid so that screenshots are stable (`[Quill] Blink`, win9x/tools/wine-blink.sh).
export QUILL_NO_BLINK=1
HERE="$(cd "$(dirname "$0")/.." && pwd)"
SHOTS=""
if [ "${1:-}" = "--shots" ]; then SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"; fi
IM=convert
command -v magick > /dev/null && IM="magick"
for t in wine Xvfb xdotool import; do
	command -v "$t" > /dev/null || { echo "wine-toolbar: $t not found; skipping"; exit 0; }
done
command -v "$IM" > /dev/null || { echo "wine-toolbar: ImageMagick not found; skipping"; exit 0; }
[ -f "$HERE/dist/QUILL32.EXE" ] || { echo "wine-toolbar: build win9x/dist/QUILL32.EXE first" >&2; exit 1; }
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-toolbar.XXXXXX")"
export DISPLAY="${QUILL_TOOLBAR_DISPLAY:-:197}" WINEDEBUG=-all
export WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-toolbar}"
unset WINEARCH WAYLAND_DISPLAY
XPID=""
WPID=""
cleanup() {
	[ -z "$WPID" ] || kill "$WPID" 2> /dev/null || true
	wineserver -k 2> /dev/null || true
	[ -z "$XPID" ] || kill "$XPID" 2> /dev/null || true
	rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "wine-toolbar: FAIL: $*" >&2; exit 1; }
shot() {
	import -window root "$WORK/$1.png"
	if [ -n "$SHOTS" ]; then cp "$WORK/$1.png" "$SHOTS/quill-toolbar-win32-$1.png"; fi
}
# The colour of one pixel of a screenshot, as #RRGGBB: NAME X Y.
colour() { $IM "$WORK/$1.png" -crop "1x1+$2+$3" +repage txt:- | tail -1 | grep -o '#[0-9A-Fa-f]\{6\}' | head -1 | tr 'a-f' 'A-F'; }
# How many pixels of the 16 by 15 cell at X Y hold a colour: NAME X Y #RRGGBB.
count() { $IM "$WORK/$1.png" -crop "16x15+$2+$3" +repage txt:- | grep -c "$4" || true; }

[ -d "$WINEPREFIX/drive_c" ] || wineboot --init > /dev/null 2>&1 || true
Xvfb "$DISPLAY" -screen 0 640x480x24 > /dev/null 2>&1 &
XPID=$!
sleep 2
cp "$HERE/dist/QUILL32.EXE" "$WORK/QUILL32.EXE"
(cd "$WORK" && wine QUILL32.EXE > /dev/null 2>&1) &
WPID=$!
id=""
for _ in $(seq 1 40); do
	sleep 2
	id="$(xdotool search --name Quill 2> /dev/null | head -1 || true)"
	[ -z "$id" ] || break
done
[ -n "$id" ] || fail "no Quill window"
sleep 5
xdotool windowfocus "$id" 2> /dev/null || true

# At 640 by 480 the window sits at the top left under a 30-pixel gap; the format row's buttons are 24 by 24 from y 79: Bold at x 316,
# Italic at 340 and Underline at 364 (a glyph is drawn 4 pixels in and 4 down, and one more of each when the button is down).
BOLD_X=316
ITALIC_X=340
UNDERLINE_X=364
BTN_Y=79
CELL_Y=$((BTN_Y + 5))
AWAY_X=500
AWAY_Y=300

xdotool mousemove "$AWAY_X" "$AWAY_Y"
sleep 1
shot start
FACE="$(colour start 300 95)"
echo "wine-toolbar: the toolbar's face is $FACE"

# 1. Bold on: the cell is the light colour with no face-coloured pixel.
xdotool mousemove $((BOLD_X + 12)) $((BTN_Y + 12)) click 1
sleep 1
xdotool mousemove "$AWAY_X" "$AWAY_Y"
sleep 1
shot bold-on
LIT="$(colour bold-on $((BOLD_X + 2)) $((BTN_Y + 16)))"
[ "$LIT" != "$FACE" ] || fail "a switched-on Bold has the face colour $FACE as its base"
squares="$(count bold-on $((BOLD_X + 5)) "$CELL_Y" "$FACE")"
[ "$squares" -eq 0 ] || fail "Bold's glyph cell holds $squares pixels of the face colour $FACE on a base of $LIT"
[ "$(count bold-on $((BOLD_X + 5)) "$CELL_Y" "$LIT")" -gt 100 ] || fail "Bold's glyph cell does not show its base colour $LIT"
echo "wine-toolbar: Bold on: the glyph's cell is $LIT with no pixel of $FACE"

# 2. Underline held down but not on: its base is the face colour.
xdotool mousemove $((UNDERLINE_X + 12)) $((BTN_Y + 12)) mousedown 1
sleep 1
shot underline-down
[ "$(count underline-down $((UNDERLINE_X + 5)) "$CELL_Y" "$FACE")" -gt 100 ] || fail "a pressed Underline's cell is not the face colour"
[ "$(count underline-down $((UNDERLINE_X + 5)) "$CELL_Y" "$LIT")" -eq 0 ] || fail "a pressed Underline's cell holds the light colour"
xdotool mouseup 1
xdotool mousemove "$AWAY_X" "$AWAY_Y"
sleep 1
echo "wine-toolbar: Underline held down: the glyph's cell is the face colour"

# 3. Italic off and flat: the face colour.
[ "$(count bold-on $((ITALIC_X + 4)) "$CELL_Y" "$FACE")" -gt 100 ] || fail "Italic's cell is not the face colour"
echo "wine-toolbar: Italic off: the glyph's cell is the face colour"
echo "wine-toolbar: OK"
