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
# argument), @mono (/MONO) and @rows 50 (/50). Everything else is key tokens; # starts a comment.
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
		@rows) [ "$a" != 50 ] || args="$args /50" ;;
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
run_scenario m1-edit-50 PLAIN.QLD m1-edit.qld
run_scenario m2-letter OUT.RTF m2-letter.rtf
run_scenario m2-mono - -
run_scenario m2-find - -
run_scenario m2-dialogs - -
run_scenario m4-about QUILL.INI m4-quill.ini
run_scenario m4-print OUT.PRN letter.print.txt
run_scenario m4-print-headers OUT.PRN headers.print.txt
run_scenario m7-picture OUT.QLD m7-picture.qld
run_scenario m8-spelling QUILL.UDC m8-quill.udc
run_scenario m2-headers - -
run_scenario m2-lists - -
run_scenario m10-menus - -
run_scenario m11-recent - -

# The graphics shell (tests/scenarios/g6-gfx.keys): the same keys are played into QUILLDOS.EXE in graphics mode and on the host through
# quill-paint's shell. QUILL.FNT on the disk is the committed test font, so a frame is the same bytes on both: every frame's CRC-32 must
# equal the host's golden (tests/golden/gfx/g6-gfx.{16,256}.sums), equal the CRC-32 of the video memory read back (the presenter put
# every changed row there), and the saved file must be the host's.
run_gfx_scenario() { # <label> <machine> <arguments> <16|256>
	local label="$1" machine="$2" args="$3" depth="$4"
	[ -z "${E2E_ONLY:-}" ] || [ "$E2E_ONLY" = "$label" ] || return 0
	local file="$ROOT/tests/scenarios/g6-gfx.keys"
	local disk="$WORK/disk-$label"
	rm -rf "$disk" "$WORK/keep"
	mkdir -p "$disk"
	cp "$HERE/dist/QUILLDOS.EXE" "$disk/"
	cp "$ROOT/tests/fonts/QUILLTST.FNT" "$disk/QUILL.FNT"
	cp "$ROOT/tests/docs/letter.rtf" "$disk/LETTER.RTF"
	local keys
	keys="$(grep -v '^@' "$file" | sed 's/#.*//' | tr -s ' \n\t' ' ' | sed 's/^ //; s/ $//')"
	export DOS_ENV=$'QUILL_DUMP=D:\\DUMP.TXT\nQUILL_TIMES=D:\\TIMES.TXT'
	export DOS_KEEP="$WORK/keep"
	# A frame takes longer to draw than a text screen, and a busy host stretches it: the keys go in slowly enough that the BIOS buffer never fills.
	export DOS_AUTOTYPE="-w 2 -p 0.6 $keys"
	export DOS_TIMEOUT=400
	local rc=0
	DOS_MACHINE="$machine" "$HERE/tools/dosrun.sh" "$disk" "QUILLDOS.EXE $args LETTER.RTF" > "$WORK/$label.out" || rc=$?
	[ "$rc" = 0 ] || fail "$label: QUILLDOS.EXE exited with $rc"
	[ ! -s "$WORK/$label.out" ] || { cat "$WORK/$label.out" >&2; fail "$label: it printed something, so it did not stay in graphics"; }
	tr -d '\r' < "$WORK/keep/DUMP.TXT" > "$WORK/$label-frames.txt"
	grep '^frame' "$WORK/$label-frames.txt" | sed 's/ video .*//' > "$WORK/$label-sums.txt"
	diff -u "$ROOT/tests/golden/gfx/g6-gfx.$depth.sums" "$WORK/$label-sums.txt" > "$WORK/$label.diff" || { head -40 "$WORK/$label.diff" >&2; fail "$label: a frame differs from the host's"; }
	if awk '/^frame/ && $4 != $6 { bad = 1; print > "/dev/stderr" } END { exit bad }' "$WORK/$label-frames.txt"; then :; else
		fail "$label: the video memory differs from the frame drawn (the presenter missed a row)"
	fi
	echo "e2e: $label: all $(grep -c '^frame' "$WORK/$label-frames.txt") frames equal the host's, and the video memory read back equals each frame"
	cmp "$disk/LETTER.RTF" "$ROOT/tests/golden/files/g6-gfx.rtf" || fail "$label: the saved LETTER.RTF differs from the host's"
	echo "e2e: $label: the saved LETTER.RTF is byte-identical to the host's"
}

run_gfx_scenario g6-gfx-12h svga_s3 /G16 16
run_gfx_scenario g6-gfx-vbe svga_s3 /G 256
# A card whose VBE has no linear frame buffer gets mode 12h under /G, and with no QUILL.FNT graphics gives way to text with a note.
run_gfx_scenario g6-gfx-nolfb vesa_nolfb /G 16
if [ -z "${E2E_ONLY:-}" ] || [ "$E2E_ONLY" = g6-gfx-nofont ]; then
	disk="$WORK/disk-nofont"
	rm -rf "$disk"
	mkdir -p "$disk"
	cp "$HERE/dist/QUILLDOS.EXE" "$disk/"
	rc=0
	DOS_AUTOTYPE="-w 2 -p 0.3 f10 end enter" "$HERE/tools/dosrun.sh" "$disk" 'QUILLDOS.EXE /G' > "$WORK/nofont.out" || rc=$?
	[ "$rc" = 0 ] || fail "g6-gfx-nofont: QUILLDOS.EXE exited with $rc"
	grep -q 'graphics mode was not used (QUILL.FNT was not found); it ran in text mode' "$WORK/nofont.out" || { cat "$WORK/nofont.out" >&2; fail "g6-gfx-nofont: no note"; }
	echo "e2e: g6-gfx-nofont: without QUILL.FNT it runs in text mode and says why"
fi
echo "e2e: ok"
