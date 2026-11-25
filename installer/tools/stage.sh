#!/usr/bin/env bash
# Fills installer/dist/ with the files the installer packs: the three builds, the help, the dictionaries and the samples.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: installer/tools/stage.sh [--build | --stand-ins]
#
#   (no option)   copy the programs the build scripts already made
#   --build       run win16/build.sh, win9x/build.sh, dos/build.sh and quill-dict first (long; needs dos/tools/fetch-tools.sh's stub)
#   --stand-ins   write tiny placeholder files instead, for `liminst check` and `liminst preview`, which read the project but never
#                 the programs; the placeholders are not programs and nothing built from them is shipped
#
# The programs come from win16/out/QUILL.EXE and QUILL.HLP, win9x/dist/QUILL32.EXE and dos/dist/QUILLDOS.EXE unless QUILL16_EXE,
# QUILL16_HLP, QUILL32_EXE or QUILLDOS_EXE name other files, and the dictionaries from QUILL_DICT_DIR (default: made by quill-dict).
# Build output goes under $QUILL_BUILD_DIR (default ~/.cache/quill16-installer-build), not into /tmp.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(dirname "$HERE")"
DIST="$HERE/dist"
BUILD_DIR="${QUILL_BUILD_DIR:-$HOME/.cache/quill16-installer-build}"

mode=copy
case "${1:-}" in
"") ;;
--build) mode=build ;;
--stand-ins) mode=stand-in ;;
*) echo "stage: unknown option $1" >&2 && exit 2 ;;
esac

# The dist folder is rebuilt each time, so a file from an earlier stage never ships. Each path is named: no wildcard removal.
for d in dos-x86 win16-x86 windows-x86 help common dict samples; do
	rm -rf "$DIST/$d"
done
mkdir -p "$DIST/dos-x86" "$DIST/win16-x86" "$DIST/windows-x86" "$DIST/help" "$DIST/common" "$DIST/dict" "$DIST/samples"

# DOS programs read text with CR LF line ends.
crlf() { sed 's/$/\r/' "$1" >"$2"; }
crlf "$HERE/README.TXT" "$DIST/common/README.TXT"
# The licence page shows the short notice (installer/LICENCE.TXT): the Windows 3.x runtime holds the page text in a 40 KB heap, and the
# whole 13 KB of both licences does not fit. The installed LICENCE.TXT is the full text.
crlf "$HERE/LICENCE-FULL.TXT" "$DIST/common/LICENCE.TXT"

# The samples are the documents the tests already use, under 8.3 names.
cp "$ROOT/tests/docs/letter.qld" "$DIST/samples/LETTER.QLD"
cp "$ROOT/tests/docs/plain.qld" "$DIST/samples/PLAIN.QLD"
cp "$ROOT/tests/docs/letter.rtf" "$DIST/samples/LETTER.RTF"
cp "$ROOT/tests/docs/pictures.rtf" "$DIST/samples/PICTURES.RTF"

if [ "$mode" = stand-in ]; then
	for f in dos-x86/QUILLDOS.EXE win16-x86/QUILL.EXE windows-x86/QUILL32.EXE help/QUILL.HLP dict/EN_CA.DIC dict/EN_GB.DIC dict/EN_US.DIC; do
		printf 'STAND-IN %s: a placeholder, not a program.\r\n' "${f##*/}" >"$DIST/$f"
	done
	echo "stage: stand-ins in $DIST"
	exit 0
fi

if [ "$mode" = build ]; then
	mkdir -p "$BUILD_DIR"
	(cd "$ROOT/win16" && CARGO_TARGET_DIR="$BUILD_DIR/win16" bash build.sh)
	(cd "$ROOT/win9x" && CARGO_TARGET_DIR="$BUILD_DIR/win9x" bash build.sh)
	(cd "$ROOT/dos" && CARGO_TARGET_DIR="$BUILD_DIR/dos" bash build.sh)
	if [ -z "${QUILL_DICT_DIR:-}" ]; then
		QUILL_DICT_DIR="$BUILD_DIR/dic"
		(cd "$ROOT" && CARGO_TARGET_DIR="$BUILD_DIR/host" cargo run -q --release --locked -p quill-dict -- "$QUILL_DICT_DIR")
	fi
fi

need() {
	[ -f "$1" ] || { echo "stage: $1 is missing; run installer/tools/stage.sh --build, or --stand-ins for a check-only stage" >&2 && exit 1; }
}

q16="${QUILL16_EXE:-$ROOT/win16/out/QUILL.EXE}"
hlp="${QUILL16_HLP:-$ROOT/win16/out/QUILL.HLP}"
q32="${QUILL32_EXE:-$ROOT/win9x/dist/QUILL32.EXE}"
qdos="${QUILLDOS_EXE:-$ROOT/dos/dist/QUILLDOS.EXE}"
dicts="${QUILL_DICT_DIR:-$BUILD_DIR/dic}"
for f in "$q16" "$hlp" "$q32" "$qdos" "$dicts/EN_CA.DIC" "$dicts/EN_GB.DIC" "$dicts/EN_US.DIC"; do need "$f"; done

cp "$q16" "$DIST/win16-x86/QUILL.EXE"
cp "$q32" "$DIST/windows-x86/QUILL32.EXE"
cp "$qdos" "$DIST/dos-x86/QUILLDOS.EXE"
cp "$hlp" "$DIST/help/QUILL.HLP"
for d in EN_CA EN_GB EN_US; do cp "$dicts/$d.DIC" "$DIST/dict/$d.DIC"; done
echo "stage: staged in $DIST"
