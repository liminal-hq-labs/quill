#!/usr/bin/env bash
# Builds the spike from Linux with cargo-xwin and packs the self-contained zip.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: build.sh [OUT_DIR]
#   OUT_DIR  where the zip goes (default: $CARGO_TARGET_DIR or ./target, under winui/spike)
# Environment: CARGO_TARGET_DIR should point outside the repository. Needs cargo-xwin and its Windows SDK cache.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
spike="$here/../spike"
stage="$("$here/stage-runtime.sh" | tail -n 1)"
export QUILL_WINUI_MANIFEST="$stage/app.manifest"
target="${CARGO_TARGET_DIR:-$spike/target}"
out="${1:-$target}"
export CARGO_TARGET_DIR="$target"

cd "$spike"
cargo xwin build --release --locked --target x86_64-pc-windows-msvc

exe="$target/x86_64-pc-windows-msvc/release/quill-winui-spike.exe"
pack="$out/quill-winui-spike"
rm -rf "$pack" "$out/quill-winui-spike.zip"
mkdir -p "$pack"
cp -r "$stage"/. "$pack"/
rm -f "$pack/app.manifest"
cp "$exe" "$pack/"
(cd "$pack" && zip -q -9 -r "$out/quill-winui-spike.zip" .)
ls -l "$exe" "$out/quill-winui-spike.zip"
