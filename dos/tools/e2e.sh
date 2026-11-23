#!/bin/bash
# Builds QUILLDOS.EXE, runs it under headless DOSBox and compares its screen dumps with the host's goldens.
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
rc=0
"$HERE/tools/dosrun.sh" "$HERE/dist" QUILLDOS.EXE > "$WORK/out.txt" || rc=$?
[ "$rc" = 0 ] || fail "QUILLDOS.EXE exited with $rc"
[ -f "$WORK/keep/DUMP.TXT" ] || fail "no screen dump"
tr -d '\r' < "$WORK/keep/DUMP.TXT" > "$WORK/dump.txt"
diff -u "$ROOT/tests/golden/screens/hello.80x25.txt" "$WORK/dump.txt" || fail "the hello screen differs from its golden"
echo "e2e: hello screen matches its golden"
echo "e2e: ok"
