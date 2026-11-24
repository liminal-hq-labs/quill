#!/usr/bin/env bash
# Starts Windows 95 in a real DOSBox-X window with QUILL32.EXE and the sample documents on D:, by way of Liminal Installer's test bed.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/play-w95.sh [play-win95.sh options] [PROGRAM.EXE...]
#   Every argument goes to scripts/win95/play-win95.sh (-b plain|ie4, -n, -a ARGS, -s SCALE, -o OUTPUT, -x, -N, -K; see docs/play.md).
# Environment:
#   LIMINAL_INSTALLER  the Liminal Installer checkout that holds scripts/win95/play-win95.sh (default: the first of
#                      ~/source/liminal-hq-labs/liminal-installer.wt-win95play, ~/source/liminal-hq-labs/liminal-installer.wt-win95 and
#                      ~/source/liminal-hq-labs/liminal-installer that has it)
#   W95_HOME           the folder with win95-base.img (default /mnt/games/media/win95-testbed when that exists, else the test bed's own default)
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
find_installer() {
	local d
	for d in "${LIMINAL_INSTALLER:-}" "$HOME/source/liminal-hq-labs/liminal-installer.wt-win95play" "$HOME/source/liminal-hq-labs/liminal-installer.wt-win95" "$HOME/source/liminal-hq-labs/liminal-installer"; do
		if [ -n "$d" ] && [ -x "$d/scripts/win95/play-win95.sh" ]; then
			echo "$d"
			return 0
		fi
	done
	return 1
}
INSTALLER="$(find_installer)" || {
	echo "play-w95: scripts/win95/play-win95.sh not found; set LIMINAL_INSTALLER to a Liminal Installer checkout that has it (the chore/win95-play branch)" >&2
	exit 1
}
if [ -z "${W95_HOME:-}" ] && [ -f /mnt/games/media/win95-testbed/win95-base.img ]; then
	export W95_HOME=/mnt/games/media/win95-testbed
fi
[ -f "$REPO/win9x/dist/QUILL32.EXE" ] || echo "play-w95: QUILL32.EXE is not built; build it with: (cd $REPO/win9x && ./build.sh)" >&2
exec "$INSTALLER/scripts/win95/play-win95.sh" -q "$REPO" "$@"
