#!/usr/bin/env bash
# Starts QUILL.EXE on Windows 3.11 under DOSBox-X the way a person does (no arguments, from a run= line), checks it stays open for 12 seconds, then closes it through File > Exit.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [SHOTS=DIR] [KEEP=1] win16/tools/e2e-launch-w31.sh [QUILL.EXE]   (default win16/out/QUILL.EXE, or $OUT_DIR/QUILL.EXE)
#
# Every other Windows 3.11 check runs the program with `/PLAY`, which draws the text grid and never starts the tooltip timer, so none of
# them started the windowed program as a user does (W16-3). This one does:
#
#   1. QUILL.EXE and QUILL.HLP go on D:, WIN.INI's run= line names D:\QUILL.EXE with nothing after it, and Windows starts (`win /3`).
#   2. The window opens (BOOT seconds, default 20, are allowed for Windows to start) and must then stay open for HOLD seconds (12).
#      The program writes STACK.LOG when its message loop ends and not before, so a STACK.LOG at any time before step 3 means it quit
#      by itself: that is the failure.
#   3. Alt+F, X (File > Exit) is sent to the emulator on a private virtual X server. STACK.LOG must then appear within 20 seconds, and
#      check_stack_log.py must find a margin of at least 2,048 bytes between the stack and the data.
#
# Needs Docker with the DOSBox-X image of win16/tools/dosbox-x.Dockerfile (quill16-dosbox-x:local, built on first use), Xvfb, xdotool
# and ImageMagick. W31_TREE names the Windows 3.11 tree (a folder holding WINDOWS/ and WIN.BAT; default the play harness's base,
# ~/.cache/quill16-play/w31-base, which is only read: it is copied to a run folder under QUILL_LAUNCH_DIR, default
# ~/.cache/quill16-launch, first). The tree is the owner's and is never committed, so this is a manual test; it exits 0 with a note when
# the tree is missing. DOSBox-X runs in a container of its own (quill16-launch-w31, W31_LAUNCH_CONTAINER) on a virtual X server of its
# own, started on a free display number and stopped by its process id, so nothing here reads or types into the desktop.
# SHOTS=DIR keeps launch-open.png (while the window is open), launch-exit.png (after File > Exit) and the logs the program wrote.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
EXE="${1:-${OUT_DIR:-$ROOT/win16/out}/QUILL.EXE}"
TREE="${W31_TREE:-$HOME/.cache/quill16-play/w31-base}"
IMAGE="${W31_IMAGE:-quill16-dosbox-x:local}"
NAME="${W31_LAUNCH_CONTAINER:-quill16-launch-w31}"
BOOT="${BOOT:-20}"
HOLD="${HOLD:-12}"
BASE="${QUILL_LAUNCH_DIR:-$HOME/.cache/quill16-launch}"

log() { printf '[e2e-launch-w31 %s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
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
	[ -z "${SHOTS:-}" ] || for f in STACK.LOG PICS.LOG QUILL.INI; do [ ! -f "$WORK/d/$f" ] || cp "$WORK/d/$f" "$SHOTS/launch-$f"; done
}

mkdir -p "$WORK/d" "$WORK/conf"
cp -a --reflink=auto "$TREE" "$WORK/c"
C="$WORK/c/WINDOWS"
# The tree's StartUp group opens Notepad and dials a modem; leave it out so only Quill starts.
sed -i '/^Group5=/Id' "$C/PROGMAN.INI"
cp "$EXE" "$WORK/d/QUILL.EXE"
[ ! -f "$(dirname "$EXE")/QUILL.HLP" ] || cp "$(dirname "$EXE")/QUILL.HLP" "$WORK/d/"
# WIN.INI's run= line takes the program with no arguments, as a double click would start it (CR LF line ends are kept).
awk 'BEGIN { IGNORECASE = 1 } /^[ \t]*run[ \t]*=/ { printf "run=D:\\QUILL.EXE\r\n"; done = 1; next } { print } END { if (!done) exit 3 }' "$C/WIN.INI" > "$C/WIN.INI.new" || fail "WIN.INI has no run= line"
mv "$C/WIN.INI.new" "$C/WIN.INI"

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

# Windows starts and the window opens, then it must stay open. STACK.LOG appears only when the program's message loop has ended.
quit_early() {
	shot launch-early
	keep_logs
	fail "QUILL.EXE ended by itself $(($(date +%s) - start)) s after the start (STACK.LOG: $(tr -d '\r\n' < "$WORK/d/STACK.LOG"))"
}
t=0
while [ "$t" -lt "$BOOT" ]; do
	sleep 1
	t=$((t + 1))
	[ ! -f "$WORK/d/STACK.LOG" ] || quit_early
done
log "window up after at most ${BOOT} s; holding it for ${HOLD} s"
t=0
while [ "$t" -lt "$HOLD" ]; do
	sleep 1
	t=$((t + 1))
	[ ! -f "$WORK/d/STACK.LOG" ] || quit_early
done
[ "$(docker inspect -f '{{.State.Running}}' "$NAME")" = true ] || fail "DOSBox-X is not running"
shot launch-open

# File > Exit.
xdotool mousemove 400 300
sleep 0.5
# Insert > Date reads sShortDate from WIN.INI (a far-pointer key slot: a near pointer there made WineVDM fault); the shot shows the date.
xdotool key alt+i
sleep 1
xdotool key d
sleep 2
shot launch-date
xdotool key alt+f
sleep 1
shot launch-menu
xdotool key x
# The Date field changed the document: answer "Save the changes?" with No.
sleep 2
xdotool key n
t=0
while [ "$t" -lt 20 ] && [ ! -f "$WORK/d/STACK.LOG" ]; do
	sleep 1
	t=$((t + 1))
done
shot launch-exit
keep_logs
[ -f "$WORK/d/STACK.LOG" ] || fail "File > Exit did not end the program (no STACK.LOG after 20 s)"
python3 "$HERE/check_stack_log.py" "$WORK/d/STACK.LOG" 2048 || fail "the stack margin is too small: $(tr -d '\r\n' < "$WORK/d/STACK.LOG")"
log "ok: $(tr -d '\r\n' < "$WORK/d/STACK.LOG")"
