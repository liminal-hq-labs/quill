#!/usr/bin/env bash
# Builds QUILL32.EXE into win9x/dist/ with no C compiler and no C library: Rust, rust-lld's lld-link and a Pentium target.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/build.sh [--bench]       (set CARGO_TARGET_DIR to keep the build output outside the tree)
#   --bench  builds with the `bench` feature (it counts the GDI and USER calls of the paint and key paths, times the phases and writes a
#            log; see docs/progress.md) into dist/QUILL32-BENCH.EXE, and leaves dist/QUILL32.EXE alone
#
# One build id (QUILL_BUILD_ID: the first eight hex digits of HEAD, the top bit set when the tree has uncommitted changes) goes to the
# program and every module, so they always come from one build (CORE-94, W9X-135). The modules and the loader's test files land in dist/
# and dist/test/ (the test files are never staged on a disk).
set -euo pipefail
cd "$(dirname "$0")"
export RUSTC_BOOTSTRAP=1
commit="$(git rev-parse --short=8 HEAD)"
build=$((0x$commit & 0x7fffffff))
if [ -n "$(git status --porcelain)" ]; then build=$((build | 0x80000000)); fi
export QUILL_BUILD_ID="$(printf '%08x' "$build")"
# The resources: the menu tree from the app, packed into a .res (see tools/res_pack.py).
(cd .. && RUSTC_BOOTSTRAP=0 cargo run -q -p quill-host --bin emit-resources > win9x/resources/quill.json)
python3 -I tools/res_pack.py resources/quill.json resources/quill.res ../assets/icon/QUILL.ICO ../assets/dialogs/dialogs.json
python3 -I tools/test_res_pack.py resources/quill.json resources/quill.res ../assets/dialogs/dialogs.json
bench=0
if [ "${1:-}" = "--bench" ]; then
	bench=1
	cargo build --release --locked -p quill-win9x --features bench
	out=dist/QUILL32-BENCH.EXE
else
	cargo build --release --locked -p quill-win9x
	out=dist/QUILL32.EXE
fi
target="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p dist
cp "$target/win9x/release/quill32.exe" "$out"
echo "build: $out ($(wc -c < "$out") bytes)"
[ "$bench" = 0 ] || exit 0

# The modules, after the program: each one is a `cdylib` with its own link flags (its build.rs), copied here as an upper-case NAME.DLL.
# None is staged yet (the picture module is the next package); the loader's test module and its variants go to dist/test/, which
# installer/tools/stage.sh never reads. The variants are the same code linked or labelled differently, so the test program can tell
# a refusal from an acceptance: another build id (clean, and development), another interface major, the program's own base.
mkdir -p dist/test
clean="$(printf '%08x' $((build & 0x7fffffff)))"
other="$(printf '%08x' $(((build & 0x7fffffff) ^ 1)))"
development="$(printf '%08x' $((((build & 0x7fffffff) ^ 1) | 0x80000000)))"
tst() { # the file's stem, then the variables to set (VAR=value ...) and the cargo features
	local name="$1"
	shift
	env "$@" cargo build --release --locked -p quill-mod-tst
	cp "$target/win9x/release/quilltst.dll" "dist/test/$name.DLL"
}
tst QUILLTST QUILL_BUILD_ID="$clean"
tst QUILLTSTR QUILL_BUILD_ID="$clean" QUILLTST_BASE=0x00400000 QUILLTST_MAP=QUILLTSTR.MAP
tst QUILLTSTB QUILL_BUILD_ID="$other" QUILLTST_MAP=QUILLTSTB.MAP
tst QUILLTSTD QUILL_BUILD_ID="$development" QUILLTST_MAP=QUILLTSTD.MAP
env QUILL_BUILD_ID="$clean" QUILLTST_MAP=QUILLTSTM.MAP cargo build --release --locked -p quill-mod-tst --features wrong-major
cp "$target/win9x/release/quilltst.dll" dist/test/QUILLTSTM.DLL
QUILL_BUILD_ID="$clean" cargo build --release --locked -p quill-mod-host
cp "$target/win9x/release/modhost.exe" dist/test/MODHOST.EXE
echo "build: dist/test/ ($(ls dist/test | wc -l) files for the loader's test)"
