#!/usr/bin/env bash
# Checks the GTK client's input in a private Xvfb: the pointer's shape over a sheet and the desk, a drag that goes past the bottom of
# the page (the view must scroll and the selection grow), and key repeat (a held key must type more than one character).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: input-checks.sh [OUT_DIR]
#
# It starts its own Xvfb on a free display and stops only that process and the clients it started. The clients run with no session
# bus, no portal and no Wayland display, so nothing reaches the desktop. Needs Xvfb and xdotool.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="${QUILL_BIN:-$here/target/debug/quill}"
out="${1:-${OUT:-$(mktemp -d)}}"
mkdir -p "$out/cfg"

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

quill() {
	env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS QUILL_NO_BLINK=1 GTK_USE_PORTAL=0 "DISPLAY=:$n" GDK_BACKEND=x11 \
		GSK_RENDERER=cairo "XDG_CONFIG_HOME=$out/cfg" "$bin" --new-instance --vendored-fonts --no-settings --size 1000x700 "$@"
}

# A document of 159 short paragraphs, so a drag can go a long way.
python3 -I - "$out/long.rtf" <<'PY'
import sys
body = "".join("Paragraph %d of a long test document that runs over the page.\\par\n" % i for i in range(1, 160))
open(sys.argv[1], "w").write("{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Times New Roman;}}\\f0\\fs24 " + body + "}")
PY

report="$(quill --self-test-input "$out/long.rtf" 2>/dev/null)"
echo "$report"
grep -q '^cursor over a sheet: text$' <<<"$report" || { echo "input: the pointer is not an I-beam over a sheet" >&2; exit 1; }
grep -q '^cursor over the desk: default$' <<<"$report" || { echo "input: the pointer is not an arrow over the desk" >&2; exit 1; }
read -r scroll from to < <(sed -n 's/.*scroll 0 -> \([0-9]*\), selection Some((\([0-9]*\), \([0-9]*\))).*/\1 \2 \3/p' <<<"$report")
[ "${scroll:-0}" -gt 200 ] || { echo "input: the drag did not scroll the view" >&2; exit 1; }
[ $((to - from)) -gt 1000 ] || { echo "input: the selection did not grow with the scroll" >&2; exit 1; }

# Key repeat: hold a key for a second and a half; the saved file must have grown by more than a few characters.
cp "$here/../../tests/docs/plain.qld" "$out/held.qld"
before="$(stat -c %s "$out/held.qld")"
export DISPLAY=":$n"
quill --quit-after 30000 "$out/held.qld" >"$out/app.log" 2>&1 &
app_pid=$!
sleep 3
xdotool mousemove 500 400 click 1
xdotool key ctrl+End
xdotool keydown a
sleep 1.5
xdotool keyup a
sleep 0.3
xdotool key ctrl+s
sleep 1.5
after="$(stat -c %s "$out/held.qld")"
[ $((after - before)) -gt 10 ] || { echo "input: a held key typed only $((after - before)) bytes" >&2; exit 1; }
echo "input: ok (pointer shape, drag scroll $scroll px and a selection of $((to - from)) characters, a held key added $((after - before)) bytes)"
