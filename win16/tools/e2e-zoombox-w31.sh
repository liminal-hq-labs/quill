#!/usr/bin/env bash
# Clicks the zoom control near the right end of the status bar on Windows 3.11 under DOSBox-X: the box lists the levels and a choice sets
# the zoom, and the minus and plus buttons step to the next level.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [SHOTS=DIR] [KEEP=1] win16/tools/e2e-zoombox-w31.sh [QUILL.EXE]   (default win16/out/QUILL.EXE, or $OUT_DIR/QUILL.EXE)
#
# One session of a fresh Windows 3.11 with tests/docs/scroll.rtf open at the default zoom (Page Width). Frames: `zoombox-start` (the
# box reads Page Width), `zoombox-open` (a click on the box opened the list: the pixels above the box changed), `zoombox-escape`
# (Escape closed it and nothing else changed), `zoombox-75` (the list's sixth entry, chosen by the keys, redrew the page and the box),
# `zoombox-plus` (the plus button went to 100), `zoombox-minus` (minus twice went to 50) and `zoombox-minus-end` (minus at 50, greyed, did
# nothing). The zoom keys, View > Zoom In and Out and Ctrl+Insert, Shift+Insert follow (`zoombox-key-*`, `zoombox-menu-*`). The margin at the right edge is checked on the host (the pointer cannot be steered onto it against the black screen).
# At the end File > Exit must leave `Zoom=50` in QUILL.INI and the stack margin as the other tests demand. Exits 0 with a note when the
# Windows 3.11 tree is missing. The mouse needs the emulator's pointer lock; see docs/testing.md.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
EXE="${1:-${OUT_DIR:-$ROOT/win16/out}/QUILL.EXE}"
TREE="${W31_TREE:-$HOME/.cache/quill16-play/w31-base}"
IMAGE="${W31_IMAGE:-quill16-dosbox-x:local}"
NAME="${W31_ZOOM_CONTAINER:-quill16-zoom-w31}"
BOOT="${BOOT:-30}"
BASE="${QUILL_LAUNCH_DIR:-$HOME/.cache/quill16-launch}"

log() { printf '[e2e-zoombox-w31 %s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
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

# The zoom control is the last 140 pixels of the status bar, which sits just under the page area: a minus button, the box (70 pixels), a
# plus button, the Page Width button (22 pixels) and an 8 pixel margin; the middle of each is where its click goes.
differ() { compare -metric AE "$WORK/$1.png" "$WORK/$2.png" null: 2>&1 | cut -d' ' -f1 | cut -d. -f1 || true; }
# How many pixels differ inside a box of two frames: A B W H X Y.
differ_box() {
	magick "$WORK/$1.png" -crop "$3x$4+$5+$6" +repage "$WORK/ca.png"
	magick "$WORK/$2.png" -crop "$3x$4+$5+$6" +repage "$WORK/cb.png"
	compare -metric AE "$WORK/ca.png" "$WORK/cb.png" null: 2>&1 | cut -d' ' -f1 | cut -d. -f1 || true
}

boot scroll.rtf width
# On a 1280 by 1024 display Windows opens the window at the same place every time (x 320 to 1120, y 312 to 912): the status bar is its
# last 18 pixels and the zoom control the last 140 of those.
MINUS_X=990
BOX_X=1035
PLUS_X=1080
RESET_X=1101
BOX_Y=903
park() { point_to 800 650; }
park
shot zoombox-start
point_to "$BOX_X" "$BOX_Y"
xdotool click 1
sleep 1.5
shot zoombox-open
# The list opens above the box, its right edge at the box's.
[ "$(differ_box zoombox-start zoombox-open 100 110 $((BOX_X - 60)) $((BOX_Y - 115)))" -gt 300 ] || fail "a click on the zoom box opened no list"
log "a click on the box opens the list"
xdotool key Escape
sleep 1
park
shot zoombox-escape
[ "$(differ zoombox-start zoombox-escape)" -lt 50 ] || fail "Escape left the window changed ($(differ zoombox-start zoombox-escape) pixels)"
log "Escape closes it with no change"
# Page Width, 200, 150, 125, 100, 75: the sixth entry. Windows 3.1 opens the list with the first one lit, so five steps down.
point_to "$BOX_X" "$BOX_Y"
xdotool click 1
sleep 1.5
for _ in 1 2 3 4 5; do
	xdotool key Down
	sleep 0.2
done
xdotool key Return
sleep 2
park
shot zoombox-75
[ "$(differ_box zoombox-start zoombox-75 600 400 440 440)" -gt 500 ] || fail "the page did not change at 75%"
log "choosing 75% redraws the page"
# Plus steps from 75 to 100.
point_to "$PLUS_X" "$BOX_Y"
xdotool click 1
sleep 2
park
shot zoombox-plus
[ "$(differ_box zoombox-75 zoombox-plus 600 400 440 440)" -gt 500 ] || fail "the plus button did not change the page"
[ "$(differ_box zoombox-75 zoombox-plus 70 14 1000 896)" -gt 50 ] || fail "the box's text did not change at 100%"
log "plus steps from 75% to 100%"
# Minus twice goes to 75 and then 50.
point_to "$MINUS_X" "$BOX_Y"
xdotool click 1
sleep 2
xdotool click 1
sleep 2
park
shot zoombox-minus
[ "$(differ_box zoombox-plus zoombox-minus 600 400 440 440)" -gt 500 ] || fail "the minus button did not change the page"
log "minus steps down to 50%"
# At 50% minus is greyed and a click does nothing.
point_to "$MINUS_X" "$BOX_Y"
xdotool click 1
sleep 2
park
shot zoombox-minus-end
[ "$(differ zoombox-minus zoombox-minus-end)" -lt 50 ] || fail "minus at 50% changed the window ($(differ zoombox-minus zoombox-minus-end) pixels)"
log "minus is greyed at 50% and does nothing"
# The Page Width button sets Page Width from 50%; once there it is greyed and a click does nothing.
point_to "$RESET_X" "$BOX_Y"
xdotool click 1
sleep 2
park
shot zoombox-reset
[ "$(differ_box zoombox-minus-end zoombox-reset 600 400 440 440)" -gt 500 ] || fail "the Page Width button did not change the page"
[ "$(differ_box zoombox-minus-end zoombox-reset 110 14 990 896)" -gt 50 ] || fail "the box's text did not change at Page Width"
log "the Page Width button sets Page Width"
point_to "$RESET_X" "$BOX_Y"
xdotool click 1
sleep 2
park
shot zoombox-reset-click
[ "$(differ zoombox-reset zoombox-reset-click)" -lt 50 ] || fail "the Page Width button changed the window at Page Width ($(differ zoombox-reset zoombox-reset-click) pixels)"
log "the Page Width button is greyed at Page Width and does nothing"
# Ctrl+0 does the same from the keyboard: plus leaves Page Width, Ctrl+0 comes back to the same picture.
point_to "$PLUS_X" "$BOX_Y"
xdotool click 1
sleep 2
park
shot zoombox-plus-again
[ "$(differ_box zoombox-reset zoombox-plus-again 600 400 440 440)" -gt 500 ] || fail "plus from Page Width did not change the page"
xdotool key ctrl+0
sleep 2
shot zoombox-ctrl0
[ "$(differ_box zoombox-reset zoombox-ctrl0 600 400 440 440)" -lt 50 ] || fail "Ctrl+0 did not bring Page Width back ($(differ_box zoombox-reset zoombox-ctrl0 600 400 440 440) pixels)"
log "Ctrl+0 sets Page Width"
# The zoom keys (WP-BAR-1K) take the buttons' steps: each key from Page Width gives the picture the matching button gave from Page Width.
# Ctrl+numpad plus is the plus button's, and Ctrl+minus and Ctrl+numpad minus are the minus button's.
same() { [ "$(differ_box "$1" "$2" 780 590 330 318)" -lt 50 ] || fail "$3 ($(differ_box "$1" "$2" 780 590 330 318) pixels differ)"; }
xdotool key ctrl+KP_Add
sleep 2
shot zoombox-key-plus
same zoombox-plus-again zoombox-key-plus "Ctrl+numpad plus is not the plus button's step"
# The keypad's 0 is Ctrl+Insert (Copy) with Num Lock off, so Num Lock goes on for it and off again.
xdotool key Num_Lock
sleep 0.5
xdotool key ctrl+KP_0
sleep 2
xdotool key Num_Lock
sleep 0.5
shot zoombox-key-kp0
same zoombox-ctrl0 zoombox-key-kp0 "Ctrl+numpad 0 did not bring Page Width back"
log "Ctrl+numpad plus steps like the plus button, Ctrl+numpad 0 is Page Width"
point_to "$MINUS_X" "$BOX_Y"
xdotool click 1
sleep 2
park
shot zoombox-minus-width
[ "$(differ_box zoombox-ctrl0 zoombox-minus-width 600 400 440 440)" -gt 500 ] || fail "minus from Page Width did not change the page"
xdotool key ctrl+0
sleep 2
xdotool key ctrl+minus
sleep 2
shot zoombox-key-minus
same zoombox-minus-width zoombox-key-minus "Ctrl+minus is not the minus button's step"
xdotool key ctrl+0
sleep 2
xdotool key ctrl+KP_Subtract
sleep 2
shot zoombox-key-kpminus
same zoombox-minus-width zoombox-key-kpminus "Ctrl+numpad minus is not the minus button's step"
xdotool key ctrl+0
sleep 2
log "Ctrl+minus and Ctrl+numpad minus step like the minus button"
# View > Zoom In (Alt+V, I) is the same step as the key.
xdotool key alt+v
sleep 1
shot zoombox-view-menu
xdotool key i
sleep 2
shot zoombox-menu-in
same zoombox-key-plus zoombox-menu-in "View > Zoom In is not the plus button's step"
xdotool key ctrl+0
sleep 2
xdotool key alt+v
sleep 1
xdotool key o
sleep 2
shot zoombox-menu-out
same zoombox-minus-width zoombox-menu-out "View > Zoom Out is not the minus button's step"
xdotool key ctrl+0
sleep 2
log "View > Zoom In and Zoom Out step like the buttons"
# Ctrl+Insert still copies and Shift+Insert still pastes: select all, copy, go to the end and paste; the document is then modified.
xdotool key ctrl+a
xdotool key ctrl+Insert
xdotool key ctrl+End
sleep 2
shot zoombox-before-paste
xdotool key shift+Insert
sleep 3
shot zoombox-after-paste
[ "$(differ_box zoombox-before-paste zoombox-after-paste 600 400 440 440)" -gt 200 ] || fail "Ctrl+Insert and Shift+Insert did not copy and paste"
xdotool key ctrl+z
sleep 2
log "Ctrl+Insert and Shift+Insert still copy and paste"
# Resting on the Page Width button shows its tooltip above the bar and its sentence in the status bar's text.
park
shot zoombox-pre-tip
point_to "$RESET_X" "$BOX_Y"
sleep 3
shot zoombox-tip
[ "$(differ_box zoombox-pre-tip zoombox-tip 170 34 $((RESET_X - 150)) 858)" -gt 100 ] || fail "no tooltip over the Page Width button"
[ "$(differ_box zoombox-pre-tip zoombox-tip 300 14 326 896)" -gt 50 ] || fail "no hint in the status bar over the Page Width button"
park
sleep 2
shot zoombox-tip-gone
[ "$(differ_box zoombox-pre-tip zoombox-tip-gone 640 54 320 858)" -lt 50 ] || fail "the tooltip or the hint stayed after the pointer left"
log "the Page Width button's tooltip and hint come and go"
finish Width zoombox modified
