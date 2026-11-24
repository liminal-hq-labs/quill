#!/usr/bin/env bash
# Checks the installer project: `liminst check` and `liminst preview` for the three targets, on stand-in files.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: installer/tools/check.sh
#
# This replaces installer/dist/ with stand-ins (see stage.sh), so run stage.sh --build again before building disks.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LIMINST="$HERE/tools/liminst.sh"

bash "$HERE/tools/stage.sh" --stand-ins
bash "$LIMINST" check
for target in dos-x86 win16-x86 windows-x86; do
	bash "$LIMINST" check -t "$target"
	bash "$LIMINST" preview -t "$target" >/dev/null
	bash "$LIMINST" preview -t "$target" --flow uninstall >/dev/null
	echo "check: $target ok"
done
