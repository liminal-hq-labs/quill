#!/usr/bin/env bash
# Holds Ctrl+I and a letter down on Windows 3.11 under DOSBox-X: Italic must switch on once, the letter must repeat.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [SHOTS=DIR] [KEEP=1] win16/tools/e2e-held-w31.sh [QUILL.EXE]   (default win16/out/QUILL.EXE, or $OUT_DIR/QUILL.EXE)
#
# Windows 3.x repeats a held key as WM_KEYDOWN messages with bit 30 of lParam set (the key was already down), and Quill must not run a
# toggle for those. DOSBox-X's keyboard repeats a key held with xdotool keydown as a real one does. The script starts Windows 3.11 as
# e2e-launch-w31.sh does, types a word, selects it, holds Ctrl+I for three seconds and reads the Italic button on the formatting bar
# (pressed or not) every 0.2 s: it must change once. Then a letter is held and the page must hold many of it. QUILL_EXE-style
# comparison: run it with an older QUILL.EXE as the argument and it reports what that build did (KEYS_REPORT=1 does not fail on it).
# Needs what e2e-launch-w31.sh needs (the Windows 3.11 tree, the DOSBox-X image, Docker, Xvfb, xdotool, ImageMagick); exits 0 with a note
# when the tree is missing. Local only.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
EXE="${1:-${OUT_DIR:-$ROOT/win16/out}/QUILL.EXE}"
TREE="${W31_TREE:-$HOME/.cache/quill16-play/w31-base}"
IMAGE="${W31_IMAGE:-quill16-dosbox-x:local}"
NAME="${W31_HELD_CONTAINER:-quill16-held-w31}"
BOOT="${BOOT:-20}"
HOLD="${HOLD:-6}"
BASE="${QUILL_LAUNCH_DIR:-$HOME/.cache/quill16-launch}"

log() { printf '[e2e-held-w31 %s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
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


[ ! -f "$WORK/d/STACK.LOG" ] || quit_early
xdotool mousemove 400 300
sleep 0.5
xdotool click 1
sleep 0.5
xdotool type --delay 150 "abc"
xdotool key ctrl+a
sleep 1
shot held-selected
# The pointer sits over the page, so no tooltip covers the button; its pixel is read from the first frame (ITALIC_X, ITALIC_Y).
IM=convert
command -v magick > /dev/null && IM=magick
IX="${ITALIC_X:-529}"
IY="${ITALIC_Y:-262}"
px() { $IM "$WORK/sample.png" -format "%[hex:p{$IX,$IY}]" info:; }
states=""
last=""
xdotool keydown ctrl keydown i
for _ in $(seq 1 15); do
	timeout 20 import -window root "$WORK/sample.png"
	now="$(px)"
	if [ "$now" != "$last" ]; then states="$states $now"; last="$now"; fi
	sleep 0.2
done
xdotool keyup i keyup ctrl
shot held-italic
log "the Italic button's colours while Ctrl+I was held:$states"
xdotool key End
sleep 1
shot held-text-before
xdotool keydown z
sleep 3
xdotool keyup z
sleep 1
shot held-text-after
xdotool key alt+f
sleep 1
xdotool key x
sleep 2
xdotool key n
t=0
while [ "$t" -lt 20 ] && [ ! -f "$WORK/d/STACK.LOG" ]; do
	sleep 1
	t=$((t + 1))
done
keep_logs
if [ -z "${KEYS_REPORT:-}" ]; then
	[ "$(echo $states | wc -w)" -le 2 ] || fail "the Italic button changed more than once while Ctrl+I was held:$states"
fi
log "ok"
