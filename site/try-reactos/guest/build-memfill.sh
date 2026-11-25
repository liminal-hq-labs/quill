#!/usr/bin/env bash
# Build memfill.exe (32-bit Windows console program, no C runtime) with clang and lld-link. Usage: build-memfill.sh OUT.exe
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -eu
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cat > "$tmp/k32.def" <<'DEF'
LIBRARY kernel32.dll
EXPORTS
GlobalMemoryStatus@4
VirtualAlloc@16
VirtualFree@12
Sleep@4
ExitProcess@4
DEF
llvm-dlltool -k -m i386 -d "$tmp/k32.def" -l "$tmp/kernel32.lib"
clang --target=i686-pc-windows-msvc -O2 -fno-stack-protector -fno-builtin -c "$here/memfill.c" -o "$tmp/memfill.obj"
lld-link /nologo /subsystem:console,4.0 /entry:start /nodefaultlib /fixed /filealign:512 "/out:$1" "$tmp/memfill.obj" "$tmp/kernel32.lib"
