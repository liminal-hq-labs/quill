#!/usr/bin/env python3
# Overwrite a file inside an ISO 9660 image in place, keeping its size and its place, so everything else on the disc (the ISO 9660 and UDF
# directories, the boot record, every other file) stays byte for byte as shipped.
# Usage: patch-iso.py ISO LBA SIZE inf FILE          replace the file with FILE's text, padded with a comment line to SIZE bytes
#        patch-iso.py ISO LBA SIZE default-os NAME   in freeldr.ini, make NAME the default boot entry (SIZE stays the same)
#        patch-iso.py ISO LBA SIZE sub OLD NEW       replace the text OLD with NEW, padded with spaces to OLD's length
# LBA and SIZE are the file's start block and size, as `xorriso -find PATH -exec report_lba` reports them.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import sys

iso, lba, size, mode, arg = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4], sys.argv[5]
extra = sys.argv[6:]
with open(iso, "r+b") as f:
	f.seek(lba * 2048)
	old = f.read(size)
	if mode == "inf":
		data = open(arg, "rb").read().replace(b"\r\n", b"\n")
		if not data.endswith(b"\n"):
			data += b"\n"
		if len(data) > size - 2:
			sys.exit("%s is %d bytes; the file on the disc holds %d" % (arg, len(data), size))
		data += b";" + b" " * (size - len(data) - 1)
	elif mode == "default-os":
		# The shorter name is paid for with a blank line, which setup's INI reader skips.
		key = b"DefaultOS="
		i = old.index(key) + len(key)
		j = i
		while old[j : j + 1] not in (b"\r", b"\n"):
			j += 1
		new = arg.encode()
		eol = b"\r\n" if old[j : j + 2] == b"\r\n" else b"\n"
		fill = (j - i) - len(new)
		if fill < 0 or fill % len(eol):
			sys.exit("cannot keep the size of freeldr.ini with %s" % arg)
		data = old[:i] + new + old[j : j + len(eol)] + eol * (fill // len(eol)) + old[j + len(eol) :]
	elif mode == "sub":
		old_text, new_text = arg.encode(), extra[0].encode()
		if old.count(old_text) != 1 or len(new_text) > len(old_text):
			sys.exit("cannot replace %r with %r" % (arg, extra[0]))
		data = old.replace(old_text, new_text + b" " * (len(old_text) - len(new_text)))
	else:
		sys.exit("unknown mode " + mode)
	assert len(data) == size
	f.seek(lba * 2048)
	f.write(data)
