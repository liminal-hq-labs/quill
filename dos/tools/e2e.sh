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
# Each scenario (tests/scenarios/NAME.keys) is played into QUILLDOS.EXE through AUTOTYPE; the same file is played on the host through the
# same quill-app code. Lines starting with @ are directives: @copy FILE (a test document put on the disk), @open FILE (a command-line
# argument) and @mono (/MONO). Everything else is key tokens; # starts a comment.
run_scenario() { # <name> <saved file on the disk or -> <golden saved file or ->
	local name="$1" saved="$2" golden="$3"
	[ -z "${E2E_ONLY:-}" ] || [ "$E2E_ONLY" = "$name" ] || return 0
	local file="$ROOT/tests/scenarios/$name.keys"
	local disk="$WORK/disk-$name"
	rm -rf "$disk" "$WORK/keep"
	mkdir -p "$disk"
	cp "$HERE/dist/QUILLDOS.EXE" "$disk/"
	local args=""
	while read -r d a; do
		case "$d" in
		@copy) cp "$ROOT/tests/docs/$a" "$disk/$(echo "$a" | tr '[:lower:]' '[:upper:]')" ;;
		@open) args="$args $a" ;;
		@mono) args="$args /MONO" ;;
		esac
	done < <(grep '^@' "$file" | sed 's/#.*//')
	local keys
	keys="$(grep -v '^@' "$file" | sed 's/#.*//' | tr -s ' \n\t' ' ' | sed 's/^ //; s/ $//')"
	export DOS_ENV='QUILL_DUMP=D:\DUMP.TXT'
	export DOS_KEEP="$WORK/keep"
	export DOS_AUTOTYPE="-w 2 -p 0.3 $keys"
	export DOS_TIMEOUT=240
	local rc=0
	"$HERE/tools/dosrun.sh" "$disk" "QUILLDOS.EXE$args" > "$WORK/$name.out" || rc=$?
	[ "$rc" = 0 ] || fail "$name: QUILLDOS.EXE exited with $rc"
	tr -d '\r' < "$WORK/keep/DUMP.TXT" > "$WORK/$name-frames.txt"
	diff -u "$ROOT/tests/golden/screens/$name.frames.txt" "$WORK/$name-frames.txt" > "$WORK/$name.diff" || { head -60 "$WORK/$name.diff" >&2; fail "$name: the screens differ from the host's"; }
	echo "e2e: $name: every screen matches the host's, character for character and attribute for attribute"
	if [ "$saved" != - ]; then
		cmp "$disk/$saved" "$ROOT/tests/golden/files/$golden" || fail "$name: the saved $saved differs from the host's"
		echo "e2e: $name: the saved $saved is byte-identical to the host's"
	fi
}

run_scenario m1-edit PLAIN.QLD m1-edit.qld
run_scenario m2-letter OUT.RTF m2-letter.rtf
run_scenario m2-mono - -
run_scenario m2-find - -
run_scenario m2-dialogs - -
run_scenario m4-about QUILL.INI m4-quill.ini
run_scenario m4-print OUT.PRN letter.print.txt
run_scenario m4-print-headers OUT.PRN headers.print.txt
run_scenario m7-picture OUT.QLD m7-picture.qld
run_scenario m2-headers - -
run_scenario m2-lists - -
echo "e2e: ok"
