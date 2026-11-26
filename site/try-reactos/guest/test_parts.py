#!/usr/bin/env python3
"""Tests for the disk parts: make-parts.mjs splits an image at the size asked (8 MiB by default), and the page, the build and the README
agree on that size (no network).

(c) Copyright 2026 Liminal HQ, Scott Morris
SPDX-License-Identifier: Apache-2.0 OR MIT

Run: python3 -I -m unittest discover -s site/try-reactos/guest -p 'test_parts.py'
The split test needs Node 22 or later (zlib's zstd) and is skipped without it.
"""
import json
import os
import re
import shutil
import subprocess
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
TRY = os.path.normpath(os.path.join(HERE, ".."))
MAKE = os.path.join(TRY, "tools", "make-parts.mjs")
MIB = 1 << 20
DEFAULT = 8 * MIB


def read(*parts):
	with open(os.path.join(TRY, *parts), encoding="utf-8") as f:
		return f.read()


def node_has_zstd():
	node = shutil.which("node")
	if not node:
		return False
	try:
		r = subprocess.run([node, "-e", "process.exit(require('node:zlib').zstdCompressSync ? 0 : 1)"], capture_output=True, timeout=30)
	except (OSError, subprocess.TimeoutExpired):
		return False
	return r.returncode == 0


class SplitTest(unittest.TestCase):
	@unittest.skipUnless(node_has_zstd(), "Node 22 or later is not installed")
	def test_default_and_explicit_sizes(self):
		with tempfile.TemporaryDirectory() as tmp:
			img = os.path.join(tmp, "disk.img")
			with open(img, "wb") as f:
				f.write(b"\x01" * 100)
				f.truncate(2 * DEFAULT + MIB)  # two full default parts and a short last one
			for args, chunk in (([], DEFAULT), ([str(MIB)], MIB), ([str(4 * MIB), "3"], 4 * MIB)):
				out = os.path.join(tmp, "out%d" % chunk)
				subprocess.run(["node", MAKE, img, out, *args], check=True, capture_output=True, timeout=120)
				with open(os.path.join(out, "manifest.json")) as f:
					m = json.load(f)
				size = 2 * DEFAULT + MIB
				self.assertEqual((m["size"], m["chunk"]), (size, chunk))
				names = sorted(n for n in os.listdir(out) if n.endswith(".img.zst"))
				self.assertEqual(len(names), -(-size // chunk))
				self.assertEqual(m["parts"], len(names))
				self.assertIn("0-%d.img.zst" % chunk, names)


class AgreeTest(unittest.TestCase):
	"""The size is written in a few places by hand; this keeps them equal."""

	def test_make_parts_default(self):
		self.assertIn("Number(chunkArg || %d)" % DEFAULT, read("tools", "make-parts.mjs"))

	def test_build_all_uses_the_default_size(self):
		self.assertRegex(read("guest", "build-all.sh"), r'make-parts\.mjs" "\$IMG" "\$DIST/parts" %d 19' % DEFAULT)

	def test_page_default(self):
		self.assertIn("const DEFAULT_CHUNK = 8 * 1024 * 1024;", read("try-reactos.js"))
		self.assertIn('placeholder="%d"' % DEFAULT, read("index.html"))

	def test_page_reads_the_manifest_first(self):
		self.assertIn("chunk || manifest.chunk || DEFAULT_CHUNK", read("try-reactos.js"))

	def test_no_stale_one_mib_default(self):
		for rel in (("harness", "desktop-shot.ts"), ("harness", "mouse-axis.ts"), ("tools", "make-parts.mjs"), ("guest", "build-all.sh")):
			self.assertNotRegex(read(*rel), r"1048576", "/".join(rel))
		readme = read("README.md")
		self.assertIsNone(re.search(r"make-parts\.mjs [^\n]* 1048576 19`\.$", readme, re.M), "README step 4 still names 1 MiB")


if __name__ == "__main__":
	unittest.main()
