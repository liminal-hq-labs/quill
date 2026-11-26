#!/usr/bin/env bash
# Drives QUILL32.EXE under Wine on a 640 by 480 Xvfb screen: a click selects a picture, a corner handle sizes it, Undo gives it back.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-picture.sh [--shots DIR]     (build first with win9x/build.sh)
#
# Opens `tests/docs/pictures.rtf`, finds the first picture by its saturated colours, clicks it and checks the frame and eight black
# handles round it; drags the bottom right handle and checks the picture grew with its proportions kept; undoes the drag in one step
# and checks the first size is back; Esc takes the handles away. Needs wine, Xvfb, xdotool, ImageMagick and Python with Pillow; exits 0
# with a note when they are missing. Uses its own display (QUILL_PICTURE_DISPLAY, default :175) and its own WINEPREFIX. Local only.
set -euo pipefail
# The caret stays solid so that screenshots are stable (`[Quill] Blink`, win9x/tools/wine-blink.sh).
export QUILL_NO_BLINK=1
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
SHOTS=""
if [ "${1:-}" = "--shots" ]; then SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"; fi
for t in wine Xvfb xdotool import; do
	command -v "$t" > /dev/null || { echo "wine-picture: $t not found; skipping"; exit 0; }
done
python3 -I -c "import PIL" 2> /dev/null || { echo "wine-picture: Pillow not found; skipping"; exit 0; }
[ -f "$HERE/dist/QUILL32.EXE" ] || { echo "wine-picture: build win9x/dist/QUILL32.EXE first" >&2; exit 1; }
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-picture.XXXXXX")"
export DISPLAY="${QUILL_PICTURE_DISPLAY:-:175}" WINEDEBUG=-all
export WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-picture}"
unset WINEARCH
XPID=""
WPID=""
cleanup() {
	[ -z "$WPID" ] || kill "$WPID" 2> /dev/null || true
	wineserver -k 2> /dev/null || true
	[ -z "$XPID" ] || kill "$XPID" 2> /dev/null || true
	rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "wine-picture: FAIL: $*" >&2; exit 1; }
shot() {
	import -window root "$WORK/$1.png"
	if [ -n "$SHOTS" ]; then cp "$WORK/$1.png" "$SHOTS/picture-$1.png"; fi
}
# The bounding box of the first picture's saturated pixels (left top right bottom) and how many black pixels there are in the eight
# handle boxes round it; SHOT. The handles are 7 pixels square and touch the frame, which is one pixel outside the picture.
measure() {
	python3 -I - "$WORK/$1.png" << 'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB")
w, h = im.size
px = im.load()
xs, ys = [], []
for y in range(130, h - 40):
	for x in range(0, w - 20):
		r, g, b = px[x, y]
		if max(r, g, b) - min(r, g, b) > 120:
			xs.append(x)
			ys.append(y)
if not xs:
	print("none")
	sys.exit(0)
# Only the first picture: the rows up to the first gap of more than 6 empty rows after it begins.
rows = sorted(set(ys))
end = rows[0]
for r in rows:
	if r - end > 6:
		break
	end = r
sel = [(x, y) for x, y in zip(xs, ys) if y <= end]
l, t, r, b = min(x for x, _ in sel), min(y for _, y in sel), max(x for x, _ in sel) + 1, end + 1
def black(x0, y0):
	n = 0
	for y in range(y0, y0 + 7):
		for x in range(x0, x0 + 7):
			if 0 <= x < w and 0 <= y < h and px[x, y] == (0, 0, 0):
				n += 1
	return n
o = (l - 1, t - 1, r + 1, b + 1)
cx, cy = (o[0] + o[2] - 7) // 2, (o[1] + o[3] - 7) // 2
boxes = [(o[0] - 7, o[1] - 7), (cx, o[1] - 7), (o[2], o[1] - 7), (o[2], cy), (o[2], o[3]), (cx, o[3]), (o[0] - 7, o[3]), (o[0] - 7, cy)]
print(l, t, r, b, sum(1 for bx, by in boxes if black(bx, by) >= 40))
PY
}
[ -d "$WINEPREFIX/drive_c" ] || wineboot --init > /dev/null 2>&1 || true
Xvfb "$DISPLAY" -screen 0 640x480x24 > /dev/null 2>&1 &
XPID=$!
sleep 2
cp "$HERE/dist/QUILL32.EXE" "$WORK/QUILL32.EXE"
cp "$ROOT/tests/docs/pictures.rtf" "$WORK/PICTURES.RTF"
(cd "$WORK" && wine QUILL32.EXE PICTURES.RTF > /dev/null 2>&1) &
WPID=$!
for _ in $(seq 1 40); do
	sleep 2
	if xdotool search --name Quill > /dev/null 2>&1; then break; fi
done
sleep 5

shot plain
read -r l t r b handles <<< "$(measure plain)"
[ "$l" != none ] || fail "no picture on the page"
echo "wine-picture: the first picture is at $l,$t to $r,$b"
[ "$handles" -eq 0 ] || fail "an unselected picture shows handles ($handles)"

# A click inside selects it: eight handles.
xdotool mousemove $(((l + r) / 2)) $(((t + b) / 2)) click 1
sleep 1
shot selected
read -r l1 t1 r1 b1 handles <<< "$(measure selected)"
echo "wine-picture: selected, $handles handles"
[ "$handles" -eq 8 ] || fail "a click did not show the frame and eight handles ($handles)"

# The bottom right handle: drag 40 pixels right and 30 down. A corner keeps the proportions.
hx=$((r1 + 1 + 3))
hy=$((b1 + 1 + 3))
xdotool mousemove "$hx" "$hy" mousedown 1
sleep 0.3
xdotool mousemove $((hx + 20)) $((hy + 15))
sleep 0.3
xdotool mousemove $((hx + 40)) $((hy + 30))
sleep 0.3
xdotool mouseup 1
sleep 1
shot sized
read -r l2 t2 r2 b2 handles <<< "$(measure sized)"
w1=$((r1 - l1)); h1=$((b1 - t1)); w2=$((r2 - l2)); h2=$((b2 - t2))
echo "wine-picture: sized from ${w1}x${h1} to ${w2}x${h2}, $handles handles"
[ "$w2" -gt $((w1 + 20)) ] || fail "the handle did not make the picture wider ($w1 to $w2)"
# w2 / h2 equals w1 / h1 to within a pixel of rounding: w2 * h1 and h2 * w1 differ by at most max(w1, h1) * 2.
d=$((w2 * h1 - h2 * w1)); d=${d#-}
[ "$d" -le $(((w1 > h1 ? w1 : h1) * 2)) ] || fail "the corner did not keep the proportions (${w1}x${h1} to ${w2}x${h2})"

# Undo is one step.
xdotool key ctrl+z
sleep 1
shot undone
read -r l3 t3 r3 b3 _ <<< "$(measure undone)"
[ "$((r3 - l3))" -eq "$w1" ] && [ "$((b3 - t3))" -eq "$h1" ] || fail "one Undo did not give the first size back ($((r3 - l3))x$((b3 - t3)), wanted ${w1}x${h1})"

# Esc takes the handles away.
xdotool mousemove $(((l3 + r3) / 2)) $(((t3 + b3) / 2)) click 1
sleep 0.5
xdotool key Escape
sleep 1
shot escaped
read -r _ _ _ _ handles <<< "$(measure escaped)"
[ "$handles" -eq 0 ] || fail "Esc left $handles handles"
echo "wine-picture: ok"
