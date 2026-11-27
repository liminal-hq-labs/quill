#!/usr/bin/env python3
# Makes the golden disk images that the dos-bridge package's tests read: a small FAT12 and a small FAT16 hard disk, each behind an MBR, with
# the files and the layouts that a reader must get right. mtools writes them, so they are what DOS tools make, not what fat.ts makes.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tests/dos-bridge/make-goldens.py     (rewrites fat12.img.gz, fat16.img.gz and manifest.json beside this script)
#
# Needs mtools. Each image has files of 0, 1, 511, 512 and 513 bytes, a long file name (its short alias comes with it), a folder with a
# file in it, a deleted file's entry, and FRAG.BIN, a file that fills a hole and carries on past other files, so its cluster chain is not
# contiguous and spans several sectors of the FAT. Contents repeat every 251 bytes, so the images compress to a few kilobytes.

import gzip
import hashlib
import json
import os
import subprocess
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SECTOR = 512
START = 63
STAMP = 1709647630  # 2024-03-05 14:07:10 UTC; TZ is UTC for mtools, and the tests compare the fields, not the instant


def content(n, salt):
	return bytes((i * 7 + salt) % 251 for i in range(n))


def make(name, total, kind, label, cluster, frag_size):
	env = dict(os.environ, MTOOLS_SKIP_CHECK="1", TZ="UTC")

	def run(*args):
		subprocess.run(args, check=True, env=env)

	with tempfile.TemporaryDirectory() as tmp:
		path = os.path.join(tmp, "disk.img")
		disk_sectors = START + total
		mbr = bytearray(SECTOR)
		entry = bytearray(16)
		entry[0] = 0x80
		entry[1:4] = bytes([1, 1, 0])
		entry[4] = kind
		entry[5:8] = bytes([0xFE, 0xFF, 0xFF])
		entry[8:12] = START.to_bytes(4, "little")
		entry[12:16] = total.to_bytes(4, "little")
		mbr[446:462] = entry
		mbr[510:512] = b"\x55\xaa"
		with open(path, "wb") as f:
			f.write(bytes(disk_sectors * SECTOR))
			f.seek(0)
			f.write(mbr)
		image = f"{path}@@{START * SECTOR}"
		run("mformat", "-i", image, "-T", str(total), "-h", "16", "-s", "63", "-H", str(START), "-c", str(cluster), "-N", "47304c44", "-v", label, "::")

		files = {}

		def put(dest, data, salt=None):
			local = os.path.join(tmp, "f" + str(len(files)))
			with open(local, "wb") as f:
				f.write(data)
			os.utime(local, (STAMP, STAMP))
			run("mcopy", "-m", "-i", image, local, "::" + dest)
			os.remove(local)
			files[dest.lstrip("/").upper()] = data

		sizes = {"EMPTY.TXT": 0, "ONE.BIN": 1, "S511.BIN": 511, "S512.BIN": 512, "S513.BIN": 513}
		for i, (n, size) in enumerate(sizes.items()):
			put("/" + n, content(size, i))
		put("/A.BIN", content(cluster * SECTOR * 3, 11))
		put("/B.BIN", content(cluster * SECTOR * 6, 12))
		put("/C.BIN", content(cluster * SECTOR * 3, 13))
		run("mdel", "-i", image, "::B.BIN")
		files.pop("B.BIN")
		put("/FRAG.BIN", content(frag_size, 14))
		run("mmd", "-i", image, "::SUB")
		put("/SUB/INNER.TXT", b"inside a folder\r\n")
		# A name too long for 8.3: mtools writes the long entries and a short alias.
		put("/My Long File Name.txt", b"long name\r\n")
		alias = subprocess.run(["mshortname", "-i", image, "::My Long File Name.txt"], check=True, env=env, capture_output=True, text=True).stdout.strip()
		files[alias.split("/")[-1].upper()] = files.pop("My Long File Name.txt".upper())

		with open(path, "rb") as f:
			raw = f.read()
	out = os.path.join(HERE, name)
	with open(out, "wb") as f:
		f.write(gzip.compress(raw, 9, mtime=0))
	return {
		"partitionType": kind,
		"label": label,
		"files": {k: {"size": len(v), "sha256": hashlib.sha256(v).hexdigest()} for k, v in sorted(files.items())},
		"time": "2024-03-05T14:07:10",
	}


def main():
	manifest = {
		"fat12.img.gz": make("fat12.img.gz", 2000, 0x01, "GOLD12", 1, 300 * 1024),
		"fat16.img.gz": make("fat16.img.gz", 6000, 0x04, "GOLD16", 1, 900 * 1024),
	}
	with open(os.path.join(HERE, "manifest.json"), "w") as f:
		json.dump(manifest, f, indent="\t")
		f.write("\n")


if __name__ == "__main__":
	main()
