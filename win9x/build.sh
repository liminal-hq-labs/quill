#!/usr/bin/env bash
# Builds QUILL32.EXE into win9x/dist/ with no C compiler and no C library: Rust, rust-lld's lld-link and a Pentium target.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/build.sh       (set CARGO_TARGET_DIR to keep the build output outside the tree)
set -euo pipefail
cd "$(dirname "$0")"
export RUSTC_BOOTSTRAP=1
# The resources: the menu tree from the app, packed into a .res (see tools/res_pack.py).
(cd .. && RUSTC_BOOTSTRAP=0 cargo run -q -p quill-host --bin emit-resources > win9x/resources/quill.json)
python3 -I tools/res_pack.py resources/quill.json resources/quill.res
python3 -I tools/test_res_pack.py resources/quill.json resources/quill.res
cargo build --release --locked
target="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p dist
cp "$target/win9x/release/quill32.exe" dist/QUILL32.EXE
echo "build: dist/QUILL32.EXE ($(wc -c < dist/QUILL32.EXE) bytes)"
