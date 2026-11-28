#!/usr/bin/env bash
# Checks that the frozen .QLD 1.0 reader is byte for byte what it was when it was frozen (WP-UNI-2, CORE-225).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
dir=crates/quill-formats/tests/compat
want="$(awk '{print $1}' "$dir/qld_1_0.rs.sha256")"
got="$(sha256sum "$dir/qld_1_0.rs" | awk '{print $1}')"
if [ "$want" != "$got" ]; then
	echo "check-frozen-reader: $dir/qld_1_0.rs changed (recorded $want, found $got)." >&2
	echo "It stands for every Quill that reads .QLD 1.0 and is never edited. Restore it with git checkout." >&2
	exit 1
fi
echo "check-frozen-reader: ok"
