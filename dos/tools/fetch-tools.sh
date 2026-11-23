#!/bin/bash
# Downloads the DOS test tools: DOSBox Staging (the headless emulator) and CWSDPMI (the DPMI host and stub), checking SHA-256 sums.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/fetch-tools.sh
#
# The tools go to $DOS_VENDOR (default: dos/vendor, which is not committed): dosbox/, cwsdpmi/ and dl/ for the downloads. The
# downloads are cached and verified on every run, so a corrupt or tampered file is never unpacked. The same URLs and sums as Desk's
# tools/fetch-tools.sh (see PROVENANCE.md). DJGPP itself is not needed: CWSDPMI's own CWSDSTUB.EXE is the stub, and SETUP.EXE then
# carries its DPMI host inside it.
#
# Exits 0 when the tools are present, 1 when they cannot be fetched (the callers treat that as "skip the DOS run").
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR="${DOS_VENDOR:-$HERE/vendor}"

DOSBOX_URL=https://github.com/dosbox-staging/dosbox-staging/releases/download/v0.83.0/dosbox-staging-linux-x86_64-v0.83.0.tar.xz
DOSBOX_SHA256=d3a94f7f1c3e68a47ec88d61145506c7904452adb0c9c5928cb8cfe2331d6c5c
CWSDPMI_URL=https://www.delorie.com/pub/djgpp/current/v2misc/csdpmi7b.zip
CWSDPMI_SHA256=deacda0488e1cdd7c4a9f32fab45662b34c0ed6b2d7d4d13bc07041b62004a8c

if [ -n "$(ls "$VENDOR"/dosbox/*/dosbox 2> /dev/null)" ] && [ -f "$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE" ]; then
	echo "fetch-tools: tools already present in $VENDOR"
	exit 0
fi

mkdir -p "$VENDOR/dl" "$VENDOR/dosbox" "$VENDOR/cwsdpmi"
cd "$VENDOR/dl"

fetch() { # <url> <sha256>: download into the cache if missing, then verify
	local file="${1##*/}"
	[ -f "$file" ] || curl -fsSL -o "$file" "$1" || return 1
	printf '%s  %s\n' "$2" "$file" | sha256sum -c - > /dev/null || { rm -f "$file"; echo "fetch-tools: checksum mismatch for $file" >&2; return 1; }
}

fetch "$DOSBOX_URL" "$DOSBOX_SHA256" || { echo "fetch-tools: could not fetch DOSBox Staging" >&2; exit 1; }
fetch "$CWSDPMI_URL" "$CWSDPMI_SHA256" || { echo "fetch-tools: could not fetch CWSDPMI" >&2; exit 1; }

tar -xJf "${DOSBOX_URL##*/}" -C ../dosbox
python3 -I -c "import sys,zipfile;zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])" "${CWSDPMI_URL##*/}" ../cwsdpmi
echo "fetch-tools: DOSBox Staging and CWSDPMI ready in $VENDOR"
