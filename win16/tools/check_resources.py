#!/usr/bin/env python3
# Fails unless an NE executable carries the resources named on the command line.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/check_resources.py EXE TYPE:ID...   (TYPE is a number, e.g. 14:1 for the icon group 1, 2:2 for bitmap 2)
#
# The packer once dropped every resource it did not know without a word, which left QUILL.EXE without its icon; this catches that.
import struct
import sys


def resources(path):
	with open(path, "rb") as f:
		data = f.read()
	ne = struct.unpack_from("<I", data, 0x3C)[0]
	if data[ne : ne + 2] != b"NE":
		raise SystemExit(f"{path}: not an NE executable")
	res_off, resname_off = struct.unpack_from("<HH", data, ne + 0x24)
	found = set()
	if res_off == resname_off:
		return found
	at = ne + res_off + 2
	while True:
		typ = struct.unpack_from("<H", data, at)[0]
		if typ == 0:
			break
		count = struct.unpack_from("<H", data, at + 2)[0]
		at += 8
		for _ in range(count):
			rid = struct.unpack_from("<H", data, at + 6)[0]
			found.add((typ & 0x7FFF, rid & 0x7FFF))
			at += 12
	return found


def main():
	if len(sys.argv) < 3:
		raise SystemExit(__doc__ or "usage: check_resources.py EXE TYPE:ID...")
	have = resources(sys.argv[1])
	missing = []
	for want in sys.argv[2:]:
		t, i = (int(v) for v in want.split(":"))
		if (t, i) not in have:
			missing.append(want)
	if missing:
		raise SystemExit(f"check_resources: {sys.argv[1]} lacks {' '.join(missing)} (has {sorted(have)})")
	print(f"check_resources: ok: {sys.argv[1]}: {len(have)} resources, {' '.join(sys.argv[2:])} present")


if __name__ == "__main__":
	main()
