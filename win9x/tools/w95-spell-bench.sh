#!/usr/bin/env bash
# Measures Check Spelling as You Type on Windows 95 in DOSBox-X at a fixed CPU speed: the idle scan's cost a word and a screenful, and the
# time from a key to the end of the repaint, with the feature on or off.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/w95-spell-bench.sh --cycles N [--off] [--zoom Width|PERCENT] [--depth BITS] [--exe BENCH.EXE] [--out DIR]
#   --cycles  DOSBox-X cycles a millisecond (`cycles=fixed N`): by DOSBox's rule of thumb 3000 is a 386SX and 12000 a 486DX-33
#   --off     starts with `[Quill] AsYouType=0`
#   --zoom    `Zoom=` in QUILL.INI: Width (the default; at 640 by 480 that is 74 %, drawn a character at a time) or a percentage such as 100
#             (a run at a time)
#   --depth   the colour depth of the guest, 8, 16 or 24 or 32: the system disk is $W95_BASE/win95-base-BITSbpp.img when it exists (a
#             copy of win95-base.img whose display is set to that depth); without --depth the base image's own depth is used. The log
#             records the depth the guest ran at (the `display:` line)
#   --exe     a QUILL32.EXE built with `win9x/build.sh --bench` (the `bench` feature; it writes D:\SPELL.LOG); default dist/QUILL32-BENCH.EXE
# The product build has no timing code. The document is a screenful of 12 point text with two misspellings in each sentence. 14 keys are
# typed after the first scan, W95_KEY_DELAY milliseconds apart (default 2000, so that each key is repainted alone; use 6000 at 3000
# cycles) from W95_KEY_TEXT. The log lands in --out as spell-CYCLES-on.log (or -off), with -zoomZ and -BITSbpp added for a zoom other than
# Width and for --depth. Other environment as in w95-spell-live.sh, with W95_SCAN_WAIT, W95_KEY_WAIT and W95_DIALOG_WAIT (seconds) to
# wait longer on a slow CPU. The test bed's own folder, container and display are kept apart from the other scripts' (W95_HOME, W95_CONTAINER and
# W95_DISPLAY below), so this can run beside them.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
CYCLES=""
ON=1
ZOOM=Width
DEPTH=""
EXE="$HERE/dist/QUILL32-BENCH.EXE"
OUT="$PWD"
while [ "$#" -gt 0 ]; do
	case "$1" in
	--cycles) CYCLES="$2"; shift 2 ;;
	--off) ON=0; shift ;;
	--zoom) ZOOM="$2"; shift 2 ;;
	--depth) DEPTH="$2"; shift 2 ;;
	--exe) EXE="$2"; shift 2 ;;
	--out) OUT="$(mkdir -p "$2" && cd "$2" && pwd)"; shift 2 ;;
	*) echo "w95-spell-bench: unknown option $1" >&2; exit 2 ;;
	esac
done
[ -n "$CYCLES" ] || { echo "w95-spell-bench: --cycles is required" >&2; exit 2; }
case "$ZOOM" in Width | [0-9] | [0-9][0-9] | [0-9][0-9][0-9]) ;; *) echo "w95-spell-bench: --zoom is Width or a percentage" >&2; exit 2 ;; esac
case "$DEPTH" in "" | 8 | 16 | 24 | 32) ;; *) echo "w95-spell-bench: --depth is 8, 16, 24 or 32" >&2; exit 2 ;; esac
[ -f "$EXE" ] || { echo "w95-spell-bench: no $EXE" >&2; exit 1; }
INSTALLER="${LIMINAL_INSTALLER:-$HOME/source/liminal-hq-labs/liminal-installer}"
BASE_DIR="${W95_BASE:-/mnt/games/media/win95-testbed}"
BASE_IMG="$BASE_DIR/win95-base.img"
if [ -n "$DEPTH" ]; then
	BASE_IMG="$BASE_DIR/win95-base-${DEPTH}bpp.img"
	[ -f "$BASE_IMG" ] || { echo "w95-spell-bench: no $BASE_IMG: make a copy of win95-base.img with the display set to $DEPTH bits" >&2; exit 1; }
fi
export W95_HOME="${W95_HOME:-$HOME/.cache/quill-w95-spell}"
export W95_DISPLAY="${W95_DISPLAY:-:177}"
export W95_CONTAINER="${W95_CONTAINER:-quill-spell-dbx}"
export W95_SHUTDOWN_WAIT="${W95_SHUTDOWN_WAIT:-150}"
# shellcheck source=/dev/null
. "$INSTALLER/scripts/win95/lib.sh"
w95_need docker Xvfb xdotool import convert sfdisk mkfs.fat mcopy truncate flock awk
w95_lock
RUN="$W95_HOME/runs/$(date +%Y%m%d-%H%M%S)-bench-$CYCLES"
mkdir -p "$RUN/stage"
trap 'w95_stop; w95_xvfb_stop; rm -f "$RUN/work.img" "$RUN/exchange.img"' EXIT
OFF=$((W95_PART_START * 512))
fail() { echo "w95-spell-bench: FAIL: $*" >&2; exit 1; }

(cd "$ROOT" && cargo run -q -p quill-dict -- "$RUN/dic" > /dev/null)
cp "$EXE" "$RUN/stage/QUILL32.EXE"
cp "$RUN/dic/EN_CA.DIC" "$RUN/stage/EN_CA.DIC"
# A screenful of 12 point text: the same sentence over and over, two misspellings in each.
python3 -I - "$RUN/stage/BENCH.RTF" << 'PY'
import sys
s = "The quick brown fox jumped over the lazy dog with some fine words and a sentense is teh error. "
open(sys.argv[1], "w", newline="").write("{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Times New Roman;}}\\f0\\fs24 " + s * 14 + "\\par}\r\n")
PY
cp --sparse=always "$BASE_IMG" "$RUN/work.img"
chmod u+w "$RUN/work.img"
mcopy -n -i "$RUN/work.img@@$OFF" ::/WINDOWS/WIN.INI "$RUN/WIN.INI"
awk 'BEGIN { IGNORECASE = 1 } /^[ \t]*run[ \t]*=/ { printf "run=D:\\QUILL32.EXE\r\n"; done = 1; next } { print } END { if (!done) exit 3 }' "$RUN/WIN.INI" > "$RUN/WIN.INI.new" || fail "WIN.INI has no run= line"
mcopy -o -i "$RUN/work.img@@$OFF" "$RUN/WIN.INI.new" ::/WINDOWS/WIN.INI
printf '[Quill]\r\nZoom=%s\r\nAsYouType=%s\r\n' "$ZOOM" "$ON" > "$RUN/QUILL.INI"
mcopy -o -i "$RUN/work.img@@$OFF" "$RUN/QUILL.INI" ::/WINDOWS/QUILL.INI
w95_make_dimage "$RUN/exchange.img" "$RUN/stage"
{
	printf '[sdl]\noutput=surface\nautolock=false\nmouse_emulation=always\n\n[dosbox]\nmachine=svga_s3\nmemsize=64\n\n'
	printf '[cpu]\ncore=normal\ncputype=pentium\ncycles=fixed %s\n\n' "$CYCLES"
	printf '[sblaster]\nsbtype=none\n\n[gus]\ngus=false\n\n[dos]\nver=7.1\n\n[autoexec]\n@echo off\n'
	printf 'imgmount c /w/%s/work.img -size %s\n' "${RUN#"$W95_HOME"/}" "$W95_C_GEOM"
	printf 'imgmount 3 /w/%s/exchange.img -t hdd -fs none -size %s\nboot -l c\n' "${RUN#"$W95_HOME"/}" "$W95_D_GEOM"
} > "$RUN/run.conf"
w95_log "run folder: $RUN (cycles $CYCLES, as you type $ON, zoom $ZOOM, depth ${DEPTH:-image default})"
T0=$(date +%s)
w95_start "$RUN/run.conf"
waited=0
until [ "$(convert "$RUN/cur.png" -format '%[hex:p{320,300}]' info: 2> /dev/null || true)" = FFFFFF ]; do
	w95_running || fail "DOSBox-X exited"
	[ "$waited" -lt 3000 ] || fail "Quill never showed its page"
	w95_shot "$RUN/cur.png" || true
	sleep 5
	waited=$((waited + 5))
done
w95_log "Quill is up after $(($(date +%s) - T0)) seconds"
# WIN.INI's run= takes no arguments, so the document is opened from File > Open.
sleep 10
w95_key ctrl+o
sleep "${W95_DIALOG_WAIT:-15}"
w95_type 'D:\BENCH.RTF'
w95_key Return
sleep "${W95_DIALOG_WAIT:-15}"
# The first scan: let it finish (a screenful at a word a tick takes seconds; a slow CPU takes longer).
sleep "${W95_SCAN_WAIT:-90}"
w95_shot "$RUN/scanned.png"
# 30 keys, 0.2 s apart, a misspelling in them.
DISPLAY="$W95_DISPLAY" xdotool type --delay "${W95_KEY_DELAY:-2000}" "${W95_KEY_TEXT:-Teh quikc fox }"
sleep "${W95_KEY_WAIT:-40}"
w95_shot "$RUN/typed.png"
w95_shutdown
tag="$CYCLES-$([ "$ON" = 1 ] && echo on || echo off)"
[ "$ZOOM" = Width ] || tag="$tag-zoom$ZOOM"
[ -z "$DEPTH" ] || tag="$tag-${DEPTH}bpp"
cp "$RUN/scanned.png" "$OUT/spell-$tag-scanned.png"
cp "$RUN/typed.png" "$OUT/spell-$tag-typed.png"
mcopy -n -i "$RUN/exchange.img@@$OFF" ::/SPELL.LOG "$OUT/spell-$tag.log" || fail "the guest wrote no SPELL.LOG (screenshots are in $OUT)"
echo "w95-spell-bench: wrote $OUT/spell-$tag.log"
