#!/usr/bin/env bash
# Takes the screenshots of the GTK client's chrome (menus, bars, dialogs, light and dark) in a private Xvfb and writes them to $OUT.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: chrome-shots.sh [OUT_DIR]
#
# It starts its own Xvfb on a free display and stops only that process. Clicks and keys go to that display with xdotool and the
# frames are X captures; nothing touches the desktop. ONLY=REGEX takes just the shots whose names match. With SPELL_DICT_DIR set to a folder that holds EN_CA.DIC (the folder is used
# as $XDG_DATA_HOME/quill), the Spelling dialog is shot too. Needs Xvfb, xdotool and ImageMagick.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="${QUILL_BIN:-$here/target/debug/quill}"
doc="$here/../../tests/docs/letter.rtf"
out="${1:-${OUT:-$(mktemp -d)}}"
mkdir -p "$out"
work="$(mktemp -d)"

n=90
while [ -e "/tmp/.X11-unix/X$n" ] || [ -e "/tmp/.X$n-lock" ]; do n=$((n + 1)); done
Xvfb ":$n" -screen 0 1400x1000x24 >"$out/xvfb.log" 2>&1 &
xvfb_pid=$!
cleanup() {
	kill "$xvfb_pid" 2>/dev/null || true
	rm -rf "$work"
}
trap cleanup EXIT
sleep 1

# shot NAME "STEPS" ARGS...: starts the client, plays the steps (c:X,Y a click, d: a double click, T: a triple click, s: a Shift click, g:X1,Y1,X2,Y2 a drag, k:KEYS a key chord, t:TEXT typed text), then captures the window.
shot() {
	local name="$1" steps="$2" t xy
	shift 2
	[ -z "${ONLY:-}" ] || [[ "$name" =~ $ONLY ]] || return 0
	local env=(-u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS QUILL_NO_BLINK=1 GTK_USE_PORTAL=0 "DISPLAY=:$n" GDK_BACKEND=x11 GSK_RENDERER=cairo "XDG_CONFIG_HOME=$work/config")
	[ -n "${SPELL_DICT_DIR:-}" ] && env+=("XDG_DATA_HOME=$SPELL_DICT_DIR")
	env "${env[@]}" "$bin" --new-instance --vendored-fonts --no-settings --zoom 100 --size 1000x700 --quit-after 25000 "$@" \
		>"$out/$name.log" 2>&1 &
	local app=$!
	sleep 2.5
	for t in $steps; do
		case "$t" in
		c:*)
			xy="${t#c:}"
			DISPLAY=":$n" xdotool mousemove "${xy%,*}" "${xy#*,}" click 1
			;;
		k:*) DISPLAY=":$n" xdotool key "${t#k:}" ;;
		t:*) DISPLAY=":$n" xdotool type --delay 80 "${t#t:}" ;;
		d:*)
			xy="${t#d:}"
			DISPLAY=":$n" xdotool mousemove "${xy%,*}" "${xy#*,}" click --repeat 2 --delay 80 1
			;;
		T:*)
			xy="${t#T:}"
			DISPLAY=":$n" xdotool mousemove "${xy%,*}" "${xy#*,}" click --repeat 3 --delay 80 1
			;;
		s:*)
			xy="${t#s:}"
			DISPLAY=":$n" xdotool mousemove "${xy%,*}" "${xy#*,}" keydown shift click 1 keyup shift
			;;
		g:*)
			IFS=, read -r x1 y1 x2 y2 <<<"${t#g:}"
			DISPLAY=":$n" xdotool mousemove "$x1" "$y1" mousedown 1 mousemove $(((x1 + x2) / 2)) $(((y1 + y2) / 2)) mousemove "$x2" "$y2" mouseup 1
			;;
		esac
		sleep 0.8
	done
	DISPLAY=":$n" import -window root "$work/full.png"
	magick "$work/full.png" -crop 1020x720+0+0 +repage "$out/$name.png"
	kill "$app" 2>/dev/null || true
	wait "$app" 2>/dev/null || true
	# A frame taken before the window mapped is a blank one: take it again.
	if [ "$(stat -c %s "$out/$name.png")" -lt 5000 ] && [ -z "${RETRY:-}" ]; then
		RETRY=1 shot "$name" "$steps" "$@"
		return
	fi
	echo "shot $name"
}

for scheme in light dark; do
	shot "chrome-$scheme" "" "--$scheme" "$doc"
done
shot chrome-menu-character-dark "c:232,61" --dark "$doc"
shot chrome-menu-view-light "c:476,61" --light "$doc"
shot chrome-font-dropdown-dark "c:250,134" --dark "$doc"
# A centre tab and a right tab set by clicking the scale; the kind button (left of the ruler) cycles the kind.
shot chrome-ruler-tabs-light "c:14,164 c:330,163 c:14,164 c:14,164 c:560,163" --light "$doc"
shot chrome-bars-hidden-light "c:476,61 c:520,129 c:476,61 c:540,161 c:476,61 c:510,193" --light "$doc"
shot chrome-dialog-fonts-light "" --light --dialog fonts "$doc"
shot chrome-dialog-tabs-dark "" --dark --dialog tabs "$doc"
shot chrome-dialog-find-light "" --light --dialog find "$doc"
shot chrome-dialog-about-dark "" --dark --dialog about "$doc"
shot chrome-unsaved-alert-light "c:500,400 k:ctrl+a c:532,134 c:971,28" --light "$doc"
if [ -n "${SPELL_DICT_DIR:-}" ]; then
	printf '%s' '{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}\f0\fs24 Hello wrold, this is a tset of the speling checker.\par}' >"$work/typos.rtf"
	shot chrome-dialog-spelling-light "" --light --dialog spelling "$work/typos.rtf"
fi
# Editing: typing, a selection, the caret and the title mark, the format bar following the caret, composing text, the refusal toast.
shot editor-typing-light "c:500,330 k:ctrl+End k:Return t:Hello,Quill." --light "$doc"
shot editor-typing-dark "c:500,330 k:ctrl+End k:Return t:Hello,Quill." --dark "$doc"
shot editor-selection-light "d:250,357 s:560,440" --light "$doc"
shot editor-drag-select-dark "g:215,332,290,332" --dark "$doc"
shot editor-triple-click-light "T:300,499" --light "$doc"
shot editor-format-bar-bold-light "c:318,357" --light "$doc"
shot editor-format-bar-heading-light "c:500,295" --light "$doc"
shot editor-format-after-ctrl-b-light "c:250,357 k:ctrl+a k:ctrl+i" --light "$doc"
shot editor-preedit-light "c:260,423" --light --preedit "tsuk" "$doc"
shot editor-refused-character-light "c:260,423" --light --commit "$(printf 'caf\xc3\xa9 \xe4\xb8\xad')" "$doc"
shot editor-unsaved-alert-dark "c:500,330 k:ctrl+End t:Edited c:971,28" --dark "$doc"
echo "chrome shots: ok ($out)"
