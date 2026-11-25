#!/usr/bin/env bash
# Draws a document in the GTK client at several scales and checks that each frame holds the page texture pixel for pixel.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: frames.sh [DOCUMENT]
#
# It starts its own Xvfb on a free display and stops only that one. The frames are an X capture at scale 1, an X capture at
# GDK_SCALE=2, and GTK's own render of the window at 1.0, 1.25, 1.5, 1.75 and 2.0 (the only way to see a fractional scale without a
# compositor that offers one). Needs Xvfb, ImageMagick's `import`, and python3 with Pillow and numpy. The pictures stay in $OUT.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="${QUILL_BIN:-$here/target/debug/quill}"
doc="${1:-$here/../../tests/docs/letter.rtf}"
out="${OUT:-$(mktemp -d)}"
mkdir -p "$out"

n=90
while [ -e "/tmp/.X11-unix/X$n" ] || [ -e "/tmp/.X$n-lock" ]; do n=$((n + 1)); done
Xvfb ":$n" -screen 0 1800x1400x24 >"$out/xvfb.log" 2>&1 &
xvfb_pid=$!
trap 'kill "$xvfb_pid" 2>/dev/null || true' EXIT
sleep 1

quill() {
	env -u DBUS_SESSION_BUS_ADDRESS "DISPLAY=:$n" GDK_BACKEND=x11 GSK_RENDERER=cairo \
		"XDG_CONFIG_HOME=$out/config" "$@" \
		"$bin" --new-instance --vendored-fonts --no-settings --zoom 100 --size 900x700 "${args[@]}" "$doc"
}

# check NAME FRAME TEXTURE LOG: compares the frame with the texture at the place the client reported.
check() {
	local name="$1" frame="$2" tex="$3" log="$4" line x y w h
	line="$(grep '^page-texture' "$log")"
	read -r x y w h < <(awk '{print $4, $5, $10, $11}' <<<"$line")
	echo "== $name: $(awk '{print "scale " $13}' <<<"$line")"
	python3 -I "$here/tools/check-frame.py" "$frame" "$tex" "$x" "$y" 0 "$w" "$h"
}

for scale in 1 2; do
	args=(--dump "$out/x$scale.page.png" --quit-after 7000)
	quill "GDK_SCALE=$scale" >"$out/x$scale.log" 2>&1 &
	app=$!
	sleep 3.5
	env "DISPLAY=:$n" import -window root "$out/x$scale.png"
	wait "$app"
	check "X capture, GDK_SCALE=$scale" "$out/x$scale.png" "$out/x$scale.page.png" "$out/x$scale.log"
done

for s in 120 150 180 210 240; do
	args=(--scale-120 "$s" --screenshot "$out/gtk$s.png")
	quill "GDK_SCALE=1" >"$out/gtk$s.log" 2>&1
	check "GTK render at $s/120" "$out/gtk$s.png" "$out/gtk$s.page.png" "$out/gtk$s.log"
done
echo "frames: ok ($out)"
