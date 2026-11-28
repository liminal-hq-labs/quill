#!/usr/bin/env bash
# Drives QUILL32.EXE under Wine on a 640 by 480 Xvfb screen and checks what it puts on the Windows clipboard and what it takes from it.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-clipboard.sh [--shots DIR]     (build first with win9x/build.sh)
#
# CLIPCHECK.EXE (win9x/clipcheck, built here) is the other program: it reads the clipboard through the Windows calls, so what it sees is
# what WordPad would see, and it writes the clipboard the way WordPad would. Checks: Copy puts `Rich Text Format` (with the bold, italic
# and both sizes of the text) and CF_TEXT on it, by Ctrl+C, Ctrl+Insert and Ctrl+X (which also deletes); Paste takes RTF written by
# another program with its bold, italic and colour, by Ctrl+V and Shift+Insert; text alone and Unicode text alone paste as text; a
# picture goes out as CF_DIB and comes back; a damaged RTF falls back to the text beside it; one Undo takes a paste back; a clipboard
# another program holds open delays Copy for a moment and never hangs Quill; the Edit menu greys Paste on an empty clipboard. Needs
# wine, Xvfb, xdotool, ImageMagick and cargo (with the toolchain of win9x/build.sh); exits 0 with a note when they are missing. Uses its
# own display (QUILL_CLIPBOARD_DISPLAY, default :186) and a WINEPREFIX of its own under the cache directory. Local only: Wine paces it.
set -euo pipefail
# The caret stays solid so that screenshots are stable (`[Quill] Blink`, win9x/tools/wine-blink.sh).
export QUILL_NO_BLINK=1
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
SHOTS=""
if [ "${1:-}" = "--shots" ]; then SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"; fi
for t in wine wineserver Xvfb xdotool import compare cargo; do
	command -v "$t" > /dev/null || { echo "wine-clipboard: $t not found; skipping"; exit 0; }
done
[ -f "$HERE/dist/QUILL32.EXE" ] || { echo "wine-clipboard: build win9x/dist/QUILL32.EXE first" >&2; exit 1; }
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-clipboard.XXXXXX")"
export DISPLAY="${QUILL_CLIPBOARD_DISPLAY:-:186}" WINEDEBUG=-all
export WINEPREFIX="${WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/quill-wine-clipboard}"
unset WINEARCH WAYLAND_DISPLAY
XPID=""
WPID=""
HPID=""
cleanup() {
	[ -z "$HPID" ] || kill "$HPID" 2> /dev/null || true
	[ -z "$WPID" ] || kill "$WPID" 2> /dev/null || true
	wineserver -k 2> /dev/null || true
	[ -z "$XPID" ] || kill "$XPID" 2> /dev/null || true
	rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "wine-clipboard: FAIL: $*" >&2; exit 1; }
shot() {
	import -window root "$WORK/$1.png"
	if [ -n "$SHOTS" ]; then cp "$WORK/$1.png" "$SHOTS/clipboard-$1.png"; fi
}
differ() { compare -metric AE "$WORK/$1.png" "$WORK/$2.png" null: 2>&1 | cut -d' ' -f1 | cut -d. -f1 || true; }

# The other program.
(cd "$HERE" && RUSTC_BOOTSTRAP=1 cargo build --release --locked -p clipcheck > /dev/null 2>&1) || fail "CLIPCHECK.EXE did not build"
TARGET="${CARGO_TARGET_DIR:-$HERE/target}"
cp "$TARGET/win9x/release/clipcheck.exe" "$WORK/CLIPCHECK.EXE"
cp "$HERE/dist/QUILL32.EXE" "$WORK/QUILL32.EXE"
cp "$ROOT/tests/docs/pic.bmp" "$WORK/PIC.BMP"
cd "$WORK"
# The seed: two sizes. Typing adds bold and italic.
printf '%s' '{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}\pard\fs48 Big\fs24  small\par}' > SEED.RTF
# What another program writes: bold, italic and a colour, in another face.
printf '%s' '{\rtf1\ansi\deff0{\fonttbl{\f0\fswiss Arial;}}{\colortbl;\red255\green0\blue0;}\pard \b Bold\b0  \cf1 red\cf0  \i it\i0\par}' > OTHER.RTF
printf 'Bold red it' > OTHER.TXT
printf 'not rich text at all' > BAD.RTF
printf 'the text beside it' > BESIDE.TXT
printf 'sentinel' > SENTINEL.TXT
# UTF-16LE: "Café ≠ ☺".
printf 'C\0a\0f\0\xe9\0 \0\x60\x22 \0\x3a\x26' > UNI.UTX
tail -c +15 PIC.BMP > PIC.DIB

clip() { wine CLIPCHECK.EXE "$@" > /dev/null 2>&1; }
# Reads the clipboard into OUT<n>.*, removing what an earlier read left.
n=0
read_clip() {
	n=$((n + 1))
	clip get "OUT$n"
	CUR="OUT$n"
}
has() { grep -qF -- "$2" "$CUR.$1" || fail "$3 (no '$2' in $CUR.$1: $(head -c 300 "$CUR.$1" 2> /dev/null | tr '\r\n' '  '))"; }
lacks() { if grep -qF -- "$2" "$CUR.$1" 2> /dev/null; then fail "$3"; fi; }
# Select everything, copy it, and read the clipboard.
copy_all() {
	xdotool key ctrl+a
	sleep 0.3
	xdotool key "${1:-ctrl+c}"
	sleep 1
	read_clip
}

Xvfb "$DISPLAY" -screen 0 640x480x24 > /dev/null 2>&1 &
XPID=$!
sleep 2
# The prefix is made with the display up: made without one, the first run in it does not come up.
[ -d "$WINEPREFIX/drive_c" ] || wineboot --init > /dev/null 2>&1 || true
(wine QUILL32.EXE SEED.RTF > /dev/null 2>&1) &
WPID=$!
for _ in $(seq 1 40); do
	sleep 2
	if xdotool search --name Quill > /dev/null 2>&1; then break; fi
done
sleep 5
xdotool mousemove 300 250 click 1
sleep 1

# 1. Copy: type bold and italic after the two sizes, copy, and read the clipboard as another program would.
xdotool key ctrl+End
xdotool type --delay 40 " typed "
xdotool key ctrl+b
xdotool type --delay 40 "bold"
xdotool key ctrl+b
xdotool type --delay 40 " "
xdotool key ctrl+i
xdotool type --delay 40 "italic"
sleep 1
shot typed
copy_all ctrl+c
has LST "Rich Text Format" "Ctrl+C did not put Rich Text Format on the clipboard"
has RTF '\b' "the copied RTF has no bold"
has RTF '\i' "the copied RTF has no italic"
has RTF '\fs48' "the copied RTF lost the large size"
has RTF '\fs24' "the copied RTF lost the small size"
has TXT "Big small typed bold italic" "CF_TEXT is not the text"
echo "wine-clipboard: Ctrl+C put Rich Text Format ($(wc -c < "$CUR.RTF") bytes) and CF_TEXT ($(wc -c < "$CUR.TXT") bytes) on the clipboard"
shot copied

# 1a. Ctrl+Insert copies the same.
clip put
copy_all ctrl+Insert
has RTF '\b' "Ctrl+Insert did not put the bold on the clipboard"
has TXT "Big small typed bold italic" "Ctrl+Insert did not put the text on the clipboard"
echo "wine-clipboard: Ctrl+Insert copies"

# 2. Paste of RTF another program wrote replaces the selection with its bold, italic and colour; the clipboard also holds the text, which
# a paste must not prefer.
clip put rtf OTHER.RTF text OTHER.TXT
xdotool key ctrl+a
sleep 0.3
xdotool key ctrl+v
sleep 1
shot pasted_rtf
copy_all ctrl+c
has TXT "Bold red it" "the pasted text is not what the other program wrote"
has RTF '\b' "the pasted RTF lost the bold"
has RTF '\i' "the pasted RTF lost the italic"
has RTF 'red255' "the pasted RTF lost the colour"
lacks RTF 'Big' "Paste did not replace the selection"
echo "wine-clipboard: Ctrl+V took the RTF with its bold, italic and colour"

# 2a. Shift+Insert pastes; one Undo takes the paste back.
xdotool key End
clip put rtf OTHER.RTF
xdotool key shift+Insert
sleep 1
copy_all ctrl+c
[ "$(grep -c 'Bold red it' "$CUR.TXT")" -eq 2 ] || fail "Shift+Insert did not paste at the caret ($(tr '\r\n' '  ' < "$CUR.TXT"))"
xdotool key ctrl+z
sleep 1
copy_all ctrl+c
[ "$(grep -c 'Bold red it' "$CUR.TXT")" -eq 1 ] || fail "one Undo left part of the paste ($(tr '\r\n' '  ' < "$CUR.TXT"))"
echo "wine-clipboard: Shift+Insert pastes and one Undo takes it back"

# 3. Cut deletes and leaves the RTF.
xdotool key ctrl+a
sleep 0.3
xdotool key ctrl+x
sleep 1
shot cut
read_clip
has RTF '\b' "Ctrl+X did not put the bold on the clipboard"
has TXT "Bold red it" "Ctrl+X did not put the text on the clipboard"
clip put text SENTINEL.TXT
xdotool key ctrl+a
sleep 0.3
xdotool key ctrl+c
sleep 1
read_clip
lacks TXT "Bold" "Ctrl+X left the text in the document"
echo "wine-clipboard: Ctrl+X cuts"

# 4. Text alone (CF_TEXT) pastes as text, in the format at the caret.
clip put text OTHER.TXT
xdotool key ctrl+a
xdotool key ctrl+v
sleep 1
copy_all ctrl+c
has TXT "Bold red it" "a paste of text alone did not arrive"
lacks RTF '\cf1' "a paste of text alone brought a colour"

# 4a. Text that is only Unicode (CF_UNICODETEXT) pastes through the CF_TEXT that Windows makes of it: Windows-1252 text where it can.
clip put utext UNI.UTX
xdotool key ctrl+a
xdotool key ctrl+v
sleep 1
copy_all ctrl+c
has TXT "Caf" "a paste of Unicode text alone did not arrive"
printf '%s' "$(od -An -tx1 -v "$CUR.TXT" | tr -d ' \n')" > uni.hex
echo "wine-clipboard: Unicode text alone pasted as the bytes $(cat uni.hex)"
grep -q '^436166e9' uni.hex || fail "the e acute of Unicode text did not arrive as Windows-1252"

# 5. A damaged RTF falls back to the text beside it.
clip put rtf BAD.RTF text BESIDE.TXT
xdotool key ctrl+a
xdotool key ctrl+v
sleep 1
copy_all ctrl+c
has TXT "the text beside it" "a damaged RTF did not fall back to the text"
echo "wine-clipboard: a damaged RTF fell back to CF_TEXT"

# 6. A picture goes out as CF_DIB and comes back the same.
clip put dib PIC.DIB
xdotool key ctrl+a
xdotool key ctrl+v
sleep 2
shot picture_pasted
xdotool key ctrl+a
sleep 0.3
clip put text SENTINEL.TXT
xdotool key ctrl+c
sleep 1
read_clip
grep -q '^8 ' "$CUR.LST" || fail "a lone picture did not put CF_DIB on the clipboard: $(tr '\n' ';' < "$CUR.LST")"
cmp -s "$CUR.DIB" PIC.DIB || fail "the DIB that came back differs from the one that went in ($(wc -c < "$CUR.DIB") and $(wc -c < PIC.DIB) bytes)"
has RTF '\pict' "the picture's RTF has no picture"
echo "wine-clipboard: a picture came back as the same CF_DIB ($(wc -c < "$CUR.DIB") bytes)"

# 7. A clipboard another program holds open: Copy gives up after a few short tries, Quill stays responsive and the clipboard is as the
# other program left it; with the clipboard free again, Copy works.
xdotool key ctrl+a
clip put text SENTINEL.TXT
(wine CLIPCHECK.EXE hold 6000 > /dev/null 2>&1) &
HPID=$!
sleep 2
xdotool key ctrl+c
xdotool type --delay 20 "z"
sleep 1
shot busy
xdotool search --name Quill > /dev/null 2>&1 || fail "Quill went away while the clipboard was held"
wait "$HPID" 2> /dev/null || true
HPID=""
read_clip
has TXT "sentinel" "a Copy against a held clipboard changed it"
xdotool key ctrl+a
sleep 0.3
xdotool key ctrl+c
sleep 1
read_clip
lacks TXT "sentinel" "Copy did not work once the clipboard was free"
has TXT "z" "the character typed while the clipboard was held did not arrive"
echo "wine-clipboard: with the clipboard held, Copy gave up, a character typed after it still arrived, and Copy worked once it was free"

# 8. The Edit menu greys Paste when the clipboard offers nothing Quill reads, and not otherwise.
clip put
xdotool mousemove 48 41 click 1
sleep 1
shot menu_empty
xdotool key Escape
sleep 0.5
xdotool key Escape
clip put text OTHER.TXT
xdotool mousemove 48 41 click 1
sleep 1
shot menu_text
xdotool key Escape
d=$(differ menu_empty menu_text)
echo "wine-clipboard: the menus differ by $d pixels (Paste's label)"
[ "$d" -gt 20 ] && [ "$d" -lt 600 ] || fail "the Edit menu does not show Paste greyed on an empty clipboard and not on text ($d pixels differ)"
echo "wine-clipboard: ok"
