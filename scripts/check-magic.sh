#!/usr/bin/env bash
# Checks assets/magic/quill.magic against the sample documents with file(1).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
magic=assets/magic/quill.magic
fail=0
check() {
	local want="$1" path="$2" got
	got="$(file -b -m "$magic" "$path")"
	if [ "$got" != "$want" ]; then
		echo "check-magic: $path: wanted '$want', got '$got'" >&2
		fail=1
	fi
}
check "Quill document, version 1.0" tests/docs/letter.qld
check "Quill document, version 1.0" tests/docs/plain.qld
check "Quill document, version 1.0" tests/golden/files/m1-edit.qld
mime="$(file -b --mime-type -m "$magic" tests/docs/letter.qld)"
[ "$mime" = "application/x-quill-document" ] || { echo "check-magic: wanted the Quill MIME type, got '$mime'" >&2; fail=1; }
# A plain RIFF file and an RTF file must not match.
[ "$(file -b -m "$magic" tests/docs/letter.rtf)" != "Quill document, version 1.0" ] || { echo "check-magic: RTF matched" >&2; fail=1; }
exit "$fail"
