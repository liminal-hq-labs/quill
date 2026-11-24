#!/usr/bin/env bash
# Runs THREAD95.EXE (spike J0's Windows 95 worker thread) under Wine on an Xvfb screen of its own and checks THREAD.LOG says PASS.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-thread.sh [THREAD95.EXE]     (build first with win9x/build.sh; the exe is in target/win9x/release/)
#
# Uses its own display (QUILL_THREAD_DISPLAY, default :177) and a WINEPREFIX of its own under the cache directory. Exits 0 with a note
# when wine or Xvfb is missing.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
EXE="${1:-${CARGO_TARGET_DIR:-$HERE/target}/win9x/release/thread95.exe}"
for t in wine Xvfb; do
	command -v "$t" > /dev/null || { echo "wine-thread: $t not found; skipping"; exit 0; }
done
[ -f "$EXE" ] || { echo "wine-thread: build $EXE first" >&2; exit 1; }
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-thread.XXXXXX")"
DISPLAY_NUM="${QUILL_THREAD_DISPLAY:-:177}"
export WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-thread}" DISPLAY="$DISPLAY_NUM"
unset WINEARCH
XPID=""
cleanup() {
	wineserver -k 2> /dev/null || true
	[ -z "$XPID" ] || kill "$XPID" 2> /dev/null || true
	rm -rf "$WORK"
}
trap cleanup EXIT
Xvfb "$DISPLAY_NUM" -screen 0 640x480x24 > /dev/null 2>&1 &
XPID=$!
for _ in $(seq 1 50); do
	[ -S "/tmp/.X11-unix/X${DISPLAY_NUM#:}" ] && break
	sleep 0.1
done
cp "$EXE" "$WORK/THREAD95.EXE"
code=0
(cd "$WORK" && WINEDEBUG=-all timeout 60 wine THREAD95.EXE > /dev/null 2>&1) || code=$?
[ -f "$WORK/THREAD.LOG" ] || { echo "wine-thread: FAIL: no THREAD.LOG (exit $code)" >&2; exit 1; }
cat "$WORK/THREAD.LOG"
grep -q "result=PASS" "$WORK/THREAD.LOG" && [ "$code" = 0 ] || { echo "wine-thread: FAIL (exit $code)" >&2; exit 1; }
echo "wine-thread: ok"
