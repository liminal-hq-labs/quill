#!/usr/bin/env bash
# Runs the module loader's test program (MODHOST.EXE) under Wine and checks its answers.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-modules.sh [--log FILE]     (build first with win9x/build.sh)
#
# First checks each test module as an image (check-dll.sh, check-imports.sh against the module's own list, scan-pentium.sh), then copies
# dist/test/ into a work folder and runs MODHOST.EXE there with WINEDEBUG=+loaddll. MODHOST.EXE loads QUILLTST.DLL by full path
# 200 times and checks the version handshake, the relocated copy (QUILLTSTR.DLL, linked at the program's own base), another build in a
# release and a development policy, another interface major, a missing file, a file that is not a PE image, a Windows 3.x (NE) file and the
# module's own checks of what it is handed. The run fails unless MODHOST.EXE exits 0 and Wine's `+loaddll` shows QUILLTSTR.DLL loaded
# away from 0x400000. `--log FILE` keeps MODHOST.LOG there. Needs wine and Xvfb; exits 0 with a note when they are missing. Uses its own
# display (QUILL_MODULES_DISPLAY, default :186) and its own WINEPREFIX under ~/.cache, which is removed with the run unless
# QUILL_KEEP_PREFIX is set. Local only.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
KEEP=""
if [ "${1:-}" = "--log" ]; then KEEP="$(mkdir -p "$(dirname "$2")" && cd "$(dirname "$2")" && pwd)/$(basename "$2")"; fi
for t in wine Xvfb; do
	command -v "$t" > /dev/null || { echo "wine-modules: $t not found; skipping"; exit 0; }
done
fail() { echo "wine-modules: FAIL: $*" >&2; exit 1; }
[ -f "$HERE/dist/test/MODHOST.EXE" ] || fail "build win9x/dist/test first (win9x/build.sh)"

# The images.
bash "$HERE/tools/check-dll.sh" "$HERE/dist/test/QUILLTST.DLL" 0x61F00000
bash "$HERE/tools/check-dll.sh" "$HERE/dist/test/QUILLTSTR.DLL" 0x00400000
for name in QUILLTSTB QUILLTSTD QUILLTSTM; do bash "$HERE/tools/check-dll.sh" "$HERE/dist/test/$name.DLL" 0x61F00000; done
(
	cd "$HERE"
	for name in QUILLTST QUILLTSTR QUILLTSTB QUILLTSTD QUILLTSTM; do
		bash tools/check-imports.sh "dist/test/$name.DLL" mods/tst/imports.allow
		bash tools/scan-pentium.sh "dist/test/$name.DLL"
	done
	bash tools/check-imports.sh dist/test/MODHOST.EXE mods/modhost/imports.allow
	bash tools/scan-pentium.sh dist/test/MODHOST.EXE
)

WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-modules.XXXXXX")"
export DISPLAY="${QUILL_MODULES_DISPLAY:-:186}" WINEDEBUG=+loaddll
export WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-modules}"
unset WINEARCH WAYLAND_DISPLAY
XPID=""
cleanup() {
	wineserver -k 2> /dev/null || true
	[ -z "$XPID" ] || kill "$XPID" 2> /dev/null || true
	rm -rf "$WORK"
	[ -n "${QUILL_KEEP_PREFIX:-}" ] || rm -rf "$WINEPREFIX"
}
trap cleanup EXIT
Xvfb "$DISPLAY" -screen 0 640x480x24 > /dev/null 2>&1 &
XPID=$!
sleep 1
[ -d "$WINEPREFIX/drive_c" ] || WINEDEBUG=-all wineboot --init > /dev/null 2>&1 || true
cp "$HERE"/dist/test/QUILLTST*.DLL "$HERE/dist/test/MODHOST.EXE" "$WORK/"
code=0
(cd "$WORK" && wine MODHOST.EXE > "$WORK/wine.log" 2>&1) || code=$?
[ -f "$WORK/MODHOST.LOG" ] || fail "MODHOST.EXE wrote no MODHOST.LOG (exit $code)"
cat "$WORK/MODHOST.LOG"
[ -z "$KEEP" ] || cp "$WORK/MODHOST.LOG" "$KEEP"
[ "$code" = 0 ] || fail "MODHOST.EXE exited with $code"
# Wine's own record of where the relocated copy went: the line reads `Loaded L"...\QUILLTSTR.DLL" at 7B540000: native`.
at="$(grep -i 'QUILLTSTR.DLL' "$WORK/wine.log" | grep -io 'at [0-9a-f]*' | head -1 | cut -d' ' -f2 | sed 's/^0*//' | tr 'a-f' 'A-F' || true)"
[ -n "$at" ] || fail "Wine's +loaddll does not show QUILLTSTR.DLL"
echo "wine-modules: QUILLTSTR.DLL loaded at 0x$at (it asks for 0x400000)"
[ "$at" != "400000" ] || fail "QUILLTSTR.DLL loaded at its own base, so no relocation was exercised"
echo "wine-modules: ok"
