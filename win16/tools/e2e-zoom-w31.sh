#!/usr/bin/env bash
# Zooms the page on Windows 3.11 under DOSBox-X: View > Zoom has to change what the page looks like, not only what QUILL.INI says.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [SHOTS=DIR] [KEEP=1] win16/tools/e2e-zoom-w31.sh [QUILL.EXE]   (default win16/out/QUILL.EXE, or $OUT_DIR/QUILL.EXE)
#
# Opens tests/docs/scroll.rtf (three sheets of numbered lines) with QUILL.INI already at `Zoom=200`, the largest the file may hold (the
# menu stops at 150), and takes a frame at each step:
#
#   zoom-200-top    the first sheet, drawn at twice its size
#   zoom-200-click  after a click in the third line, to see the caret lands where the pointer is
#   zoom-200-end    after Ctrl+End, the foot of the last sheet at the bottom of the page area
#   zoom-150, zoom-75, zoom-50, zoom-width, zoom-100    View > Zoom, each from the menu, the first sheet again with the caret
#                   still on the character the click chose
#
# check_zoom.py reads them: the text's left margin, the line pitch and the caret's height must follow the zoom (they are the same
# whatever Windows' fonts are, because they come from the layout), the sheet must be as wide as the zoom says where it fits in the
# window and fill it where it does not, the vertical bar's thumb must be shorter the larger the zoom, and the click must have put the
# caret on the character under the pointer. On the build before the zoom was drawn every frame was the 100 percent page, so the
# margins, the pitch and the thumb are the same in all of them and the script fails at the first comparison.
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
WORK="$(mktemp -d "$BASE/run.XXXXXX")"
XPID=""
cleanup() {
	set +e
	docker rm -f "$NAME" > /dev/null 2>&1
	[ -z "$XPID" ] || { kill "$XPID" 2> /dev/null && wait "$XPID" 2> /dev/null; }
	[ -n "${KEEP:-}" ] || rm -rf "$WORK"
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
	[ -z "${SHOTS:-}" ] || for f in STACK.LOG PICS.LOG QUILL.INI; do [ ! -f "$WORK/d/$f" ] || cp "$WORK/d/$f" "$SHOTS/zoom-$f"; done
}

mkdir -p "$WORK/d" "$WORK/conf"
cp -a --reflink=auto "$TREE" "$WORK/c"
C="$WORK/c/WINDOWS"
# The tree's StartUp group opens Notepad and dials a modem; leave it out so only Quill starts.
sed -i '/^Group5=/Id' "$C/PROGMAN.INI"
cp "$EXE" "$WORK/d/QUILL.EXE"
[ ! -f "$(dirname "$EXE")/QUILL.HLP" ] || cp "$(dirname "$EXE")/QUILL.HLP" "$WORK/d/"
cp "$ROOT/tests/docs/scroll.rtf" "$WORK/d/SCROLL.RTF"
printf '[Quill]\r\nZoom=200\r\n' > "$WORK/d/QUILL.INI"
# WIN.INI's run= line takes no arguments, so Windows starts from the DOS prompt with the program and its file on the command line.
awk 'BEGIN { IGNORECASE = 1 } /^[ \t]*run[ \t]*=/ { printf "run=\r\n"; next } { print }' "$C/WIN.INI" > "$C/WIN.INI.new"
mv "$C/WIN.INI.new" "$C/WIN.INI"

cat > "$WORK/conf/run.conf" << 'CONF'
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
set PATH=c:\windows;c:\utils
cd \windows
win /3 D:\QUILL.EXE D:\SCROLL.RTF
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
# The emulator takes the host pointer on the first click and from then on passes only how far it moves; Windows scales that (a move of
# four or more pixels goes twice as far), so the pointer is steered by looking at where it is.
xdotool mousemove 500 400
sleep 0.5
xdotool click 1
sleep 1
shot zoom-first
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

# Where the page area is: the vertical bar's top and x, from the window's frame (see find_vbar.py).
read -r BX BY0 BY1 < <(python3 -I "$HERE/find_vbar.py" "$WORK/zoom-first.png")
log "vertical bar at x=$BX, page area from y=$BY0"

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
# The caret stays on the character the click chose, so each frame has it as far from the margin as the zoom says.
for step in "1 150" "7 75" "p width" "0 100" "5 50"; do
	zoom_to "${step%% *}"
	shot "zoom-${step##* }"
done
clicks_to_foot 50

# The frames are read before the program is asked to end, so a build that did not draw the zoom says so and not what its keys did.
drawn=0
python3 -I "$HERE/check_zoom.py" check "$WORK" "$BX" "$BY0" || drawn=1

# File > Exit; the document is not changed, so it ends at once.
xdotool key alt+f
sleep 1
xdotool key x
t=0
while [ "$t" -lt 20 ] && [ ! -f "$WORK/d/STACK.LOG" ]; do
	sleep 1
	t=$((t + 1))
done
keep_logs
[ -f "$WORK/d/STACK.LOG" ] || fail "File > Exit did not end the program (no STACK.LOG after 20 s)"

[ "$drawn" = 0 ] || fail "the page was not drawn at the zoom the menu chose"
python3 "$HERE/check_stack_log.py" "$WORK/d/STACK.LOG" 2048 || fail "the stack margin is too small: $(tr -d '\r\n' < "$WORK/d/STACK.LOG")"
grep -qi '^Zoom=50' "$WORK/d/QUILL.INI" || fail "QUILL.INI does not keep the last zoom (50): $(tr -d '\r' < "$WORK/d/QUILL.INI")"
log "ok: $(tr -d '\r\n' < "$WORK/d/STACK.LOG")"
