#!/usr/bin/env bash
# Drives QUILL32.EXE under Wine on a 640 by 480 Xvfb screen and checks that the caret blinks like the system's, and only when asked to.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-blink.sh [--shots DIR]     (build first with win9x/build.sh)
#
# Three runs, each a fresh start with "Hello" typed into the empty page:
#   1. `QUILL_NO_BLINK=1`: the caret is steady, and screenshots 0.3 s apart are identical (this is how the other Wine scripts run);
#   2. `[Quill] Blink=0` in QUILL.INI, no environment switch: the same;
#   3. blinking on: screenshots over about two seconds hold exactly two pictures, the steady one and one without the caret, which
#      differ only inside a few pixels beside the end of the text; a key puts the caret back on at once; and while a prompt is open
#      the caret is away and the window holds still.
# Needs wine, Xvfb, xdotool and ImageMagick; exits 0 with a note when they are missing. Uses its own display (QUILL_BLINK_DISPLAY,
# default :178) and a WINEPREFIX of its own under the cache directory. Local only: Wine paces it.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
SHOTS=""
if [ "${1:-}" = "--shots" ]; then SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"; fi
IM=convert
command -v magick > /dev/null && IM="magick"
for t in wine Xvfb xdotool import compare; do
	command -v "$t" > /dev/null || { echo "wine-blink: $t not found; skipping"; exit 0; }
done
[ -f "$HERE/dist/QUILL32.EXE" ] || { echo "wine-blink: build win9x/dist/QUILL32.EXE first" >&2; exit 1; }
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-blink.XXXXXX")"
export DISPLAY="${QUILL_BLINK_DISPLAY:-:178}" WINEDEBUG=-all
export WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-blink}"
unset WINEARCH
XPID=""
WPID=""
stop_quill() {
	[ -z "$WPID" ] || kill "$WPID" 2> /dev/null || true
	WPID=""
	wineserver -k 2> /dev/null || true
	sleep 1
}
cleanup() {
	stop_quill
	[ -z "$XPID" ] || kill "$XPID" 2> /dev/null || true
	rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "wine-blink: FAIL: $*" >&2; exit 1; }
shot() {
	import -window root "$WORK/$1.png"
	if [ -n "$SHOTS" ]; then cp "$WORK/$1.png" "$SHOTS/blink-$1.png"; fi
}
# The picture's checksum, so equal screenshots have equal names.
sum() { $IM "$WORK/$1.png" -format '%#' info:; }
# How many pixels differ between two screenshots.
differ() { compare -metric AE "$WORK/$1.png" "$WORK/$2.png" null: 2>&1 | cut -d' ' -f1 | cut -d. -f1 || true; }
# The box the difference between two screenshots lies in, as "W H X Y".
box() { $IM "$WORK/$1.png" "$WORK/$2.png" -compose difference -composite -threshold 0 -trim -format '%w %h %[fx:page.x] %[fx:page.y]' info:; }
# The checksum of a box of a screenshot: SHOT W H X Y.
bsum() { $IM "$WORK/$1.png" -crop "$2x$3+$4+$5" +repage -format '%#' info:; }

[ -d "$WINEPREFIX/drive_c" ] || WINEDEBUG=-all wineboot --init > /dev/null 2>&1 || true
Xvfb "$DISPLAY" -screen 0 640x480x24 > /dev/null 2>&1 &
XPID=$!
sleep 2

# Starts a fresh QUILL32.EXE with the environment given, clicks the page and types.
start() {
	cp "$HERE/dist/QUILL32.EXE" "$WORK/QUILL32.EXE"
	(cd "$WORK" && env "$@" wine QUILL32.EXE > /dev/null 2>&1) &
	WPID=$!
	# Until the window is on the screen (the first start in a new prefix takes a while).
	for _ in $(seq 1 60); do
		sleep 2
		if xdotool search --name Quill > /dev/null 2>&1; then
			import -window root "$WORK/up.png"
			[ "$($IM "$WORK/up.png" -format '%[fx:mean>0.2?1:0]' info:)" = 1 ] && break
		fi
	done
	sleep 3
	xdotool mousemove 300 250 click 1
	sleep 1
	xdotool type --delay 80 Hello
	sleep 1
}

# 1. The environment switch: steady.
rm -f "$WINEPREFIX/drive_c/windows/QUILL.INI"
start QUILL_NO_BLINK=1
for i in 1 2 3 4 5 6; do
	shot "env$i"
	sleep 0.3
done
for i in 2 3 4 5 6; do
	[ "$(sum env1)" = "$(sum env$i)" ] || fail "QUILL_NO_BLINK=1: the window changed between screenshots ($(differ env1 env$i) pixels)"
done
stop_quill
echo "wine-blink: QUILL_NO_BLINK=1 keeps the window byte-stable"

# 2. The setting: steady, and the same picture as the environment switch.
printf '[Quill]\r\nBlink=0\r\n' > "$WINEPREFIX/drive_c/windows/QUILL.INI"
start QUILL_X=1
for i in 1 2 3 4 5 6; do
	shot "ini$i"
	sleep 0.3
done
for i in 1 2 3 4 5 6; do
	[ "$(sum env1)" = "$(sum ini$i)" ] || fail "Blink=0: the window differs from the steady one ($(differ env1 ini$i) pixels)"
done
stop_quill
rm -f "$WINEPREFIX/drive_c/windows/QUILL.INI"
echo "wine-blink: [Quill] Blink=0 keeps the window byte-stable"

# 3. Blinking: the system's blink time (Wine's is 500 ms), so a screenshot every 0.15 s over two seconds sees both phases.
start QUILL_X=1
steady="$(sum env1)"
off=""
for i in $(seq 1 14); do
	shot "b$i"
	s="$(sum "b$i")"
	if [ "$s" != "$steady" ]; then
		[ -z "$off" ] && off="$i"
		[ "$s" = "$(sum "b$off")" ] || fail "blinking: a third picture appeared in the window"
	fi
	sleep 0.15
done
[ -n "$off" ] || fail "blinking: the caret never went off"
on=0
for i in $(seq 1 14); do [ "$(sum "b$i")" = "$steady" ] && on=1; done
[ "$on" = 1 ] || fail "blinking: the caret never showed (no screenshot matches the steady window)"
b="$(box env1 "b$off")"
read -r w h x y <<< "$b"
echo "wine-blink: the phases differ in a box ${w}x${h} at $x,$y ($(differ env1 "b$off") pixels)"
[ "$w" -le 5 ] || fail "blinking: the caret's phases differ over $w pixels of width, more than the caret"
[ "$(differ env1 "b$off")" -le 80 ] || fail "blinking: more than the caret changed between phases"

# A key puts the caret back on at once, whatever phase the timer was in.
for _ in $(seq 1 20); do
	shot probe
	[ "$(sum probe)" != "$steady" ] && break
	sleep 0.1
done
[ "$(sum probe)" != "$steady" ] || fail "blinking: could not catch the caret off"
xdotool key End
shot key
[ "$(sum key)" = "$steady" ] || fail "blinking: the caret was not solid straight after a key"
echo "wine-blink: a key makes the caret solid"


# A dialog takes the keyboard focus, and with it the caret: the pixels where the caret was match the page without it, and stay so.
xdotool key ctrl+o
sleep 3
shot dlg1
sleep 1.2
shot dlg2
[ "$(bsum dlg1 "$w" "$h" "$x" "$y")" = "$(bsum "b$off" "$w" "$h" "$x" "$y")" ] || fail "a dialog is open and the caret still shows"
[ "$(sum dlg1)" = "$(sum dlg2)" ] || fail "a dialog is open and the window changed between screenshots"
echo "wine-blink: ok"
