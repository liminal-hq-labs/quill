#!/usr/bin/env bash
# Builds the Win16 programs: Rust to an ELF with rust-lld, then tools/ne_pack.py to an NE executable.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: win16/build.sh [--regen]
#
# Writes out/PROBE.EXE, out/ICONTEST.EXE (the 16-colour Windows 3.x icon) and out/ICONT256.EXE (the same program with the 256-colour one). `--regen` rebuilds src/imports.rs from tools/imports.tbl first. Set CARGO_TARGET_DIR to
# keep the build output outside the tree and OUT_DIR to move the executables.

cd "$(dirname "${BASH_SOURCE[0]}")"
export RUSTC_BOOTSTRAP=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p "$CARGO_TARGET_DIR"

if [ "${1:-}" = "--regen" ]; then
	python3 tools/gen_imports.py tools/imports.tbl src/imports.rs
fi

# Each program is linked with its own script: `link.ld` (one code segment) or one generated from its segments.toml, and with
# --emit-relocs when it has several segments, so the packer can see the calls that cross them.
out="${OUT_DIR:-out}"
mkdir -p "$out"
elf_dir="$CARGO_TARGET_DIR/win16/release"
build_prog() {
	local prog="$1" script="$PWD/link.ld" extra=()
	if [ -f "examples/$prog/segments.toml" ]; then
		script="$CARGO_TARGET_DIR/$prog.ld"
		python3 tools/gen_link.py "examples/$prog/segments.toml" "$script"
		extra=(-C link-arg=--emit-relocs)
	fi
	# The linker script's checksum goes in as a cfg that nothing reads, so cargo links again when the script changes.
	local sum
	sum="$(cksum <"$script" | cut -d' ' -f1)"
	if ! cargo rustc --release --bin "$prog" -- -C "link-arg=-T$script" --cfg "w16_ld=\"$sum\"" --check-cfg 'cfg(w16_ld, values(any()))' ${extra[@]+"${extra[@]}"} 2>"$CARGO_TARGET_DIR/build.log"; then
		cat "$CARGO_TARGET_DIR/build.log" >&2
		exit 1
	fi
}
for prog in probe icontest segtest; do
	build_prog "$prog"
	RES_ARGS=()
	[ -f "examples/$prog/resources.json" ] && RES_ARGS=(--resources "examples/$prog/resources.json")
	python3 tools/check_branches.py "$elf_dir/$prog"
	upper="$(echo "$prog" | tr '[:lower:]' '[:upper:]')"
	BUDGET_ARGS=()
	if [ -f "examples/$prog/segments.toml" ]; then
		while read -r b; do BUDGET_ARGS+=(--budget "$b"); done < <(python3 -I tools/segment_budgets.py "examples/$prog/segments.toml")
	fi
	python3 tools/ne_pack.py --module "$upper" --description "Quill Win16 $prog" --map "$out/$upper.map" \
		${BUDGET_ARGS[@]+"${BUDGET_ARGS[@]}"} ${RES_ARGS[@]+"${RES_ARGS[@]}"} "$elf_dir/$prog" "$out/$upper.EXE"
	python3 tools/check_segments.py "$out/$upper.EXE"
	echo "built: $out/$upper.EXE ($(wc -c <"$out/$upper.EXE") bytes)"
done
python3 tools/ne_pack.py --module ICONT256 --description "Quill Win16 icontest (256 colours)" \
	--resources examples/icontest/resources-256.json "$elf_dir/icontest" "$out/ICONT256.EXE"
echo "built: $out/ICONT256.EXE ($(wc -c <"$out/ICONT256.EXE") bytes)"
