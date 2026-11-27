#!/usr/bin/env bash
# Takes the screenshots of the GTK client's dialogs (each in light and dark at scale 1 and 2), the File menu opened with Alt+F, the
# mnemonics shown, and the real Open and Save As choosers under a headless Weston, and writes them to $OUT.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: dialog-shots.sh [OUT_DIR]
#
# It starts its own Xvfb on a free display (and, for the choosers, its own headless Weston in a private runtime folder with a private
# session bus) and stops only those processes and the clients it started. Keys go to that display with xdotool; nothing touches the
# desktop. ONLY=REGEX takes just the shots whose names match; WESTON=0 leaves the choosers out. Needs Xvfb, xdotool, ImageMagick and,
# for the choosers, weston, weston-screenshooter and dbus-run-session. The Spelling dialog reads the dictionary in tests/docs.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="${QUILL_BIN:-$here/target/debug/quill}"
docs="$here/../../tests/docs"
doc="$docs/letter.rtf"
out="${1:-${OUT:-$(mktemp -d)}}"
mkdir -p "$out"
work="$(mktemp -d)"

n=90
while [ -e "/tmp/.X11-unix/X$n" ] || [ -e "/tmp/.X$n-lock" ]; do n=$((n + 1)); done
Xvfb ":$n" -screen 0 2400x1700x24 >"$out/xvfb.log" 2>&1 &
xvfb_pid=$!
weston_pid=""
cleanup() {
	[ -z "$weston_pid" ] || kill "$weston_pid" 2>/dev/null || true
	kill "$xvfb_pid" 2>/dev/null || true
	# The private portal that a chooser starts may still hold its mount for a moment.
	sleep 1
	rm -rf "$work" 2>/dev/null || true
}
trap cleanup EXIT
sleep 1

# shot NAME SCALE STEPS ARGS...: starts the client at GDK_SCALE=SCALE, plays the steps (k:KEYS a key chord, t:TEXT typed text, kd:KEY
# and ku:KEY a key held and let go, c:X,Y a click in logical pixels), then captures the window.
shot() {
	local name="$1" scale="$2" steps="$3" t xy
	shift 3
	[ -z "${ONLY:-}" ] || [[ "$name" =~ $ONLY ]] || return 0
	local env=(-u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS QUILL_NO_BLINK=1 GTK_USE_PORTAL=0 "DISPLAY=:$n" GDK_BACKEND=x11 "GDK_SCALE=$scale"
		GSK_RENDERER=cairo "XDG_CONFIG_HOME=$work/config" "XDG_DATA_HOME=$work/config")
	rm -rf "$work/config"
	(cd "$docs" && env "${env[@]}" "$bin" --new-instance --vendored-fonts --no-settings --zoom 100 --size 1000x700 --quit-after 25000 "$@" \
		>"$out/$name.log" 2>&1) &
	local app=$!
	sleep 3
	for t in $steps; do
		case "$t" in
		c:*)
			xy="${t#c:}"
			DISPLAY=":$n" xdotool mousemove $((${xy%,*} * scale)) $((${xy#*,} * scale)) click 1
			;;
		k:*) DISPLAY=":$n" xdotool key "${t#k:}" ;;
		kd:*) DISPLAY=":$n" xdotool keydown "${t#kd:}" ;;
		ku:*) DISPLAY=":$n" xdotool keyup "${t#ku:}" ;;
		t:*) DISPLAY=":$n" xdotool type --delay 80 "${t#t:}" ;;
		esac
		sleep 0.8
	done
	DISPLAY=":$n" import -window root "$work/full.png"
	magick "$work/full.png" -crop "$((1020 * scale))x$((720 * scale))+0+0" +repage "$out/$name.png"
	kill "$app" 2>/dev/null || true
	wait "$app" 2>/dev/null || true
	# A frame taken before the window mapped is a blank one: take it again.
	if [ "$(stat -c %s "$out/$name.png")" -lt 5000 ] && [ -z "${RETRY:-}" ]; then
		RETRY=1 shot "$name" "$scale" "$steps" "$@"
		return
	fi
	echo "shot $name"
}

printf '%s' '{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}\f0\fs24 Hello wrold, this is a tset of the speling checker.\par}' >"$work/typos.rtf"

# Scale 1 first: a smaller window leaves a bigger one's pixels behind it on a bare X server.
extra() {
	# The File menu opened by Alt+F, with the mnemonics underlined, and the mnemonics of two dialogs with Alt held.
	shot "menu-file-alt-f-light" 1 "kd:alt k:f ku:alt" --light "$doc"
	shot "menu-file-alt-f-dark" 1 "kd:alt k:f ku:alt" --dark "$doc"
	shot "dialog-mnemonics-light" 1 "k:alt+n kd:alt" --light --dialog find "$doc"
	shot "dialog-mnemonics-dark" 1 "k:alt+n kd:alt" --dark --dialog find "$doc"
	shot "dialog-fonts-mnemonics-light" 1 "k:alt+s kd:alt" --light --dialog fonts "$doc"
}
for scale in 1 2; do
	for scheme in light dark; do
		s="$scheme-${scale}x"
		shot "dialog-find-$s" "$scale" "" "--$scheme" --dialog find "$doc"
		shot "dialog-replace-$s" "$scale" "" "--$scheme" --dialog replace "$doc"
		shot "dialog-fonts-$s" "$scale" "" "--$scheme" --dialog fonts "$doc"
		shot "dialog-tabs-$s" "$scale" "" "--$scheme" --dialog tabs "$doc"
		shot "dialog-spelling-$s" "$scale" "" "--$scheme" --dialog spelling "$work/typos.rtf"
		shot "dialog-about-$s" "$scale" "" "--$scheme" --dialog about "$doc"
		# The unsaved-changes question: type, then Exit (Ctrl+Q) asks.
		shot "dialog-question-$s" "$scale" "c:500,330 t:Edited k:ctrl+q" "--$scheme" "$doc"
	done
	[ "$scale" = 1 ] && extra
done

[ "${WESTON:-1}" = 0 ] && exit 0

# The real choosers, under a headless Weston with a private runtime folder and session bus. They are shown, not answered: a headless
# Weston has no input device. HOME is a scratch folder so no real home folder shows.
runtime="$work/run"
mkdir -p "$runtime" "$work/home/Documents"
chmod 700 "$runtime"
cp "$doc" "$work/home/Documents/Letter.rtf"
cp "$docs/plain.qld" "$work/home/Documents/Notes.qld"
cp "$docs/pic.png" "$work/home/Documents/Picture.png"
XDG_RUNTIME_DIR="$runtime" weston --debug --backend=headless --renderer=pixman --width=1100 --height=760 --socket=quill-shots \
	--no-config >"$out/weston.log" 2>&1 &
weston_pid=$!
sleep 2

chooser() {
	local name="$1" flag="$2" scheme="$3"
	[ -z "${ONLY:-}" ] || [[ "$name" =~ $ONLY ]] || return 0
	(
		cd "$work/home/Documents"
		env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=quill-shots GDK_BACKEND=wayland GTK_USE_PORTAL=0 ADW_DISABLE_PORTAL=1 GIO_USE_VFS=local GSK_RENDERER=cairo \
			HOME="$work/home" XDG_CONFIG_HOME="$work/home/.config" XDG_DATA_HOME="$work/home/.local/share" QUILL_NO_BLINK=1 \
			dbus-run-session -- "$bin" --new-instance --vendored-fonts --no-settings --size 1000x700 --quit-after 14000 \
			"--$scheme" "$flag" "$work/home/Documents/Letter.rtf" >"$out/$name.log" 2>&1
	) &
	local app=$!
	sleep 8
	(cd "$work" && XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=quill-shots weston-screenshooter >/dev/null 2>&1) || true
	local shot
	shot="$(ls -t "$work"/wayland-screenshot-*.png 2>/dev/null | head -1 || true)"
	if [ -n "$shot" ]; then
		magick "$shot" +repage "$out/$name.png"
		rm -f "$shot"
		echo "shot $name"
	else
		echo "no frame for $name" >&2
	fi
	kill "$app" 2>/dev/null || true
	wait "$app" 2>/dev/null || true
}
chooser dialog-open-weston --open-dialog light
chooser dialog-save-as-weston --save-as-dialog light
