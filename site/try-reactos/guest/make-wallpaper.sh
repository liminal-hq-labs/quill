#!/usr/bin/env bash
# Render the Quill wallpaper SVG as a 1024 x 768 24-bit BMP for the ReactOS desktop.
# The 3840 x 2160 art keeps its emblem inside a centre 4:3 crop (x 480 to 3360), so the crop is cut and scaled with no distortion.
# Usage: make-wallpaper.sh quill-wallpaper.svg OUT.bmp
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -eu
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
rsvg-convert -w 3840 -h 2160 "$1" -o "$tmp/full.png"
magick "$tmp/full.png" -crop 2880x2160+480+0 +repage -filter Lanczos -resize 1024x768! -type TrueColor "BMP3:$2"
