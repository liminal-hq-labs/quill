#!/bin/bash
# Checks the DOS workspace: format, lints, then builds QUILLDOS.EXE and compares its screens when the tools are present.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/check.sh
#
# The format and lint checks always run. The build and the DOSBox run need the tools from dos/tools/fetch-tools.sh; without them
# the run is skipped with a note, so the script is safe in a checkout that has no DOS tools.
set -euo pipefail
cd "$(dirname "$0")"

cargo fmt --all -- --check
RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings
tools/e2e.sh
tools/rt-test.sh
