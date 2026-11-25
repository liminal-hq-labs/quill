#!/bin/bash
# Drives QUILLDOS.EXE's mouse through the real INT 33h of DOSBox's built-in driver, with xdotool in a virtual X server of its own.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/mouse-x11.sh [shots-dir]
#
# Needs Xvfb, xdotool and ImageMagick's `import`, and the DOS tools (dos/tools/fetch-tools.sh); exits 0 with a note when they are missing.
# Starts an Xvfb on a free display of its own (never the desktop's: DISPLAY is set explicitly for every command, and only the PID this
# script started is stopped), runs QUILLDOS.EXE in graphics under DOSBox's x11 output with `mouse_capture = seamless`, and plays
# tests/scenarios/g8-real.keys by hand: a click and a drag at the same pixels (a double click is not played here: the program sees the second press only after it has drawn the frame for the first, which an emulated 486 takes longer over than the double-click time, so the clicks are two; the wheel is not played because DOSBox under Xvfb gave the driver no wheel counts). The last frame the program drew
# before the keys that exit must be the host's (tests/golden/gfx/g8-real.{16,256}.final), whatever the number of frames the
# polling in between made. With a directory it also keeps screenshots of the window.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
VENDOR="${DOS_VENDOR:-$HERE/vendor}"
SHOTS="${1:-}"
for t in Xvfb xdotool import; do
	command -v "$t" > /dev/null || { echo "mouse-x11: $t not found; skipping"; exit 0; }
done
if [ ! -f "$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE" ] || [ -z "$(ls "$VENDOR"/dosbox/*/dosbox 2> /dev/null)" ]; then
	echo "mouse-x11: DOS tools not present; skipping (run dos/tools/fetch-tools.sh)"
	exit 0
fi
"$HERE/build.sh" > /dev/null
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-mouse.XXXXXX")"
XPID=""
cleanup() {
	[ -z "$XPID" ] || kill "$XPID" 2> /dev/null || true
	rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "mouse-x11: FAIL: $*" >&2; exit 1; }

# A display number nobody uses.
N=""
for n in $(seq 60 98); do
	if [ ! -e "/tmp/.X11-unix/X$n" ] && [ ! -e "/tmp/.X$n-lock" ]; then N=$n; break; fi
done
[ -n "$N" ] || fail "no free display"
Xvfb ":$N" -screen 0 1024x768x24 -nolisten tcp > "$WORK/xvfb.log" 2>&1 &
XPID=$!
for _ in $(seq 1 50); do [ -e "/tmp/.X11-unix/X$N" ] && break; sleep 0.2; done
[ -e "/tmp/.X11-unix/X$N" ] || fail "the virtual X server did not start"
x() { DISPLAY=":$N" "$@"; }

# The pace of the emulated program is its own, so each action is given time to be seen, drawn and dumped.
SETTLE="${MOUSE_SETTLE:-7}"
run() { # <label> <arguments> <16|256>
	local label="$1" args="$2" depth="$3"
	local disk="$WORK/disk-$label" keep="$WORK/keep-$label"
	mkdir -p "$disk"
	cp "$HERE/dist/QUILLDOS.EXE" "$disk/"
	cp "$ROOT/tests/fonts/QUILLTST.FNT" "$disk/QUILL.FNT"
	cp "$ROOT/tests/docs/letter.rtf" "$disk/LETTER.RTF"
	(
		export DISPLAY=":$N" DOS_X11=1 DOS_KEEP="$keep" DOS_TIMEOUT=300 DOS_VENDOR="$VENDOR"
		export DOS_ENV=$'QUILL_DUMP=D:\\DUMP.TXT'
		export DOS_AUTOTYPE="-w $((SETTLE * 9 + 20)) f10 end enter"
		export DOS_CONF=$'[mouse]\nmouse_capture = seamless\n[sdl]\nwindowresolution = 640x480'
		"$HERE/tools/dosrun.sh" "$disk" "QUILLDOS.EXE $args LETTER.RTF" > "$WORK/$label.out" 2>&1
	) &
	local pid=$! win="" i
	for i in $(seq 1 60); do
		win="$(x xdotool search --class dosbox 2> /dev/null | head -1 || true)"
		[ -z "$win" ] || break
		sleep 1
	done
	[ -n "$win" ] || fail "$label: DOSBox's window did not appear"
	local X Y
	eval "$(x xdotool getwindowgeometry --shell "$win")"
	shot() { [ -z "$SHOTS" ] || { mkdir -p "$SHOTS"; x import -window root -crop "640x480+$X+$Y" +repage "$SHOTS/$1"; }; }
	sleep "$SETTLE"
	# Into the window first, so the driver sees a move before the press.
	x xdotool mousemove $((X + 200)) $((Y + 300)); sleep 1
	x xdotool mousemove $((X + 60)) $((Y + 95)); sleep "$SETTLE"
	shot "$label-cursor.png"
	x xdotool click 1; sleep "$SETTLE"
	x xdotool mousemove $((X + 150)) $((Y + 154)); sleep 2
	x xdotool mousedown 1; sleep 2
	x xdotool mousemove $((X + 240)) $((Y + 154)); sleep 2
	x xdotool mousemove $((X + 330)) $((Y + 154)); sleep 2
	x xdotool mouseup 1; sleep "$SETTLE"
	shot "$label-select.png"
	local rc=0
	wait "$pid" || rc=$?
	[ "$rc" = 0 ] || { cat "$WORK/$label.out" >&2; fail "$label: QUILLDOS.EXE or DOSBox exited with $rc"; }
	tr -d '\r' < "$keep/DUMP.TXT" > "$WORK/$label-frames.txt"
	local total last want
	total="$(grep -c '^frame' "$WORK/$label-frames.txt")"
	# The keys that exit (F10, End, Enter) change nothing on the page, so the last frame is the page as the pointer left it.
	last="$(grep '^frame' "$WORK/$label-frames.txt" | tail -1 | awk '{print "crc32 " $4}')"
	want="$(cat "$ROOT/tests/golden/gfx/g8-real.$depth.final")"
	[ "$last" = "$want" ] || { grep '^frame' "$WORK/$label-frames.txt" | awk '{print $1, $2, $4}' >&2; fail "$label: the last frame before the exit is '$last' and the host's is '$want' ($total frames)"; }
	echo "mouse-x11: $label: a click and a drag through INT 33h end on the host's frame ($total frames drawn)"
}

run g8-real-12h /G16 16
run g8-real-vbe /G 256
echo "mouse-x11: ok"
