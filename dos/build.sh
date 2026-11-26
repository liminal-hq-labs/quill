#!/bin/bash
# Builds QUILLDOS.EXE into dos/dist/ with no C compiler and no C library.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/build.sh            (needs dos/tools/fetch-tools.sh to have run, for CWSDSTUB.EXE)
#
#   dos/quill-dos (Rust, no_std)     --cargo -Zbuild-std, target dos-elf.json, linked by rust-lld with link.ld--> ELF
#   the ELF + CWSDPMI's CWSDSTUB.EXE --dos/tools/elf2go32.py-------------------------------------------------> dist/QUILLDOS.EXE
#
# CWSDSTUB.EXE is a go32 stub with the CWSDPMI DPMI host inside it, so QUILLDOS.EXE needs no CWSDPMI.EXE beside it. Set
# CARGO_TARGET_DIR to keep the build output out of the checkout.
set -euo pipefail
cd "$(dirname "$0")"

VENDOR="$(tools/vendor-dir.sh "$PWD/vendor")"
STUB="$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE"
[ -f "$STUB" ] || { echo "build: $STUB not found; run dos/tools/fetch-tools.sh" >&2; exit 1; }

RUSTC_BOOTSTRAP=1 cargo build --release --locked
TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p dist
tools/elf2go32.py "$STUB" "$TARGET_DIR/dos-elf/release/quill-dos" dist/QUILLDOS.EXE
echo "build: dist/QUILLDOS.EXE ($(wc -c < dist/QUILLDOS.EXE) bytes)"
