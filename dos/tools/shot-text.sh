#!/bin/bash
# Takes a 720 by 400 screenshot of QUILLDOS.EXE in text mode from DOSBox Staging running in a private Xvfb of its own, with Check Spelling
# as You Type on.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/shot-text.sh <out.png> [mono] <keys>
#
#   <out.png>  where the picture goes
#   mono       run with /MONO (the reverse-video marks)
#   <keys>     the keys to type (DOSBox AUTOTYPE names; letters one by one), 0.8 s apart so the quiet comes between them
#
# The X server is started here on a free display number, its PID recorded and only that PID stopped at the end; no other display or
# emulator is touched, and nothing is captured from the desktop. The program is left waiting for a key; the run's own time limit ends it.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
OUT="$1"
shift
ARGS=""
if [ "${1:-}" = mono ]; then
	ARGS=" /MONO"
	shift
fi
KEYS="$*"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-shot-text.XXXXXX")"
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
cp "$ROOT/tests/docs/en_ca.dic" "$disk/EN_CA.DIC"
printf '[Quill]\r\nAsYouType=1\r\n' > "$disk/QUILL.INI"
count=$(wc -w <<< "$KEYS")

n=90
while [ -e "/tmp/.X11-unix/X$n" ]; do n=$((n + 1)); done
Xvfb ":$n" -screen 0 720x400x24 -nolisten tcp > /dev/null 2>&1 &
XPID=$!
for _ in $(seq 1 50); do
	[ -e "/tmp/.X11-unix/X$n" ] && break
	sleep 0.1
done
[ -e "/tmp/.X11-unix/X$n" ] || { echo "shot-text: the X server did not start" >&2; exit 1; }
echo "shot-text: Xvfb on :$n, pid $XPID"

wait_s=$((8 + count * 1))
unset WAYLAND_DISPLAY
export DISPLAY=":$n" DOS_X11=1 DOS_TIMEOUT=$((wait_s + 4))
export DOS_CONF=$'[sdl]\nwindow_size = 720x400\nwindowresolution = 720x400\n[render]\naspect = false\ninteger_scaling = off\nviewport_resolution = fit'
export DOS_AUTOTYPE="-w 2 -p 0.8 $KEYS"
"$HERE/tools/dosrun.sh" "$disk" "QUILLDOS.EXE$ARGS" > /dev/null 2>&1 &
RUN=$!
sleep "$wait_s"
import -display ":$n" -window root "$OUT"
wait "$RUN" || true
echo "shot-text: wrote $OUT"
