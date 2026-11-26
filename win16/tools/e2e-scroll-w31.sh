#!/usr/bin/env bash
# Scrolls an empty page on Windows 3.11 under DOSBox-X: the vertical bar's arrows, track and thumb, Page Down, Page Up and Ctrl+End all
# have to bring the rest of the sheet into view, and the bar has to be enabled with nothing typed.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [SHOTS=DIR] [KEEP=1] win16/tools/e2e-scroll-w31.sh [QUILL.EXE]   (default win16/out/QUILL.EXE, or $OUT_DIR/QUILL.EXE)
#
# Starts QUILL.EXE as e2e-launch-w31.sh does and, on the new empty document, takes a frame (scroll-0-start.png), clicks the bar's lower
# arrow three times (scroll-1-arrow.png) and the track below the thumb (scroll-2-track.png), presses Page Down (scroll-3-pgdn.png) and
# Ctrl+End (scroll-4-end.png), then Page Up and Ctrl+Home (scroll-5-home.png), and ends through File > Exit. compare_frames.py reads the
# thumb's place in each frame and looks for the foot of the sheet at the bottom of the page: the thumb must move down at each of the first
# four steps, the foot must show at the end and the first frame must come back. The mouse needs the emulator's pointer lock; see
# docs/testing.md. It exits 0 with a note when the Windows 3.11 tree is missing, as the launch test does.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
EXE="${1:-${OUT_DIR:-$ROOT/win16/out}/QUILL.EXE}"
TREE="${W31_TREE:-$HOME/.cache/quill16-play/w31-base}"
IMAGE="${W31_IMAGE:-quill16-dosbox-x:local}"
NAME="${W31_SCROLL_CONTAINER:-quill16-scroll-w31}"
BOOT="${BOOT:-30}"
BASE="${QUILL_LAUNCH_DIR:-$HOME/.cache/quill16-launch}"

log() { printf '[e2e-scroll-w31 %s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
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
	[ -z "${SHOTS:-}" ] || for f in STACK.LOG PICS.LOG QUILL.INI; do [ ! -f "$WORK/d/$f" ] || cp "$WORK/d/$f" "$SHOTS/scroll-$f"; done
}

mkdir -p "$WORK/d" "$WORK/conf"
cp -a --reflink=auto "$TREE" "$WORK/c"
C="$WORK/c/WINDOWS"
# The tree's StartUp group opens Notepad and dials a modem; leave it out so only Quill starts.
sed -i '/^Group5=/Id' "$C/PROGMAN.INI"
cp "$EXE" "$WORK/d/QUILL.EXE"
[ ! -f "$(dirname "$EXE")/QUILL.HLP" ] || cp "$(dirname "$EXE")/QUILL.HLP" "$WORK/d/"
# The page is shown at 100 percent, wider than the window, so the horizontal bar is up and the vertical bar ends where find_vbar.py says
# (Page Width, the default, fits the window and takes the bar away).
printf '[Quill]\r\nZoom=100\r\n' > "$WORK/d/QUILL.INI"
# WIN.INI's run= line takes the program with no arguments, as a double click would start it (CR LF line ends are kept).
awk 'BEGIN { IGNORECASE = 1 } /^[ \t]*run[ \t]*=/ { printf "run=D:\\QUILL.EXE\r\n"; done = 1; next } { print } END { if (!done) exit 3 }' "$C/WIN.INI" > "$C/WIN.INI.new" || fail "WIN.INI has no run= line"
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
win /3
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
shot scroll-first
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

# The vertical bar and where its lower arrow is, from the window's frame.
read -r BX BY0 BY1 < <(python3 -I "$HERE/find_vbar.py" "$WORK/scroll-first.png")
log "vertical bar at x=$BX, y=$BY0..$BY1"
arrow_y=$((BY1 - 6))
point_to "$BX" "$arrow_y"
# The first frame has the pointer where the rest do, on the bar and off the page.
shot scroll-0-start
for _ in 1 2 3; do
	xdotool click 1
	sleep 0.6
done
shot scroll-1-arrow
point_to "$BX" $((BY1 - 60))
xdotool click 1
sleep 1
shot scroll-2-track
xdotool key Next
sleep 1
shot scroll-3-pgdn
xdotool key ctrl+End
sleep 1
shot scroll-4-end
xdotool key Prior
sleep 0.5
xdotool key ctrl+Home
sleep 1
shot scroll-5-home

# File > Exit.
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

# Each step moved the page, and Ctrl+Home brought back the first frame.
python3 -I "$HERE/compare_frames.py" "$WORK" || fail "the page did not scroll as a person would expect"
python3 "$HERE/check_stack_log.py" "$WORK/d/STACK.LOG" 2048 || fail "the stack margin is too small: $(tr -d '\r\n' < "$WORK/d/STACK.LOG")"
log "ok: $(tr -d '\r\n' < "$WORK/d/STACK.LOG")"
