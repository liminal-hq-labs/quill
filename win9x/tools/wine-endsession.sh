#!/usr/bin/env bash
# Ends the Wine session while QUILL32.EXE holds unsaved work and checks that Quill asks, that Cancel stops the shutdown, and that a
# real end leaves its work behind.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-endsession.sh   (needs wine, xvfb-run, xdotool; exits 0 with a note when they are missing)
#
# `wineboot --end-session` sends WM_QUERYENDSESSION to every top-level window, and WM_ENDSESSION when none answers FALSE, as Windows
# does for a log off or a shutdown (CORE-7, W9X-8). Three runs, each on a modified copy of LETTER.RTF (an `x` typed at its start):
#   1. Cancel in the question: wineboot reports the end cancelled (exit status 1), LETTER.RTF is unchanged and `~Quill-LETTER.RTF.rcv` holds the text
#      (a refusal still copies it aside, for systems that shut down whatever the answer).
#   2. Yes (Save): the end goes on and LETTER.RTF holds the typed `x`.
#   3. No (Don't Save): the end goes on, LETTER.RTF is unchanged, and `~Quill-LETTER.RTF.rcv` beside the document holds the typed text.
# The question must be answered within a few seconds: Wine then offers to terminate the program, as Windows does a program that does not
# answer. Each run has its own scratch directory and Wine prefix, which is stopped with `wineserver -k` for that prefix only.
set -euo pipefail
# The caret stays solid so that screenshots are stable (`[Quill] Blink`, win9x/tools/wine-blink.sh).
export QUILL_NO_BLINK=1
cd "$(dirname "$0")/../.."

for tool in wine xvfb-run xdotool; do
	command -v "$tool" > /dev/null || { echo "wine-endsession: $tool not found; skipping"; exit 0; }
done
exe=win9x/dist/QUILL32.EXE
[ -f "$exe" ] || { echo "wine-endsession: build $exe first (win9x/build.sh)"; exit 1; }
export WINEDEBUG=-all
unset WINEARCH
work="$(mktemp -d "${TMPDIR:-/tmp}/quill-endsession.XXXXXX")"
export WINEPREFIX="$work/prefix"
trap 'wineserver -k 2> /dev/null || true; [ -n "${KEEP:-}" ] || rm -rf "$work"' EXIT
cp "$exe" "$work/QUILL32.EXE"

cat > "$work/drive.sh" << 'DRIVE'
#!/usr/bin/env bash
set -u
cd "$1"
mode="$2"
cp "$3" LETTER.RTF
rm -f "~Quill-LETTER.RTF.rcv"
wine QUILL32.EXE LETTER.RTF > /dev/null 2>&1 &
sleep 14
xdotool mousemove 300 300 click 1
xdotool type x
sleep 1
(timeout 60 wine wineboot --end-session > /dev/null 2>&1; echo $? > wineboot.status) &
sleep 2
xdotool search --onlyvisible --name '^Quill$' > /dev/null || { echo "wine-endsession: FAIL: $mode: no question"; exit 1; }
case "$mode" in
cancel) xdotool key Escape ;;
yes) xdotool key y ;;
no) xdotool key n ;;
esac
sleep 8
exit 0
DRIVE
chmod +x "$work/drive.sh"

run() {
	rm -f "$work/wineboot.status"
	timeout 120 xvfb-run -a -s "-screen 0 1000x700x24" bash -c "'$work/drive.sh' '$work' $1 '$PWD/tests/docs/letter.rtf'; code=\$?; wineserver -k; exit \$code"
}

fail=0
run cancel
[ "$(cat "$work/wineboot.status" 2> /dev/null)" = 1 ] || { echo "wine-endsession: FAIL: Cancel did not cancel the end of the session"; fail=1; }
cmp -s "$PWD/tests/docs/letter.rtf" "$work/LETTER.RTF" || { echo "wine-endsession: FAIL: Cancel changed the file"; fail=1; }
[ -s "$work/~Quill-LETTER.RTF.rcv" ] || { echo "wine-endsession: FAIL: Cancel left no recovery file"; fail=1; }
echo "wine-endsession: cancel checked"
run yes
grep -aq 'x' "$work/LETTER.RTF" && ! cmp -s "$PWD/tests/docs/letter.rtf" "$work/LETTER.RTF" || { echo "wine-endsession: FAIL: Yes did not save"; fail=1; }
echo "wine-endsession: yes checked"
run no
cmp -s "$PWD/tests/docs/letter.rtf" "$work/LETTER.RTF" || { echo "wine-endsession: FAIL: No changed the file"; fail=1; }
[ -s "$work/~Quill-LETTER.RTF.rcv" ] || { echo "wine-endsession: FAIL: no recovery file after the session ended"; fail=1; }
echo "wine-endsession: no checked"
[ "$fail" = 0 ] && echo "wine-endsession: ok"
exit $fail
