#!/usr/bin/env bash
# Reserves the next free block of decision ids: scripts/next-id.sh PREFIX COUNT OWNER [--source PATH].
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Finds the first free id after the highest one reserved in docs/id-ranges.toml or used in docs/decisions.md for PREFIX,
# appends a block of COUNT ids for OWNER with the status reserved, regenerates docs/id-ranges.md and prints the block.
# Commit both files with the plan that names the block.

if [ "$#" -lt 3 ]; then
	echo "usage: scripts/next-id.sh PREFIX COUNT OWNER [--source PATH]" >&2
	exit 2
fi

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$here/check-ids.py" next "$@"
