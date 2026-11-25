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
# AFTERGLOW_SRC=DIR uses an already fetched and checked tree instead of fetching (fetch-afterglow.sh does the check).
# The pointer set and the user32.dll patch are not in the pinned source, so they are not applied.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
PIN=${AFTERGLOW_PIN:-$HERE/afterglow.pin}
# shellcheck source=afterglow.pin
. "$PIN"
OUT=${1:?usage: prepare-afterglow.sh OUT_DIR}
for t in python3 llvm-rc lld-link rsvg-convert magick; do
	command -v "$t" >/dev/null || { echo "prepare-afterglow: $t is needed (see the header of this script)" >&2; exit 2; }
done
python3 -c 'import numpy' 2>/dev/null || { echo "prepare-afterglow: python3 needs numpy" >&2; exit 2; }
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
python3 -I "$HERE/quillify.py" "$SRC/$AFTERGLOW_WALLPAPER" "$OUT/quill-wallpaper.bmp"
echo "prepare-afterglow: wrote afterglow.msstyles, afterglow.reg and quill-wallpaper.bmp to $OUT"
