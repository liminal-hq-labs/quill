#!/usr/bin/env bash
# Builds W32SPIKE.EXE into spikes/w32-worker/dist/ with the same toolchain and target as QUILL32.EXE.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/w32-worker/tools/build.sh     (set CARGO_TARGET_DIR to keep the build output outside the tree)
set -euo pipefail
cd "$(dirname "$0")/.."
export RUSTC_BOOTSTRAP=1
cargo build --release --locked
target="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p dist
cp "$target/win9x/release/w32spike.exe" dist/W32SPIKE.EXE
echo "build: dist/W32SPIKE.EXE ($(wc -c < dist/W32SPIKE.EXE) bytes)"
