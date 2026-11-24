#!/usr/bin/env bash
# Builds Quill's three installer disks: a 1.44 MB floppy image per build.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: installer/tools/build-disks.sh [--stage]
#
#   --stage    run `stage.sh --build` first, so the disks hold freshly built programs
#
# Needs installer/dist/ filled by stage.sh, mtools, and `liminst` with its runtimes (see liminst.sh). For each target it runs
# `liminst build`, then post-processes the floppy with mtools: the volume label becomes `QUILL DOS`, `QUILL W31` or `QUILL W95`
# and the root README.TXT is Quill's own, not the generic one. The images land in installer/out/:
#
#   QUILL-DOS.IMG  QUILL-W31.IMG  QUILL-W95.IMG   the floppy images
#   quill-*.zip                                    the same files as a zip, from liminst
#   SIZES.txt                                      per disk: the free space, the root files and every file in the pack (raw size)
#   SHA256SUMS                                     the images' and zips' checksums
#
# There is no all-in-one disk: liminst writes one SETUP.EXE per target into the image root, so one image cannot hold three
# installers. Each build is its own disk until the installer can share one pack between targets.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(dirname "$HERE")"
OUT="$HERE/out"
LIMINST="$HERE/tools/liminst.sh"
MIN_FREE="${QUILL_MIN_FREE:-65536}"

for tool in mcopy mlabel minfo mdir; do
	command -v "$tool" >/dev/null || { echo "build-disks: $tool not found; install mtools" >&2 && exit 1; }
done

if [ "${1:-}" = "--stage" ]; then bash "$HERE/tools/stage.sh" --build; fi
[ -f "$HERE/dist/win16-x86/QUILL.EXE" ] || { echo "build-disks: installer/dist is empty; run installer/tools/stage.sh --build" >&2 && exit 1; }
if grep -q '^STAND-IN' "$HERE/dist/win16-x86/QUILL.EXE" 2>/dev/null && [ -z "${QUILL_ALLOW_STAND_INS:-}" ]; then
	echo "build-disks: installer/dist holds stand-ins; run installer/tools/stage.sh --build (or set QUILL_ALLOW_STAND_INS=1 for a dry run)" >&2
	exit 1
fi

# The version is the one the programs were built as.
VERSION="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
[ -n "$VERSION" ] || { echo "build-disks: no version in [workspace.package] of Cargo.toml" >&2 && exit 1; }

rm -rf "$OUT"
mkdir -p "$OUT"
: >"$OUT/SIZES.txt"
printf 'Quill %s installer disks (1.44 MB floppy: 1457664 bytes)\n' "$VERSION" >>"$OUT/SIZES.txt"

# target id, image name, volume label
disks=("dos-x86 QUILL-DOS 'QUILL DOS'" "win16-x86 QUILL-W31 'QUILL W31'" "windows-x86 QUILL-W95 'QUILL W95'")
for d in "${disks[@]}"; do
	eval "set -- $d"
	target="$1" name="$2" label="$3"
	bash "$LIMINST" build -t "$target" --version "$VERSION" -o "$OUT/$target" >"$OUT/$target.log" 2>&1 || {
		cat "$OUT/$target.log" >&2
		echo "build-disks: liminst build failed for $target" >&2
		exit 1
	}
	src="$(find "$OUT/$target" -maxdepth 1 -name '*-1440.img' -print -quit)"
	[ -n "$src" ] || { cat "$OUT/$target.log" >&2 && echo "build-disks: no floppy image for $target (the files did not fit?)" >&2 && exit 1; }
	img="$OUT/$name.IMG"
	cp "$src" "$img"
	mlabel -i "$img" "::$label"
	mcopy -i "$img" -o "$HERE/dist/common/README.TXT" ::README.TXT
	zip="$(find "$OUT/$target" -maxdepth 1 -name '*.zip' -print -quit)"
	[ -z "$zip" ] || cp "$zip" "$OUT/$(basename "$zip")"

	# mtools writes the free space as "505 856 bytes free".
	free="$(mdir -i "$img" :: | sed -n 's/^ *\([0-9 ]*[0-9]\) bytes free.*/\1/p' | tr -d ' ')"
	[ -n "$free" ] || { echo "build-disks: cannot read the free space of $img" >&2 && exit 1; }
	pack="$OUT/$target/$(basename "$(find "$OUT/$target" -mindepth 1 -maxdepth 1 -type d -print -quit)")/SETUP.LPK"
	{
		printf '\n== %s (%s, label "%s")\n' "$name.IMG" "$target" "$label"
		printf 'free on the disk: %s bytes (%s KiB)\n' "$free" "$((free / 1024))"
		printf 'files in the root:\n'
		mdir -i "$img" :: | sed -n 's/^\([A-Z0-9_~]\{1,8\}\) \{1,\}\([A-Z0-9]\{0,3\}\) \{1,\}\([0-9][0-9 ]*[0-9]\) \([0-9-]\{10\}\) .*/\1.\2|\3/p' |
			awk -F'|' '{ gsub(/ /, "", $2); printf "  %-14s %9d\n", $1, $2 }'
		printf 'pack contents (liminst lpk list):\n'
		bash "$LIMINST" lpk list "$pack" | sed 's/^/  /'
	} >>"$OUT/SIZES.txt"
	if [ "$free" -lt "$MIN_FREE" ]; then
		echo "build-disks: $name.IMG has only $free bytes free, under $MIN_FREE" >&2
		exit 1
	fi
	echo "build-disks: $name.IMG, $free bytes free"
done
(cd "$OUT" && sha256sum QUILL-DOS.IMG QUILL-W31.IMG QUILL-W95.IMG ./*.zip >SHA256SUMS)
echo "build-disks: done; see $OUT/SIZES.txt"
