#!/usr/bin/env bash
# Check Spelling as You Type on Windows 3.11 under DOSBox-X: words typed with `AsYouType=1` get a red wave 0.3 s after the typing stops,
# the right button on a marked word opens a pop-up menu (Windows 3.1 sends its choice as a WM_COMMAND), a suggestion replaces the word and
# its wave goes, and Shift+F10 opens the same menu at the caret.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [SHOTS=DIR] [KEEP=1] win16/tools/e2e-spell-w31.sh [QUILL.EXE]   (default win16/out/QUILL.EXE, or $OUT_DIR/QUILL.EXE)
#
# One session of a fresh Windows 3.11 with a new document, `EN_CA.DIC` beside the program, and `AsYouType=1` in QUILL.INI. Frames:
# `spell-typed` (the words are on the page, and "Teh" and "jumpd" have a wave in the pure red nothing else on the screen is drawn in),
# `spell-menu` (the right button on "Teh" opened the menu: the pixels around the pointer changed), `spell-fixed` (the first suggestion
# replaced the word and the wave under it is gone), `spell-keymenu` (Shift+F10 opened the menu at the caret on "jumpd"). The Windows 3.11
# tree is copied for the run; exits 0 with a note when it is missing. The mouse needs the emulator's pointer lock; see docs/testing.md.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
EXE="${1:-${OUT_DIR:-$ROOT/win16/out}/QUILL.EXE}"
TREE="${W31_TREE:-$HOME/.cache/quill16-play/w31-base}"
IMAGE="${W31_IMAGE:-quill16-dosbox-x:local}"
NAME="${W31_SPELL_CONTAINER:-quill16-spell-w31}"
BOOT="${BOOT:-30}"
BASE="${QUILL_LAUNCH_DIR:-$HOME/.cache/quill16-launch}"

log() { printf '[e2e-spell-w31 %s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
fail() {
	log "FAIL: $*"
	exit 1
}

if [ ! -f "$TREE/WINDOWS/WIN.COM" ]; then
	log "no Windows 3.11 tree at $TREE; skipping (set W31_TREE)"
	exit 0
fi
for t in docker xdotool Xvfb import cmp python3; do command -v "$t" > /dev/null || fail "$t not found"; done
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
Xvfb ":$n" -screen 0 1280x1024x24 -ac -nolisten tcp > /dev/null 2>&1 &
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
# for the window, takes the pointer.
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
	printf '[Quill]\r\nBlink=0\r\nAsYouType=1\r\nZoom=%s\r\n' "$2" > "$WORK/d/QUILL.INI"
	cp "$ROOT/tests/docs/en_ca.dic" "$WORK/d/EN_CA.DIC"
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
# File > Exit (and No to the question a changed document asks when there is a third word). Then the stack's margin and QUILL.INI ($1 is the
# zoom it must hold, $2 the name its logs are kept under).
finish() {
	xdotool key alt+f
	sleep 1
	xdotool key x
	# A document that was changed asks first (the picture session sized a picture): No.
	if [ -n "${3:-}" ]; then
		sleep 1
		xdotool key n
	fi
	local t=0
	while [ "$t" -lt 20 ] && [ ! -f "$WORK/d/STACK.LOG" ]; do
		sleep 1
		t=$((t + 1))
	done
	keep_logs "$2"
	[ -f "$WORK/d/STACK.LOG" ] || fail "File > Exit did not end the program (no STACK.LOG after 20 s)"
	python3 "$HERE/check_stack_log.py" "$WORK/d/STACK.LOG" 2048 || fail "the stack margin is too small: $(tr -d '\r\n' < "$WORK/d/STACK.LOG")"
	grep -qi "^Zoom=$1" "$WORK/d/QUILL.INI" || fail "QUILL.INI does not keep the last zoom ($1): $(tr -d '\r' < "$WORK/d/QUILL.INI")"
	log "ok: $(tr -d '\r\n' < "$WORK/d/STACK.LOG")"
	docker rm -f "$NAME" > /dev/null 2>&1 || true
	[ -n "${KEEP:-}" ] || rm -rf "$WORK"
	WORK=""
}

# The pure red pixels (255, 0, 0) of a frame inside the page area, as `count x_min y_min x_max y_max` (the wave's colour; nothing else in
# Quill or Windows 3.1's default scheme is pure red), or `0` when there are none.
red() {
	python3 -I - "$WORK/$1.png" << 'PY'
import sys
from PIL import Image

image = Image.open(sys.argv[1]).convert("RGB")
# The page area only: the toolbar has a red letter on its colour button.
points = [(x, y) for y in range(432, 890) for x in range(335, 1088) if image.getpixel((x, y)) == (255, 0, 0)]
if points:
    xs, ys = [p[0] for p in points], [p[1] for p in points]
    print(len(points), min(xs), min(ys), max(xs), max(ys))
else:
    print(0)
PY
}
differ_box() {
	magick "$WORK/$1.png" -crop "$3x$4+$5+$6" +repage "$WORK/ca.png"
	magick "$WORK/$2.png" -crop "$3x$4+$5+$6" +repage "$WORK/cb.png"
	compare -metric AE "$WORK/ca.png" "$WORK/cb.png" null: 2>&1 | cut -d' ' -f1 | cut -d. -f1 || true
}

boot plain.qld width
park() { point_to 800 650; }
park
# The sample's own text goes first: select all, delete.
xdotool key ctrl+a
sleep 0.5
xdotool key Delete
sleep 1
# Teh, a word that is fine, a word that is not, and a word that is, with a space after each so the caret leaves it.
xdotool type --delay 120 "Teh quick brown fox jumpd over "
# The dictionary is read once (a second or two on the emulated 486) and the scan runs in 55 ms steps.
sleep 12
park
shot spell-typed
read -r count x0 y0 x1 y1 < <(red spell-typed)
[ "${count:-0}" -gt 6 ] || fail "no red wave on the page after typing two misspelt words (red pixels: ${count:-0})"
log "the wave is drawn: $count red pixels between $x0,$y0 and $x1,$y1"

# The right button on "Teh", the first word, whose wave starts at x0: the pointer goes just above it.
word_x=$((x0 + 6))
word_y=$((y0 - 6))
point_to "$word_x" "$word_y"
shot spell-before
xdotool click 3
sleep 2
shot spell-menu
[ "$(differ_box spell-before spell-menu 160 140 $((word_x - 10)) $((word_y - 10)))" -gt 400 ] || fail "the right button on a misspelt word opened no menu"
log "the right button opens the suggestions menu"
# Windows 3.1 pop-up: Down lights the first suggestion, Enter takes it (a WM_COMMAND with the suggestion's id).
xdotool key Down
sleep 0.3
xdotool key Return
sleep 3
park
shot spell-fixed
read -r count2 _ < <(red spell-fixed)
[ "${count2:-0}" -lt "$count" ] || fail "the first suggestion did not remove a wave (red pixels $count before, ${count2:-0} after)"
log "a suggestion replaced the word and its wave went ($count to ${count2:-0} red pixels)"

# Shift+F10 needs the caret in or against the word: six steps back from the end of "jumpd over " put it at the end of "jumpd".
for _ in 1 2 3 4 5 6; do
	xdotool key Left
	sleep 0.2
done
xdotool key shift+F10
sleep 2
shot spell-keymenu
[ "$(differ_box spell-fixed spell-keymenu 1280 1024 0 0)" -gt 400 ] || fail "Shift+F10 opened no menu"
log "Shift+F10 opens the menu at the caret"
xdotool key Escape
sleep 1
# File > Exit; the document is changed, so No to the question. QUILL.INI keeps the setting and the stack margin is as the others demand.
xdotool key alt+f
sleep 1
xdotool key x
sleep 1
xdotool key n
t=0
while [ "$t" -lt 20 ] && [ ! -f "$WORK/d/STACK.LOG" ]; do
	sleep 1
	t=$((t + 1))
done
keep_logs spell
[ -f "$WORK/d/STACK.LOG" ] || fail "File > Exit did not end the program (no STACK.LOG after 20 s)"
python3 "$HERE/check_stack_log.py" "$WORK/d/STACK.LOG" 2048 || fail "the stack margin is too small: $(tr -d '\r\n' < "$WORK/d/STACK.LOG")"
grep -qi "^AsYouType=1" "$WORK/d/QUILL.INI" || fail "QUILL.INI lost AsYouType=1: $(tr -d '\r' < "$WORK/d/QUILL.INI")"
log "ok: $(tr -d '\r\n' < "$WORK/d/STACK.LOG")"
docker rm -f "$NAME" > /dev/null 2>&1 || true
[ -n "${KEEP:-}" ] || rm -rf "$WORK"
WORK=""
