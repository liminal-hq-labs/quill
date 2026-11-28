#!/usr/bin/env bash
# Builds QUILL32.EXE into win9x/dist/ with no C compiler and no C library: Rust, rust-lld's lld-link and a Pentium target.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/build.sh [--bench]       (set CARGO_TARGET_DIR to keep the build output outside the tree)
#   --bench  builds with the `bench` feature (it counts the GDI and USER calls of the paint and key paths, times the phases and writes a
#            log; see docs/progress.md) into dist/QUILL32-BENCH.EXE, and leaves dist/QUILL32.EXE alone
set -euo pipefail
cd "$(dirname "$0")"
export RUSTC_BOOTSTRAP=1
# The resources: the menu tree from the app, packed into a .res (see tools/res_pack.py).
(cd .. && RUSTC_BOOTSTRAP=0 cargo run -q -p quill-host --bin emit-resources > win9x/resources/quill.json)
python3 -I tools/res_pack.py resources/quill.json resources/quill.res ../assets/icon/QUILL.ICO ../assets/dialogs/dialogs.json
python3 -I tools/test_res_pack.py resources/quill.json resources/quill.res ../assets/dialogs/dialogs.json
if [ "${1:-}" = "--bench" ]; then
	cargo build --release --locked --features bench
	out=dist/QUILL32-BENCH.EXE
else
	cargo build --release --locked
	out=dist/QUILL32.EXE
fi
target="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p dist
cp "$target/win9x/release/quill32.exe" "$out"
echo "build: $out ($(wc -c < "$out") bytes)"
if [ "${1:-}" != "--bench" ]; then
	# HOSTLINK.EXE, the ReactOS demo's guest helper: the same target and link line as QUILL32.EXE, without its resource file (the env
	# variable replaces the config's rustflags, which name quill.res). A budget guard keeps it tiny: 24 KiB.
	flags=(-C link-arg=/NODEFAULTLIB -C link-arg=/ENTRY:start -C link-arg=/SUBSYSTEM:WINDOWS,4.0 -C link-arg=/FIXED -C link-arg=/DYNAMICBASE:NO -C llvm-args=-min-jump-table-entries=1000)
	CARGO_ENCODED_RUSTFLAGS=$(IFS=$'\x1f'; echo "${flags[*]}") cargo build --release --locked -p quill-hostlink-exe
	cp "$target/win9x/release/hostlink.exe" dist/HOSTLINK.EXE
	# HLCLIP.EXE is the same package's test-only program (the end-to-end check's RTF copy); it is never put on a product disk.
	cp "$target/win9x/release/hlclip.exe" dist/HLCLIP.EXE
	size=$(wc -c < dist/HOSTLINK.EXE)
	echo "build: dist/HOSTLINK.EXE ($size bytes)"
	[ "$size" -le 24576 ] || { echo "build: HOSTLINK.EXE is over its 24 KiB budget" >&2; exit 1; }
fi
