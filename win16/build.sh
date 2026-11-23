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

if ! cargo build --release 2>"$CARGO_TARGET_DIR/build.log"; then
	cat "$CARGO_TARGET_DIR/build.log" >&2
	exit 1
fi

out="${OUT_DIR:-out}"
mkdir -p "$out"
elf_dir="$CARGO_TARGET_DIR/win16/release"
for prog in probe icontest; do
	RES_ARGS=()
	[ -f "examples/$prog/resources.json" ] && RES_ARGS=(--resources "examples/$prog/resources.json")
	upper="$(echo "$prog" | tr '[:lower:]' '[:upper:]')"
	python3 tools/ne_pack.py --module "$upper" --description "Quill Win16 $prog" \
		${RES_ARGS[@]+"${RES_ARGS[@]}"} "$elf_dir/$prog" "$out/$upper.EXE"
	echo "built: $out/$upper.EXE ($(wc -c <"$out/$upper.EXE") bytes)"
done
python3 tools/ne_pack.py --module ICONT256 --description "Quill Win16 icontest (256 colours)" \
	--resources examples/icontest/resources-256.json "$elf_dir/icontest" "$out/ICONT256.EXE"
echo "built: $out/ICONT256.EXE ($(wc -c <"$out/ICONT256.EXE") bytes)"
