#!/usr/bin/env bash
# Drives QUILL32.EXE under Wine on a 640 by 480 Xvfb screen and checks the zoom box at the right end of the status bar.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-zoom.sh [--shots DIR]     (build first with win9x/build.sh)
#
# Checks that the box shows the zoom the view starts with; that a click on it opens the list of levels and Escape closes it without a
# change; that choosing 75% from the list (by the keys) changes the page, the box and `[Quill] Zoom` in `QUILL.INI`; that choosing
# Page Width puts the first back; that a click on the status bar anywhere else opens nothing; and that a screen too narrow for the
# box leaves it out, so a click at the right end opens nothing. Needs wine, Xvfb, xdotool and ImageMagick; exits 0 with a note when
# they are missing. Uses its own display (QUILL_ZOOM_DISPLAY, default :181) and a WINEPREFIX of its own under the cache directory.
# Local only: Wine paces it.
set -euo pipefail
# The caret stays solid so that screenshots are stable (`[Quill] Blink`, win9x/tools/wine-blink.sh).
export QUILL_NO_BLINK=1
HERE="$(cd "$(dirname "$0")/.." && pwd)"
SHOTS=""
if [ "${1:-}" = "--shots" ]; then SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"; fi
IM=convert
command -v magick > /dev/null && IM="magick"
for t in wine Xvfb xdotool import compare; do
	command -v "$t" > /dev/null || { echo "wine-zoom: $t not found; skipping"; exit 0; }
done
[ -f "$HERE/dist/QUILL32.EXE" ] || { echo "wine-zoom: build win9x/dist/QUILL32.EXE first" >&2; exit 1; }
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-zoom.XXXXXX")"
export DISPLAY="${QUILL_ZOOM_DISPLAY:-:181}" WINEDEBUG=-all
export WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-zoom}"
unset WINEARCH
INI="$WINEPREFIX/drive_c/windows/QUILL.INI"
XPID=""
WPID=""
cleanup() {
	[ -z "$WPID" ] || kill "$WPID" 2> /dev/null || true
	wineserver -k 2> /dev/null || true
	[ -z "$XPID" ] || kill "$XPID" 2> /dev/null || true
	rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "wine-zoom: FAIL: $*" >&2; exit 1; }
shot() {
	import -window root "$WORK/$1.png"
	if [ -n "$SHOTS" ]; then cp "$WORK/$1.png" "$SHOTS/quill-zoom-win32-$1.png"; fi
}
# How many pixels differ between two screenshots.
differ() { compare -metric AE "$WORK/$1.png" "$WORK/$2.png" null: 2>&1 | cut -d' ' -f1 | cut -d. -f1 || true; }
# How many pixels differ inside a box of two screenshots: A B W H X Y.
differ_box() {
	$IM "$WORK/$1.png" -crop "$3x$4+$5+$6" +repage "$WORK/ca.png"
	$IM "$WORK/$2.png" -crop "$3x$4+$5+$6" +repage "$WORK/cb.png"
	compare -metric AE "$WORK/ca.png" "$WORK/cb.png" null: 2>&1 | cut -d' ' -f1 | cut -d. -f1 || true
}
# The Zoom value in QUILL.INI, or "none".
ini_zoom() {
	[ -f "$INI" ] || return 0
	tr -d '\r' < "$INI" | sed -n 's/^Zoom=//p' | head -1 || true
}

[ -d "$WINEPREFIX/drive_c" ] || WINEDEBUG=-all wineboot --init > /dev/null 2>&1 || true
Xvfb "$DISPLAY" -screen 0 640x480x24 > /dev/null 2>&1 &
XPID=$!
sleep 2
rm -f "$INI"
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
# The box is the last 88 pixels of the status bar, which is the bottom 18 pixels of the 640 by 480 screen.
BOX_X=590
BOX_Y=466
# The list opens above the box, over the page's right edge.
LIST="120 150 520 330"

shot start
echo "wine-zoom: started; QUILL.INI has no Zoom: $(ini_zoom)"

# 1. A click on the box opens the list; Escape closes it and nothing changes.
xdotool mousemove "$BOX_X" "$BOX_Y" click 1
sleep 1
shot open
read -r lw lh lx ly <<< "$LIST"
[ "$(differ_box start open "$lw" "$lh" "$lx" "$ly")" -gt 500 ] || fail "a click on the zoom box opened no list"
xdotool key Escape
sleep 1
shot escaped
[ "$(differ start escaped)" -eq 0 ] || fail "Escape left the window changed ($(differ start escaped) pixels)"
[ "$(ini_zoom)" = "" ] || fail "Escape changed the zoom"
echo "wine-zoom: a click opens the list; Escape closes it with no change"

# 2. 75%: the sixth entry (Page Width, 200, 150, 125, 100, 75).
xdotool mousemove "$BOX_X" "$BOX_Y" click 1
sleep 1
for _ in 1 2 3 4 5 6; do xdotool key Down; done
sleep 1
shot highlighted
xdotool key Return
sleep 2
shot z75
[ "$(ini_zoom)" = "75" ] || fail "choosing 75% left Zoom=$(ini_zoom) in QUILL.INI"
[ "$(differ_box start z75 600 300 10 130)" -gt 500 ] || fail "the page did not change at 75%"
[ "$(differ_box start z75 88 14 548 458)" -gt 50 ] || fail "the box's text did not change"
echo "wine-zoom: choosing 75% sets Zoom=75, redraws the page and the box"

# 3. The list ticks the current level: Down once and Return on the first entry goes back to Page Width.
xdotool mousemove "$BOX_X" "$BOX_Y" click 1
sleep 1
shot open75
xdotool key Down
xdotool key Return
sleep 2
shot width
[ "$(ini_zoom)" = "Width" ] || fail "choosing Page Width left Zoom=$(ini_zoom)"
[ "$(differ start width)" -eq 0 ] || fail "Page Width does not look like the start ($(differ start width) pixels)"
echo "wine-zoom: Page Width brings the first window back"

# 4. A click elsewhere on the status bar opens nothing.
xdotool mousemove 60 466 click 1
sleep 1
shot elsewhere
[ "$(differ start elsewhere)" -eq 0 ] || fail "a click on the status bar's message pane changed the window"

# 5. A screen too narrow for the box leaves it out: the window is as wide as the screen, 520 pixels, and a click at its right end opens
# no list.
kill "$WPID" 2> /dev/null || true
WPID=""
wineserver -k 2> /dev/null || true
kill "$XPID" 2> /dev/null || true
wait "$XPID" 2> /dev/null || true
Xvfb "$DISPLAY" -screen 0 520x400x24 > /dev/null 2>&1 &
XPID=$!
sleep 2
rm -f "$INI"
(cd "$WORK" && wine QUILL32.EXE > /dev/null 2>&1) &
WPID=$!
for _ in $(seq 1 40); do
	sleep 2
	id="$(xdotool search --name Quill 2> /dev/null | head -1 || true)"
	[ -z "$id" ] || break
done
sleep 5
xdotool windowfocus "$id" 2> /dev/null || true
shot narrow
xdotool mousemove 490 386 click 1
sleep 1
shot narrow-click
[ "$(differ narrow narrow-click)" -eq 0 ] || fail "the narrow window opened a list ($(differ narrow narrow-click) pixels)"
echo "wine-zoom: a window too narrow for the box leaves it out"
echo "wine-zoom: ok"
