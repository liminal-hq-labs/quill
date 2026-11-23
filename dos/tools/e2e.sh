#!/bin/bash
# Builds QUILLDOS.EXE, plays the editing script into it under headless DOSBox and compares its screens and its saved file with the host's.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/e2e.sh
#
# Needs the DOS tools (dos/tools/fetch-tools.sh); exits 0 with a note when they are missing, so a checkout without them is not
# failed. QUILLDOS.EXE writes each screen it shows to the file QUILL_DUMP names, read back from video memory, as `=== screen N ===`
# frames; they are compared with tests/golden/screens/, the host's goldens, character for character and attribute for attribute.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
VENDOR="${DOS_VENDOR:-$HERE/vendor}"

if [ ! -f "$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE" ] || [ -z "$(ls "$VENDOR"/dosbox/*/dosbox 2> /dev/null)" ]; then
	echo "e2e: DOS tools not present; skipping (run dos/tools/fetch-tools.sh)"
	exit 0
fi

"$HERE/build.sh"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-e2e.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
fail() { echo "e2e: FAIL: $*" >&2; exit 1; }

export DOS_ENV='QUILL_DUMP=D:\DUMP.TXT'
export DOS_KEEP="$WORK/keep"
# The editing script. The same key tokens the host plays (tests/scenarios/m1-edit.keys) go in through AUTOTYPE.
DISK="$WORK/disk"
mkdir -p "$DISK"
cp "$HERE/dist/QUILLDOS.EXE" "$DISK/"
cp "$ROOT/tests/docs/plain.qld" "$DISK/PLAIN.QLD"
keys="$(sed 's/#.*//' "$ROOT/tests/scenarios/m1-edit.keys" | tr -s ' \n\t' ' ' | sed 's/^ //; s/ $//')"
export DOS_AUTOTYPE="-w 2 -p 0.3 $keys"
export DOS_TIMEOUT=120
rc=0
"$HERE/tools/dosrun.sh" "$DISK" QUILLDOS.EXE > "$WORK/m1.out" || rc=$?
[ "$rc" = 0 ] || fail "QUILLDOS.EXE exited with $rc in the editing script"
tr -d '\r' < "$WORK/keep/DUMP.TXT" > "$WORK/m1-frames.txt"
diff -u "$ROOT/tests/golden/screens/m1-edit.frames.txt" "$WORK/m1-frames.txt" > "$WORK/m1.diff" || { head -60 "$WORK/m1.diff" >&2; fail "the editing script's screens differ from the host's"; }
echo "e2e: editing script: every screen matches the host's, character for character and attribute for attribute"
cmp "$DISK/PLAIN.QLD" "$ROOT/tests/golden/files/m1-edit.qld" || fail "the saved PLAIN.QLD differs from the host's"
echo "e2e: editing script: the saved .QLD is byte-identical to the host's"
echo "e2e: ok"
