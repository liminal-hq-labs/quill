#!/usr/bin/env bash
# Checks site/try-reactos/vendor/v86-host-link/: every file matches SHA256SUMS, nothing is there that SHA256SUMS does not list, and no test-only file
# is among them.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: bash tools/check-v86-host-link.sh
set -euo pipefail

dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/site/try-reactos/vendor/v86-host-link"
fail() { echo "check-v86-host-link: $*" >&2; exit 1; }

[ -f "$dir/SHA256SUMS" ] || fail "SHA256SUMS is missing"
[ -f "$dir/PROVENANCE.md" ] || fail "PROVENANCE.md is missing"
(cd "$dir" && sha256sum --check --strict --quiet SHA256SUMS) || fail "a file does not match SHA256SUMS; run tools/fetch-v86-host-link.sh instead of editing the vendored files"

listed="$(awk '{ print $2 }' "$dir/SHA256SUMS" | LC_ALL=C sort)"
present="$(cd "$dir" && find . -type f ! -name SHA256SUMS ! -name PROVENANCE.md | sed 's|^\./||' | LC_ALL=C sort)"
[ "$listed" = "$present" ] || fail "the files here and the files in SHA256SUMS differ"

# Test-only programs: the test modules and their variants, the break helper, the RTF copier, the Wine helper, the tray's test build, the audio probe.
if grep -E -i '(^|/)(HLTEST[A-Z]*\.DLL|HLBRK\.EXE|HLCLIP\.EXE|HOSTLINKW\.EXE|HLTRAYT\.EXE|HLAUDIO[CP]\.DLL)$' <<<"$present"; then
	fail "a test-only file is vendored"
fi
for need in host-link.js guest/HOSTLINK.EXE guest/HLAUDIO.DLL guest/HLTRAY.EXE kit/hostlink-kit.iso; do
	grep -qx "$need" <<<"$present" || fail "$need is missing"
done
echo "check-v86-host-link: $(wc -l <<<"$present") files match SHA256SUMS"
