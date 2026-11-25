#!/usr/bin/env bash
# Builds the Win16 programs: Rust to an ELF with rust-lld, then tools/ne_pack.py to an NE executable.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: win16/build.sh [--regen]
#
# Writes out/QUILL.EXE and out/QUILL.HLP (the help file it opens), out/PROBE.EXE, out/ICONTEST.EXE (the 16-colour Windows 3.x icon) and out/ICONT256.EXE (the same program with the 256-colour one). `--regen` rebuilds src/imports.rs from tools/imports.tbl first. Set CARGO_TARGET_DIR to
# keep the build output outside the tree and OUT_DIR to move the executables.

cd "$(dirname "${BASH_SOURCE[0]}")"
export RUSTC_BOOTSTRAP=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p "$CARGO_TARGET_DIR"

if [ "${1:-}" = "--regen" ]; then
	python3 tools/gen_imports.py tools/imports.tbl src/imports.rs
	python3 tools/gen_imports.py --count tools/imports.tbl src/imports_count.rs
fi

# W16_FEATURES=bench builds the paint benchmark (QUILLB.EXE) and nothing else.
bench=0
case ",$(printf %s "${W16_FEATURES:-}" | tr " " ","),"  in *,bench,*) bench=1 ;; esac

# One cargo build links every program; the packer then places each program's code in segments (examples/<name>/segments.json, if any).
if ! cargo build --release ${W16_FEATURES:+--features $W16_FEATURES} 2>"$CARGO_TARGET_DIR/build.log"; then
	cat "$CARGO_TARGET_DIR/build.log" >&2
	exit 1
fi

out="${OUT_DIR:-out}"
mkdir -p "$out"
elf_dir="$CARGO_TARGET_DIR/win16/release"
for prog in probe icontest segtest quill stackprobe helptest; do
	[ "$bench" = 0 ] || [ "$prog" = quill ] || continue
	RES_ARGS=()
	[ -f "examples/$prog/resources.json" ] && RES_ARGS=(--resources "examples/$prog/resources.json")
	[ -f "examples/$prog/segments.json" ] && RES_ARGS+=(--segments "examples/$prog/segments.json")
	PACK_ARGS=()
	if [ "$prog" = quill ]; then
		# The sizes are constants in the program, which reports its stack use against the first.
		stack="$(sed -n 's/^const STACK_SIZE: u32 = \(0x[0-9A-Fa-f]*\);.*/\1/p' examples/quill/main.rs)"
		heap="$(sed -n 's/^const HEAP_SIZE: u32 = \(0x[0-9A-Fa-f]*\);.*/\1/p' examples/quill/main.rs)"
		# The benchmark's counters and log text take some of the data segment's room, so its stack is 2 KB shorter (it never runs deep).
		[ "$bench" = 0 ] || stack="$(printf '0x%X' $((stack - 0x800)))"
		PACK_ARGS=(--stack "$stack" --heap "$heap")
	fi
	upper="$(echo "$prog" | tr '[:lower:]' '[:upper:]')"
	[ "$bench" = 0 ] || upper="${upper}B"
	python3 tools/ne_pack.py --module "$upper" --description "Quill Win16 $prog" --map "$out/$upper.MAP" \
		${PACK_ARGS[@]+"${PACK_ARGS[@]}"} ${RES_ARGS[@]+"${RES_ARGS[@]}"} "$elf_dir/$prog" "$out/$upper.EXE"
	python3 tools/check_segments.py "$out/$upper.EXE"
	python3 tools/check_cpu.py "$out/$upper.EXE"
	# The icon, the toolbar bitmap and the five dialogs.
	[ "$prog" != quill ] || python3 tools/check_resources.py "$out/$upper.EXE" 14:1 2:2 5:1001 5:1002 5:1003 5:1004 5:1005
	echo "built: $out/$upper.EXE ($(wc -c <"$out/$upper.EXE") bytes)"
done
[ "$bench" = 0 ] || exit 0
# The help file goes beside QUILL.EXE, where Help > Contents looks for it (the program's own directory, from GetModuleFileName).
out_abs="$(cd "$out" && pwd)"
(cd .. && env -u CARGO_TARGET_DIR -u RUSTC_BOOTSTRAP cargo run -q --release --locked -p quill-help --bin hlpc -- assets/help/quill.qhm "$out_abs/QUILL.HLP")
python3 tools/ne_pack.py --module ICONT256 --description "Quill Win16 icontest (256 colours)" \
	--resources examples/icontest/resources-256.json "$elf_dir/icontest" "$out/ICONT256.EXE"
echo "built: $out/ICONT256.EXE ($(wc -c <"$out/ICONT256.EXE") bytes)"
