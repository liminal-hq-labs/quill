#!/usr/bin/env bash
# Fails when the executable holds an instruction a Pentium lacks: MMX, SSE and later vector code, CMOV, or the newer bit tricks.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/scan-pentium.sh [QUILL32.EXE]
#
# The prebuilt Rust `core` assumes SSE2 (D33), so the build compiles `core` from source for a Pentium; this is the check that nothing
# else slipped in, such as a prebuilt object. Needs llvm-objdump.
set -euo pipefail
cd "$(dirname "$0")/.."
exe="${1:-dist/QUILL32.EXE}"
command -v llvm-objdump > /dev/null || { echo "scan-pentium: llvm-objdump not found; skipping"; exit 0; }
# A vector register, a conditional move, or one of the instructions Pentium-class chips lack.
bad="$(llvm-objdump -d --no-show-raw-insn "$exe" | grep -Ei '(xmm|ymm|zmm)[0-9]|[[:space:]]%?mm[0-7]([^0-9a-z]|$)|[[:space:]](cmov[a-z]+|emms|movbe|popcnt|lzcnt|tzcnt|prefetch[a-z0-9]*|[ls]fence|mfence)[[:space:]]' || true)"
if [ -n "$bad" ]; then
	echo "scan-pentium: instructions a Pentium lacks:" >&2
	echo "$bad" | head -20 >&2
	exit 1
fi
echo "scan-pentium: ok: $exe has no MMX, SSE, CMOV or later instructions"
