#!/bin/bash
# Takes a 640 by 480 screenshot of QUILLDOS.EXE in graphics mode from DOSBox Staging running in a private Xvfb of its own.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/shot.sh <out.png> <machine> <arguments> <scenario.keys> <document> [keys]
#
#   <out.png>       where the picture goes
#   <machine>       svga_s3 (the VBE mode, with /G) or any machine dosrun.sh knows
#   <arguments>     QUILLDOS.EXE's arguments, for example /G16
#   <scenario.keys> a scenario of tests/scenarios; its @copy files go on the disk and its keys are typed
#   <document>      the file QUILLDOS.EXE opens (a name from the scenario's @copy lines), or - for none
#   [keys]          the keys to type instead of the scenario's, so the screen stops where the picture should be
#
# With SHOT_MOUSE_LINES=N the first N lines of the scenario are played as a QUILL_MOUSE script instead of being typed (pointer
# directives and keys alike, played by QUILLDOS.EXE in place of the driver), and the program holds on the last frame for the picture:
# a menu open under the pointer, a button held down, a tip, a thumb mid-drag. SHOT_MOUSE_WAIT_S says how long to wait for it (default 30).
#
# The X server is started here on a free display number, its PID recorded and only that PID stopped at the end; no other display
# or emulator is touched, and nothing is captured from the desktop. DOSBox draws into a window of exactly 640 by 480, so the picture
# is the card's frame pixel for pixel. The program is left waiting for a key, and the run's own time limit ends it.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
OUT="$1" MACHINE="$2" ARGS="$3" SCEN="$4" DOC="$5" KEYS="${6:-}"
file="$ROOT/tests/scenarios/$SCEN.keys"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-shot.XXXXXX")"
XPID=""
cleanup() {
	[ -z "$XPID" ] || kill "$XPID" 2> /dev/null || true
	rm -rf "$WORK"
}
trap cleanup EXIT

"$HERE/build.sh" > /dev/null
disk="$WORK/disk"
mkdir -p "$disk"
cp "$HERE/dist/QUILLDOS.EXE" "$disk/"
cp "$ROOT/tests/fonts/QUILLTST.FNT" "$disk/QUILL.FNT"
[ -z "${SHOT_FNT:-}" ] || cp "$SHOT_FNT" "$disk/QUILL.FNT"
while read -r d a; do
	[ "$d" != @copy ] || cp "$ROOT/tests/docs/$a" "$disk/$(echo "$a" | tr '[:lower:]' '[:upper:]')"
done < <(grep '^@' "$file" | sed 's/#.*//')
if [ -z "$KEYS" ]; then
	KEYS="$(grep -v '^@' "$file" | sed 's/#.*//' | tr -s ' \n\t' ' ' | sed 's/^ //; s/ $//')"
fi
count=$(wc -w <<< "$KEYS")

n=90
while [ -e "/tmp/.X11-unix/X$n" ]; do n=$((n + 1)); done
Xvfb ":$n" -screen 0 640x480x24 -nolisten tcp > /dev/null 2>&1 &
XPID=$!
for _ in $(seq 1 50); do
	[ -e "/tmp/.X11-unix/X$n" ] && break
	sleep 0.1
done
[ -e "/tmp/.X11-unix/X$n" ] || { echo "shot: the X server did not start" >&2; exit 1; }
echo "shot: Xvfb on :$n, pid $XPID"

wait_s=$((6 + count * ${SHOT_PACE_S:-2}))
[ -z "${SHOT_MOUSE_LINES:-}" ] || wait_s="${SHOT_MOUSE_WAIT_S:-30}"
export DISPLAY=":$n" DOS_X11=1 DOS_MACHINE="$MACHINE" DOS_TIMEOUT=$((wait_s + 4))
export DOS_CONF=$'[sdl]\nwindow_size = 640x480\nwindowresolution = 640x480\n[render]\naspect = false\ninteger_scaling = off\nviewport_resolution = fit'
export DOS_ENV='QUILL_DUMP=D:\DUMP.TXT'
export DOS_AUTOTYPE="-w 2 -p 0.6 $KEYS"
if [ -n "${SHOT_MOUSE_LINES:-}" ]; then
	head -n "$SHOT_MOUSE_LINES" "$file" > "$disk/MOUSE.TXT"
	export DOS_ENV=$'QUILL_DUMP=D:\\DUMP.TXT\nQUILL_MOUSE=MOUSE.TXT\nQUILL_HOLD=1'
	unset DOS_AUTOTYPE
fi
args="QUILLDOS.EXE $ARGS"
[ "$DOC" = - ] || args="$args $DOC"
"$HERE/tools/dosrun.sh" "$disk" "$args" > /dev/null 2>&1 &
RUN=$!
sleep "$wait_s"
import -display ":$n" -window root "$OUT"
wait "$RUN" || true
echo "shot: wrote $OUT"
