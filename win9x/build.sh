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
