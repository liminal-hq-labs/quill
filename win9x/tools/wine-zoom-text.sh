#!/usr/bin/env bash
# Shows tests/docs/zoomtext.rtf in QUILL32.EXE under Wine at 50, 75, 100, 125, 150 and 200 percent, checks that the words are as wide as
# the 100 percent words times the zoom, and compares the text block with the golden frames.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-zoom-text.sh [--shots DIR] [--update]     (build first with win9x/build.sh)
#
# For each zoom it writes `[Quill] Zoom=N` and `Blink=0` to QUILL.INI, starts QUILL32.EXE on the document, moves the caret to the empty
# last paragraph (so it does not sit on the first word) and takes the screen. Then `tools/zoom_text.py frames` runs the measured check
# (the same on every platform), and each frame's text block is compared with `tests/golden/zoom/win32-wine-zNNN.png` (ImageMagick
# `compare -metric AE -fuzz 10%`, at most 0.5 % of the crop's pixels). The golden frames are Wine's own drawing, so they are compared only
# when `wine --version` equals the version recorded in `tests/golden/zoom/WINE_VERSION`; otherwise only the measured check runs.
#   --shots DIR  also writes the whole-screen frames (`win32-wine-page-zNNN.png`) and their 3x text crops (`...-crop3x.png`) there
#   --update     rewrites the golden frames and WINE_VERSION from this run
# Needs wine, Xvfb, xdotool, ImageMagick and Python with Pillow; exits 0 with a note when they are missing. Uses its own display
# (QUILL_ZOOM_TEXT_DISPLAY, default :183, a 900 by 700 screen) and a WINEPREFIX of its own under the cache directory. Local only.
set -euo pipefail
# The caret stays solid so that screenshots are stable (`[Quill] Blink`, win9x/tools/wine-blink.sh).
export QUILL_NO_BLINK=1
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
SHOTS=""
UPDATE=0
while [ $# -gt 0 ]; do
	case "$1" in
		--shots) SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"; shift 2 ;;
		--update) UPDATE=1; shift ;;
		*) echo "wine-zoom-text: unknown argument $1" >&2; exit 2 ;;
	esac
done
for t in wine Xvfb xdotool import compare identify; do
	command -v "$t" > /dev/null || { echo "wine-zoom-text: $t not found; skipping"; exit 0; }
done
python3 -I -c "import PIL" 2> /dev/null || { echo "wine-zoom-text: Pillow not found; skipping"; exit 0; }
EXE="${QUILL32_EXE:-$HERE/dist/QUILL32.EXE}"
[ -f "$EXE" ] || { echo "wine-zoom-text: build win9x/dist/QUILL32.EXE first" >&2; exit 1; }
GOLD="$ROOT/tests/golden/zoom"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-zoom-text.XXXXXX")"
export DISPLAY="${QUILL_ZOOM_TEXT_DISPLAY:-:183}" WINEDEBUG=-all
export WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-zoom-text}"
unset WINEARCH WAYLAND_DISPLAY
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
fail() { echo "wine-zoom-text: FAIL: $*" >&2; exit 1; }

[ -d "$WINEPREFIX/drive_c" ] || wineboot --init > /dev/null 2>&1 || true
Xvfb "$DISPLAY" -screen 0 900x700x24 > /dev/null 2>&1 &
XPID=$!
sleep 2
cp "$EXE" "$WORK/QUILL32.EXE"
cp "$ROOT/tests/docs/zoomtext.rtf" "$WORK/ZOOMTEXT.RTF"
mkdir -p "$WORK/frames"
for z in 50 75 100 125 150 200; do
	printf '[Quill]\r\nZoom=%s\r\nBlink=0\r\n' "$z" > "$INI"
	(cd "$WORK" && wine QUILL32.EXE ZOOMTEXT.RTF > /dev/null 2>&1) &
	WPID=$!
	id=""
	for _ in $(seq 1 40); do
		sleep 2
		id="$(xdotool search --name Quill 2> /dev/null | head -1 || true)"
		[ -z "$id" ] || break
	done
	[ -n "$id" ] || fail "no Quill window at $z%"
	sleep 5
	xdotool windowfocus "$id" 2> /dev/null || true
	xdotool key ctrl+End
	sleep 1
	import -window root "$WORK/frames/z$(printf '%03d' "$z").png"
	kill "$WPID" 2> /dev/null || true
	WPID=""
	wineserver -k 2> /dev/null || true
	sleep 1
done
rm -f "$INI"

for z in 050 075 100 125 150 200; do
	python3 -I "$ROOT/tools/zoom_text.py" crop "$WORK/frames/z$z.png" "$WORK/frames/crop$z.png" || fail "no text in the $z frame"
	if [ -n "$SHOTS" ]; then
		cp "$WORK/frames/z$z.png" "$SHOTS/win32-wine-page-z$z.png"
		python3 -I "$ROOT/tools/zoom_text.py" crop "$WORK/frames/z$z.png" "$SHOTS/win32-wine-page-z$z-crop3x.png" --scale 3
	fi
done

python3 -I "$ROOT/tools/zoom_text.py" frames "$WORK/frames" || fail "a zoomed word is not as wide as the 100% word times the zoom, or two touch"
echo "wine-zoom-text: the words of every zoom are as wide as at 100% times the zoom"

WINE_NOW="$(wine --version)"
if [ "$UPDATE" -eq 1 ]; then
	mkdir -p "$GOLD"
	for z in 050 075 100 125 150 200; do cp "$WORK/frames/crop$z.png" "$GOLD/win32-wine-z$z.png"; done
	printf '%s\n' "$WINE_NOW" > "$GOLD/WINE_VERSION"
	echo "wine-zoom-text: golden frames rewritten for $WINE_NOW"
	exit 0
fi
if [ ! -f "$GOLD/WINE_VERSION" ] || [ "$(cat "$GOLD/WINE_VERSION")" != "$WINE_NOW" ]; then
	echo "wine-zoom-text: golden frames are for $(cat "$GOLD/WINE_VERSION" 2> /dev/null || echo 'no Wine') and this is $WINE_NOW; compared the measured check only"
	echo "wine-zoom-text: ok"
	exit 0
fi
for z in 050 075 100 125 150 200; do
	want="$GOLD/win32-wine-z$z.png"
	[ -f "$want" ] || fail "no golden frame $want"
	[ "$(identify -format '%wx%h' "$want")" = "$(identify -format '%wx%h' "$WORK/frames/crop$z.png")" ] || fail "the $z text block is not the size of its golden frame"
	area="$(identify -format '%[fx:w*h]' "$want")"
	differ="$(compare -metric AE -fuzz 10% "$want" "$WORK/frames/crop$z.png" null: 2>&1 | cut -d' ' -f1 | cut -d. -f1 || true)"
	[ "$differ" -le $((area / 200)) ] || fail "the $z text block differs from its golden frame in $differ of $area pixels"
done
echo "wine-zoom-text: the text blocks match the golden frames"
echo "wine-zoom-text: ok"
