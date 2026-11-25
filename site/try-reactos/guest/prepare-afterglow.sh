#!/usr/bin/env bash
# Make the Afterglow files for the demo image from the pinned source (guest/afterglow.pin): fetch it, build the ReactOS style, check its hash,
# take the apply registry file and put the Quill emblem on the dusk wallpaper. Nothing of Afterglow is committed here.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: prepare-afterglow.sh OUT_DIR       (OUT_DIR is what build-image.sh reads as AFTERGLOW_DIR)
# Writes OUT_DIR/afterglow.msstyles, OUT_DIR/afterglow.reg (selects the style, scheme Dusk, and sets the caption metrics) and
# OUT_DIR/quill-wallpaper.bmp (1024 x 768, 24-bit, from quillify.py). The default build is used: classic scroll bars.
# Needs curl, tar, python3 with numpy (the style generator), llvm-rc and lld-link (LLVM), rsvg-convert and ImageMagick 7.
# WALLPAPER_STYLE=smooth (default) or stepped picks the wallpaper (afterglow.pin). The smooth one is rendered as Afterglow's own script does for a
# ReactOS BMP, with its seeded +-1 level triangular dither after the 4:3 crop (quillify.py --dither 1; the noise is byte-for-byte Afterglow's);
# the stepped one is left as drawn. Needs numpy for that. The guest's display is true colour (the bochsmp driver offers 32 bits only, and the
# screen shows the BMP's pixels exactly), so the 24-bit dither is all it needs. WALLPAPER_RGB565=1 also reduces the smooth one to 5-6-5 levels
# with an ordered dither (quillify.py --rgb565), for a build that is shown on a 16-bit display.
# AFTERGLOW_SRC=DIR uses an already fetched and checked tree instead of fetching (fetch-afterglow.sh does the check).
# Also builds the 16 pointers with desktop/pointers/gen-cursors.py (python3 with numpy and Pillow) into OUT_DIR/cursors and copies
# afterglow-pointers.reg to OUT_DIR/pointers.reg. The user32.dll patch needs the demo disk's file: see prepare-pointers.sh.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
PIN=${AFTERGLOW_PIN:-$HERE/afterglow.pin}
# shellcheck source=afterglow.pin
. "$PIN"
OUT=${1:?usage: prepare-afterglow.sh OUT_DIR}
for t in python3 llvm-rc lld-link rsvg-convert magick; do
	command -v "$t" >/dev/null || { echo "prepare-afterglow: $t is needed (see the header of this script)" >&2; exit 2; }
done
python3 -c 'import numpy, PIL' 2>/dev/null || { echo "prepare-afterglow: python3 needs numpy and Pillow" >&2; exit 2; }
SRC=${AFTERGLOW_SRC:-$(bash "$HERE/fetch-afterglow.sh")}
rm -rf "$OUT"
mkdir -p "$OUT"
BUILD=$(mktemp -d)
trap 'rm -rf "$BUILD"' EXIT
bash "$SRC/desktop/style/build.sh" "$BUILD/style" >&2
GOT=$(sha256sum "$BUILD/style/afterglow.msstyles" | cut -d' ' -f1)
if [ "$GOT" != "$AFTERGLOW_MSSTYLES_SHA256" ]; then
	echo "prepare-afterglow: afterglow.msstyles is $GOT, expected $AFTERGLOW_MSSTYLES_SHA256 (afterglow.pin); the toolchain or the source differs" >&2
	exit 1
fi
cp "$BUILD/style/afterglow.msstyles" "$OUT/afterglow.msstyles"
cp "$SRC/$AFTERGLOW_APPLY_REG" "$OUT/afterglow.reg"
case "${WALLPAPER_STYLE:-smooth}" in
	smooth) python3 -I "$HERE/quillify.py" "$SRC/$AFTERGLOW_WALLPAPER" "$OUT/quill-wallpaper.bmp" --dither 1 ${WALLPAPER_RGB565:+--rgb565} ;;
	stepped) python3 -I "$HERE/quillify.py" "$SRC/${AFTERGLOW_WALLPAPER_STEPPED:?}" "$OUT/quill-wallpaper.bmp" ;;
	*) echo "prepare-afterglow: WALLPAPER_STYLE must be smooth or stepped, not ${WALLPAPER_STYLE}" >&2; exit 2 ;;
esac
mkdir -p "$OUT/cursors"
python3 -I "$SRC/desktop/pointers/gen-cursors.py" "$OUT/cursors" >&2
[ "$(find "$OUT/cursors" -type f | wc -l)" -eq 16 ] || { echo "prepare-afterglow: gen-cursors.py did not write 16 pointer files" >&2; exit 1; }
cp "$SRC/desktop/pointers/afterglow-pointers.reg" "$OUT/pointers.reg"
echo "prepare-afterglow: wrote afterglow.msstyles, afterglow.reg, pointers.reg, cursors/ (16 files) and quill-wallpaper.bmp to $OUT"
