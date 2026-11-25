#!/usr/bin/env bash
# Build the original Afterglow visual style: draw the bitmaps and write the INI and resource script (gen.py), compile the resources
# with llvm-rc and link a resource-only x86 DLL with lld-link (/Brepro: no timestamp, so the same sources give the same bytes).
# No other visual style is read or fetched.
# Usage: build.sh [WORKDIR] [--dist]   (default WORKDIR ~/.cache/reactos/web/afterglow-original; the result is WORKDIR/build/afterglow.msstyles;
#        --dist also copies it to dist/ with its SHA-256, and the pointers to cursors/)
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -eu
here=$(cd "$(dirname "$0")" && pwd)
W=${1:-$HOME/.cache/reactos/web/afterglow-original}
B=$W/build
rm -rf "$B"
mkdir -p "$B"
python3 "$here/gen.py" "$B" --preview "$B/contact-sheet.png"
python3 "$here/cursors/gen-cursors.py" "$B/cursors" --sheet "$B/cursors-sheet.png"
(
	cd "$B"
	llvm-rc /FO afterglow.res afterglow.rc
	lld-link /dll /noentry /machine:x86 /nodefaultlib /Brepro /out:afterglow.msstyles afterglow.res
)
ls -l "$B/afterglow.msstyles"
sha256sum "$B/afterglow.msstyles"
if [ "${2:-}" = --dist ]; then
	cp "$B/afterglow.msstyles" "$here/dist/afterglow.msstyles"
	(cd "$here/dist" && sha256sum afterglow.msstyles > afterglow.msstyles.sha256)
	for f in "$B"/cursors/afterglow-*.cur "$B"/cursors/afterglow-*.ani; do cp "$f" "$here/cursors/"; done
fi
