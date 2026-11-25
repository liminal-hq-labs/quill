#!/usr/bin/env bash
# Win+R, type the command, Enter, in the build VM.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
W=${VMDIR:-$HOME/.cache/reactos/web/p2}
HERE=$(dirname "$0")
echo "sendkey meta_l-r" | VMDIR="$W" python3 -I "$HERE/mon.py" > /dev/null
sleep 3.5
"$HERE/typ.sh" "$1"
sleep 0.5
echo "sendkey ret" | VMDIR="$W" python3 -I "$HERE/mon.py" > /dev/null
