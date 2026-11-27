#!/usr/bin/env bash
# Makes the test disk for the host link's end-to-end check: a private sparse copy of the QAGRR image with HOSTLINK.EXE on C:\Quill\ and in the
# All Users Startup folder, so the helper starts at logon. A test-only copy mechanism: the product disk gets the helper from qagrr.toml's
# [[files]] entry and starts it from the Run key (quill-run.reg), neither of which runs here. The source image is only read.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# HLCLIP.EXE (the test-only RTF copy program), when it sits beside HOSTLINK.EXE, goes on C:\Quill\ too.
#
# usage: harness/host-link-disk.sh HOSTLINK.EXE OUT.img [SOURCE.img]      (SOURCE defaults to ~/.cache/quill-reactos-liminal-work/images/qagrr.img)
set -euo pipefail
exe=${1:?usage: host-link-disk.sh HOSTLINK.EXE OUT.img [SOURCE.img]}
out=${2:?usage: host-link-disk.sh HOSTLINK.EXE OUT.img [SOURCE.img]}
source_image=${3:-$HOME/.cache/quill-reactos-liminal-work/images/qagrr.img}
command -v mcopy > /dev/null || { echo "host-link-disk: mtools is needed" >&2; exit 1; }
free_kb=$(df --output=avail -k "$(dirname "$out")" | tail -1)
[ "$free_kb" -gt 25000000 ] || { echo "host-link-disk: under 25 GB free, stopping" >&2; exit 1; }
# KEEP_IMAGE=1 puts a newer helper into the image an earlier run made, without copying the 4 GB again.
[ "${KEEP_IMAGE:-}" = 1 ] && [ -f "$out" ] || cp --sparse=always "$source_image" "$out"
# The partition starts at sector 2048.
part="$out@@1048576"
mcopy -o -i "$part" "$exe" ::/Quill/HOSTLINK.EXE
mcopy -o -i "$part" "$exe" "::/Documents and Settings/All Users/Start Menu/Programs/Startup/HOSTLINK.EXE"
if [ -f "$(dirname "$exe")/HLCLIP.EXE" ]; then mcopy -o -i "$part" "$(dirname "$exe")/HLCLIP.EXE" ::/Quill/HLCLIP.EXE; fi
mdir -i "$part" ::/Quill | tail -5
echo "host-link-disk: $out"
