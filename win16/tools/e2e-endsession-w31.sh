#!/usr/bin/env bash
# Types into Quill on Windows 3.11 under DOSBox-X, then chooses Exit Windows in the Program Manager, and checks that Quill asks about the unsaved text, that Cancel keeps Windows running, and that No lets Windows end with a copy of the text beside the program.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [SHOTS=DIR] [KEEP=1] win16/tools/e2e-endsession-w31.sh [QUILL.EXE]   (default win16/out/QUILL.EXE, or $OUT_DIR/QUILL.EXE)
#
# Windows sends WM_QUERYENDSESSION to every program when Exit Windows is confirmed, and WM_ENDSESSION when none refused (W16-8). The
# steps, on the harness of e2e-saveclose-w31.sh (a private virtual X server, a container of its own, the owner's Windows 3.11 tree
# copied to a run folder; the script exits 0 with a note when the tree is missing):
#
#   1. QUILL.EXE starts from WIN.INI's run= line and "Hello" is typed: the document is modified.
#   2. Alt+Tab brings the Program Manager forward, Alt+F4 asks to exit Windows and Return confirms. Quill's question ("Save the
#      changes to (Untitled) before Quill closes?") must come up on top, and Escape (Cancel) must leave Windows running: DOSBox-X
#      is still up and QUILL.EXE has written no STACK.LOG (it writes one when its message loop ends). Quill also leaves a recovery copy
#      (~Q and six hex digits, .RCV, beside the program: a document with no name) holding "Hello" (a refused shutdown still copies the text aside, for systems such as ReactOS that shut down whatever the answer).
#   3. Exit Windows again and answer No (N). Windows must end (the container stops by itself: the autoexec's last line is `exit`),
#      and the recovery copy, written again by WM_ENDSESSION, must be in D: and hold "Hello" (the Cancel step's copy is deleted first).
#
# Needs the same as e2e-saveclose-w31.sh: Docker with quill16-dosbox-x:local, Xvfb, xdotool, ImageMagick; W31_TREE,
# QUILL_ENDSESSION_DIR and W31_ENDSESSION_CONTAINER (here quill16-endsession-w31) work the same way. The tree is the owner's and
# is never committed, so this is a manual test. BOOT (seconds allowed for Windows to start, default 20) and END_WAIT (seconds allowed for Windows to end, default 40)
# can be raised on a loaded machine.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
EXE="${1:-${OUT_DIR:-$ROOT/win16/out}/QUILL.EXE}"
TREE="${W31_TREE:-$HOME/.cache/quill16-play/w31-base}"
IMAGE="${W31_IMAGE:-quill16-dosbox-x:local}"
NAME="${W31_ENDSESSION_CONTAINER:-quill16-endsession-w31}"
BOOT="${BOOT:-20}"
HOLD="${HOLD:-5}"
END_WAIT="${END_WAIT:-40}"
BASE="${QUILL_ENDSESSION_DIR:-$HOME/.cache/quill16-endsession}"

# The recovery copy of the untitled document: ~Q and six hex digits, beside the program (the D: drive).
rcv_copy() { for f in "$WORK"/d/~Q??????.RCV; do [ ! -f "$f" ] || { printf %s "$f"; return; }; done; }
log() { printf '[e2e-endsession-w31 %s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
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
	[ -z "${SHOTS:-}" ] || for f in STACK.LOG PICS.LOG; do [ ! -f "$WORK/d/$f" ] || cp "$WORK/d/$f" "$SHOTS/endsession-$f"; done; [ -z "$(rcv_copy)" ] || cp "$(rcv_copy)" "$SHOTS/endsession-recovery.RCV"
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
	shot endsession-early
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
shot endsession-open

# Type: the document is modified.
xdotool mousemove 400 300
sleep 0.5
xdotool click 1
sleep 0.5
xdotool type --delay 150 "Hello"
sleep 1
shot endsession-typed
[ ! -f "$WORK/d/STACK.LOG" ] || quit_early

# Exit Windows from the Program Manager: Alt+Tab, then Alt+F4 and Return on its confirmation.
exit_windows() {
	xdotool key alt+Tab
	sleep 2
	xdotool key alt+F4
	sleep 2
	shot "endsession-$1-confirm"
	xdotool key Return
	sleep 3
	shot "endsession-$1-question"
}
exit_windows cancel
# Quill's question is on the screen now; Escape is Cancel.
xdotool key Escape
sleep 4
shot endsession-cancelled
[ "$(docker inspect -f '{{.State.Running}}' "$NAME")" = true ] || fail "Windows ended although the question was cancelled"
[ ! -f "$WORK/d/STACK.LOG" ] || { keep_logs; fail "Quill ended although the question was cancelled"; }
# The copy of the text is made before the refusal, for systems that shut down whatever the answer (ReactOS).
RCV="$(rcv_copy)"; [ -n "$RCV" ] && grep -qa 'Hello' "$RCV" || fail "Cancel left no recovery copy holding the typed text"
rm -f "$RCV"
log "cancel kept Windows running"

# Again, and answer No: the session ends.
exit_windows no
xdotool key n
t=0
while [ "$t" -lt "$END_WAIT" ] && [ "$(docker inspect -f '{{.State.Running}}' "$NAME")" = true ]; do
	sleep 1
	t=$((t + 1))
done
keep_logs
[ "$(docker inspect -f '{{.State.Running}}' "$NAME")" != true ] || { shot endsession-stuck; fail "Windows did not end within ${END_WAIT} s of No"; }
RCV="$(rcv_copy)"; [ -n "$RCV" ] || fail "no recovery copy after Windows ended"
grep -qa 'Hello' "$RCV" || fail "the recovery copy does not hold the typed text"
log "ok: Cancel kept Windows running; No ended it and left a recovery copy ($(wc -c < "$RCV") bytes)"
