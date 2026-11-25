#!/usr/bin/env bash
# Screenshot of the build VM's own screen as a PNG (QEMU monitor screendump). Usage: shot.sh OUT.png
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
W=${VMDIR:-$HOME/.cache/reactos/web/p2}
HERE=$(dirname "$0")
echo "screendump $W/s.ppm" | VMDIR="$W" python3 -I "$HERE/mon.py" > /dev/null
sleep 0.5
magick "$W/s.ppm" "$1"
