#!/usr/bin/env python3 -I
# Prints the SHA-256 of a PE file with the bytes that change on every link of the same code zeroed: the file header's time stamp, the
# time stamp of each debug directory entry, and the CodeView record's GUID and PDB file name (which carries a hash that differs on every
# link). Two builds of one source in one folder give one digest.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I win9x/tools/pe-digest.py FILE...      (a digest and the file's size per line)
import hashlib
import struct
import sys


def normalised(data: bytes) -> bytes:
	data = bytearray(data)
	pe = struct.unpack_from("<I", data, 0x3C)[0]
	if data[pe : pe + 4] != b"PE\0\0":
		raise SystemExit("pe-digest: not a PE file")
	data[pe + 8 : pe + 12] = bytes(4)
	sections = struct.unpack_from("<H", data, pe + 6)[0]
	optional_size = struct.unpack_from("<H", data, pe + 20)[0]
	optional = pe + 24
	magic = struct.unpack_from("<H", data, optional)[0]
	directories = optional + (96 if magic == 0x10B else 112)
	debug_rva, debug_size = struct.unpack_from("<II", data, directories + 6 * 8)
	table = optional + optional_size

	def to_offset(rva: int) -> int:
		for i in range(sections):
			name, vsize, vaddr, rawsize, rawptr = struct.unpack_from("<8sIIII", data, table + 40 * i)
			if vaddr <= rva < vaddr + max(vsize, rawsize):
				return rva - vaddr + rawptr
		raise SystemExit("pe-digest: a debug entry outside every section")

	if debug_rva and debug_size:
		start = to_offset(debug_rva)
		for entry in range(start, start + debug_size, 28):
			data[entry + 4 : entry + 8] = bytes(4)
			kind, size, _rva, pointer = struct.unpack_from("<IIII", data, entry + 12)
			if kind == 2 and data[pointer : pointer + 4] == b"RSDS":
				end = data.index(b"\0", pointer + 24)
				data[pointer + 4 : pointer + 20] = bytes(16)
				data[pointer + 24 : end] = bytes(end - pointer - 24)
	return bytes(data)


for path in sys.argv[1:]:
	with open(path, "rb") as handle:
		raw = handle.read()
	print(hashlib.sha256(normalised(raw)).hexdigest(), len(raw), path)
