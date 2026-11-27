#!/usr/bin/env bash
# Runs W32SPIKE.EXE as the Windows 95 shell program (WIN.INI run=) in DOSBox-X at a fixed CPU speed, waits for the window to turn green or
# red, shuts Windows down and copies SPIKE.LOG out of the exchange disk.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/w32-worker/tools/w95-run.sh --cycles N [--mem MB] [--out DIR] [KEY=VALUE...]
#   --cycles  DOSBox-X `cycles=fixed N` (by DOSBox's rule of thumb 3000 is a 386SX and 12000 a 486DX-33)
#   --mem     the guest's memory in MB (default 64; the plan's Win32 rule is 8)
#   --out     where to put w95-CYCLES-MEMmb[-TAG].log and the final screenshot (default the current folder)
#   KEY=VALUE lines for SPIKE.CFG (doc, keys, keyms, cycles, stressdoc, exit, yield, phases; see main.rs); `hold=1` is added unless exit=abrupt
# Reuses the installer repo's Windows 95 test bed (scripts/win95/lib.sh) and Quill's win95-base.img (an OSR2 install), with its own folder,
# container and display so it can run beside the other scripts. The system disk is copied for the run and the copy deleted afterwards.
# W95_RUN_WAIT is the seconds to wait for the verdict (default 3000).
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
CYCLES=""
MEM=64
OUT="$PWD"
TAG=""
CFG=()
while [ "$#" -gt 0 ]; do
	case "$1" in
	--cycles) CYCLES="$2"; shift 2 ;;
	--mem) MEM="$2"; shift 2 ;;
	--tag) TAG="-$2"; shift 2 ;;
	--out) OUT="$(mkdir -p "$2" && cd "$2" && pwd)"; shift 2 ;;
	*=*) CFG+=("$1"); shift ;;
	*) echo "w95-run: unknown option $1" >&2; exit 2 ;;
	esac
done
[ -n "$CYCLES" ] || { echo "w95-run: --cycles is required" >&2; exit 2; }
EXE="$HERE/dist/W32SPIKE.EXE"
[ -f "$EXE" ] || { echo "w95-run: build first (tools/build.sh)" >&2; exit 1; }
INSTALLER="${LIMINAL_INSTALLER:-$HOME/source/liminal-hq-labs/liminal-installer}"
BASE_IMG="${W95_BASE:-/mnt/games/media/win95-testbed}/win95-base.img"
[ -f "$BASE_IMG" ] || { echo "w95-run: no $BASE_IMG" >&2; exit 1; }
export W95_HOME="${W95_HOME:-$HOME/.cache/w32spike-w95}"
export W95_DISPLAY="${W95_DISPLAY:-:178}"
export W95_CONTAINER="${W95_CONTAINER:-w32spike-dbx}"
export W95_SHUTDOWN_WAIT="${W95_SHUTDOWN_WAIT:-150}"
# shellcheck source=/dev/null
. "$INSTALLER/scripts/win95/lib.sh"
w95_need docker Xvfb xdotool import convert sfdisk mkfs.fat mcopy truncate flock awk
w95_lock
RUN="$W95_HOME/runs/$(date +%Y%m%d-%H%M%S)-$CYCLES"
mkdir -p "$RUN/stage"
trap 'w95_stop; w95_xvfb_stop; rm -f "$RUN/work.img" "$RUN/exchange.img"' EXIT
OFF=$((W95_PART_START * 512))
fail() { echo "w95-run: FAIL: $*" >&2; exit 1; }

cp "$EXE" "$RUN/stage/W32SPIKE.EXE"
hold=1
for kv in "${CFG[@]}"; do [ "$kv" != exit=abrupt ] || hold=0; done
printf '%s\r\n' "${CFG[@]}" "hold=$hold" > "$RUN/stage/SPIKE.CFG"
cp --sparse=always "$BASE_IMG" "$RUN/work.img"
chmod u+w "$RUN/work.img"
mcopy -n -i "$RUN/work.img@@$OFF" ::/WINDOWS/WIN.INI "$RUN/WIN.INI"
awk 'BEGIN { IGNORECASE = 1 } /^[ \t]*run[ \t]*=/ { printf "run=D:\\W32SPIKE.EXE\r\n"; done = 1; next } { print } END { if (!done) exit 3 }' "$RUN/WIN.INI" > "$RUN/WIN.INI.new" || fail "WIN.INI has no run= line"
mcopy -o -i "$RUN/work.img@@$OFF" "$RUN/WIN.INI.new" ::/WINDOWS/WIN.INI
w95_make_dimage "$RUN/exchange.img" "$RUN/stage"
{
	printf '[sdl]\noutput=surface\nautolock=false\nmouse_emulation=always\n\n[dosbox]\nmachine=svga_s3\nmemsize=%s\n\n' "$MEM"
	printf '[cpu]\ncore=normal\ncputype=pentium\ncycles=fixed %s\n\n' "$CYCLES"
	printf '[sblaster]\nsbtype=none\n\n[gus]\ngus=false\n\n[dos]\nver=7.1\n\n[autoexec]\n@echo off\n'
	printf 'imgmount c /w/%s/work.img -size %s\n' "${RUN#"$W95_HOME"/}" "$W95_C_GEOM"
	printf 'imgmount 3 /w/%s/exchange.img -t hdd -fs none -size %s\nboot -l c\n' "${RUN#"$W95_HOME"/}" "$W95_D_GEOM"
} > "$RUN/run.conf"
w95_log "run folder: $RUN (cycles $CYCLES, memory $MEM MB, config: ${CFG[*]:-defaults})"
T0=$(date +%s)
w95_start "$RUN/run.conf"
pixel() { convert "$RUN/cur.png" -format '%[hex:p{320,300}]' info: 2> /dev/null || true; }
verdict=""
seen_white=0
while :; do
	w95_running || fail "DOSBox-X exited"
	[ $(($(date +%s) - T0)) -lt "${W95_RUN_WAIT:-3000}" ] || break
	w95_shot "$RUN/cur.png" || true
	p="$(pixel)"
	# The display may be 256 colours, so the green and the red come back as the nearest palette entries: tell them apart by channel.
	case "$p" in
	FFFFFF*) seen_white=1 ;;
	[0-9A-F][0-9A-F][0-9A-F][0-9A-F][0-9A-F][0-9A-F]*)
		r=$((16#${p:0:2})) g=$((16#${p:2:2})) b=$((16#${p:4:2}))
		if [ "$seen_white" = 1 ] && [ "$g" -gt $((r + 64)) ] && [ "$g" -gt $((b + 64)) ]; then
			verdict=green
		elif [ "$seen_white" = 1 ] && [ "$r" -gt $((g + 64)) ] && [ "$r" -gt $((b + 64)) ]; then
			verdict=red
		elif [ "$seen_white" = 1 ] && [ "$hold" = 0 ]; then
			verdict=gone
		fi
		;;
	esac
	[ -z "$verdict" ] || break
	sleep 10
done
w95_log "verdict colour: ${verdict:-none within the wait} after $(($(date +%s) - T0)) seconds"
w95_shot "$OUT/w95-$CYCLES-${MEM}mb$TAG.png" || true
w95_shutdown
if mcopy -n -i "$RUN/exchange.img@@$OFF" ::/SPIKE.LOG "$OUT/w95-$CYCLES-${MEM}mb$TAG.log"; then
	echo "w95-run: wrote $OUT/w95-$CYCLES-${MEM}mb$TAG.log (window: ${verdict:-none})"
else
	fail "the guest wrote no SPIKE.LOG (window: ${verdict:-none})"
fi
