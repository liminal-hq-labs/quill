#!/usr/bin/env bash
# Print preview on Windows 3.11 under DOSBox-X: File > Print Preview has to show the sheets as they print, and Esc has to give the page back.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [SHOTS=DIR] [KEEP=1] [UPDATE_GOLDEN=1] win16/tools/e2e-preview-w31.sh [QUILL.EXE]   (default win16/out/QUILL.EXE, or $OUT_DIR/QUILL.EXE)
#
# Each session is a fresh Windows 3.11 with QUILL.EXE open on a test document (`Zoom=100`, no blink), driven from the keyboard (the
# Windows menu by Alt+F, V) and, for the buttons, the pointer. `check_preview.py` reads the frames:
#
#   scroll.rtf (two sheets)  the page before; File > Print Preview (one sheet fitted under the bar of six buttons, the status line
#                     saying `Page 1 of 2`); Page Down (`Page 2 of 2`); Page Up; T (two sheets, and the button says `One Page`); Z (actual
#                     size, the button says `Whole Page`, the arrow keys scroll it); Esc, after which the page area must be the first frame
#                     to the pixel; then the buttons by the pointer: Next Page, Previous Page and Close.
#   print3.rtf        the header (`Page N of N`, right) and the footer (centred) of the first and the second sheet, each with its page's
#                     number.
#   zoomtext.rtf      the sheet's words, each as wide as at 100 percent times the preview's percentage, and no two touching
#                     (`check_preview.py words`, which is `tools/zoom_text.py`'s check on the preview's frame).
#
# The preview's frames are compared pixel for pixel with tests/golden/zoom/win16-311-preview-*.png (DOSBox-X is deterministic;
# UPDATE_GOLDEN=1 rewrites them). Exits 0 with a note when the Windows 3.11 tree is missing. The mouse needs the emulator's pointer
# lock; see docs/testing.md.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
EXE="${1:-${OUT_DIR:-$ROOT/win16/out}/QUILL.EXE}"
TREE="${W31_TREE:-$HOME/.cache/quill16-play/w31-base}"
IMAGE="${W31_IMAGE:-quill16-dosbox-x:local}"
NAME="${W31_PREVIEW_CONTAINER:-quill16-preview-w31}"
BOOT="${BOOT:-30}"
BASE="${QUILL_PREVIEW_DIR:-$HOME/.cache/quill16-preview}"

log() { printf '[e2e-preview-w31 %s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
fail() {
	log "FAIL: $*"
	exit 1
}

if [ ! -f "$TREE/WINDOWS/WIN.COM" ]; then
	log "no Windows 3.11 tree at $TREE; skipping (set W31_TREE)"
	exit 0
fi
for t in docker xdotool Xvfb import compare cmp python3; do command -v "$t" > /dev/null || fail "$t not found"; done
[ -f "$EXE" ] || fail "$EXE is missing; build it with win16/build.sh"
if ! docker image inspect "$IMAGE" > /dev/null 2>&1; then
	docker build -q -t "$IMAGE" -f "$HERE/dosbox-x.Dockerfile" "$HERE" > /dev/null
fi

mkdir -p "$BASE"
WORK=""
XPID=""
cleanup() {
	set +e
	docker rm -f "$NAME" > /dev/null 2>&1
	[ -z "$XPID" ] || { kill "$XPID" 2> /dev/null && wait "$XPID" 2> /dev/null; }
	[ -n "${KEEP:-}" ] || [ -z "$WORK" ] || rm -rf "$WORK"
}
trap cleanup EXIT

# A private X server on a free display number, stopped by its own process id.
unset WAYLAND_DISPLAY XAUTHORITY
n=90
while [ -e "/tmp/.X11-unix/X$n" ] || [ -e "/tmp/.X$n-lock" ]; do n=$((n + 1)); done
Xvfb ":$n" -screen 0 1024x1000x24 -ac -nolisten tcp > /dev/null 2>&1 &
XPID=$!
export DISPLAY=":$n"
for _ in $(seq 1 50); do
	[ -S "/tmp/.X11-unix/X$n" ] && break
	sleep 0.1
done
[ -S "/tmp/.X11-unix/X$n" ] || fail "the virtual X server did not start"

[ -z "${SHOTS:-}" ] || { mkdir -p "$SHOTS" && SHOTS="$(cd "$SHOTS" && pwd)"; }
shot() {
	timeout 20 import -window root "$WORK/$1.png" 2> /dev/null || return 0
	[ -z "${SHOTS:-}" ] || cp "$WORK/$1.png" "$SHOTS/$1.png"
}
keep_logs() {
	[ -z "${SHOTS:-}" ] || for f in STACK.LOG PICS.LOG QUILL.INI; do [ ! -f "$WORK/d/$f" ] || cp "$WORK/d/$f" "$SHOTS/$1-$f"; done
}

# Starts Windows 3.11 with QUILL.EXE open on the test document $1 (a file of tests/docs) at the zoom $2 (what QUILL.INI holds), waits
# for the window, takes the pointer and finds the page area ($BX, $BY0 and $BY1: the vertical bar's x and its top and bottom).
boot() {
	WORK="$(mktemp -d "$BASE/run.XXXXXX")"
	mkdir -p "$WORK/d" "$WORK/conf"
	cp -a --reflink=auto "$TREE" "$WORK/c"
	local C="$WORK/c/WINDOWS" doc
	doc="$(basename "$1" | tr '[:lower:]' '[:upper:]')"
	# The tree's StartUp group opens Notepad and dials a modem; leave it out so only Quill starts.
	sed -i '/^Group5=/Id' "$C/PROGMAN.INI"
	cp "$EXE" "$WORK/d/QUILL.EXE"
	[ ! -f "$(dirname "$EXE")/QUILL.HLP" ] || cp "$(dirname "$EXE")/QUILL.HLP" "$WORK/d/"
	cp "$ROOT/tests/docs/$1" "$WORK/d/$doc"
	printf '[Quill]\r\nBlink=0\r\nZoom=%s\r\n' "$2" > "$WORK/d/QUILL.INI"
	# WIN.INI's run= line takes no arguments, so Windows starts from the DOS prompt with the program and its file on the command line.
	awk 'BEGIN { IGNORECASE = 1 } /^[ \t]*run[ \t]*=/ { printf "run=\r\n"; next } { print }' "$C/WIN.INI" > "$C/WIN.INI.new"
	mv "$C/WIN.INI.new" "$C/WIN.INI"
	cat > "$WORK/conf/run.conf" << CONF
[sdl]
output=surface
fullscreen=false
windowresolution=800x600
autolock=true
mouse_emulation=locked
[dosbox]
machine=svga_s3
memsize=32
[cpu]
core=normal
cputype=pentium
cycles=max
[dos]
ems=false
xms=true
umb=true
[autoexec]
@echo off
mount c /c
mount d /d
c:
set PATH=c:\\windows;c:\\utils
cd \\windows
win /3 D:\\QUILL.EXE D:\\$doc
exit
CONF
	docker rm -f "$NAME" > /dev/null 2>&1 || true
	docker run -d --name "$NAME" --user "$(id -u):$(id -g)" --network host -v /tmp/.X11-unix:/tmp/.X11-unix -e DISPLAY="$DISPLAY" \
		-e SDL_AUDIODRIVER=dummy -e HOME=/tmp -v "$WORK/c:/c" -v "$WORK/d:/d" -v "$WORK/conf:/conf" --entrypoint dosbox-x \
		"$IMAGE" -conf /conf/run.conf -nomenu -fastlaunch > /dev/null
	# Waits for Windows to start and the window to open.
	sleep "$BOOT"
	[ ! -f "$WORK/d/STACK.LOG" ] || fail "QUILL.EXE ended by itself"
	[ "$(docker inspect -f '{{.State.Running}}' "$NAME")" = true ] || fail "DOSBox-X is not running"
	# The emulator takes the host pointer on the first click and from then on passes only how far it moves; Windows scales that (a move
	# of four or more pixels goes twice as far), so the pointer is steered by looking at where it is.
	xdotool mousemove 500 400
	sleep 0.5
	xdotool click 1
	sleep 1
	shot first
	read -r BX BY0 BY1 < <(python3 -I "$HERE/find_vbar.py" "$WORK/first.png")
	log "vertical bar at x=$BX, page area from y=$BY0"
}

tip() {
	shot pointer
	python3 -I "$HERE/find_pointer.py" "$WORK/pointer.png"
}
# Moves the guest's pointer to (x, y) in the screenshot, within three pixels.
point_to() {
	local px py dx dy
	for _ in $(seq 1 14); do
		read -r px py < <(tip) || fail "the pointer is not in the picture"
		dx=$(($1 - px))
		dy=$(($2 - py))
		if [ "${dx#-}" -le 3 ] && [ "${dy#-}" -le 3 ]; then
			return 0
		fi
		[ "${dx#-}" -ge 4 ] && dx=$((dx / 2))
		[ "${dy#-}" -ge 4 ] && dy=$((dy / 2))
		# The emulator drops a very long move, so no step is more than 100 pixels (the guest goes twice as far).
		[ "$dx" -gt 100 ] && dx=100
		[ "$dx" -lt -100 ] && dx=-100
		[ "$dy" -gt 100 ] && dy=100
		[ "$dy" -lt -100 ] && dy=-100
		xdotool mousemove_relative -- "$dx" "$dy"
		sleep 0.3
	done
	fail "could not steer the pointer to $1,$2 (at $px,$py)"
}

# View the menu of File > Print Preview: Alt+F, then V.
open_preview() {
	xdotool key alt+f
	sleep 1
	xdotool key v
	sleep 2
}

# File > Exit (and No to the question a changed document asks). Then the stack's margin.
finish() {
	xdotool key alt+f
	sleep 1
	xdotool key x
	local t=0
	while [ "$t" -lt 20 ] && [ ! -f "$WORK/d/STACK.LOG" ]; do
		sleep 1
		t=$((t + 1))
	done
	keep_logs "$1"
	[ -f "$WORK/d/STACK.LOG" ] || fail "File > Exit did not end the program (no STACK.LOG after 20 s)"
	python3 "$HERE/check_stack_log.py" "$WORK/d/STACK.LOG" 2048 || fail "the stack margin is too small: $(tr -d '\r\n' < "$WORK/d/STACK.LOG")"
	log "ok: $(tr -d '\r\n' < "$WORK/d/STACK.LOG")"
	docker rm -f "$NAME" > /dev/null 2>&1 || true
	[ -n "${KEEP:-}" ] || rm -rf "$WORK"
	WORK=""
}

drawn=0
GOLD="$ROOT/tests/golden/zoom"

# The window's client area cut out of frame $1 ($WORK/$1.png) into $WORK/$1-client.png.
client_of() {
	shot "$1"
	python3 -I "$HERE/check_preview.py" client "$WORK/$1.png" "$BX" "$BY0" "$WORK/$1-client.png"
	[ -z "${SHOTS:-}" ] || cp "$WORK/$1-client.png" "$SHOTS/$1-client.png"
}
# The page area of frame $1, as the zoom test cuts it.
page_of() {
	shot "$1"
	python3 -I "$HERE/check_zoom.py" crop "$WORK/$1.png" "$BX" "$BY0" "$BY1" "$WORK/$1-page.png"
	[ -z "${SHOTS:-}" ] || cp "$WORK/$1-page.png" "$SHOTS/$1-page.png"
}
# Where the pointer waits while a frame is taken: over the title bar, out of the client area.
park() { point_to $((BX - 200)) $((BY0 - 107)); }

# Compares the client area $1 with the golden $2 (UPDATE_GOLDEN=1 writes it).
golden_of() {
	if [ -n "${UPDATE_GOLDEN:-}" ]; then
		mkdir -p "$GOLD"
		cp "$WORK/$1-client.png" "$GOLD/$2"
	elif [ -f "$GOLD/$2" ]; then
		local diff
		diff="$(compare -metric AE "$GOLD/$2" "$WORK/$1-client.png" null: 2>&1 || true)"
		diff="${diff%% *}"
		if [ "$diff" != 0 ]; then
			log "the frame differs from $2 in $diff pixels"
			drawn=1
		fi
	else
		log "no golden $2 yet (UPDATE_GOLDEN=1 writes it)"
		drawn=1
	fi
}
# $1 and $2 are two images of one size: they must not differ by a pixel (they must, when $3 is "differ").
same() {
	local diff
	diff="$(compare -metric AE "$WORK/$1" "$WORK/$2" null: 2>&1 || true)"
	diff="${diff%% *}"
	if [ "${3:-}" = differ ]; then
		[ "$diff" != 0 ] || { log "$1 and $2 are the same frame"; drawn=1; }
	elif [ "$diff" != 0 ]; then
		log "$1 and $2 differ in $diff pixels"
		drawn=1
	fi
}
# Presses the preview's button whose middle is x pixels from the client area's left edge (Print 38, Next Page 122, Previous Page 236,
# Two Pages 352, Zoom 458, Close 546) and waits for it to draw.
press_at() {
	point_to $((BX - 790 + $1)) $((BY0 - 62))
	xdotool click 1
	sleep 2.5
	park
	sleep 1
}

# ---- the sheets and the keys -----------------------------------------------------------------------------------------------------

keys_session() {
	boot scroll.rtf 100
	park
	page_of page-before
	open_preview
	client_of preview-one
	python3 -I "$HERE/check_preview.py" sheets "$WORK/preview-one-client.png" 1 || drawn=1
	golden_of preview-one win16-311-preview-one.png
	xdotool key Next
	sleep 1.5
	client_of preview-page2
	golden_of preview-page2 win16-311-preview-page2.png
	same preview-one-client.png preview-page2-client.png differ
	xdotool key Prior
	sleep 1.5
	client_of preview-back
	same preview-one-client.png preview-back-client.png
	xdotool key t
	sleep 1.5
	client_of preview-two
	python3 -I "$HERE/check_preview.py" sheets "$WORK/preview-two-client.png" 2 || drawn=1
	golden_of preview-two win16-311-preview-two.png
	xdotool key t
	sleep 1
	xdotool key z
	sleep 1.5
	client_of preview-actual
	golden_of preview-actual win16-311-preview-actual.png
	xdotool key --repeat 12 Down
	sleep 1.5
	client_of preview-actual-down
	same preview-actual-client.png preview-actual-down-client.png differ
	golden_of preview-actual-down win16-311-preview-actual-down.png
	xdotool key z
	sleep 1
	xdotool key Escape
	sleep 2
	page_of page-after
	# Esc gives the page back as it was, to the pixel.
	same page-before-page.png page-after-page.png
	# The buttons, by the pointer: Next Page, Previous Page, Two Pages (and back), Zoom (and back), then Close.
	open_preview
	press_at 122
	client_of click-next
	same preview-page2-client.png click-next-client.png
	press_at 236
	client_of click-previous
	same preview-one-client.png click-previous-client.png
	press_at 352
	client_of click-two
	same preview-two-client.png click-two-client.png
	press_at 352
	press_at 458
	client_of click-zoom
	same preview-actual-client.png click-zoom-client.png
	press_at 458
	press_at 546
	page_of click-close
	same page-before-page.png click-close-page.png
	finish preview-keys
}

# ---- the header and the footer ---------------------------------------------------------------------------------------------------

headers_session() {
	boot print3.rtf 100
	park
	open_preview
	client_of preview-header1
	golden_of preview-header1 win16-311-preview-header1.png
	xdotool key Next
	sleep 1.5
	client_of preview-header2
	golden_of preview-header2 win16-311-preview-header2.png
	same preview-header1-client.png preview-header2-client.png differ
	finish preview-headers
}

# ---- the words -------------------------------------------------------------------------------------------------------------------

words_session() {
	boot zoomtext.rtf 100
	park
	open_preview
	client_of preview-text
	golden_of preview-text win16-311-preview-text.png
	python3 -I "$HERE/check_preview.py" sheets "$WORK/preview-text-client.png" 1 || drawn=1
	python3 -I "$HERE/check_preview.py" words "$WORK/preview-text-client.png" "$GOLD/win16-311-z100.png" || drawn=1
	finish preview-words
}

case "${SESSION:-all}" in
	keys) keys_session ;;
	headers) headers_session ;;
	words) words_session ;;
	*)
		keys_session
		headers_session
		words_session
		;;
esac
[ "$drawn" = 0 ] || fail "the print preview was not as expected"
log "ok: the print preview drew and closed as expected"
