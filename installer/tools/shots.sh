#!/usr/bin/env bash
# Helpers for the installer tests that take screenshots (sourced, not run).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# A test that is given SHOTS=<dir> runs on its own virtual X server (never the desktop's), watches the emulator's window with
# `import -window root` and keeps the distinct frames. The emulator is a window on that server, so nothing here reads or types into
# the host's screen. Functions:
#
#   shots_xvfb "$0" "$@"        re-run the script under xvfb-run when SHOTS is set and it is not already
#   shots_display_start         start a private X server (no access control, nobody else can reach it by accident: it is on a free
#                               display number from 90 up) and export DISPLAY; for a container, which cannot use xvfb-run's cookie
#   shots_display_stop          stop it
#   shots_watch_start DIR       start capturing a frame about every second into DIR (clears DIR first)
#   shots_watch_stop            stop the capture
#   shots_distinct DIR OUTDIR   copy the frames that differ from the one before to OUTDIR/NN.png, black frames left out
#   shots_crop GEOMETRY FILE    crop a frame to WxH+X+Y
#   shots_bounds FILE...        print the box that holds the picture in all of the frames (black borders left out)
#   shots_keep FILE NAME        write FILE to $SHOTS/NAME.png
#
# Needs xvfb-run and ImageMagick (`import`, `convert`).

shots_xvfb() {
	[ -n "${SHOTS:-}" ] || return 0
	[ -z "${QUILL_SHOTS_XVFB:-}" ] || return 0
	if ! command -v xvfb-run >/dev/null || ! command -v import >/dev/null; then
		echo "SHOTS needs xvfb-run and ImageMagick" >&2
		exit 1
	fi
	mkdir -p "$SHOTS"
	SHOTS="$(cd "$SHOTS" && pwd)"
	export SHOTS
	# The desktop's display never reaches the emulator.
	unset WAYLAND_DISPLAY XAUTHORITY
	QUILL_SHOTS_XVFB=1 exec xvfb-run -a -s "-screen 0 ${SHOTS_SCREEN:-1024x768x24}" "$@"
}

# ImageMagick 7 wants `magick`; `convert` is the older spelling of the same program.
_im() { if command -v magick >/dev/null; then magick "$@"; else convert "$@"; fi; }

SHOTS_XPID=""
shots_display_start() {
	local n=90 geom="${SHOTS_SCREEN:-1024x768x24}"
	unset WAYLAND_DISPLAY XAUTHORITY
	while [ -e "/tmp/.X11-unix/X$n" ] || [ -e "/tmp/.X$n-lock" ]; do n=$((n + 1)); done
	Xvfb ":$n" -screen 0 "$geom" -ac -nolisten tcp >/dev/null 2>&1 &
	SHOTS_XPID=$!
	export DISPLAY=":$n"
	for _ in $(seq 1 50); do
		[ -S "/tmp/.X11-unix/X$n" ] && return 0
		sleep 0.1
	done
	echo "shots: the virtual X server did not start" >&2
	return 1
}
shots_display_stop() {
	[ -z "$SHOTS_XPID" ] || { kill "$SHOTS_XPID" 2>/dev/null || true; wait "$SHOTS_XPID" 2>/dev/null || true; }
	SHOTS_XPID=""
}

# shots_window FILE: the emulator's window (or the whole display when there is none yet) as a PNG.
shots_window() {
	local id
	id="$(xdotool search --onlyvisible --class dosbox 2>/dev/null | head -n 1 || true)"
	if [ -n "$id" ]; then
		timeout 20 import -window "$id" "$1" 2>/dev/null || timeout 20 import -window root "$1"
	else
		timeout 20 import -window root "$1"
	fi
}

SHOTS_PID=""
shots_watch_start() {
	rm -rf "$1"
	mkdir -p "$1"
	(
		i=0
		while true; do
			i=$((i + 1))
			import -window root "$1/f$(printf %05d "$i").png" 2>/dev/null || true
			sleep "${SHOTS_INTERVAL:-0.6}"
		done
	) &
	SHOTS_PID=$!
}

shots_watch_stop() {
	[ -z "$SHOTS_PID" ] || { kill "$SHOTS_PID" 2>/dev/null || true; wait "$SHOTS_PID" 2>/dev/null || true; }
	SHOTS_PID=""
}

shots_distinct() {
	local dir="$1" out="$2" prev="" sum f n=0
	mkdir -p "$out"
	for f in "$dir"/f*.png; do
		[ -s "$f" ] || continue
		# A frame of an empty screen (the emulator not up yet, or already gone) is not a page.
		[ "$(_im "$f" -format '%[fx:maxima]' info: 2>/dev/null)" != 0 ] || continue
		sum="$(_im "$f" rgb:- 2>/dev/null | md5sum | cut -d' ' -f1)"
		[ "$sum" != "$prev" ] || continue
		prev="$sum"
		n=$((n + 1))
		cp "$f" "$out/$(printf %02d "$n").png"
	done
	echo "$n"
}

# The smallest box that holds the picture of every frame named (the bounds of the non-black part, joined).
shots_bounds() {
	local f
	for f in "$@"; do _im "$f" -format '%@\n' info:; done | awk -F'[x+]' '
		NR == 1 || $3 < x0 { x0 = $3 } NR == 1 || $4 < y0 { y0 = $4 }
		NR == 1 || $3 + $1 > x1 { x1 = $3 + $1 } NR == 1 || $4 + $2 > y1 { y1 = $4 + $2 }
		END { printf "%dx%d+%d+%d\n", x1 - x0, y1 - y0, x0, y0 }'
}

shots_crop() { _im "$2" -crop "$1" +repage "$2"; }

shots_keep() {
	mkdir -p "$SHOTS"
	cp "$1" "$SHOTS/$2.png"
}
