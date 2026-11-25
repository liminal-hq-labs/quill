#!/usr/bin/env bash
# Checkpoint A's static checks on W32SPIKE.EXE: the worker module reaches only kernel32 and only what worker.allow lists, every import of the
# EXE is on Quill32's win9x/tools/imports.allow or is one of the few new ones listed in new-imports.allow, and no instruction is newer than a
# Pentium (win9x/tools/scan-pentium.sh).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/w32-worker/tools/check.sh [W32SPIKE.EXE]
set -euo pipefail
cd "$(dirname "$0")/.."
exe="$(realpath "${1:-dist/W32SPIKE.EXE}")"
command -v llvm-objdump > /dev/null || { echo "check: llvm-objdump not found" >&2; exit 1; }
fail=0
norm() { grep -v '^#' | grep -v '^$' | tr 'A-Z' 'a-z' | sort -u; }

# 1. The worker module's own import block: every `fn Name(` in its extern block must be on worker.allow, and the block must be kernel32.
blocks="$(grep -c 'name = "' src/worker.rs)"
[ "$blocks" = 1 ] && grep -q 'name = "kernel32"' src/worker.rs || { echo "check: worker.rs must have exactly one import block, for kernel32" >&2; fail=1; }
declared="$(awk '/^extern "system" \{/ {on=1; next} on && /^\}/ {on=0} on && /^\tfn / {sub(/^\tfn /, ""); sub(/\(.*/, ""); print "kernel32.dll!" $0}' src/worker.rs | norm)"
allow="$(norm < worker.allow)"
while IFS= read -r f; do
	grep -qxF "$f" <<< "$allow" || { echo "check: worker.rs imports $f, which is not on worker.allow" >&2; fail=1; }
done <<< "$declared"
echo "check: worker.rs declares $(wc -l <<< "$declared") imports, all kernel32"

# 2. The EXE's imports against Quill32's list plus the new ones.
got="$(llvm-objdump -p "$exe" | awk '/DLL Name:/ {dll=tolower($3)} /^ +[0-9]+ +[A-Za-z_]/ {print dll "!" $2}' | norm)"
base="$(norm < ../../win9x/tools/imports.allow)"
new="$(norm < new-imports.allow)"
while IFS= read -r imp; do
	if grep -qxF "$imp" <<< "$base"; then continue; fi
	if grep -qxF "$imp" <<< "$new"; then echo "check: new on Quill32's list: $imp"; continue; fi
	echo "check: not on any allowlist: $imp" >&2
	fail=1
done <<< "$got"
case "$got" in *w) echo "check: a wide-character entry point" >&2; fail=1 ;; esac
echo "check: $(wc -l <<< "$got") imports in $exe"

# 3. Pentium.
../../win9x/tools/scan-pentium.sh "$exe" || fail=1
[ "$fail" = 0 ] || exit 1
echo "check: ok"
