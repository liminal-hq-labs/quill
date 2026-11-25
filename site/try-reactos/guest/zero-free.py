#!/usr/bin/env python3
# Zero every free cluster of the FAT32 partition in a raw disk image (partition at 1 MiB), then the caller can punch holes with
# `fallocate --dig-holes`. Deleted files leave their data behind; zeroing it lets the image compress and splits into tiny parts.
# Usage: zero-free.py IMAGE [PARTITION_OFFSET=1048576]
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import struct, sys

path = sys.argv[1]
base = int(sys.argv[2]) if len(sys.argv) > 2 else 1048576
f = open(path, "r+b")
f.seek(base)
b = f.read(512)
bps, spc, res, nfats = struct.unpack("<HBHB", b[11:17])
total = struct.unpack("<I", b[32:36])[0]
spf = struct.unpack("<I", b[36:40])[0]
first_data = res + nfats * spf
clusters = (total - first_data) // spc
f.seek(base + res * bps)
fat = f.read(spf * bps)
csize = spc * bps
zero = bytes(csize)
cleared = touched = 0
for n in range(2, clusters + 2):
	if struct.unpack_from("<I", fat, n * 4)[0] & 0x0FFFFFFF:
		continue
	off = base + (first_data + (n - 2) * spc) * bps
	f.seek(off)
	if f.read(csize) != zero:
		f.seek(off)
		f.write(zero)
		cleared += 1
	touched += 1
f.close()
print(f"{touched} free clusters, {cleared} held data and were zeroed ({cleared * csize / 2**20:.1f} MiB)")
