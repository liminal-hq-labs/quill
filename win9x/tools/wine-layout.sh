#!/usr/bin/env bash
# Starts QUILL32.EXE under Wine on Xvfb screens of several sizes and checks that its window fits and that typing paints through one off-screen copy.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-layout.sh [--shots DIR]     (build first with win9x/build.sh)
#
# For each of 640x480, 800x600, 1024x768 and 1280x1024 it checks that the window is no larger than the screen, then types twenty
# characters with a Wine call trace on and checks, per key, that
#   - nothing is drawn straight onto the window's device context except a single BitBlt,
#   - the whole client area is not invalidated (the toolbars and the ruler are left alone),
#   - the system never asks for the background to be erased.
# Needs wine, Xvfb, xdotool and ImageMagick's import (for --shots); exits 0 with a note when they are missing. Uses its own Xvfb
# display (QUILL_LAYOUT_DISPLAY, default :173) and a WINEPREFIX of its own under the cache directory.
set -euo pipefail
# The caret stays solid so that screenshots are stable (`[Quill] Blink`, win9x/tools/wine-blink.sh).
export QUILL_NO_BLINK=1
HERE="$(cd "$(dirname "$0")/.." && pwd)"
SHOTS=""
[ "${1:-}" = "--shots" ] && SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"
for t in wine Xvfb xdotool python3; do
	command -v "$t" > /dev/null || { echo "wine-layout: $t not found; skipping"; exit 0; }
done
[ -f "$HERE/dist/QUILL32.EXE" ] || { echo "wine-layout: build win9x/dist/QUILL32.EXE first" >&2; exit 1; }
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-layout.XXXXXX")"
DISPLAY_NUM="${QUILL_LAYOUT_DISPLAY:-:173}"
XPID=""
WPID=""
cleanup() {
	[ -z "$WPID" ] || kill "$WPID" 2> /dev/null || true
	WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-layout}" wineserver -k 2> /dev/null || true
	[ -z "$XPID" ] || kill "$XPID" 2> /dev/null || true
	rm -rf "$WORK"
}
[ -z "${KEEP_LOGS:-}" ] || cleanup() { echo "logs in $WORK"; }
trap cleanup EXIT
export WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-layout}" DISPLAY="$DISPLAY_NUM"
unset WINEARCH
fail() { echo "wine-layout: FAIL: $*" >&2; exit 1; }

# Counts, from a Wine call trace, what typing did: prints "keys full-invalidations erases direct-draws blits".
analyse() {
	python3 -I - "$1" << 'PY'
import re, sys
keys = full = erase = direct = blits = 0
started = False
paint_dc = None
draws = ("NtGdiPatBlt", "NtGdiExtTextOutW", "NtGdiExtTextOutA", "NtGdiSetDIBitsToDeviceInternal", "NtGdiPolyPolyDraw", "NtGdiLineTo", "NtGdiStretchBlt")
for line in open(sys.argv[1], errors="replace"):
	if "Call window proc" in line and "msg=WM_CHAR" in line:
		keys += 1
		started = True
	if not started:
		continue
	if "Call window proc" in line and "msg=WM_ERASEBKGND" in line:
		erase += 1
	m = re.search(r"Call win32u\.NtUserInvalidateRect\((\w+),(\w+),", line)
	if m and int(m.group(2), 16) == 0:
		full += 1
	m = re.search(r"Ret  win32u\.NtUserBeginPaint\(\) retval=(\w+)", line)
	if m:
		paint_dc = m.group(1)
	m = re.search(r"Call win32u\.(NtGdi\w+)\((\w+)", line)
	if m and paint_dc and m.group(2) == paint_dc:
		if m.group(1) == "NtGdiBitBlt":
			blits += 1
		elif m.group(1) in draws:
			direct += 1
print(keys, full, erase, direct, blits)
PY
}

# The prefix is made without the call trace, which would make its set-up take minutes.
[ -d "$WINEPREFIX/drive_c" ] || WINEDEBUG=-all wineboot --init > /dev/null 2>&1 || true

for res in 640x480 800x600 1024x768 1280x1024; do
	w="${res%x*}"
	h="${res#*x}"
	Xvfb "$DISPLAY_NUM" -screen 0 "${res}x24" > /dev/null 2>&1 &
	XPID=$!
	sleep 2
	kill -0 "$XPID" 2> /dev/null || fail "$res: Xvfb did not start on $DISPLAY_NUM (is the display in use?)"
	mkdir -p "$WORK/$res"
	cp "$HERE/dist/QUILL32.EXE" "$WORK/$res/QUILL32.EXE"
	(cd "$WORK/$res" && WINEDEBUG=+relay,-all,+relay wine QUILL32.EXE > /dev/null 2> >(grep -aE 'win32u\.(NtGdi|NtUserInvalidateRect|NtUserBeginPaint)|window proc' > "$WORK/$res.log")) &
	WPID=$!
	id=""
	for _ in $(seq 1 60); do
		sleep 2
		id="$(xdotool search --name Quill 2> /dev/null | head -1 || true)"
		[ -z "$id" ] || break
	done
	[ -n "$id" ] || fail "$res: no Quill window"
	sleep 5
	geo="$(xdotool getwindowgeometry "$id" | sed -n 's/.*Geometry: //p')"
	gw="${geo%x*}"
	gh="${geo#*x}"
	[ "$gw" -le "$w" ] && [ "$gh" -le "$h" ] || fail "$res: the window is $geo, larger than the screen"
	echo "wine-layout: $res: the window is $geo, inside the screen"
	# No window manager runs on the display, so give the window the keyboard focus by hand and by a click on the page.
	xdotool windowfocus "$id" 2> /dev/null || true
	xdotool mousemove $((w / 2)) $((h / 2)) click 1
	sleep 1
	xdotool type --delay 150 "abcdefghijklmnopqrst"
	sleep 2
	[ -z "$SHOTS" ] || import -window root "$SHOTS/layout-$res.png" 2> /dev/null || echo "wine-layout: could not save the screenshot for $res"
	kill "$WPID" 2> /dev/null || true
	wineserver -k 2> /dev/null || true
	WPID=""
	sleep 2
	kill "$XPID" 2> /dev/null || true
	wait "$XPID" 2> /dev/null || true
	XPID=""
	read -r keys full erase direct blits <<< "$(analyse "$WORK/$res.log")"
	[ "$keys" -ge 20 ] || fail "$res: only $keys key messages reached the window"
	[ "$full" -eq 0 ] || fail "$res: $full full-window invalidations for $keys keys"
	[ "$erase" -eq 0 ] || fail "$res: $erase background erases for $keys keys"
	[ "$direct" -eq 0 ] || fail "$res: $direct draw calls straight onto the window for $keys keys"
	[ "$blits" -ge "$keys" ] || fail "$res: $blits copies for $keys keys"
	echo "wine-layout: $res: $keys keys: $full full invalidations, $erase erases, $direct direct draws, $blits copies"
done
echo "wine-layout: ok"
