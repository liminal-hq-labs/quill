#!/usr/bin/env bash
# Plays the test scenarios into QUILL32.EXE under Wine (on Xvfb) and compares its screens and saved files with the host's goldens.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-e2e.sh [SCENARIO...]     (default: every scenario with a golden)
#
# QUILL32.EXE reads the scenario's key tokens with /PLAY and writes every screen it shows with /DUMP, as the DOS build does under
# DOSBox (dos/tools/e2e.sh). Needs wine and Xvfb; exits 0 with a note when they are missing. The run has an X server of its own, on a
# free display number and stopped by its own process id, and never the desktop's: WAYLAND_DISPLAY is unset (Wine 11 prefers a Wayland
# display to $DISPLAY and the program would open on the desktop, where the run never ends). WINEPREFIX defaults to
# ~/.cache/quill-wineprefix, made on the first run (about ten seconds) and kept; delete it to start again.
#
# Scenarios whose Open, Save As and Insert Picture show the file dialog on the host (the DOS builds link it) run here with `@prompt`,
# the one-line name box the Windows builds' /PLAY answers with, against tests/golden/prompt (`cargo test -p quill-host --test golden`
# keeps those; QUILL_UPDATE_GOLDEN=1 rewrites them).
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
command -v wine > /dev/null && command -v Xvfb > /dev/null || { echo "wine-e2e: wine or Xvfb not found; skipping"; exit 0; }
"$HERE/build.sh"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-wine.XXXXXX")"
XPID=""
cleanup() {
	set +e
	[ -z "$XPID" ] || { kill "$XPID" 2> /dev/null && wait "$XPID" 2> /dev/null; }
	rm -rf "$WORK"
}
trap cleanup EXIT
export WINEPREFIX="${WINEPREFIX:-$HOME/.cache/quill-wineprefix}" WINEDEBUG=-all
unset WINEARCH WAYLAND_DISPLAY XAUTHORITY
fail() { echo "wine-e2e: FAIL: $*" >&2; exit 1; }
n=90
while [ -e "/tmp/.X11-unix/X$n" ] || [ -e "/tmp/.X$n-lock" ]; do n=$((n + 1)); done
Xvfb ":$n" -screen 0 1024x768x24 -ac -nolisten tcp > /dev/null 2>&1 &
XPID=$!
export DISPLAY=":$n"
for _ in $(seq 1 50); do
	[ -S "/tmp/.X11-unix/X$n" ] && break
	sleep 0.1
done
[ -S "/tmp/.X11-unix/X$n" ] || fail "the virtual X server did not start"

# scenario name, saved file, golden saved file
declare -A SAVED=([m1-edit]="PLAIN.QLD m1-edit.qld" [m2-letter]="OUT.RTF m2-letter.rtf" [m4-about]="QUILL.INI m4-quill.ini" [m4-print]="OUT.PRN letter.print.txt" [m4-print-headers]="OUT.PRN headers.print.txt" [m7-picture]="OUT.QLD m7-picture.qld" [m8-spelling]="QUILL.UDC m8-quill.udc")
names=("$@")
[ ${#names[@]} -gt 0 ] || names=(m1-edit m2-letter m2-mono m2-find m2-lists m2-headers m2-dialogs m4-about m4-print m4-print-headers m7-picture m8-spelling m15-picture-select m14-pagebreak-enter m14-paste-undo m14-cut-guard)
for name in "${names[@]}"; do
	d="$WORK/$name"
	mkdir -p "$d"
	cp "$HERE/dist/QUILL32.EXE" "$d/"
	# The Windows builds never link the file dialog (they open the system's, and /PLAY answers with the one-line name box), so a scenario
	# that the host plays with the dialog is played here with `@prompt`, against the host's prompt goldens in tests/golden/prompt.
	{ printf '@prompt\n'; cat "$ROOT/tests/scenarios/$name.keys"; } > "$d/SCRIPT.TXT"
	golden_dir="$ROOT/tests/golden/screens"
	[ ! -f "$ROOT/tests/golden/prompt/$name.frames.txt" ] || golden_dir="$ROOT/tests/golden/prompt"
	while read -r kw a; do
		[ "$kw" = @copy ] && cp "$ROOT/tests/docs/$a" "$d/$(echo "$a" | tr '[:lower:]' '[:upper:]')"
	done < <(grep '^@copy' "$ROOT/tests/scenarios/$name.keys" | sed 's/#.*//')
	(cd "$d" && timeout 180 wine QUILL32.EXE /PLAY SCRIPT.TXT /DUMP OUT.TXT > /dev/null 2>&1) || fail "$name: QUILL32.EXE did not finish"
	tr -d '\r' < "$d/OUT.TXT" > "$d/frames.txt"
	diff -q "$golden_dir/$name.frames.txt" "$d/frames.txt" > /dev/null || fail "$name: the screens differ from the host's"
	echo "wine-e2e: $name: every screen matches the host's"
	if [ -n "${SAVED[$name]:-}" ]; then
		read -r out golden <<< "${SAVED[$name]}"
		golden_file="$ROOT/tests/golden/files/$golden"
		[ ! -f "$ROOT/tests/golden/prompt/$golden" ] || golden_file="$ROOT/tests/golden/prompt/$golden"
		cmp "$d/$out" "$golden_file" || fail "$name: the saved $out differs from the host's"
		echo "wine-e2e: $name: the saved $out is byte-identical to the host's"
	fi
done
echo "wine-e2e: ok"
