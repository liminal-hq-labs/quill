#!/usr/bin/env bash
# Fails when the executable imports a function that is not on the Windows 95 allowlist, or a wide-character (`...W`) entry point.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/check-imports.sh [EXE [ALLOWLIST]]      (QUILL32.EXE and tools/imports.allow by default; HOSTLINK.EXE has tools/hostlink-imports.allow)
#
# `tools/imports.allow` lists `dll!Function` per line: only functions that Windows 95's KERNEL32, USER32, GDI32 and COMDLG32 export
# (the list is checked by hand against the Win32 reference's "Windows 95" column, not read from a Windows 95 image).
set -euo pipefail
cd "$(dirname "$0")/.."
exe="${1:-dist/QUILL32.EXE}"
allowfile="${2:-tools/imports.allow}"
command -v llvm-objdump > /dev/null || { echo "check-imports: llvm-objdump not found; skipping"; exit 0; }
got="$(llvm-objdump -p "$exe" | awk '/DLL Name:/ {dll=tolower($3)} /^ +[0-9]+ +[A-Za-z_]/ {print dll "!" $2}' | sort -u)"
allow="$(grep -v '^#' "$allowfile" | grep -v '^$' | tr 'A-Z' 'a-z' | sort -u)"
fail=0
while IFS= read -r imp; do
	[ -n "$imp" ] || continue
	lower="$(echo "$imp" | tr 'A-Z' 'a-z')"
	if ! grep -qxF "$lower" <<< "$allow"; then
		echo "check-imports: not on the Windows 95 allowlist: $imp" >&2
		fail=1
	fi
	case "$imp" in *W) echo "check-imports: wide-character entry point: $imp" >&2; fail=1 ;; esac
done <<< "$got"
[ "$fail" = 0 ] || exit 1
echo "check-imports: ok: $(echo "$got" | wc -l) imports, all on the allowlist"
