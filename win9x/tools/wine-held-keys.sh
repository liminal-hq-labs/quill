#!/usr/bin/env bash
# Holds keys down for QUILL32.EXE under Wine on a 640 by 480 Xvfb screen: a toggle acts once, text keeps repeating.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-held-keys.sh [--shots DIR]     (build first with win9x/build.sh; QUILL_EXE names another build to try)
#
# The X server repeats a key held with xdotool keydown, and Wine turns each repeat into a WM_KEYDOWN with bit 30 of lParam set ("the key
# was already down"). Ctrl+I held for three seconds must switch Italic on once (its button pressed, FAFAFA, where it was F5F5F5), held again
# it switches off, and a letter held must type many times. With QUILL_EXE set (an older build) the script only reports what it saw.
# Needs wine, Xvfb, xdotool and ImageMagick; exits 0 with a note when they are missing. Own display (QUILL_HELD_DISPLAY, default :176)
# and a WINEPREFIX of its own under the cache directory. Local only: Wine paces it.
set -euo pipefail
export QUILL_NO_BLINK=1
HERE="$(cd "$(dirname "$0")/.." && pwd)"
SHOTS=""
if [ "${1:-}" = "--shots" ]; then SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"; fi
IM=convert
command -v magick > /dev/null && IM="magick"
for t in wine Xvfb xdotool import; do
	command -v "$t" > /dev/null || { echo "wine-held-keys: $t not found; skipping"; exit 0; }
done
EXE="${QUILL_EXE:-$HERE/dist/QUILL32.EXE}"
[ -f "$EXE" ] || { echo "wine-held-keys: build win9x/dist/QUILL32.EXE first" >&2; exit 1; }
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-held.XXXXXX")"
unset WAYLAND_DISPLAY
export DISPLAY="${QUILL_HELD_DISPLAY:-:176}" WINEDEBUG=-all
export WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-held}"
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
fail() { echo "wine-held-keys: FAIL: $*" >&2; exit 1; }
shot() {
	import -window root "$WORK/$1.png"
	if [ -n "$SHOTS" ]; then cp "$WORK/$1.png" "$SHOTS/held-$1.png"; fi
}
px() { $IM "$WORK/$1.png" -format "%[hex:p{$2,$3}]" info:; }
# Dark pixels in the text row of the page.
ink() { $IM "$WORK/$1.png" -crop "500x30+100+195" +repage -colorspace Gray -threshold 30% -negate -format '%[fx:round(mean*w*h)]' info:; }
# The Italic button's colour is sampled while a chord is held; HELD_STATES collects the changes.
HELD_STATES=""
hold() { # <chord> [sample]: keys down in order for about three seconds (sampling the button, when asked), then up in reverse
	local keys i last="" now
	IFS=+ read -ra keys <<< "$1"
	HELD_STATES=""
	for k in "${keys[@]}"; do xdotool keydown "$k"; done
	for _ in $(seq 1 12); do
		if [ -n "${2:-}" ]; then
			import -window root "$WORK/sample.png"
			now=$(px sample 343 98)
			if [ "$now" != "$last" ]; then HELD_STATES="$HELD_STATES $now"; last=$now; fi
		fi
		sleep 0.2
	done
	for ((i = ${#keys[@]} - 1; i >= 0; i--)); do xdotool keyup "${keys[$i]}"; done
	sleep 1
}

[ -d "$WINEPREFIX/drive_c" ] || { WINEDEBUG=-all wineboot --init > /dev/null 2>&1 || true; wineserver -w 2> /dev/null || true; }
Xvfb "$DISPLAY" -screen 0 640x480x24 > /dev/null 2>&1 &
XPID=$!
sleep 2
cp "$EXE" "$WORK/QUILL32.EXE"
(cd "$WORK" && wine QUILL32.EXE > /dev/null 2>&1) &
WPID=$!
for _ in $(seq 1 40); do
	sleep 2
	if xdotool search --name Quill > /dev/null 2>&1; then break; fi
done
sleep 5
xdotool mousemove 300 250 click 1
sleep 1
shot start

xdotool type --delay 80 "abc"
xdotool key ctrl+a
sleep 1
shot selected
before=$(px selected 343 98)
hold ctrl+i sample
changes_on="$HELD_STATES"
shot italic_on
on=$(px italic_on 343 98)
hold ctrl+i sample
changes_off="$HELD_STATES"
shot italic_off
off=$(px italic_off 343 98)
echo "wine-held-keys: the Italic button was $before, then while Ctrl+I was held:$changes_on (ends $on); held again:$changes_off (ends $off)"

xdotool key End
sleep 1
shot text_before
hold z
shot text_after
a=$(ink text_before)
b=$(ink text_after)
echo "wine-held-keys: ink in the text row: $a before and $b after holding Z"

if [ -n "${QUILL_EXE:-}" ]; then exit 0; fi
[ "$before" = F5F5F5 ] && [ "$on" = FAFAFA ] && [ "$off" = F5F5F5 ] || fail "a held Ctrl+I did not act once (button $before, $on, $off)"
[ "$changes_on" = " F5F5F5 FAFAFA" ] || [ "$changes_on" = " FAFAFA" ] || fail "the button changed more than once while Ctrl+I was held:$changes_on"
[ "$b" -gt $((a + 100)) ] || fail "a held letter did not repeat ($a, then $b)"
echo "wine-held-keys: ok"
