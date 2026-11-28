#!/bin/bash
# Holds keys down for QUILLDOS.EXE through DOSBox's own keyboard (typematic repeat), with xdotool in a virtual X server of its own.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/held-keys-x11.sh [shots-dir]
#
# Needs Xvfb, xdotool, ImageMagick's `import` and python3, and the DOS tools (dos/tools/fetch-tools.sh); exits 0 with a note when they are
# missing. Starts an Xvfb on a free display of its own (DISPLAY is set explicitly for every command, WAYLAND_DISPLAY is dropped, and only the
# PID this script started is stopped) and runs QUILLDOS.EXE under DOSBox's x11 output, where a key held down is repeated by the emulated keyboard
# controller as a real one is: the BIOS buffer gets a new key every few hundredths of a second, which is what Quill reads.
#
# Graphics (`/G16` and `/G`): tests/scenarios/g18-held-keys.keys by hand. Ctrl+I, Ctrl+B and Ctrl+U are each held for KEYS_HOLD seconds (default
# 1.5) and Left is held to the start of the line; the frame the program ends on (the question about saving) must be the host's
# (tests/golden/gfx/g18-held-keys.{16,256}.final): every toggle acted once and the cursor key kept repeating, whatever the number of
# frames in between. Text screen: Insert is held; the status line's OVR must have come on once and stayed on. KEYS_EXE names another
# QUILLDOS.EXE to run (to show what an older build did: the script then reports its counts and does not fail on them).
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
VENDOR="$("$HERE/tools/vendor-dir.sh" "$HERE/vendor")"
SHOTS="${1:-}"
EXE="${KEYS_EXE:-$HERE/dist/QUILLDOS.EXE}"
HOLD="${KEYS_HOLD:-1.5}"
for t in Xvfb xdotool import python3; do
	command -v "$t" > /dev/null || { echo "held-keys-x11: $t not found; skipping"; exit 0; }
done
if [ ! -f "$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE" ] || [ -z "$(ls "$VENDOR"/dosbox/*/dosbox 2> /dev/null)" ]; then
	echo "held-keys-x11: DOS tools not present; skipping (run dos/tools/fetch-tools.sh)"
	exit 0
fi
[ -n "${KEYS_EXE:-}" ] || "$HERE/build.sh" > /dev/null
unset WAYLAND_DISPLAY
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-keys.XXXXXX")"
XPID=""
cleanup() {
	[ -z "$XPID" ] || kill "$XPID" 2> /dev/null || true
	rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "held-keys-x11: FAIL: $*" >&2; exit 1; }

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

SETTLE="${KEYS_SETTLE:-3}"
hold() { # <chord, e.g. ctrl+i>: the keys down in order, for HOLD seconds, then up in reverse
	local keys i
	IFS=+ read -ra keys <<< "$1"
	for k in "${keys[@]}"; do x xdotool keydown "$k"; done
	sleep "$HOLD"
	for ((i = ${#keys[@]} - 1; i >= 0; i--)); do x xdotool keyup "${keys[$i]}"; done
	sleep "$SETTLE"
}
tap() { x xdotool key "$@"; sleep 1; }

run() { # <label> <arguments> <16|256|text>
	local label="$1" args="$2" depth="$3"
	local disk="$WORK/disk-$label" keep="$WORK/keep-$label"
	mkdir -p "$disk"
	cp "$EXE" "$disk/QUILLDOS.EXE"
	cp "$ROOT/tests/fonts/QUILLTST.FNT" "$disk/QUILL.FNT"
	(
		export DISPLAY=":$N" DOS_X11=1 DOS_KEEP="$keep" DOS_TIMEOUT=300 DOS_VENDOR="$VENDOR"
		export DOS_ENV=$'QUILL_DUMP=D:\\DUMP.TXT'
		export DOS_CONF=$'[sdl]\nwindowresolution = 640x480'
		"$HERE/tools/dosrun.sh" "$disk" "QUILLDOS.EXE $args" > "$WORK/$label.out" 2>&1
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
	sleep 12
	x xdotool mousemove $((X + 300)) $((Y + 300)); sleep 1
	if [ "$depth" = text ]; then
		hold Insert
		shot "$label-insert.png"
		tap ctrl+q
	else
		tap h; tap i; tap ctrl+a
		hold ctrl+i
		shot "$label-italic.png"
		hold ctrl+b
		hold ctrl+u
		tap Right
		hold Left
		shot "$label-end.png"
		tap ctrl+q
		sleep "$SETTLE"
		shot "$label-ask.png"
		tap n
	fi
	local rc=0
	wait "$pid" || rc=$?
	[ "$rc" = 0 ] || { cat "$WORK/$label.out" >&2; fail "$label: QUILLDOS.EXE or DOSBox exited with $rc"; }
	tr -d '\r' < "$keep/DUMP.TXT" > "$WORK/$label-frames.txt"
	if [ "$depth" = text ]; then
		python3 - "$WORK/$label-frames.txt" "$label" "${KEYS_EXE:-}" << 'PY' || exit 1
import sys
frames = open(sys.argv[1]).read().split("=== screen ")[1:]
state, flips = False, 0
for f in frames:
	rows = f.split("\n")
	status = rows[25] if len(rows) > 25 else ""
	on = "OVR" in status
	if on != state:
		flips += 1
		state = on
print(f"held-keys-x11: {sys.argv[2]}: Insert held {len(frames)} screens: OVR flipped {flips} times and ended {'on' if state else 'off'}")
if not sys.argv[3] and not (flips == 1 and state):
	print("held-keys-x11: FAIL: the toggle must act once for a hold", file=sys.stderr)
	sys.exit(1)
PY
		return
	fi
	local total want got
	total="$(grep -c '^frame' "$WORK/$label-frames.txt")"
	# The answer 'n' to the question draws one more frame as the program leaves; the one before it is the question.
	got="$(grep '^frame' "$WORK/$label-frames.txt" | tail -2 | head -1 | awk '{print "crc32 " $4}')"
	want="$(cat "$ROOT/tests/golden/gfx/g18-held-keys.$depth.final")"
	if [ "$got" = "$want" ]; then
		echo "held-keys-x11: $label: Ctrl+I, Ctrl+B and Ctrl+U each held $HOLD s acted once and Left repeated: the host's frame ($total frames drawn)"
	elif [ -n "${KEYS_EXE:-}" ]; then
		echo "held-keys-x11: $label: with $KEYS_EXE the last frame is '$got', the host's is '$want' ($total frames drawn)"
	else
		fail "$label: the frame before the exit is '$got' and the host's is '$want' ($total frames)"
	fi
}

run g18-held-12h /G16 16
run g18-held-vbe /G 256
run g18-held-text "" text
echo "held-keys-x11: ok"
