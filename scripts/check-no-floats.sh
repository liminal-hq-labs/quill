#!/usr/bin/env bash
# Fails when a portable crate names a floating-point type (D9: integer and fixed-point arithmetic only).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail
cd "$(dirname "$0")/.."

hits=$(grep -rnE '\b(f32|f64)\b' crates/quill-core/src crates/quill-formats/src || true)
if [ -n "$hits" ]; then
	echo "check-no-floats: floating point in a portable crate:" >&2
	echo "$hits" >&2
	exit 1
fi
echo "check-no-floats: ok"
