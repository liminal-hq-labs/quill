#!/usr/bin/env bash
# Runs W32SPIKE.EXE under Wine on Xvfb with a SPIKE.CFG and prints its SPIKE.LOG.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/w32-worker/tools/wine-run.sh [--one-cpu] [--out DIR] [KEY=VALUE...]
#   --one-cpu  pins Wine to one host CPU with taskset (a one-processor machine, like Windows 95 in DOSBox-X); the default lets it use all
#   --out DIR  keeps SPIKE.LOG there as wine[-1cpu].log (default: print only)
#   KEY=VALUE  lines for SPIKE.CFG (doc, keys, keyms, cycles, stressdoc, exit, yield, phases; see main.rs)
# WINEPREFIX defaults to $HOME/.cache/w32spike-wine (never inside the repository). The run is killed after W32_TIMEOUT seconds (default 900).
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ONE=0
OUT=""
CFG=()
while [ "$#" -gt 0 ]; do
	case "$1" in
	--one-cpu) ONE=1; shift ;;
	--out) OUT="$(mkdir -p "$2" && cd "$2" && pwd)"; shift 2 ;;
	*=*) CFG+=("$1"); shift ;;
	*) echo "wine-run: unknown option $1" >&2; exit 2 ;;
	esac
done
command -v wine > /dev/null && command -v xvfb-run > /dev/null || { echo "wine-run: wine or xvfb-run not found" >&2; exit 1; }
[ -f "$HERE/dist/W32SPIKE.EXE" ] || { echo "wine-run: build first (tools/build.sh)" >&2; exit 1; }
export WINEPREFIX="${WINEPREFIX:-$HOME/.cache/w32spike-wine}" WINEDEBUG=-all
unset WINEARCH
WORK="$(mktemp -d "${TMPDIR:-/tmp}/w32spike.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
cp "$HERE/dist/W32SPIKE.EXE" "$WORK/"
printf '%s\r\n' "${CFG[@]}" > "$WORK/SPIKE.CFG"
pre=()
[ "$ONE" = 0 ] || pre=(taskset -c 0)
code=0
(cd "$WORK" && timeout "${W32_TIMEOUT:-900}" xvfb-run -a "${pre[@]}" wine W32SPIKE.EXE > /dev/null 2>&1) || code=$?
echo "wine-run: exit code $code"
[ -f "$WORK/SPIKE.LOG" ] || { echo "wine-run: no SPIKE.LOG" >&2; exit 1; }
tr -d '\r' < "$WORK/SPIKE.LOG"
[ -z "$OUT" ] || cp "$WORK/SPIKE.LOG" "$OUT/wine$([ "$ONE" = 1 ] && echo -1cpu).log"
