#!/usr/bin/env bash
# Shows tests/docs/zoomtext.rtf in QUILL32.EXE under Wine at 50, 75, 100, 125, 150 and 200 percent, checks that the words are as wide as
# the 100 percent words times the zoom, and compares the text block with the golden frames.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-zoom-text.sh [--preview] [--shots DIR] [--update]     (build first with win9x/build.sh)
#
# For each zoom it writes `[Quill] Zoom=N` and `Blink=0` to QUILL.INI, starts QUILL32.EXE on the document, moves the caret to the empty
# last paragraph (so it does not sit on the first word) and takes the screen. Then `tools/zoom_text.py frames` runs the measured check
# (the same on every platform), and each frame's text block is compared with `tests/golden/zoom/win32-wine-zNNN.png` (ImageMagick
# `compare -metric AE -fuzz 10%`, at most 0.5 % of the crop's pixels). The golden frames are Wine's own drawing, so they are compared only
# when `wine --version` equals the version recorded in `tests/golden/zoom/WINE_VERSION`; otherwise only the measured check runs.
#   --preview    checks the print preview instead (see below)
#   --shots DIR  also writes the whole-screen frames (`win32-wine-page-zNNN.png`) and their 3x text crops (`...-crop3x.png`) there
#   --update     rewrites the golden frames and WINE_VERSION from this run
# With --preview the window is moved to the screen's top left corner on a 1280 by 1100 screen, and the page view at 100 % is the
# reference. File > Print Preview is opened by the keys (Alt+F, V) and `tools/zoom_text.py preview` checks every sheet's words against the
# reference's times the sheet's own scale (its width over the paper's): the window cannot be resized (no window manager, and Quill
# sizes itself), so the scale is chosen with the paper. One sheet of a letter page at about 49 %, of a page 7 inches tall at about
# 77 %, and at actual size (the Zoom button, key Z); two sheets (Two Pages, key T) of tests/docs/zoomtext2.rtf on a letter page at
# about 49 % and on a page 5.5 inches wide and 7 tall at about 75 %. The frames' sheets are compared with
# `tests/golden/zoom/win32-wine-preview-NAME.png`.
# Needs wine, Xvfb, xdotool, ImageMagick and Python with Pillow; exits 0 with a note when they are missing. Uses its own display
# (QUILL_ZOOM_TEXT_DISPLAY, default :183, a 900 by 700 screen) and a WINEPREFIX of its own under the cache directory. Local only.
set -euo pipefail
# The caret stays solid so that screenshots are stable (`[Quill] Blink`, win9x/tools/wine-blink.sh).
export QUILL_NO_BLINK=1
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
SHOTS=""
UPDATE=0
PREVIEW=0
while [ $# -gt 0 ]; do
	case "$1" in
		--shots) SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"; shift 2 ;;
		--update) UPDATE=1; shift ;;
		--preview) PREVIEW=1; shift ;;
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
SCREEN=900x700x24
[ "$PREVIEW" -eq 0 ] || SCREEN=1280x1100x24
Xvfb "$DISPLAY" -screen 0 $SCREEN > /dev/null 2>&1 &
XPID=$!
sleep 2
cp "$EXE" "$WORK/QUILL32.EXE"
cp "$ROOT/tests/docs/zoomtext.rtf" "$WORK/ZOOMTEXT.RTF"
mkdir -p "$WORK/frames"
if [ "$PREVIEW" -eq 1 ]; then
	cp "$ROOT/tests/docs/zoomtext2.rtf" "$WORK/ZOOMTEXT2.RTF"
	# The same text on shorter paper (7 inches tall), and on narrower paper with the text block's width kept (5.5 inches wide).
	sed 's/\\paperh15840/\\paperh10080/' "$ROOT/tests/docs/zoomtext.rtf" > "$WORK/SHORT.RTF"
	sed 's/\\paperw12240\\paperh15840\\margl360\\margr6480/\\paperw7920\\paperh10080\\margl360\\margr2160/' "$ROOT/tests/docs/zoomtext2.rtf" > "$WORK/NARROW2.RTF"
	# start DOC: runs the document at 100 % with the window at the corner; sets $id
	start() {
		printf '[Quill]\r\nZoom=100\r\nBlink=0\r\n' > "$INI"
		(cd "$WORK" && wine QUILL32.EXE "$1" > /dev/null 2>&1) &
		WPID=$!
		id=""
		for _ in $(seq 1 40); do
			sleep 2
			id="$(xdotool search --name Quill 2> /dev/null | head -1 || true)"
			[ -z "$id" ] || break
		done
		[ -n "$id" ] || fail "no Quill window for $1"
		sleep 5
		xdotool windowfocus "$id" 2> /dev/null || true
		xdotool windowmove "$id" 0 0
		sleep 1
	}
	stop() {
		kill "$WPID" 2> /dev/null || true
		WPID=""
		wineserver -k 2> /dev/null || true
		sleep 1
	}
	# shot NAME W H: the window cut out of the screen
	shot() { sleep 2; import -window root -crop "${2}x${3}+0+0" +repage "$WORK/frames/pv-$1.png"; }
	# preview_of DOC: starts DOC and opens File > Print Preview by the keys
	preview_of() {
		start "$1"
		xdotool key alt+f
		sleep 1
		xdotool key v
		sleep 2
	}
	start ZOOMTEXT.RTF
	xdotool key ctrl+End
	sleep 1
	import -window root -crop 860x631+0+0 +repage "$WORK/frames/ref.png"
	xdotool key alt+f
	sleep 1
	xdotool key v
	shot one-49 860 631
	xdotool key z
	shot one-actual 860 631
	stop
	preview_of SHORT.RTF
	shot one-77 860 631
	stop
	preview_of ZOOMTEXT2.RTF
	xdotool key t
	shot two-49 860 631
	stop
	preview_of NARROW2.RTF
	xdotool key t
	shot two-75-w528 860 631
	stop
	rm -f "$INI"
	NAMES="one-49 one-77 one-actual two-49 two-75-w528"
	for n in $NAMES; do
		python3 -I "$ROOT/tools/zoom_text.py" crop-preview "$WORK/frames/pv-$n.png" "$WORK/frames/crop-$n.png" || fail "no text in the $n preview"
		if [ -n "$SHOTS" ]; then
			cp "$WORK/frames/pv-$n.png" "$SHOTS/win32-wine-preview-$n.png"
			python3 -I "$ROOT/tools/zoom_text.py" crop-preview "$WORK/frames/pv-$n.png" "$SHOTS/win32-wine-preview-$n-crop3x.png" --scale 3
		fi
	done
	python3 -I "$ROOT/tools/zoom_text.py" preview "$WORK/frames" || fail "a preview word is not as wide as the 100% word times the preview's scale, or two touch"
	echo "wine-zoom-text: the words of every preview are as wide as at 100% times the preview's scale"
	WINE_NOW="$(wine --version)"
	if [ "$UPDATE" -eq 1 ]; then
		mkdir -p "$GOLD"
		for n in $NAMES; do cp "$WORK/frames/crop-$n.png" "$GOLD/win32-wine-preview-$n.png"; done
		echo "wine-zoom-text: preview golden frames rewritten for $WINE_NOW"
		exit 0
	fi
	if [ ! -f "$GOLD/WINE_VERSION" ] || [ "$(cat "$GOLD/WINE_VERSION")" != "$WINE_NOW" ]; then
		echo "wine-zoom-text: golden frames are for $(cat "$GOLD/WINE_VERSION" 2> /dev/null || echo 'no Wine') and this is $WINE_NOW; compared the measured check only"
		echo "wine-zoom-text: ok"
		exit 0
	fi
	for n in $NAMES; do
		want="$GOLD/win32-wine-preview-$n.png"
		[ -f "$want" ] || fail "no golden frame $want"
		[ "$(identify -format '%wx%h' "$want")" = "$(identify -format '%wx%h' "$WORK/frames/crop-$n.png")" ] || fail "the $n preview's text is not the size of its golden frame"
		area="$(identify -format '%[fx:w*h]' "$want")"
		differ="$(compare -metric AE -fuzz 10% "$want" "$WORK/frames/crop-$n.png" null: 2>&1 | cut -d' ' -f1 | cut -d. -f1 || true)"
		[ "$differ" -le $((area / 200)) ] || fail "the $n preview's text differs from its golden frame in $differ of $area pixels"
	done
	echo "wine-zoom-text: the previews match the golden frames"
	echo "wine-zoom-text: ok"
	exit 0
fi
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
