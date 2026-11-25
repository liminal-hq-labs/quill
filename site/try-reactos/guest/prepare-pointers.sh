#!/usr/bin/env bash
# Make the Afterglow pointers the system pointers of the demo disk: patch a COPY of the guest's user32.dll with the pinned Afterglow
# tool. The result is for the private demo disk only; it is ReactOS code (LGPL-2.1) and is never committed or redistributed.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: prepare-pointers.sh DISK_IMG AFTERGLOW_DIR
#   DISK_IMG       a ReactOS 0.4.16 disk (the golden one; read only) whose first partition starts at 1 MiB
#   AFTERGLOW_DIR  the folder prepare-afterglow.sh wrote (it must hold cursors/); user32-afterglow.dll is written there
# Takes ::/ReactOS/system32/user32.dll with mtools, patches it with desktop/pointers/patch-user32.py from the pinned source
# (AFTERGLOW_SRC, or fetch-afterglow.sh) and checks the sum of the result against desktop/pointers/SHA256SUMS.user32.
# The tool refuses any other user32.dll, so a changed guest stops here.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
IMG=${1:?usage: prepare-pointers.sh DISK_IMG AFTERGLOW_DIR}
OUT=${2:?usage: prepare-pointers.sh DISK_IMG AFTERGLOW_DIR}
export MTOOLS_SKIP_CHECK=1
[ -d "$OUT/cursors" ] || { echo "prepare-pointers: $OUT/cursors is missing (run prepare-afterglow.sh first)" >&2; exit 2; }
SRC=${AFTERGLOW_SRC:-$(bash "$HERE/fetch-afterglow.sh")}
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mcopy -n -i "$IMG@@1048576" ::/ReactOS/system32/user32.dll "$TMP/user32.dll"
python3 -I "$SRC/desktop/pointers/patch-user32.py" "$TMP/user32.dll" "$OUT/cursors" "$TMP/user32-afterglow.dll" >&2
WANT=$(cut -d' ' -f1 "$SRC/desktop/pointers/SHA256SUMS.user32")
GOT=$(sha256sum "$TMP/user32-afterglow.dll" | cut -d' ' -f1)
[ "$GOT" = "$WANT" ] || { echo "prepare-pointers: patched user32.dll is $GOT, expected $WANT" >&2; exit 1; }
cp "$TMP/user32-afterglow.dll" "$OUT/user32-afterglow.dll"
echo "prepare-pointers: wrote $OUT/user32-afterglow.dll ($GOT)"
