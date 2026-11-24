#!/usr/bin/env bash
# Fails when a portable crate names a floating-point type (D9: integer and fixed-point arithmetic only).
# The crates checked are the root workspace's packages whose `[package.metadata.quill]` says `float = false`,
# so a new portable crate opts in from its own Cargo.toml and no list here needs an edit.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail
cd "$(dirname "$0")/.."

dirs=$(cargo metadata --format-version 1 --no-deps | python3 -I -c '
import json, os, sys
root = os.getcwd()
for p in json.load(sys.stdin)["packages"]:
	if ((p.get("metadata") or {}).get("quill") or {}).get("float") is False:
		print(os.path.relpath(os.path.join(os.path.dirname(p["manifest_path"]), "src"), root))
' | sort)

if [ -z "$dirs" ]; then
	echo "check-no-floats: no crate declares float = false" >&2
	exit 1
fi

# shellcheck disable=SC2086
hits=$(grep -rnE '\b(f32|f64)\b' $dirs || true)
if [ -n "$hits" ]; then
	echo "check-no-floats: floating point in a portable crate:" >&2
	echo "$hits" >&2
	exit 1
fi
dialogs=$(grep -rnE '\b(f32|f64)\b' crates/quill-dialogs/src || true)
if [ -n "$dialogs" ]; then
	echo "check-no-floats: floating point in quill-dialogs:" >&2
	echo "$dialogs" >&2
	exit 1
fi
echo "check-no-floats: ok"
