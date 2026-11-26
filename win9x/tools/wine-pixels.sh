#!/usr/bin/env bash
# Plays one script in two builds of QUILL32.EXE under Wine, captures the screen at each state and counts the pixels that differ (W9X-3).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-pixels.sh OLD.EXE NEW.EXE [--shots DIR] [--only NAME[,NAME...]]
#        win9x/tools/wine-pixels.sh --calls BENCH.EXE [--update] [--shots DIR] [--only NAME[,NAME...]]
#
# The first form is the proof a change to the painter must pass: `compare -metric AE` between the same state of the two builds is 0 at
# every state, or the script exits 1 and says which states differ and by how many pixels. Nothing is stored: Wine draws text with
# FreeType, so a capture from one machine is not another's. Build OLD.EXE from the base (`git stash` or a worktree, `win9x/build.sh`,
# copy `win9x/dist/QUILL32.EXE`) and NEW.EXE from the change. The same file twice must give 0 everywhere.
#
# `--shots DIR` keeps the NEW capture of every state and, for a state that differs, the OLD one and an image of the differing pixels.
#
# The second form plays the script once in a build made with `win9x/build.sh --bench` and writes what the counters saw for the last key
# and the last page paint at each state (calls of each GDI and USER function, characters and lines drawn, pixels filled and bytes
# blitted and paints of the toolbars and the ruler; no times) in the order of the states, to compare with `tests/golden/win9x/paint-calls.txt`: it exits 1 on a difference and
# `--update` writes the file instead. A change that moves the counts says so by changing that file in the same commit.
#
# The states: a letter at Page Width and at 75, 100, 150 and 200 percent; the bench text (a screenful of 12 point text) with the caret
# at the start and the end of a line and scrolled by an odd amount; lists, pictures, headers and a Date field; a selection across a format change;
# Check Spelling as You Type with misspellings marked; print preview. Needs wine, Xvfb, xdotool and ImageMagick; exits 0 with a note
# when they are missing. Uses its own display (QUILL_PIXELS_DISPLAY, default :176) and a WINEPREFIX of its own under the cache
# directory, at 640 by 480. Local only: Wine paces it.
set -euo pipefail
# The caret stays solid so that screenshots are stable (`[Quill] Blink`, win9x/tools/wine-blink.sh).
export QUILL_NO_BLINK=1
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
OLD=""
NEW=""
CALLS=""
UPDATE=0
SHOTS=""
ONLY=""
while [ "$#" -gt 0 ]; do
	case "$1" in
	--shots) SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"; shift 2 ;;
	--only) ONLY=",$2,"; shift 2 ;;
	--calls) CALLS="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"; shift 2 ;;
	--update) UPDATE=1; shift ;;
	-*) echo "wine-pixels: unknown option $1" >&2; exit 2 ;;
	*)
		if [ -z "$OLD" ]; then OLD="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
		elif [ -z "$NEW" ]; then NEW="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
		else echo "wine-pixels: too many files" >&2; exit 2; fi
		shift ;;
	esac
done
for t in wine Xvfb xdotool import compare convert; do
	command -v "$t" > /dev/null || { echo "wine-pixels: $t not found; skipping"; exit 0; }
done
if [ -n "$CALLS" ]; then
	[ -f "$CALLS" ] || { echo "wine-pixels: no $CALLS (build it with win9x/build.sh --bench)" >&2; exit 1; }
else
	[ -f "$OLD" ] && [ -f "$NEW" ] || { echo "usage: wine-pixels.sh OLD.EXE NEW.EXE | --calls BENCH.EXE" >&2; exit 2; }
fi
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-pixels.XXXXXX")"
export DISPLAY="${QUILL_PIXELS_DISPLAY:-:176}" WINEDEBUG=-all
export WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-pixels}"
unset WINEARCH
XPID=""
WPID=""
cleanup() {
	[ -z "$WPID" ] || kill "$WPID" 2> /dev/null || true
	wineserver -k 2> /dev/null || true
	[ -z "$XPID" ] || kill "$XPID" 2> /dev/null || true
	rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "wine-pixels: FAIL: $*" >&2; exit 1; }

# ---- the documents ---------------------------------------------------------------------------------------------------------------------
mkdir -p "$WORK/docs"
for f in letter lists pictures headers; do cp "$ROOT/tests/docs/$f.rtf" "$WORK/docs/$f.rtf"; done
# The bench text: a screenful of 12 point text, two misspellings in each sentence.
python3 -I - "$WORK/docs/bench.rtf" << 'PY'
import sys
s = "The quick brown fox jumped over the lazy dog with some fine words and a sentense is teh error. "
open(sys.argv[1], "w", newline="").write("{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Times New Roman;}}\\f0\\fs24 " + s * 40 + "\\par}\r\n")
PY
# A Date field in the body: the screen shows today's date where it is drawn.
printf '%s\r\n' '{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}\f0\fs24 Today is {\field{\*\fldinst DATE}{\fldrslt 1/1/1990}} and that is the date.\par}' > "$WORK/docs/fields.rtf"
(cd "$ROOT" && cargo run -q -p quill-dict -- "$WORK/dic" > /dev/null)
cp "$WORK/dic/EN_CA.DIC" "$WORK/docs/EN_CA.DIC"

# ---- driving one build -----------------------------------------------------------------------------------------------------------------
# The log the bench build writes, as a Windows path under Wine's Z: drive.
BENCHLOG="$WORK/bench.log"
winpath() { printf 'Z:%s' "${1//\//\\}"; }

# snap OUTDIR NAME: waits for the screen to settle (two captures a second apart agree), then keeps the capture.
snap() {
	local out="$1" name="$2" tries=0
	import -window root "$out/$name.png"
	while [ "$tries" -lt 12 ]; do
		sleep 1
		import -window root "$WORK/again.png"
		if [ "$(compare -metric AE "$out/$name.png" "$WORK/again.png" null: 2>&1 | cut -d' ' -f1 | cut -d. -f1)" = 0 ]; then break; fi
		cp "$WORK/again.png" "$out/$name.png"
		tries=$((tries + 1))
	done
	if [ -n "$CALLS" ]; then record "$out" "$name"; fi
}

# record OUTDIR NAME: what the counters saw for the last key and the last page paint, without the paint's number or any time.
record() {
	{
		echo "== $2"
		tr -d '\r' < "$BENCHLOG" | awk '/^last key:/ { sub(/^last key: n=[0-9]+ /, "key: "); print; getline; print; next }
			/^last page paint/ { sub(/^last page paint #[0-9]+ of [0-9]+: n=[0-9]+ /, "paint: "); print; getline; print; next }'
	} >> "$1/calls.txt"
}

want() {
	[ -n "$ONLY" ] || return 0
	case "$ONLY" in *",$1,"*) return 0 ;; esac
	return 1
}

# session EXE OUTDIR DOCUMENT ZOOM SPELL STEPS...: starts the build on the document with the settings, then runs the steps (functions)
# with OUTDIR set; a step is `name:command args`.
session() {
	local exe="$1" out="$2" doc="$3" zoom="$4" spell="$5"
	shift 5
	local any=0 s
	for s in "$@"; do want "${s%%:*}" && any=1; done
	[ "$any" = 1 ] || return 0
	{
		printf '[Quill]\r\nZoom=%s\r\nAsYouType=%s\r\n' "$zoom" "$spell"
		[ -z "$CALLS" ] || printf 'BenchLog=%s\r\nBenchEach=1\r\n' "$(winpath "$BENCHLOG")"
	} > "$WINEPREFIX/drive_c/windows/QUILL.INI"
	rm -f "$BENCHLOG"
	cp "$exe" "$WORK/docs/QUILL32.EXE"
	(cd "$WORK/docs" && wine QUILL32.EXE "$doc" > /dev/null 2>&1) &
	WPID=$!
	for _ in $(seq 1 40); do
		sleep 2
		if xdotool search --name Quill > /dev/null 2>&1; then break; fi
	done
	# The page is up when the white of the paper shows left of the middle; the first start in a new prefix can take a minute.
	for _ in $(seq 1 60); do
		sleep 2
		import -window root "$WORK/ready.png" 2> /dev/null || true
		[ "$(convert "$WORK/ready.png" -format '%[hex:p{30,300}]' info: 2> /dev/null || true)" != FFFFFF ] || break
	done
	sleep 3
	xdotool mousemove 300 300 click 1
	sleep 1
	for s in "$@"; do
		local name="${s%%:*}" cmd="${s#*:}"
		want "$name" || continue
		# shellcheck disable=SC2086
		$cmd
		# The pointer stays over the page: with no window manager the keyboard goes to the window under it.
		xdotool mousemove 300 300
		sleep 1
		snap "$out" "$name"
	done
	kill "$WPID" 2> /dev/null || true
	wineserver -k 2> /dev/null || true
	WPID=""
	sleep 2
}

k() { xdotool key "$@"; sleep 0.4; }
typ() { xdotool type --delay 30 "$1"; sleep 0.5; }
nothing() { :; }
# Bold in the middle of a line and the whole line selected across it.
across() { k ctrl+a Delete; typ "plain "; k ctrl+b; typ "bold "; k ctrl+b; typ "plain again"; k shift+Home; }
odd() {
	k ctrl+Home
	xdotool click 5
	sleep 0.5
	for _ in 1 2 3 4 5 6 7; do k Down; done
}
below() { for _ in $(seq 1 25); do k Down; done; }
preview() { k alt+f; sleep 0.5; k v; sleep 2; }
settle() { sleep "$1"; }

play() {
	local exe="$1" out="$2"
	mkdir -p "$out"
	: > "$out/calls.txt"
	for z in Width 75 100 150 200; do
		session "$exe" "$out" letter.rtf "$z" 0 "letter-$z:nothing"
	done
	session "$exe" "$out" bench.rtf Width 0 "bench-top:nothing" "bench-line-start:k Home" "bench-line-end:k End" "bench-odd:odd" "bench-below:below"
	session "$exe" "$out" bench.rtf 100 0 "bench100-top:nothing" "bench100-odd:odd"
	session "$exe" "$out" lists.rtf Width 0 "lists:nothing"
	session "$exe" "$out" lists.rtf 100 0 "lists100:nothing"
	session "$exe" "$out" pictures.rtf Width 0 "pictures:nothing" "pictures-below:below"
	session "$exe" "$out" pictures.rtf 100 0 "pictures100:nothing"
	session "$exe" "$out" fields.rtf Width 0 "fields:nothing"
	session "$exe" "$out" headers.rtf Width 0 "headers:nothing"
	session "$exe" "$out" headers.rtf 100 0 "headers100:nothing"
	session "$exe" "$out" letter.rtf Width 0 "across-format:across"
	session "$exe" "$out" letter.rtf 100 0 "across-format-100:across"
	session "$exe" "$out" bench.rtf Width 1 "spell-on:settle 12" "spell-on-odd:odd" "spell-on-end:k End"
	session "$exe" "$out" bench.rtf 100 1 "spell-on100:settle 12"
	session "$exe" "$out" letter.rtf Width 0 "preview:preview"
	session "$exe" "$out" lists.rtf 100 0 "preview-lists100:preview"
}

[ -d "$WINEPREFIX/drive_c" ] || WINEDEBUG=-all wineboot --init > /dev/null 2>&1 || true
Xvfb "$DISPLAY" -screen 0 640x480x24 > /dev/null 2>&1 &
XPID=$!
sleep 2
kill -0 "$XPID" 2> /dev/null || fail "Xvfb did not start on $DISPLAY (is the display in use?)"

if [ -n "$CALLS" ]; then
	play "$CALLS" "$WORK/calls"
	GOLDEN="$ROOT/tests/golden/win9x/paint-calls.txt"
	if [ "$UPDATE" = 1 ]; then
		mkdir -p "$(dirname "$GOLDEN")"
		# shellcheck disable=SC2016
		{
			printf '# What the counters of a `win9x/build.sh --bench` build saw at each state of win9x/tools/wine-pixels.sh, under Wine: the\n'
			printf '# last key and the last page paint (calls of each GDI and USER function, characters and lines drawn, pixels filled, bytes\n'
			printf '# blitted, paints of the toolbars and the ruler). Regenerate with `wine-pixels.sh --calls win9x/dist/QUILL32-BENCH.EXE --update` when a change moves them on purpose.\n'
			cat "$WORK/calls/calls.txt"
		} > "$GOLDEN"
		echo "wine-pixels: wrote $GOLDEN"
		exit 0
	fi
	[ -n "$ONLY" ] && { cat "$WORK/calls/calls.txt"; exit 0; }
	if diff <(grep -v '^#' "$GOLDEN") "$WORK/calls/calls.txt" > "$WORK/calls.diff"; then
		echo "wine-pixels: the call counts equal $GOLDEN"
		exit 0
	fi
	head -40 "$WORK/calls.diff" >&2
	fail "the call counts differ from $GOLDEN"
fi

play "$OLD" "$WORK/old"
play "$NEW" "$WORK/new"
bad=0
total=0
for f in "$WORK"/old/*.png; do
	name="$(basename "$f" .png)"
	total=$((total + 1))
	n="$(compare -metric AE "$f" "$WORK/new/$name.png" "$WORK/$name-diff.png" 2>&1 | cut -d' ' -f1 | cut -d. -f1 || true)"
	case "$n" in '' | *[!0-9]*) n="not comparable" ;; esac
	[ -z "$SHOTS" ] || cp "$WORK/new/$name.png" "$SHOTS/$name.png"
	if [ "$n" = 0 ]; then
		printf 'wine-pixels: %-22s 0 pixels differ\n' "$name"
	else
		printf 'wine-pixels: %-22s %s PIXELS DIFFER\n' "$name" "$n"
		bad=$((bad + 1))
		if [ -n "$SHOTS" ]; then
			cp "$f" "$SHOTS/$name-old.png"
			cp "$WORK/$name-diff.png" "$SHOTS/$name-diff.png"
		fi
	fi
done
[ "$total" -gt 0 ] || fail "no states were captured"
[ "$bad" -eq 0 ] || fail "$bad of $total states differ"
echo "wine-pixels: ok: $total states, 0 pixels differ in any"
