#!/usr/bin/env bash
# Checks the Try Quill page's cache busting: a bundle is named by its content, the manifest names it, a changed input changes both and
# removes the old file, and the page's own versions are current.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: bash scripts/check-try.sh
#
# Builds the bundle three times into a scratch folder with scripts/build-try.sh, from stand-in programs (the disk's contents do not matter
# here, only that they differ): twice from the same inputs, which must give the same name, and once after one file changed, which must give
# a new name, a new manifest, and no trace of the old bundle. Needs mtools and python3; FreeDOS and CuteMouse come from the pinned
# addresses, or from $QUILL_TRY_DIR's download folder if they are already there. Leaves the checkout alone (the page's versions are only
# checked, never written; scripts/stamp-try.py writes them).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(dirname "$HERE")"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
OUT="$SCRATCH/out"
mkdir -p "$OUT" "$SCRATCH/in"

fail() {
	echo "check-try: $*" >&2
	exit 1
}

head -c 4096 /dev/zero | tr '\0' 'a' >"$SCRATCH/in/QUILLDOS.EXE"
head -c 2048 /dev/zero | tr '\0' 'f' >"$SCRATCH/in/QUILL.FNT"
head -c 1024 /dev/zero | tr '\0' 'd' >"$SCRATCH/in/EN_CA.DIC"

build() {
	QUILL_TRY_OUT="$OUT" QUILLDOS_EXE="$SCRATCH/in/QUILLDOS.EXE" QUILL_FNT="$SCRATCH/in/QUILL.FNT" QUILL_DICT="$SCRATCH/in/EN_CA.DIC" \
		SOURCE_DATE_EPOCH="${1:-1767225600}" bash "$HERE/build-try.sh" >"$SCRATCH/build.log" 2>&1 || {
		cat "$SCRATCH/build.log" >&2
		fail "build-try.sh failed"
	}
}

# One field of the manifest.
field() {
	python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])' "$OUT/try-manifest.json" "$1"
}

# The manifest and the folder agree: one bundle, named by its hash, with the size and sum the manifest gives.
verify() {
	local name sum size
	name="$(field bundle)"
	sum="$(field sha256)"
	size="$(field size)"
	[ -f "$OUT/$name" ] || fail "$1: the manifest names $name, which is not in the folder"
	[ "$name" = "quill.${sum:0:12}.jsdos" ] || fail "$1: $name is not named by the first 12 hex of sha256 $sum"
	[ "$(sha256sum "$OUT/$name" | cut -d' ' -f1)" = "$sum" ] || fail "$1: the sha256 in the manifest is not the file's"
	[ "$(wc -c <"$OUT/$name")" = "$size" ] || fail "$1: the size in the manifest is not the file's"
	[ "$(find "$OUT" -maxdepth 1 -name 'quill*.jsdos' | wc -l)" = 1 ] || fail "$1: the folder holds more than the bundle the manifest names"
	[ ! -e "$OUT/quill.jsdos" ] || fail "$1: the unhashed bundle is still there"
	python3 -I - "$OUT/try-manifest.json" <<'PY' || fail "$1: the manifest lacks a field the page reads"
import json, re, sys
m = json.load(open(sys.argv[1]))
assert re.fullmatch(r"quill\.[0-9a-f]{12}\.jsdos", m["bundle"])
assert re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ", m["built"])
for k in ("js-dos.js", "js-dos.css", "emulators"):
	assert re.fullmatch(r"[0-9a-f]{8}", m["assets"][k]), k
PY
}

# A first build.
build
verify "first build"
first="$(field bundle)"

# The same inputs again: the same bundle (a repeatable build), whenever it is made.
build 1800000000
verify "second build"
[ "$(field bundle)" = "$first" ] || fail "the same inputs gave a different bundle name"
[ "$(field built)" = "2027-01-15T08:00:00Z" ] || fail "built is not the build's time (SOURCE_DATE_EPOCH)"

# One file changed: a new name, a new sum, a new manifest, and the old bundle gone.
printf 'changed' >>"$SCRATCH/in/QUILLDOS.EXE"
old_manifest="$(cat "$OUT/try-manifest.json")"
build
verify "build after a change"
second="$(field bundle)"
[ "$second" != "$first" ] || fail "a changed program kept the bundle name $first"
[ ! -e "$OUT/$first" ] || fail "the old bundle $first was not removed"
[ "$(cat "$OUT/try-manifest.json")" != "$old_manifest" ] || fail "the manifest did not change"

# The bundle carries the two shared hard disks, empty and well formed (docs/plans/dos-page-bridge.md): the right sizes and geometry, a
# partition table with correct end values, a FAT12 volume on C: and a FAT16 one on D:, SYNC.COM on the floppy, and at most 64 KB more
# than a bundle without them (the two images compressed).
python3 -I - "$OUT/$(field bundle)" <<'PY' || fail "the bundle's hard disks are not as the page expects"
import sys, zipfile

z = zipfile.ZipFile(sys.argv[1])
names = z.namelist()
for want in (".jsdos/dosbox.conf", "fl.img", "c.img", "d.img"):
	assert want in names, want + " is not in the bundle"
conf = z.read(".jsdos/dosbox.conf").decode()
assert "imgmount 2 c.img -size 512,63,16,16 -t hdd -fs none" in conf, "c.img is not mounted as C:"
assert "imgmount 3 d.img -size 512,63,16,64 -t hdd -fs none" in conf, "d.img is not mounted as D:"
assert conf.index("imgmount 3") < conf.index("boot fl.img"), "the disks are mounted after the boot"
for name, cylinders, kind, fs, label in (("c.img", 16, 0x01, b"FAT12   ", b"QUILLDOCS"), ("d.img", 64, 0x06, b"FAT16   ", b"FROMPC")):
	data = z.read(name)
	total = cylinders * 16 * 63
	assert len(data) == total * 512, f"{name} is {len(data)} bytes, not {total * 512}"
	assert data[510:512] == b"\x55\xaa", f"{name} has no partition table"
	entry = data[446:462]
	assert entry[4] == kind, f"{name}'s partition type is {entry[4]:02x}"
	assert int.from_bytes(entry[8:12], "little") == 63 and int.from_bytes(entry[12:16], "little") == total - 63, f"{name}'s partition is not sectors 63 to the end"
	# The end CHS is the disk's last sector (without it FreeDOS says "suspect partition").
	assert entry[5:8] == bytes([15, 63 | (((cylinders - 1) >> 8) << 6), (cylinders - 1) & 0xFF]), f"{name}'s end CHS is wrong"
	boot = data[63 * 512 : 64 * 512]
	assert boot[54:62] == fs, f"{name} is not {fs.decode().strip()}"
	assert boot[24:26] == (63).to_bytes(2, "little") and boot[26:28] == (16).to_bytes(2, "little"), f"{name}'s BPB geometry is not 63 by 16"
	assert int.from_bytes(boot[28:32], "little") == 63, f"{name}'s hidden sectors are not 63"
	assert label in data[63 * 512 : 130 * 512], f"{name} has no label {label.decode()}"
sizes = {i.filename: i.compress_size for i in z.infolist()}
assert sizes["c.img"] + sizes["d.img"] <= 64 * 1024, f"the two images are {sizes['c.img'] + sizes['d.img']} bytes zipped, over 64 KB"
PY
# SYNC.COM is on the floppy: INT 21h AH=0Dh and a read of drive 80h, 19 bytes (scripts/make-bridge-disks.py has the listing).
python3 -I - "$OUT/$(field bundle)" <<'PY' || fail "SYNC.COM is not on the floppy as built"
import sys, zipfile

floppy = zipfile.ZipFile(sys.argv[1]).read("fl.img")
want = bytes.fromhex("b40dcd21b80102b90100ba8000bb0002cd13c3")
assert want in floppy, "SYNC.COM's bytes are not on fl.img"
PY

# The page's own files carry current versions, and the player's are named by the manifest.
python3 -I "$HERE/stamp-try.py" --check

# The page reads the manifest uncached and does not name a bundle itself.
grep -q "cache: \"no-store\"" "$ROOT/site/try/try.js" || fail "try.js does not fetch the manifest with no-store"
grep -q "opfsRoot" "$ROOT/site/try/try.js" || fail "try.js does not give js-dos a storage folder named by the build"
grep -q 'window.quillBridge.attach(arg)' "$ROOT/site/try/try.js" || fail "try.js lacks the one line that hands the player to the bridge"
[ "$(grep -c quillBridge "$ROOT/site/try/try.js")" = 1 ] || fail "try.js has more than the one bridge line"
grep -q 'bridge.js?v=' "$ROOT/site/try/index.html" || fail "index.html does not load bridge.js, stamped"
grep -q 'data-module="lib/dos-bridge.js?v=' "$ROOT/site/try/index.html" || fail "index.html does not name the bridge module, stamped"
! grep -Eq 'quill(\.[0-9a-f]{12})?\.jsdos' "$ROOT/site/try/index.html" "$ROOT/site/try/try.js" || fail "a page file names a bundle; only the manifest may"

echo "check-try: ok ($first -> $second)"
