#!/usr/bin/env python3
# Builds the two hard disk images the Try Quill page shares with FreeDOS, and SYNC.COM: C: (the guest writes) and D: (the page writes).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I scripts/make-bridge-disks.py OUT_DIR
#
# Writes OUT_DIR/c.img, OUT_DIR/d.img and OUT_DIR/SYNC.COM (docs/plans/dos-page-bridge.md, SITE-60).
#
#   c.img  8 MB: 16 cylinders, 16 heads, 63 sectors; one primary FAT12 partition (type 01h) from sector 63; label QUILLDOCS; empty
#   d.img  32 MB: 64 cylinders, 16 heads, 63 sectors; one primary FAT16 partition (type 06h) from sector 63; label FROMPC; empty
#
# The partition table is written here, not by sfdisk: its end CHS made FreeDOS print "suspect partition", and writing it ourselves
# gives the same bytes on every machine. The file systems are made by mtools (mformat) at the partition's byte offset, with a fixed
# volume serial number and zeroed label times, so the same inputs give the same bytes (and the same bundle name). The geometry here
# is the one scripts/try-dosbox.conf passes to `imgmount`, and the one site/try/packages/dos-bridge reads back out of the BPB.

import os
import subprocess
import sys

SECTOR = 512
HEADS = 16
SPT = 63
START = 63  # the partition's first sector: head 1, sector 1 of cylinder 0, so the first track holds the partition table alone


def chs(lba):
	"""The cylinder-head-sector bytes of an MBR entry for a sector number (cylinders past 1023 saturate, as BIOSes expect)."""
	cylinder, rest = divmod(lba, HEADS * SPT)
	head, sector = divmod(rest, SPT)
	sector += 1
	cylinder = min(cylinder, 1023)
	return bytes([head, (sector & 0x3F) | ((cylinder >> 8) << 6), cylinder & 0xFF])


def make(path, cylinders, kind, label, serial, cluster_sectors):
	total = cylinders * HEADS * SPT
	size = total - START
	mbr = bytearray(SECTOR)
	entry = bytearray(16)
	entry[0] = 0x80
	entry[1:4] = chs(START)
	entry[4] = kind
	entry[5:8] = chs(total - 1)
	entry[8:12] = START.to_bytes(4, "little")
	entry[12:16] = size.to_bytes(4, "little")
	mbr[446:462] = entry
	mbr[510:512] = b"\x55\xaa"
	# A zeroed file first, as scripts/make-freedos-image.py does: the image is the full disk, so DOSBox's geometry matches its length.
	with open(path, "wb") as f:
		f.write(bytes(total * SECTOR))
		f.seek(0)
		f.write(mbr)
	env = dict(os.environ, MTOOLS_SKIP_CHECK="1", TZ="UTC")
	subprocess.run(
		[
			"mformat", "-i", f"{path}@@{START * SECTOR}", "-T", str(size), "-h", str(HEADS), "-s", str(SPT), "-H", str(START),
			"-c", str(cluster_sectors), "-N", serial, "-v", label, "::",
		],
		check=True,
		env=env,
	)
	# The volume label's entry carries the time mformat ran; zero it so the image is the same bytes every time.
	with open(path, "r+b") as f:
		data = bytearray(f.read())
	base = START * SECTOR
	reserved = int.from_bytes(data[base + 14 : base + 16], "little")
	fats = data[base + 16]
	per_fat = int.from_bytes(data[base + 22 : base + 24], "little")
	root = base + (reserved + fats * per_fat) * SECTOR
	root_entries = int.from_bytes(data[base + 17 : base + 19], "little")
	for off in range(root, root + root_entries * 32, 32):
		if data[off] == 0:
			break
		if data[off + 11] & 0x08:
			data[off + 13 : off + 14] = b"\x00"
			for field in (14, 16, 18, 22, 24):
				data[off + field : off + field + 2] = (0).to_bytes(2, "little")
	with open(path, "wb") as f:
		f.write(data)


# SYNC.COM: INT 21h AH=0Dh (disk reset: FreeDOS writes its buffers and forgets what it has read), then INT 13h AH=02h reading one sector
# of the first hard disk (80h, which is C:) into the program's own segment (DOSBox's stdio buffer for the disk is flushed by a read).
#
#	B4 0D          mov ah, 0Dh
#	CD 21          int 21h
#	B8 01 02       mov ax, 0201h     ; read, one sector
#	B9 01 00       mov cx, 0001h     ; cylinder 0, sector 1
#	BA 80 00       mov dx, 0080h     ; head 0, drive 80h
#	BB 00 02       mov bx, 0200h     ; ES:BX = this segment, 200h (inside the COM's 64 KB segment)
#	CD 13          int 13h
#	C3             ret
SYNC_COM = bytes.fromhex("b40dcd21b80102b90100ba8000bb0002cd13c3")


def main():
	out = sys.argv[1]
	os.makedirs(out, exist_ok=True)
	make(os.path.join(out, "c.img"), 16, 0x01, "QUILLDOCS", "51494331", 4)
	make(os.path.join(out, "d.img"), 64, 0x06, "FROMPC", "51494432", 2)
	with open(os.path.join(out, "SYNC.COM"), "wb") as f:
		f.write(SYNC_COM)


if __name__ == "__main__":
	main()
