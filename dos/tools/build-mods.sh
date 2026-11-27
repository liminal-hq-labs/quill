#!/bin/bash
# Builds the DOS modules and packs each into a `.DMD` file: dmdpack (host), the module crates in dos/mods for the DOS target, then
# `dmdpack pack`, with a second link of the same sources at another base and `dmdpack cross` to prove every absolute word is listed.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/build-mods.sh             the shipped modules into dos/dist/ (none yet: prints that and builds nothing)
#        dos/tools/build-mods.sh --test DIR  the loader's test modules into DIR (never dos/dist/, which the installer stages):
#                                            QUILLTST.DMD linked at 0x400000, QUILLTS8.DMD at 0x800000, QUILLTSC.DMD at 0xC00000 (the
#                                            same sources, which `dmdpack cross` compares), and QUILLBIG.DMD (the same with a megabyte of .bss)
#
# Each linked ELF is kept in $CARGO_TARGET_DIR/mods (dos/target/mods) beside the build for debugging: `gdb`'s `add-symbol-file X.elf`
# with the load address the loader logs, minus the link base. QUILL_BUILD_ID (a decimal number) is the build id the modules say they are
# from; unset, it is the first eight hex digits of HEAD with the top bit set when the tree has uncommitted changes (`quill_modabi::build_id`).
# The paths of the checkout, the toolchain and the cargo home are mapped to fixed names so no build host's folder is in a module.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-$HERE/target}"

TEST_DIR=""
case "${1:-}" in
	"") ;;
	--test) TEST_DIR="${2:?--test needs a folder}" ;;
	*) echo "usage: dos/tools/build-mods.sh [--test DIR]" >&2; exit 2 ;;
esac

# The shipped modules: name:file pairs. None until WP-MOD-3 (QUILLPIC.DMD) and WP-MOD-4 (QUILLDLG.DMD) add theirs.
SHIPPED=()
if [ -z "$TEST_DIR" ] && [ ${#SHIPPED[@]} -eq 0 ]; then
	echo "build-mods: no modules to build yet"
	exit 0
fi

if [ -z "${QUILL_BUILD_ID:-}" ]; then
	commit="$(git -C "$ROOT" rev-parse --short=8 HEAD)"
	id=$((16#$commit & 0x7FFFFFFF))
	if ! git -C "$ROOT" diff --quiet HEAD -- . 2> /dev/null; then id=$((id | 0x80000000)); fi
	QUILL_BUILD_ID="$id"
fi
export QUILL_BUILD_ID

(cd "$HERE/module" && cargo build --release --locked --quiet -p dmdpack --target-dir "$TARGET_DIR/dmdpack")
DMDPACK="$TARGET_DIR/dmdpack/x86_64-unknown-linux-gnu/release/dmdpack"
[ -x "$DMDPACK" ] || { echo "build-mods: $DMDPACK was not built" >&2; exit 1; }

SYSROOT="$(rustc --print sysroot)"
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"
US=$'\x1f'
FLAGS=(-Clink-arg=-nostdlib -Clink-arg=--emit-relocs -Clink-arg=-zmax-page-size=0x1000 -Zlocation-detail=none -Zfmt-debug=none
	"--remap-path-prefix=$CARGO_HOME_DIR=/cargo" "--remap-path-prefix=$SYSROOT=/rust" "--remap-path-prefix=$ROOT=/quill")
CARGO_ENCODED_RUSTFLAGS="$(printf "%s$US" "${FLAGS[@]}")"
CARGO_ENCODED_RUSTFLAGS="${CARGO_ENCODED_RUSTFLAGS%"$US"}"
export CARGO_ENCODED_RUSTFLAGS

mkdir -p "$TARGET_DIR/mods"

# build NAME FILE BASE [FEATURES]: link the crate NAME at BASE, keep the ELF, pack it as FILE into $OUT.
build() {
	local name="$1" file="$2" base="$3" features="${4:-}" elf
	(cd "$HERE/mods" && DESK_LINK_BASE="$base" RUSTC_BOOTSTRAP=1 cargo build --release --locked --quiet \
		--target-dir "$TARGET_DIR/modules" -p "$name" ${features:+--features "$features"})
	elf="$TARGET_DIR/mods/${file%.DMD}.elf"
	cp "$TARGET_DIR/modules/dos-elf/release/$name" "$elf"
	"$DMDPACK" pack --link-base "$base" "$elf" "$OUT/$file"
}

if [ -n "$TEST_DIR" ]; then
	OUT="$TEST_DIR"
	mkdir -p "$OUT"
	build quilltst QUILLTST.DMD 0x400000
	build quilltst QUILLTS8.DMD 0x800000
	build quilltst QUILLTSC.DMD 0xC00000
	build quilltst QUILLBIG.DMD 0x400000 big
	"$DMDPACK" cross "$OUT/QUILLTST.DMD" "$OUT/QUILLTS8.DMD"
	"$DMDPACK" cross "$OUT/QUILLTST.DMD" "$OUT/QUILLTSC.DMD"
	echo "build-mods: build id $QUILL_BUILD_ID"
	ls -l "$OUT"/*.DMD
fi
