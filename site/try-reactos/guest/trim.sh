#!/usr/bin/env bash
# Delete what the demo does not need from a ReactOS FAT32 disk image, offline, with mtools, and log the bytes saved per item.
# Usage: trim.sh IMAGE LIST LOG   (IMAGE is a COPY; the VM must be off. LIST lines: "f PATH" or "d PATH", paths under the FAT32 root.)
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -eu
IMG=$1; LIST=$2; LOG=$3
export MTOOLS_SKIP_CHECK=1
D="$IMG@@1048576"
free() { mdir -i "$D" ::/ | awk '/bytes free/ {gsub(/ /,""); sub(/bytesfree/,""); print}'; }
before_all=$(free)
: > "$LOG"
while read -r kind path; do
	case "$kind" in ''|'#'*) continue ;; esac
	b=$(free)
	if [ "$kind" = d ]; then mdeltree -i "$D" "::/$path" >/dev/null 2>&1 || echo "MISSING $path" >> "$LOG"
	else mdel -i "$D" "::/$path" >/dev/null 2>&1 || echo "MISSING $path" >> "$LOG"; fi
	a=$(free)
	printf '%s\t%s\t%d\n' "$kind" "$path" $((a - b)) >> "$LOG"
done < "$LIST"
after_all=$(free)
printf 'TOTAL\t\t%d\n' $((after_all - before_all)) >> "$LOG"
tail -1 "$LOG"
