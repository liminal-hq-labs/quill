#!/usr/bin/env bash
# Types into Quill on Windows 3.11 under DOSBox-X, saves with Save As, types more, closes with File > Exit and Yes, and checks the program ends cleanly.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [SHOTS=DIR] [KEEP=1] [BADSLOT=1] win16/tools/e2e-saveclose-w31.sh [QUILL.EXE]   (default win16/out/QUILL.EXE, or $OUT_DIR/QUILL.EXE)
#
# This is the close that crashed under WineVDM on ReactOS: the program writes PICS.LOG after its message loop ends, and a corrupt
# picture-slot byte made that writer jump off the end of the code. The steps, on the harness of e2e-launch-w31.sh (a private virtual X
# server, a container of its own, the owner's Windows 3.11 tree copied to a run folder; the script exits 0 with a note when the tree
# is missing):
#
#   1. QUILL.EXE starts from WIN.INI's run= line with no arguments, and the window opens (BOOT seconds, default 20).
#   2. "Hello" is typed, File > Save As names `D:\TEST` (no extension), and TEST.QLD must appear (the dialog or Quill adds `.QLD`).
#   3. " world" is typed, then File > Exit and Yes (save). The program must end by itself: STACK.LOG and PICS.LOG appear within
#      20 seconds, the stack margin is at least 2,048 bytes, the Program Manager comes back (no fault box, so no `saveclose-early` shot),
#      TEST.QLD holds the text typed after the Save As, and PICS.LOG has no `bad state`.
#
# BADSLOT=1 needs a QUILL.EXE built with `W16_FEATURES=diag` (the switch is not in the shipped build). It writes QUILL.CMD with
# `/BADSLOT`, which plants 0x78 in picture slot 1 before the message loop, and then requires PICS.LOG to say `slot 1: bad state 0x78`,
# CRUMBS.LOG to hold the slots' bytes, and the exit to be as clean as above. Without the check in `picture::write_log` the exit
# jumps off the end of segment 8 and Windows shows a General Protection Fault.
#
# Needs the same as e2e-launch-w31.sh: Docker with quill16-dosbox-x:local, Xvfb, xdotool, ImageMagick; W31_TREE, QUILL_SAVECLOSE_DIR and
# W31_SAVECLOSE_CONTAINER (here quill16-saveclose-w31) work the same way. The tree is the owner's and is never committed, so this is a
# manual test. BOOT and SAVE_WAIT (seconds allowed for the Save As to write the file, default 20) can be raised on a loaded machine.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
EXE="${1:-${OUT_DIR:-$ROOT/win16/out}/QUILL.EXE}"
TREE="${W31_TREE:-$HOME/.cache/quill16-play/w31-base}"
IMAGE="${W31_IMAGE:-quill16-dosbox-x:local}"
NAME="${W31_SAVECLOSE_CONTAINER:-quill16-saveclose-w31}"
BOOT="${BOOT:-20}"
HOLD="${HOLD:-5}"
SAVE_WAIT="${SAVE_WAIT:-20}"
BASE="${QUILL_SAVECLOSE_DIR:-$HOME/.cache/quill16-saveclose}"

log() { printf '[e2e-saveclose-w31 %s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
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
if [ -n "${BADSLOT:-}" ] && ! grep -qa 'BADSLOT\|ALIAS tag=' "$EXE"; then fail "BADSLOT=1 needs a QUILL.EXE built with W16_FEATURES=diag"; fi
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
	[ -z "${SHOTS:-}" ] || for f in STACK.LOG PICS.LOG CRUMBS.LOG QUILL.INI TEST.QLD; do [ ! -f "$WORK/d/$f" ] || cp "$WORK/d/$f" "$SHOTS/saveclose-$f"; done
}

mkdir -p "$WORK/d" "$WORK/conf"
cp -a --reflink=auto "$TREE" "$WORK/c"
C="$WORK/c/WINDOWS"
# The tree's StartUp group opens Notepad and dials a modem; leave it out so only Quill starts.
sed -i '/^Group5=/Id' "$C/PROGMAN.INI"
cp "$EXE" "$WORK/d/QUILL.EXE"
# The caret stays steady, so every frame is the same whatever moment it is taken at ([Quill] Blink=0, W16-17).
printf '[Quill]\r\nBlink=0\r\n' > "$WORK/d/QUILL.INI"
[ ! -f "$(dirname "$EXE")/QUILL.HLP" ] || cp "$(dirname "$EXE")/QUILL.HLP" "$WORK/d/"
# WIN.INI's run= line takes the program with no arguments, as a double click would start it (CR LF line ends are kept).
awk 'BEGIN { IGNORECASE = 1 } /^[ \t]*run[ \t]*=/ { printf "run=D:\\QUILL.EXE\r\n"; done = 1; next } { print } END { if (!done) exit 3 }' "$C/WIN.INI" > "$C/WIN.INI.new" || fail "WIN.INI has no run= line"
mv "$C/WIN.INI.new" "$C/WIN.INI"
# The test switch goes in QUILL.CMD, which the program reads when its command line is empty.
[ -z "${BADSLOT:-}" ] || printf '/BADSLOT\r\n' > "$WORK/d/QUILL.CMD"

cat > "$WORK/conf/run.conf" << 'CONF'
[sdl]
output=surface
fullscreen=false
windowresolution=800x600
autolock=false
mouse_emulation=always
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
start="$(date +%s)"

# Windows starts and the window opens. STACK.LOG appears only when the program's message loop has ended.
quit_early() {
	shot saveclose-early
	keep_logs
	fail "QUILL.EXE ended by itself $(($(date +%s) - start)) s after the start (STACK.LOG: $(tr -d '\r\n' < "$WORK/d/STACK.LOG"))"
}
t=0
while [ "$t" -lt "$((BOOT + HOLD))" ]; do
	sleep 1
	t=$((t + 1))
	[ ! -f "$WORK/d/STACK.LOG" ] || quit_early
done
[ "$(docker inspect -f '{{.State.Running}}' "$NAME")" = true ] || fail "DOSBox-X is not running"
shot saveclose-open

# Type, then Save As with a name that has no extension.
xdotool mousemove 400 300
sleep 0.5
xdotool click 1
sleep 0.5
xdotool type --delay 150 "Hello"
xdotool key alt+f
sleep 1
shot saveclose-menu
xdotool key a
sleep 3
shot saveclose-saveas
xdotool type --delay 150 'D:\TEST'
sleep 1
xdotool key Return
t=0
while [ "$t" -lt "$SAVE_WAIT" ] && [ ! -f "$WORK/d/TEST.QLD" ]; do
	sleep 1
	t=$((t + 1))
done
shot saveclose-saved
[ -f "$WORK/d/TEST.QLD" ] || {
	keep_logs
	fail "Save As did not write TEST.QLD within ${SAVE_WAIT} s"
}
[ ! -f "$WORK/d/STACK.LOG" ] || quit_early

# More text, then File > Exit and Yes: Quill saves over TEST.QLD and ends.
xdotool type --delay 150 " world"
sleep 1
xdotool key alt+f
sleep 1
xdotool key x
sleep 2
shot saveclose-save-question
xdotool key y
t=0
while [ "$t" -lt 20 ] && { [ ! -f "$WORK/d/STACK.LOG" ] || [ ! -f "$WORK/d/PICS.LOG" ]; }; do
	sleep 1
	t=$((t + 1))
done
sleep 3
shot saveclose-exit
keep_logs
[ -f "$WORK/d/STACK.LOG" ] || fail "File > Exit did not end the program (no STACK.LOG after 20 s)"
[ -f "$WORK/d/PICS.LOG" ] || fail "no PICS.LOG after File > Exit: the exit did not reach the end of the program"
python3 "$HERE/check_stack_log.py" "$WORK/d/STACK.LOG" 2048 || fail "the stack margin is too small: $(tr -d '\r\n' < "$WORK/d/STACK.LOG")"
grep -qa 'world' "$WORK/d/TEST.QLD" || fail "TEST.QLD does not hold the text typed after the Save As"
if [ -n "${BADSLOT:-}" ]; then
	grep -qa 'slot 1: bad state 0x78' "$WORK/d/PICS.LOG" || fail "PICS.LOG does not report the planted slot: $(tr -d '\r' < "$WORK/d/PICS.LOG")"
	[ -f "$WORK/d/CRUMBS.LOG" ] && grep -qa '^SLOT tag=' "$WORK/d/CRUMBS.LOG" || fail "CRUMBS.LOG does not hold the slots' bytes"
else
	! grep -qa 'bad state' "$WORK/d/PICS.LOG" || fail "PICS.LOG reports a bad state: $(tr -d '\r' < "$WORK/d/PICS.LOG")"
fi
log "ok: $(tr -d '\r\n' < "$WORK/d/STACK.LOG") | $(tr -d '\r' < "$WORK/d/PICS.LOG" | head -c 120 | tr '\n' ' ')"
