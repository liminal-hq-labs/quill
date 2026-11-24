#!/usr/bin/env bash
# Types a misspelling into QUILL32.EXE under Wine and checks, by pixels, that a red wavy underline appears under the word, only under it,
# and goes when the word is corrected from the right-click menu.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-spell-live.sh [--shots DIR]     (build first with win9x/build.sh)
#
# Runs twice, at Page Width and at 100 percent (`[Quill] Zoom` in the Windows directory's QUILL.INI), on a 640 by 480 Xvfb screen of its
# own (QUILL_SPELL_DISPLAY, default :175) and a WINEPREFIX of its own. For each zoom it checks that correct text has no red pixels, that
# a typed misspelling has red pixels only in a short band under the word, that the band is as wide as the word's ink (the wave is drawn
# from the layout's positions), that the right-click menu opens (the page changes) and that choosing the first suggestion takes the
# red away. Needs wine, Xvfb, xdotool and ImageMagick; exits 0 with a note when they are missing. Local only: Wine paces it.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
SHOTS=""
if [ "${1:-}" = "--shots" ]; then SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"; fi
for t in wine Xvfb xdotool import convert; do
	command -v "$t" > /dev/null || { echo "wine-spell-live: $t not found; skipping"; exit 0; }
done
[ -f "$HERE/dist/QUILL32.EXE" ] || { echo "wine-spell-live: build win9x/dist/QUILL32.EXE first" >&2; exit 1; }
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-spell-live.XXXXXX")"
export DISPLAY="${QUILL_SPELL_DISPLAY:-:175}" WINEDEBUG=-all
export WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-spell-live}"
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
fail() { echo "wine-spell-live: FAIL: $*" >&2; exit 1; }
shot() {
	import -window root "$WORK/$1.png"
	if [ -n "$SHOTS" ]; then cp "$WORK/$1.png" "$SHOTS/spell-live-$1.png"; fi
}
# The red pixels of a screenshot's page area (x 10 to 600, y 150 to 420) as "x y" lines.
red() { convert "$WORK/$1.png" -crop 590x270+10+150 +repage txt:- | awk -F'[,: ]+' '/#FF0000/ { print $1 + 10, $2 + 150 }'; }
# The dark pixels (text ink) in a rectangle of a screenshot: SHOT X0 X1 Y0 Y1.
ink() { convert "$WORK/$1.png" -crop "$(($3 - $2 + 1))x$(($5 - $4 + 1))+$2+$4" +repage txt:- 2> /dev/null | awk -F'[(),: ]+' 'NR > 1 && $3 < 90 && $4 < 90 && $5 < 90 { n++ } END { print n + 0 }'; }

(cd "$ROOT" && cargo run -q -p quill-dict -- "$WORK/dic" > /dev/null)
[ -d "$WINEPREFIX/drive_c" ] || WINEDEBUG=-all wineboot --init > /dev/null 2>&1 || true
Xvfb "$DISPLAY" -screen 0 640x480x24 > /dev/null 2>&1 &
XPID=$!
sleep 2
cp "$HERE/dist/QUILL32.EXE" "$WORK/QUILL32.EXE"
cp "$WORK/dic/EN_CA.DIC" "$WORK/EN_CA.DIC"

run_case() {
	local zoom="$1" name="$2"
	printf '[Quill]\r\nZoom=%s\r\n' "$zoom" > "$WINEPREFIX/drive_c/windows/QUILL.INI"
	(cd "$WORK" && wine QUILL32.EXE > /dev/null 2>&1) &
	WPID=$!
	for _ in $(seq 1 40); do
		sleep 2
		if xdotool search --name Quill > /dev/null 2>&1; then break; fi
	done
	sleep 5
	xdotool mousemove 300 300 click 1
	sleep 1
	# Correct text, then a pause long enough for the idle timer and the dictionary to load: no red anywhere.
	xdotool type --delay 40 "The quick brown fox jumped over the lazy dog. "
	sleep 4
	shot "$name-clean"
	local n
	n=$(red "$name-clean" | wc -l)
	[ "$n" -eq 0 ] || fail "$name: $n red pixels under correct text"
	# A misspelling: while the word is being typed, and the caret is on it, nothing is marked.
	xdotool type --delay 40 "Teh"
	sleep 2
	shot "$name-typing"
	n=$(red "$name-typing" | wc -l)
	[ "$n" -eq 0 ] || fail "$name: $n red pixels while the word was being typed"
	# The space ends the word: it is checked and marked.
	xdotool type --delay 40 " quikc"
	sleep 3
	shot "$name-one"
	local one
	one=$(red "$name-one" | wc -l)
	[ "$one" -gt 8 ] || fail "$name: no wavy underline under the finished word ($one red pixels)"
	xdotool type --delay 40 " fox."
	sleep 4
	shot "$name-marked"
	red "$name-marked" > "$WORK/red.txt"
	n=$(wc -l < "$WORK/red.txt")
	[ "$n" -gt "$one" ] || fail "$name: the second word was not marked ($n red pixels, $one before it)"
	# Two words are marked (Teh and quikc), on the same line: two bands, each a few pixels tall.
	local ymin ymax
	ymin=$(awk 'NR == 1 || $2 < m { m = $2 } END { print m }' "$WORK/red.txt")
	ymax=$(awk 'NR == 1 || $2 > m { m = $2 } END { print m }' "$WORK/red.txt")
	echo "wine-spell-live: $name: $n red pixels between y=$ymin and y=$ymax"
	[ "$((ymax - ymin))" -le 5 ] || fail "$name: the wave is $((ymax - ymin + 1)) pixels tall"
	# The wave lies under ink: dark pixels in the rows just above it, over the same columns, for the whole of the wave.
	local xmin xmax
	xmin=$(awk 'NR == 1 || $1 < m { m = $1 } END { print m }' "$WORK/red.txt")
	xmax=$(awk 'NR == 1 || $1 > m { m = $1 } END { print m }' "$WORK/red.txt")
	local above
	above=$(ink "$name-marked" "$xmin" "$xmax" $((ymin - 14)) $((ymin - 1)))
	[ "$above" -gt 20 ] || fail "$name: no text above the wave ($above dark pixels)"
	# Right-click the last marked word (quikc): the menu opens; its first suggestion corrects the word and takes its wave away, and the
	# other word keeps its own.
	local wx=$((xmax - 8))
	xdotool mousemove "$wx" $((ymin - 6)) click 3
	sleep 2
	shot "$name-menu"
	compare -metric AE "$WORK/$name-marked.png" "$WORK/$name-menu.png" null: 2> "$WORK/diff.txt" || true
	local d
	d=$(cut -d' ' -f1 < "$WORK/diff.txt" | cut -d. -f1)
	[ "${d:-0}" -gt 400 ] || fail "$name: the right-click menu did not open ($d pixels differ)"
	xdotool key Down Return
	sleep 3
	shot "$name-fixed"
	local after
	after=$(red "$name-fixed" | awk -v lo="$((xmax - 20))" '$1 >= lo' | wc -l)
	[ "$after" -eq 0 ] || fail "$name: the corrected word still has $after red pixels"
	[ "$(red "$name-fixed" | wc -l)" -ge "$one" ] || fail "$name: the other word lost its wave"
	echo "wine-spell-live: ok at $name"
	kill "$WPID" 2> /dev/null || true
	wineserver -k 2> /dev/null || true
	WPID=""
	sleep 2
}

run_case Width width
run_case 100 zoom100
echo "wine-spell-live: ok"
