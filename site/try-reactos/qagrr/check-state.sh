#!/usr/bin/env bash
# Check a saved state against the disk parts it was built with: serve DIST on 127.0.0.1, restore the state in headless Chromium and verify
# the Afterglow desktop, Quill's window and a File > Open dialog from the guest's own pixels (the emulator canvas; no host desktop capture).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: check-state.sh DIST [SHOTS_DIR]
#   DIST        the folder with parts/ and state.l22.bin.zst (from build-with-rl.sh and build-state.sh)
#   SHOTS_DIR   where the canvas screenshots go (default DIST/check)
# Environment: PORT (default 18087), RUNTIME (bun or node), CHROMIUM. Also prints the disk-touching session's numbers (chunk-session.ts).
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
try=$(cd "$here/.." && pwd)
[ $# -ge 1 ] || { echo "usage: check-state.sh DIST [SHOTS_DIR]" >&2; exit 2; }
dist=$(cd "$1" && pwd)
shots=${2:-$dist/check}
[ -f "$dist/state.l22.bin.zst" ] && [ -f "$dist/parts/manifest.json" ] || { echo "$dist needs state.l22.bin.zst and parts/manifest.json" >&2; exit 1; }
port=${PORT:-18087}
runtime=${RUNTIME:-bun}
command -v "$runtime" >/dev/null || runtime=node
[ -f "$try/v86/v86.wasm" ] || bash "$try/tools/fetch-v86.sh"
[ -d "$try/harness/node_modules/playwright-core" ] || (cd "$try/harness" && bun install --frozen-lockfile)

server_pid=
trap '[ -z "$server_pid" ] || kill "$server_pid" 2>/dev/null || true' EXIT
(cd "$try/harness" && PORT=$port IMAGES_DIR=$dist LOG_FILE=$dist/check-requests.json exec "$runtime" serve.ts) >"$dist/check-serve.log" 2>&1 &
server_pid=$!
for _ in $(seq 1 60); do curl -fs "http://127.0.0.1:$port/__stats" >/dev/null 2>&1 && break; sleep 1; done

echo "== restore and look"
(cd "$try/harness" && BASE=http://127.0.0.1:$port "$runtime" check-state.ts "$shots")
echo "== disk-touching session"
(cd "$try/harness" && BASE=http://127.0.0.1:$port "$runtime" chunk-session.ts --name check --param disk=/images/parts/.img.zst)
