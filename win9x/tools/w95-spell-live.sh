#!/usr/bin/env bash
# Runs QUILL32.EXE on a real Windows 95 in DOSBox-X (Liminal Installer's test bed, headless, in a container and display of its own),
# types misspellings and takes screenshots of the wave, the right-click menu and a correction, at Page Width and at 100 percent.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/w95-spell-live.sh [--shots DIR] [--cycles max|N] [--zoom Width|100] [--off]     (build first with win9x/build.sh)
#   --cycles  the DOSBox-X CPU: `max` (default) or a fixed number of cycles a millisecond (by DOSBox's rule of thumb 3000 is a 386SX and
#             12000 a 486DX-33)
#   --zoom    the zoom QUILL.INI starts with (default Width)
#   --off     starts with `[Quill] AsYouType=0`, for comparing with the feature on
# Environment: LIMINAL_INSTALLER (a checkout with scripts/win95; default ~/source/liminal-hq-labs/liminal-installer), W95_BASE (the folder
# that holds win95-base.img, read only; default /mnt/games/media/win95-testbed), W95_HOME (this script's own folder for runs and
# state; default $TMPDIR/quill-w95-spell), W95_DISPLAY (default :177) and W95_CONTAINER (default quill-spell-dbx). It never touches the
# default display, container or folder of the other test bed scripts, so a Windows 95 window left open by hand is not disturbed.
# What it checks, by pixels: no red under correct text or under the word being typed; a red wave under each finished misspelling;
# the right-click menu opens (the page changes); the first suggestion takes its wave away. The screenshots go to --shots.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
SHOTS=""
CYCLES=max
ZOOM=Width
ON=1
while [ "$#" -gt 0 ]; do
	case "$1" in
	--shots) SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"; shift 2 ;;
	--cycles) CYCLES="$2"; shift 2 ;;
	--zoom) ZOOM="$2"; shift 2 ;;
	--off) ON=0; shift ;;
	*) echo "w95-spell-live: unknown option $1" >&2; exit 2 ;;
	esac
done
INSTALLER="${LIMINAL_INSTALLER:-$HOME/source/liminal-hq-labs/liminal-installer}"
[ -f "$INSTALLER/scripts/win95/lib.sh" ] || { echo "w95-spell-live: no test bed at $INSTALLER/scripts/win95; set LIMINAL_INSTALLER" >&2; exit 1; }
BASE_DIR="${W95_BASE:-/mnt/games/media/win95-testbed}"
[ -f "$BASE_DIR/win95-base.img" ] || { echo "w95-spell-live: no win95-base.img in $BASE_DIR" >&2; exit 1; }
[ -f "$HERE/dist/QUILL32.EXE" ] || { echo "w95-spell-live: build win9x/dist/QUILL32.EXE first" >&2; exit 1; }
export W95_HOME="${W95_HOME:-${TMPDIR:-/tmp}/quill-w95-spell}"
export W95_DISPLAY="${W95_DISPLAY:-:177}"
export W95_CONTAINER="${W95_CONTAINER:-quill-spell-dbx}"
# shellcheck source=/dev/null
. "$INSTALLER/scripts/win95/lib.sh"
# shellcheck source=/dev/null
. "$INSTALLER/scripts/win95/mouse.sh"
w95_need docker Xvfb xdotool import convert compare sfdisk mkfs.fat mcopy truncate flock awk
w95_lock
RUN="$W95_HOME/runs/$(date +%Y%m%d-%H%M%S)-spell"
mkdir -p "$RUN/stage"
trap 'w95_stop; w95_xvfb_stop; rm -f "$RUN/work.img" "$RUN/exchange.img"' EXIT
OFF=$((W95_PART_START * 512))
fail() { echo "w95-spell-live: FAIL: $*" >&2; exit 1; }
shot() {
	w95_shot "$RUN/$1.png"
	if [ -n "$SHOTS" ]; then cp "$RUN/$1.png" "$SHOTS/w95-spell-$1.png"; fi
}
# The red pixels of a screenshot's page area as "x y" lines.
red() { convert "$RUN/$1.png" -crop 590x300+10+140 +repage txt:- 2> /dev/null | awk -F'[,: ]+' '/#FF0000/ { print $1 + 10, $2 + 140 }'; }
ink() { convert "$RUN/$1.png" -crop "$(($3 - $2 + 1))x$(($5 - $4 + 1))+$2+$4" +repage txt:- 2> /dev/null | awk -F'[(),: ]+' 'NR > 1 && $3 < 90 && $4 < 90 && $5 < 90 { n++ } END { print n + 0 }'; }

# The pointer's centre in a screenshot: the dark pixels of the I-beam in the white page below the text (x 20 to 636, y from $PTR_Y0 to 400).
PTR_Y0=230
pointer_at() {
	convert "$RUN/$1.png" -crop "616x$((400 - PTR_Y0))+20+$PTR_Y0" +repage txt:- 2> /dev/null | awk -F'[(),: ]+' -v y_off="$PTR_Y0" 'NR > 1 && $3 < 110 && $4 < 110 && $5 < 110 { if (!n++ || $1 < x0) x0 = $1; if ($1 > x1) x1 = $1; if (!y0 || $2 < y0) y0 = $2; if ($2 > y1) y1 = $2 } END { if (n) print 20 + int((x0 + x1) / 2), y_off + int((y0 + y1) / 2) }'
}
# goto X Y: walks the pointer to X Y over white paper and corrects from where it is seen, up to eight times: the guest pointer is not
# always where the walk's arithmetic puts it.
goto() {
	local tx="$1" ty="$2" seen sx sy
	w95_mouse_move "$tx" "$ty"
	for _ in 1 2 3 4 5 6 7 8; do
		sleep 0.6
		w95_shot "$RUN/ptr.png"
		seen="$(pointer_at ptr)"
		[ -n "$seen" ] || return 0
		read -r sx sy <<< "$seen"
		[ "$((sx - tx))" -ge -1 ] && [ "$((sx - tx))" -le 1 ] && [ "$((sy - ty))" -ge -1 ] && [ "$((sy - ty))" -le 1 ] && return 0
		w95_log "pointer seen at $sx $sy, wanted $tx $ty"
		echo "$sx $sy" > "$W95_MOUSE_POS"
		w95_mouse_move "$tx" "$ty"
	done
}

# The disks: a throwaway copy of the base, and an exchange disk with the program and a dictionary.
(cd "$ROOT" && cargo run -q -p quill-dict -- "$RUN/dic" > /dev/null)
cp "$HERE/dist/QUILL32.EXE" "$RUN/stage/QUILL32.EXE"
cp "$RUN/dic/EN_CA.DIC" "$RUN/stage/EN_CA.DIC"
cp --sparse=always "$BASE_DIR/win95-base.img" "$RUN/work.img"
chmod u+w "$RUN/work.img"
mcopy -n -i "$RUN/work.img@@$OFF" ::/WINDOWS/WIN.INI "$RUN/WIN.INI"
awk 'BEGIN { IGNORECASE = 1 } /^[ \t]*run[ \t]*=/ { printf "run=D:\\QUILL32.EXE\r\n"; done = 1; next } { print } END { if (!done) exit 3 }' "$RUN/WIN.INI" > "$RUN/WIN.INI.new" || fail "WIN.INI has no run= line"
mcopy -o -i "$RUN/work.img@@$OFF" "$RUN/WIN.INI.new" ::/WINDOWS/WIN.INI
printf '[Quill]\r\nZoom=%s\r\nAsYouType=%s\r\nBlink=0\r\n' "$ZOOM" "$ON" > "$RUN/QUILL.INI"
mcopy -o -i "$RUN/work.img@@$OFF" "$RUN/QUILL.INI" ::/WINDOWS/QUILL.INI
w95_make_dimage "$RUN/exchange.img" "$RUN/stage"
{
	printf '[sdl]\noutput=surface\nautolock=false\nmouse_emulation=always\n\n'
	printf '[dosbox]\nmachine=svga_s3\nmemsize=64\n\n'
	if [ "$CYCLES" = max ]; then c=max; else c="fixed $CYCLES"; fi
	printf '[cpu]\ncore=normal\ncputype=pentium\ncycles=%s\n\n' "$c"
	printf '[sblaster]\nsbtype=none\n\n[gus]\ngus=false\n\n[dos]\nver=7.1\n\n[autoexec]\n@echo off\n'
	printf 'imgmount c /w/%s/work.img -size %s\n' "${RUN#"$W95_HOME"/}" "$W95_C_GEOM"
	printf 'imgmount 3 /w/%s/exchange.img -t hdd -fs none -size %s\nboot -l c\n' "${RUN#"$W95_HOME"/}" "$W95_D_GEOM"
} > "$RUN/run.conf"
w95_log "run folder: $RUN (cycles $CYCLES, zoom $ZOOM, as you type $ON)"
w95_start "$RUN/run.conf"

# Quill is up when the page's white shows in the middle of the window under the toolbars.
waited=0
until [ "$(convert "$RUN/cur.png" -format '%[hex:p{320,300}]' info: 2> /dev/null || true)" = FFFFFF ]; do
	w95_running || fail "DOSBox-X exited"
	[ "$waited" -lt 1500 ] || fail "Quill never showed its page"
	w95_shot "$RUN/cur.png" || true
	sleep 5
	waited=$((waited + 5))
done
w95_log "Quill is up after about $waited seconds"
sleep 5
w95_mouse_capture
goto 320 300
w95_mouse_xdo click 1
sleep 2
w95_type "The quick brown fox jumped over the lazy dog. "
sleep 6
shot clean
[ "$(red clean | wc -l)" -eq 0 ] || fail "red pixels under correct text"
w95_type "Teh"
sleep 4
shot typing
[ "$(red typing | wc -l)" -eq 0 ] || fail "red pixels while the word was being typed"
w95_type " quikc fox."
[ "$ON" = 0 ] || sleep 12
shot marked
n=$(red marked | wc -l)
if [ "$ON" = 0 ]; then
	[ "$n" -eq 0 ] || fail "red pixels with the feature off"
	echo "w95-spell-live: ok (off: no marks)"
	exit 0
fi
[ "$n" -gt 20 ] || fail "no wavy underline after the misspellings ($n red pixels)"
red marked > "$RUN/red.txt"
ymin=$(awk 'NR == 1 || $2 < m { m = $2 } END { print m }' "$RUN/red.txt")
ymax=$(awk 'NR == 1 || $2 > m { m = $2 } END { print m }' "$RUN/red.txt")
xmin=$(awk 'NR == 1 || $1 < m { m = $1 } END { print m }' "$RUN/red.txt")
xmax=$(awk 'NR == 1 || $1 > m { m = $1 } END { print m }' "$RUN/red.txt")
echo "w95-spell-live: $n red pixels, x $xmin to $xmax, y $ymin to $ymax"
[ "$((ymax - ymin))" -le 5 ] || fail "the wave is $((ymax - ymin + 1)) pixels tall"
[ "$(ink marked "$xmin" "$xmax" $((ymin - 14)) $((ymin - 1)))" -gt 20 ] || fail "no text above the wave"
# The pointer is steered over white paper below the line, then moved up onto the word, a short hop that cannot go far wrong. The guest's
# pointer does not always go where it is told (it can end up in a corner), so when the menu does not open the keyboard's way is used:
# the caret goes back to the end of the word and Shift+F10 opens the same menu.
PTR_Y0=$((ymax + 14))
goto $((xmax - 8)) $((ymin + 36))
w95_mouse_move $((xmax - 8)) $((ymin - 6))
w95_mouse_xdo click 3
sleep 3
shot menu
METHOD="the right button"
# A menu is a block of the system's light grey in the page, which has none.
grey() { convert "$RUN/$1.png" -crop 330x230+280+190 +repage txt:- 2> /dev/null | grep -c '#C0C0C0' || true; }
if [ "$(grey menu)" -lt 1500 ]; then
	METHOD="Shift+F10"
	w95_key Escape Left Left Left Left Left shift+F10
	sleep 3
	shot menu
fi
echo "w95-spell-live: the menu opened with $METHOD"
w95_key Down Return
sleep 8
shot fixed
after=$(red fixed | awk -v lo="$((xmax - 20))" '$1 >= lo' | wc -l)
[ "$after" -eq 0 ] || fail "the corrected word still has $after red pixels"
[ "$(red fixed | wc -l)" -gt 8 ] || fail "the other word lost its wave"
# Find > Check Spelling as You Type is ticked, shows its hint on the status line, and switches the marks off.
w95_key alt Right Right Right Down Down Down Down Down
sleep 2
shot find-menu
w95_key Return
sleep 4
shot toggled
[ "$(red toggled | wc -l)" -eq 0 ] || fail "the marks stayed after Check Spelling as You Type was switched off"
echo "w95-spell-live: ok"
