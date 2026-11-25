#!/usr/bin/env bash
# Opens a document in the real GTK window under Xvfb, types into it, saves with Ctrl+S and compares the saved file with the bytes a
# headless `Document` saves for the same text.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: smoke-edit.sh [OUT_DIR]
#
# It starts its own Xvfb on a free display and stops only that process and the client it started. The client runs with no session
# bus, no portal and no Wayland display, so nothing reaches the desktop. Needs Xvfb, xdotool and ImageMagick. The text is `LINES` of
# tests/smoke.rs, typed with xdotool.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="${QUILL_BIN:-$here/target/debug/quill}"
out="${1:-${OUT:-$(mktemp -d)}}"
mkdir -p "$out/expected" "$out/window" "$out/cfg"

n=90
while [ -e "/tmp/.X11-unix/X$n" ] || [ -e "/tmp/.X$n-lock" ]; do n=$((n + 1)); done
Xvfb ":$n" -screen 0 1400x1000x24 >"$out/xvfb.log" 2>&1 &
xvfb_pid=$!
app_pid=""
cleanup() {
	[ -z "$app_pid" ] || kill "$app_pid" 2>/dev/null || true
	kill "$xvfb_pid" 2>/dev/null || true
}
trap cleanup EXIT
sleep 1

# The bytes a headless document saves for the same text.
(cd "$here" && QUILL_SMOKE_DIR="$out/expected" cargo test --offline --test smoke -- --ignored >"$out/expected.log" 2>&1)

# The window: a copy of the document, the same text typed with xdotool, Ctrl+S.
cp "$here/../../tests/docs/plain.qld" "$out/window/note.qld"
export DISPLAY=":$n"
env -u DBUS_SESSION_BUS_ADDRESS -u WAYLAND_DISPLAY QUILL_NO_BLINK=1 GDK_BACKEND=x11 GSK_RENDERER=cairo GTK_USE_PORTAL=0 \
	XDG_CONFIG_HOME="$out/cfg" "$bin" --new-instance --vendored-fonts --no-settings --size 1000x700 --quit-after 60000 \
	"$out/window/note.qld" >"$out/app.log" 2>&1 &
app_pid=$!
sleep 3

xdotool mousemove 500 400 click 1
xdotool key ctrl+End
xdotool key Return
xdotool type --delay 60 "Hello, Quill."
xdotool key Return
xdotool type --delay 60 "Caf"
xdotool key eacute
xdotool type --delay 60 " au lait"
sleep 0.5
import -window root "$out/typed.png"
xdotool key ctrl+s
sleep 1.5
import -window root "$out/saved.png"

cmp "$out/window/note.qld" "$out/expected/note.qld"
echo "smoke: ok, the window saved $(stat -c %s "$out/window/note.qld") bytes and they equal the headless document's ($out)"
