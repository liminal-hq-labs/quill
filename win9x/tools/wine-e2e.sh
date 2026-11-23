#!/usr/bin/env bash
# Plays the test scenarios into QUILL32.EXE under Wine (on Xvfb) and compares its screens and saved files with the host's goldens.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-e2e.sh [SCENARIO...]     (default: every scenario with a golden)
#
# QUILL32.EXE reads the scenario's key tokens with /PLAY and writes every screen it shows with /DUMP, as the DOS build does under
# DOSBox (dos/tools/e2e.sh). Needs wine and xvfb-run; exits 0 with a note when they are missing. WINEPREFIX defaults to a scratch
# directory under $TMPDIR.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
command -v wine > /dev/null && command -v xvfb-run > /dev/null || { echo "wine-e2e: wine or xvfb-run not found; skipping"; exit 0; }
"$HERE/build.sh"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-wine.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
export WINEPREFIX="${WINEPREFIX:-${TMPDIR:-/tmp}/quill-wineprefix}" WINEDEBUG=-all
unset WINEARCH
fail() { echo "wine-e2e: FAIL: $*" >&2; exit 1; }

# scenario name, saved file, golden saved file
declare -A SAVED=([m1-edit]="PLAIN.QLD m1-edit.qld" [m2-letter]="OUT.RTF m2-letter.rtf" [m4-about]="QUILL.INI m4-quill.ini" [m4-print]="OUT.PRN letter.print.txt" [m4-print-headers]="OUT.PRN headers.print.txt")
names=("$@")
[ ${#names[@]} -gt 0 ] || names=(m1-edit m2-letter m2-mono m2-find m2-lists m2-headers m2-dialogs m4-about m4-print m4-print-headers)
for name in "${names[@]}"; do
	d="$WORK/$name"
	mkdir -p "$d"
	cp "$HERE/dist/QUILL32.EXE" "$d/"
	cp "$ROOT/tests/scenarios/$name.keys" "$d/SCRIPT.TXT"
	while read -r kw a; do
		[ "$kw" = @copy ] && cp "$ROOT/tests/docs/$a" "$d/$(echo "$a" | tr '[:lower:]' '[:upper:]')"
	done < <(grep '^@copy' "$ROOT/tests/scenarios/$name.keys" | sed 's/#.*//')
	(cd "$d" && timeout 180 xvfb-run -a wine QUILL32.EXE /PLAY SCRIPT.TXT /DUMP OUT.TXT > /dev/null 2>&1) || fail "$name: QUILL32.EXE did not finish"
	tr -d '\r' < "$d/OUT.TXT" > "$d/frames.txt"
	diff -q "$ROOT/tests/golden/screens/$name.frames.txt" "$d/frames.txt" > /dev/null || fail "$name: the screens differ from the host's"
	echo "wine-e2e: $name: every screen matches the host's"
	if [ -n "${SAVED[$name]:-}" ]; then
		read -r out golden <<< "${SAVED[$name]}"
		cmp "$d/$out" "$ROOT/tests/golden/files/$golden" || fail "$name: the saved $out differs from the host's"
		echo "wine-e2e: $name: the saved $out is byte-identical to the host's"
	fi
done
echo "wine-e2e: ok"
