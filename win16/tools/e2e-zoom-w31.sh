#!/usr/bin/env bash
# Zooms the page on Windows 3.11 under DOSBox-X: View > Zoom has to change what the page looks like, not only what QUILL.INI says.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [SHOTS=DIR] [KEEP=1] win16/tools/e2e-zoom-w31.sh [QUILL.EXE]   (default win16/out/QUILL.EXE, or $OUT_DIR/QUILL.EXE)
#
# Two sessions, each a fresh Windows 3.11. The first opens tests/docs/scroll.rtf (two sheets of numbered lines) with QUILL.INI already
# at `Zoom=200`, the largest the file may hold (the menu stops at 150), and takes a frame at each step:
#
#   zoom-200-top      the first sheet, drawn at twice its size
#   zoom-200-end      after Ctrl+End
#   zoom-200-foot     after the track has been clicked down to the foot of the last sheet
#   zoom-200-click    after a click in the fifth line, to see the caret lands where the pointer is
#   zoom-200-select   after Shift+Home, the selection from the caret to the start of the line
#   zoom-150, zoom-75, zoom-width, zoom-100, zoom-50    View > Zoom, each from the menu, with the caret still on the character the
#                     click chose; zoom-50-foot is the foot of the sheet again
#
# check_zoom.py reads them from what the layout fixes and Windows' fonts cannot change: the text's left margin, the caret's height,
# the sheet's edges, the ruler's scale (it must start where the text does and span the column), the selection's cells (no gaps), the
# scroll range (the foot takes more clicks on the track the larger the zoom). On the build before the zoom was drawn every frame was the
# 100 percent page, so the margins, the heights and the widths are the same in all of them and the script fails at the first comparison.
#
# The second session opens tests/docs/pictures.rtf at 200 percent (a PNG, which Windows 3.x shows as a grey box the size it prints, and
# a bitmap), clicks the first picture, and reads the frame (`zoompic-200-select`) for its frame and eight handles, then chooses 100, 50
# and 150 from the menu and reads the box's size and the handles again (`zoompic-100`, `zoompic-50`, `zoompic-150`): the box scales,
# the handles stay seven pixels. At 100 it drags the bottom right handle by 32 and 24 pixels (`zoompic-100-dragged`), and the picture
# has to have grown.
#
# The mouse needs the emulator's pointer lock; see docs/testing.md. Exits 0 with a note when the Windows 3.11 tree is missing, as the
# launch test does. The Windows menu is driven from the keyboard (Alt+V, Z, then the item's mnemonic).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
EXE="${1:-${OUT_DIR:-$ROOT/win16/out}/QUILL.EXE}"
TREE="${W31_TREE:-$HOME/.cache/quill16-play/w31-base}"
IMAGE="${W31_IMAGE:-quill16-dosbox-x:local}"
NAME="${W31_ZOOM_CONTAINER:-quill16-zoom-w31}"
BOOT="${BOOT:-30}"
BASE="${QUILL_LAUNCH_DIR:-$HOME/.cache/quill16-launch}"

log() { printf '[e2e-zoom-w31 %s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
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
Xvfb ":$n" -screen 0 1024x768x24 -ac -nolisten tcp > /dev/null 2>&1 &
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
	printf '[Quill]\r\nZoom=%s\r\n' "$2" > "$WORK/d/QUILL.INI"
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
# View > Zoom > the item whose mnemonic is $1.
zoom_to() {
	xdotool key alt+v
	sleep 0.5
	xdotool key z
	sleep 0.5
	xdotool key "$1"
	sleep 1.5
}
# Where the pointer waits while a frame is taken: on the page, out of the way of the text, and off the scroll bar.
park() { point_to $((BX - 40)) $((BY0 + 300)); }
# Clicks the vertical bar's track below the thumb until the foot of the sheet shows (at most 14 times) and writes how many it took to
# $WORK/clicks-$1, so the scroll range can be compared between zooms.
clicks_to_foot() {
	local n=0
	point_to "$BX" $((BY1 - 24))
	while [ "$n" -lt 14 ]; do
		shot "zoom-$1-foot"
		python3 -I "$HERE/check_zoom.py" foot "$WORK/zoom-$1-foot.png" "$BX" "$BY0" && break
		xdotool click 1
		sleep 0.7
		n=$((n + 1))
	done
	echo "$n" > "$WORK/clicks-$1"
	log "the foot of the sheet showed after $n clicks on the track at $1"
	park
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

drawn=0

# ---- the text session ----------------------------------------------------------------------------------------------------------

boot scroll.rtf 200
park
shot zoom-200-top
xdotool key ctrl+End
sleep 1
shot zoom-200-end
xdotool key ctrl+Home
sleep 1
clicks_to_foot 200
xdotool key ctrl+Home
sleep 1
# The click goes to the fifth line of text, a little way along it, and the line is read from the first frame.
read -r CLICK_X CLICK_Y < <(python3 -I "$HERE/check_zoom.py" click "$WORK/zoom-200-top.png" "$BX" "$BY0")
log "clicking at $CLICK_X,$CLICK_Y"
point_to "$CLICK_X" "$CLICK_Y"
xdotool click 1
sleep 1
park
shot zoom-200-click
# Shift+Home selects back to the start of the line; the click again puts the caret back and drops the selection.
xdotool key shift+Home
sleep 1
shot zoom-200-select
point_to "$CLICK_X" "$CLICK_Y"
xdotool click 1
sleep 1
park
# The caret stays on the character the click chose, so each frame has it as far from the margin as the zoom says.
for step in "1 150" "7 75" "p width" "0 100" "5 50"; do
	zoom_to "${step%% *}"
	shot "zoom-${step##* }"
done
clicks_to_foot 50

# The frames are read before the program is asked to end, so a build that did not draw the zoom says so and not what its keys did.
python3 -I "$HERE/check_zoom.py" check "$WORK" "$BX" "$BY0" || drawn=1
finish 50 zoom

# ---- the picture session -------------------------------------------------------------------------------------------------------

boot pictures.rtf 200
park
shot zoompic-200-top
# The first picture is the PNG, a grey box; its middle is where to click.
read -r PIC_X PIC_Y < <(python3 -I "$HERE/check_zoom.py" picture-at "$WORK/zoompic-200-top.png" "$BX" "$BY0")
log "clicking the picture at $PIC_X,$PIC_Y"
point_to "$PIC_X" "$PIC_Y"
xdotool click 1
sleep 1
park
shot zoompic-200-select
# The picture stays selected through the zoom: its box scales and the frame and the handles stay a few pixels wide.
for step in "1 150" "5 50" "0 100"; do
	zoom_to "${step%% *}"
	shot "zoompic-${step##* }"
done
# At 100 percent, the bottom right handle is dragged 32 pixels right and 24 down, and the picture follows (proportions kept).
read -r HX HY < <(python3 -I "$HERE/check_zoom.py" handle "$WORK/zoompic-100.png" "$BX" "$BY0")
log "dragging the handle at $HX,$HY"
point_to "$HX" "$HY"
xdotool mousedown 1
sleep 0.3
point_to $((HX + 32)) $((HY + 24))
xdotool mouseup 1
sleep 1
park
shot zoompic-100-dragged
python3 -I "$HERE/check_zoom.py" pictures "$WORK" "$BX" "$BY0" || drawn=1
finish 100 zoompic changed

[ "$drawn" = 0 ] || fail "the page was not drawn at the zoom the menu chose"
