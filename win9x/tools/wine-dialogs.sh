#!/usr/bin/env bash
# Opens every dialog of QUILL32.EXE under Wine and checks that each is a window of its own with the right title, never text cells on the page.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/wine-dialogs.sh   (needs wine, xvfb-run, xdotool and ImageMagick; exits 0 with a note when they are missing)
#
# The window clicks the menus (the positions are those of the default window, centred on the 1000 by 700 screen, at 70,47 with no window
# manager; the script measures where the window is and shifts every position by the difference), then asks
# `xdotool search --name` for the dialog's window by title. A dialog drawn in text cells would not be a window and the search would
# find nothing (D86). One screenshot per dialog is left in $DIALOG_SHOTS (default: a scratch directory).
set -euo pipefail
# The caret stays solid so that screenshots are stable (`[Quill] Blink`, win9x/tools/wine-blink.sh).
export QUILL_NO_BLINK=1
cd "$(dirname "$0")/../.."

for tool in wine xvfb-run xdotool import; do
	command -v "$tool" > /dev/null || { echo "wine-dialogs: $tool not found; skipping"; exit 0; }
done
exe=win9x/dist/QUILL32.EXE
[ -f "$exe" ] || { echo "wine-dialogs: build $exe first (win9x/build.sh)"; exit 1; }
export WINEPREFIX="${WINEPREFIX:-${TMPDIR:-/tmp}/quill-wineprefix}" WINEDEBUG=-all
unset WINEARCH
work="$(mktemp -d "${TMPDIR:-/tmp}/quill-dialogs.XXXXXX")"
shots="${DIALOG_SHOTS:-$work}"
mkdir -p "$shots"
cp "$exe" "$work/QUILL32.EXE"
cp tests/docs/letter.rtf "$work/LETTER.RTF"
# The Spelling dialog needs a dictionary beside the program.
cargo run -q -p quill-dict -- "$work/dic" > /dev/null
cp "$work/dic/EN_CA.DIC" "$work/EN_CA.DIC"
trap 'rm -rf "$work"' EXIT

cat > "$work/drive.sh" << 'DRIVE'
#!/usr/bin/env bash
set -u
shots="$1"
fail=0
# The window opens centred on the work area, so every position below is given for a window at 70,47 and shifted to where it is.
shift_x=0
shift_y=0
click() { xdotool mousemove "$(( $1 + shift_x ))" "$(( $2 + shift_y ))"; sleep 1; xdotool click 1; sleep 1; }
# menu X Y: opens the menu whose title is at x=X (the menu bar is at y=58) and chooses the item at y=Y. The pointer rests a moment before
# each click; a click sent as the pointer arrives is lost when the previous menu has only just closed.
menu() { click "$1" 58; click "$(( $1 + 12 ))" "$2"; sleep 2; }
expect() {
	# expect TITLE SHOT: the dialog's window must exist; a screenshot is kept; Escape closes it.
	if xdotool search --onlyvisible --name "$1" > /dev/null 2>&1; then
		echo "wine-dialogs: ok: $1 is a window"
	else
		echo "wine-dialogs: FAIL: no window named $1" >&2
		fail=1
	fi
	import -window root "$shots/$2.png"
	xdotool key Escape
	sleep 1
}
sleep 13
geometry="$(xdotool search --onlyvisible --name '^Quill - ' getwindowgeometry --shell | head -4)"
shift_x=$(( $(sed -n 's/^X=//p' <<< "$geometry" | head -1) - 70 ))
shift_y=$(( $(sed -n 's/^Y=//p' <<< "$geometry" | head -1) - 47 ))
menu 440 94;  expect "About Quill" about
menu 188 78;  expect "Find" find
menu 188 112; expect "Replace" replace
menu 234 224; expect "Fonts" fonts
menu 295 292; expect "Tabs" tabs
menu 83 95;   expect "Open" open
menu 83 138;  expect "Save As" save-as
menu 151 77;  expect "Insert Picture" insert-picture
# Unsaved changes: type, then File > New asks in a message box.
xdotool key x
sleep 1
menu 83 78
expect "^Quill$" confirm-save
# Spelling: a made-up word is the first the check stops at.
xdotool type "qqzx "; sleep 1
menu 188 138; expect "Spelling" spelling
exit $fail
DRIVE
chmod +x "$work/drive.sh"
(cd "$work" && timeout 240 xvfb-run -a -s "-screen 0 1000x700x24" bash -c "wine QUILL32.EXE LETTER.RTF > /dev/null 2>&1 & ./drive.sh '$shots'; code=\$?; wineserver -k; exit \$code")
echo "wine-dialogs: ok: every dialog is a window; screenshots in $shots"
