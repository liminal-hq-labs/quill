#!/usr/bin/env bash
# Starts QUILL.EXE on Windows 3.11 under DOSBox-X with a file named on its command line, and checks that the document opens.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [OPEN_ARG='D:\\LETTER.QLD'] [SHOTS=DIR] [KEEP=1] win16/tools/e2e-openarg-w31.sh [QUILL.EXE]   (default win16/out/QUILL.EXE, or $OUT_DIR/QUILL.EXE)
#
# `QUILL.EXE FILE` opens FILE on every build, and the Win16 program once started with an empty "Untitled" instead. The steps, on the
# harness of e2e-launch-w31.sh (a private virtual X server, a container of its own, the owner's Windows 3.11 tree copied to a run
# folder; the script exits 0 with a note when the tree is missing):
#
#   1. WIN.INI's run= line is emptied and Windows starts from the DOS prompt as `win /3 D:\QUILL.EXE <OPEN_ARG>`, which gives the
#      program a command line (the run= line takes none). LETTER.QLD is on D:. The window opens (BOOT seconds, default 20).
#   2. A click puts the caret in the page and "Z" is typed. File > Exit and Yes save the document under the name it opened with: an
#      Untitled document would ask for a name (Save As) and never end. STACK.LOG must then appear within 20 seconds, and LETTER.QLD
#      must still hold the letter's text, so the document that was opened is the one that was saved. The shots show the title bar
#      ("Quill - D:\\LETTER.QLD") and the save question naming the file.
#
# Needs the same as e2e-launch-w31.sh: Docker with quill16-dosbox-x:local, Xvfb, xdotool, ImageMagick; W31_TREE, QUILL_OPENARG_DIR and
# W31_OPENARG_CONTAINER (here quill16-openarg-w31) work the same way. The tree is the owner's and is never committed, so this is a
# manual test. SHOTS=DIR keeps openarg-open.png (the title bar names the document) and the logs.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
EXE="${1:-${OUT_DIR:-$ROOT/win16/out}/QUILL.EXE}"
TREE="${W31_TREE:-$HOME/.cache/quill16-play/w31-base}"
IMAGE="${W31_IMAGE:-quill16-dosbox-x:local}"
NAME="${W31_OPENARG_CONTAINER:-quill16-openarg-w31}"
BOOT="${BOOT:-20}"
OPEN_ARG="${OPEN_ARG:-D:\\LETTER.QLD}"
HOLD="${HOLD:-5}"
BASE="${QUILL_OPENARG_DIR:-$HOME/.cache/quill16-saveclose}"

log() { printf '[e2e-openarg-w31 %s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
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
	[ -z "${SHOTS:-}" ] || for f in STACK.LOG PICS.LOG QUILL.INI LETTER.QLD; do [ ! -f "$WORK/d/$f" ] || cp "$WORK/d/$f" "$SHOTS/openarg-$f"; done
}

mkdir -p "$WORK/d" "$WORK/conf"
cp -a --reflink=auto "$TREE" "$WORK/c"
C="$WORK/c/WINDOWS"
# The tree's StartUp group opens Notepad and dials a modem; leave it out so only Quill starts.
sed -i '/^Group5=/Id' "$C/PROGMAN.INI"
cp "$EXE" "$WORK/d/QUILL.EXE"
[ ! -f "$(dirname "$EXE")/QUILL.HLP" ] || cp "$(dirname "$EXE")/QUILL.HLP" "$WORK/d/"
# The document the program is told to open.
cp "$ROOT/tests/docs/letter.qld" "$WORK/d/LETTER.QLD"
# WIN.INI's run= line takes no arguments, so Windows starts from the DOS prompt with the program and its file on the command line.
awk 'BEGIN { IGNORECASE = 1 } /^[ \t]*run[ \t]*=/ { printf "run=\r\n"; next } { print }' "$C/WIN.INI" > "$C/WIN.INI.new"
mv "$C/WIN.INI.new" "$C/WIN.INI"

cat > "$WORK/conf/run.conf" << CONF
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
win /3 D:\\QUILL.EXE ${OPEN_ARG}
exit
CONF
docker rm -f "$NAME" > /dev/null 2>&1 || true
docker run -d --name "$NAME" --user "$(id -u):$(id -g)" --network host -v /tmp/.X11-unix:/tmp/.X11-unix -e DISPLAY="$DISPLAY" \
	-e SDL_AUDIODRIVER=dummy -e HOME=/tmp -v "$WORK/c:/c" -v "$WORK/d:/d" -v "$WORK/conf:/conf" --entrypoint dosbox-x \
	"$IMAGE" -conf /conf/run.conf -nomenu -fastlaunch > /dev/null
start="$(date +%s)"


# Windows starts and the window opens. STACK.LOG appears only when the program's message loop has ended.
quit_early() {
	shot openarg-early
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
shot openarg-open

# One more character, then File > Exit and Yes: the document saves over the file it opened, or (Untitled) asks for a name.
xdotool mousemove 400 300
sleep 0.5
xdotool click 1
sleep 0.5
xdotool type --delay 150 "Z"
sleep 1
xdotool key alt+f
sleep 1
xdotool key x
sleep 2
shot openarg-save-question
xdotool key y
t=0
while [ "$t" -lt 20 ] && [ ! -f "$WORK/d/STACK.LOG" ]; do
	sleep 1
	t=$((t + 1))
done
sleep 2
shot openarg-exit
keep_logs
[ -f "$WORK/d/STACK.LOG" ] || fail "the program did not end after Yes (no STACK.LOG): the command line's file did not open, so Yes asked for a name"
grep -qa 'Dear' "$WORK/d/LETTER.QLD" || fail "LETTER.QLD no longer holds the letter's text"
log "ok: LETTER.QLD opened from the command line and saved under its own name"
