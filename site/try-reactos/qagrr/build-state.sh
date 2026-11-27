#!/usr/bin/env bash
# Build the saved v86 machine state for the QAGRR disk parts: serve the parts, boot them in v86 in headless Chromium until the Afterglow
# desktop shows Quill with the demo letter (the Run key from quill-run.reg opens it on a cold boot), save the state and compress it.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: build-state.sh DIST
#   DIST   the folder build-with-rl.sh wrote: DIST/parts/ (manifest.json and the zstd parts). The state is written to DIST/state.l22.bin.zst.
# Environment:
#   PORT           the harness server's port (default 18086; it binds 127.0.0.1)
#   RUNTIME        bun or node (default bun when installed)
#   STATE_TIMEOUT  seconds allowed for the boot and the save (default 1500)
#   CHROMIUM       the browser (default /usr/bin/chromium; none is downloaded)
#
# The state is not reproducible byte for byte. It is a memory and CPU snapshot taken after a boot whose timing depends on the host (when the
# shell's start-up work finishes, which cache lines and free pages hold what), so two runs differ even from the same parts, and the compressed
# file differs in size by a few per cent. What is stable is what it shows (the desktop, Quill with the demo letter open); `check-state.sh`
# verifies that, not the bytes. The disk parts, by contrast, are checked by SHA-256 in the build manifest. Keep a state with the parts it was
# taken from: a state restored over a different disk may find files moved under it.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
try=$(cd "$here/.." && pwd)
[ $# -eq 1 ] || { echo "usage: build-state.sh DIST" >&2; exit 2; }
dist=$(cd "$1" && pwd)
[ -f "$dist/parts/manifest.json" ] || { echo "$dist/parts/manifest.json is missing: run build-with-rl.sh first" >&2; exit 1; }
case $dist in "$(git -C "$here" rev-parse --show-toplevel)"/*) echo "DIST must be outside the repository: $dist" >&2; exit 2 ;; esac

port=${PORT:-18086}
runtime=${RUNTIME:-bun}
command -v "$runtime" >/dev/null || runtime=node
timeout_seconds=${STATE_TIMEOUT:-1500}

# The emulator and the harness's one dependency, when they are not there yet.
[ -f "$try/v86/v86.wasm" ] && [ -f "$try/v86/libv86.mjs" ] && [ -f "$try/v86/bios/seabios.bin" ] || bash "$try/tools/fetch-v86.sh"
if [ ! -d "$try/harness/node_modules/playwright-core" ]; then
	if command -v bun >/dev/null; then (cd "$try/harness" && bun install --frozen-lockfile)
	else (cd "$try/harness" && npm install --no-save --no-package-lock --no-audit --no-fund playwright-core@1.65.0); fi
fi

server_pid=
cleanup() { [ -z "$server_pid" ] || kill "$server_pid" 2>/dev/null || true; rm -f "$dist/state-qagrr.bin"; }
trap cleanup EXIT
(cd "$try/harness" && PORT=$port IMAGES_DIR=$dist LOG_FILE=$dist/state-requests.json exec "$runtime" serve.ts) >"$dist/state-serve.log" 2>&1 &
server_pid=$!
for _ in $(seq 1 60); do curl -fs "http://127.0.0.1:$port/__stats" >/dev/null 2>&1 && break; sleep 1; done
curl -fs "http://127.0.0.1:$port/__stats" >/dev/null || { echo "the harness server did not start; see $dist/state-serve.log" >&2; exit 1; }

echo "== boot in v86 and save the state"
rm -f "$dist/state-qagrr.bin"
(cd "$try/harness" && BASE=http://127.0.0.1:$port timeout "$timeout_seconds" "$runtime" build-state.ts --name state-qagrr --param disk=/images/parts/.img.zst)
kill "$server_pid" 2>/dev/null || true
server_pid=

echo "== zstd -22 --ultra"
zstd -22 --ultra -T0 -f -q "$dist/state-qagrr.bin" -o "$dist/state.l22.bin.zst"
ls -l "$dist/state.l22.bin.zst"
echo "done: $dist/state.l22.bin.zst"
