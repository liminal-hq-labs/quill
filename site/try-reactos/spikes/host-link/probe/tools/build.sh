#!/usr/bin/env bash
# Builds HLPROBE.EXE into dist/H.EXE with the same toolchain and target as QUILL32.EXE (a short name, because it is typed into the Run box).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/build.sh     (set CARGO_TARGET_DIR to keep the build output outside the tree)
set -euo pipefail
cd "$(dirname "$0")/.."
export RUSTC_BOOTSTRAP=1
cargo build --release
target="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p dist
cp "$target/win9x/release/hlprobe.exe" dist/H.EXE
echo "build: dist/H.EXE ($(wc -c < dist/H.EXE) bytes)"
