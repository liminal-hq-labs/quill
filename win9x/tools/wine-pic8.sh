#!/usr/bin/env bash
# Opens an 8-bit picture whose palette is shorter than its depth in QUILL32.EXE under Wine and checks that the picture is drawn, not the grey box (W9X-10).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-pic8.sh [QUILL32.EXE]      (default win9x/dist/QUILL32.EXE)
#
# `tests/win9x/pic8-short-palette.rtf` holds a 64 by 48 bitmap with `biClrUsed` 16 whose pixels run past the table. The picture is a red
# gradient with a black diagonal where the indices leave the table; a grey box has no red in it. The capture must hold at least 1,500
# red pixels. Needs wine, xvfb-run, ImageMagick; exits 0 with a note when they are missing. Uses a WINEPREFIX of its own under the cache.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
EXE="${1:-$HERE/dist/QUILL32.EXE}"
for t in wine xvfb-run import convert; do
	command -v "$t" > /dev/null || { echo "wine-pic8: $t not found; skipping"; exit 0; }
done
[ -f "$EXE" ] || { echo "wine-pic8: no $EXE (build it with win9x/build.sh)" >&2; exit 1; }
work="$(mktemp -d "${TMPDIR:-/tmp}/quill-pic8.XXXXXX")"
trap 'rm -rf "$work"' EXIT
cp "$EXE" "$work/QUILL32.EXE"
cp "$ROOT/tests/win9x/pic8-short-palette.rtf" "$work/PIC8.RTF"
cat > "$work/drive.sh" << 'DRIVE'
#!/usr/bin/env bash
cd "$1"
wine QUILL32.EXE PIC8.RTF > /dev/null 2>&1 &
sleep 12
import -window root "$1/shot.png"
DRIVE
export WINEDEBUG=-all WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-pic8}"
timeout 120 xvfb-run -a -s "-screen 0 800x600x24" bash -c "bash '$work/drive.sh' '$work'; wineserver -k" > /dev/null 2>&1 || true
[ -f "$work/shot.png" ] || { echo "wine-pic8: FAIL: no capture" >&2; exit 1; }
red="$(convert "$work/shot.png" txt:- | grep -cE '#F[0-9A-F][0-2][0-9A-F][0-4][0-9A-F] ' || true)"
echo "wine-pic8: $red red pixels"
[ "$red" -ge 1500 ] || { echo "wine-pic8: FAIL: the 8-bit picture was not drawn"; exit 1; }
echo "wine-pic8: ok"
