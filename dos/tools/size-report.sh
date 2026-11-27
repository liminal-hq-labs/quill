#!/bin/bash
# Reports where QUILLDOS.EXE's bytes go: the total against the budget, the bytes of each crate and the largest symbols, from the ELF.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/size-report.sh [elf] [top]
#
#   [elf]  the linked ELF before elf2go32 (default: quill-dos under CARGO_TARGET_DIR, or dos/target, as dos/build.sh leaves it)
#   [top]  how many symbols to list (default 25)
#
# A symbol belongs to the first crate its demangled name mentions, so a generic function instantiated for quill-dos's types is
# charged to the crate that defines it. The totals count text and data symbols only; BSS (the stacks, the heap pages) is not in the
# executable. Compare two builds by running the report on each and diffing the "by crate" tables. Needs nm, size and python3.
#
# The budgets (the executable and each module) are in installer/budgets.toml, which the size gate (installer/tools/size-gate.py)
# reads too; the gate also measures the packed sizes on the disks.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ELF="${1:-${CARGO_TARGET_DIR:-$HERE/target}/dos-elf/release/quill-dos}"
TOP="${2:-25}"
GATE="$HERE/../installer/tools/size-gate.py"
BUDGET="$(python3 -I "$GATE" get executable.QUILLDOS.budget)"
[ -f "$ELF" ] || { echo "size-report: $ELF not found; run dos/build.sh" >&2; exit 1; }

echo "== sections ($ELF)"
size "$ELF"
exe="$HERE/dist/QUILLDOS.EXE"
if [ -f "$exe" ]; then
	bytes="$(wc -c < "$exe")"
	echo "QUILLDOS.EXE: $bytes bytes of $BUDGET ($((BUDGET - bytes)) left)"
fi

echo
echo "== modules (dos/dist/*.DMD against installer/budgets.toml)"
modules=0
for module in "$HERE"/dist/*.DMD; do
	[ -f "$module" ] || continue
	modules=1
	name="$(basename "$module")"
	bytes="$(wc -c < "$module")"
	limit="$(python3 -I "$GATE" modules | awk -v n="$name" '$1 == n { print $2 }')"
	if [ -n "$limit" ]; then
		if [ "$bytes" -le "$limit" ]; then echo "$name: $bytes bytes of $limit ($((limit - bytes)) left)"; else echo "$name: $bytes bytes of $limit (OVER by $((bytes - limit)))"; fi
	else
		echo "$name: $bytes bytes, NO BUDGET in installer/budgets.toml"
	fi
done
[ "$modules" = 1 ] || echo "no modules"

echo
echo "== by crate (bytes of text and data symbols)"
nm -C -S --size-sort "$ELF" | awk '
	$3 ~ /^[tTdDrR]$/ {
		size = strtonum("0x" $2)
		name = $0
		sub(/^[^ ]+ [^ ]+ [^ ] /, "", name)
		while (substr(name, 1, 1) == "<") name = substr(name, 2)
		crate = name
		if (match(crate, /^[A-Za-z_][A-Za-z0-9_]*::/)) crate = substr(crate, 1, RLENGTH - 2)
		else crate = "(other)"
		sum[crate] += size
		total += size
	}
	END {
		for (c in sum) printf "%9d  %s\n", sum[c], c | "sort -rn"
		close("sort -rn")
		printf "%9d  total\n", total
	}'

echo
echo "== largest symbols"
nm -C -S --size-sort "$ELF" | awk '$3 ~ /^[tTdDrR]$/' | tail -n "$TOP" | tac | awk '{
	size = strtonum("0x" $2)
	name = $0
	sub(/^[^ ]+ [^ ]+ [^ ] /, "", name)
	printf "%9d  %s\n", size, substr(name, 1, 150)
}'
