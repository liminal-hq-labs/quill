#!/usr/bin/env python3
# Stamps the ReactOS page's own files with a version, so a returning visitor gets a changed file at a new address (SITE-14).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I site/try-reactos/tools/stamp-web.py          (restamp index.html and try-reactos.js)
#        python3 -I site/try-reactos/tools/stamp-web.py --check  (the versions are current; nothing is written)
#
# Every `?v=XXXXXXXX` on an href or src in index.html, and on a module specifier in try-reactos.js, host-link.js and sound.js, is set to the first 8 hex of the
# SHA-256 of the file it names. try-reactos.js is stamped first (its own hash covers the versions of its modules), then index.html.
# This is the page-side twin of scripts/stamp-try.py, which does the same for site/try/; the two share no code because the Try Quill
# page's list of files differs and its script has a check in CI of its own.

import hashlib
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PAGE_DIR = os.path.normpath(os.path.join(HERE, ".."))
HTML_REF = re.compile(r'((?:href|src)=")([^"?#]+)\?v=([0-9a-f]*)(")')
JS_REF = re.compile(r'(from\s+")(\.[^"?#]+)\?v=([0-9a-f]*)(")')


def sha256_file(path):
	h = hashlib.sha256()
	with open(path, "rb") as f:
		for block in iter(lambda: f.read(1 << 20), b""):
			h.update(block)
	return h.hexdigest()


def stamp(path, pattern, check):
	"""Set the ?v= of each versioned reference in `path`. Returns the list of stale or unresolvable references."""
	with open(path, encoding="utf-8") as f:
		text = f.read()
	stale = []

	def fix(m):
		target = os.path.normpath(os.path.join(os.path.dirname(path), m.group(2)))
		if not os.path.isfile(target):
			stale.append(f"{os.path.basename(path)}: {m.group(2)}: no such file")
			return m.group(0)
		want = sha256_file(target)[:8]
		if m.group(3) != want:
			stale.append(f"{os.path.basename(path)}: {m.group(2)}: ?v={m.group(3)} should be ?v={want}")
		return m.group(1) + m.group(2) + "?v=" + want + m.group(4)

	new = pattern.sub(fix, text)
	if not check and new != text:
		with open(path, "w", encoding="utf-8") as f:
			f.write(new)
	return stale


def stamp_all(page_dir, check):
	# The modules first, each stamped with the hash of what it imports, then the script that imports them, then the page.
	stale = []
	for name in ("host-link.js", "sound.js"):
		glue = os.path.join(page_dir, name)
		if os.path.isfile(glue):
			stale += stamp(glue, JS_REF, check)
	stale += stamp(os.path.join(page_dir, "try-reactos.js"), JS_REF, check)
	stale += stamp(os.path.join(page_dir, "index.html"), HTML_REF, check)
	return stale


def main():
	page_dir = os.path.normpath(sys.argv[sys.argv.index("--root") + 1]) if "--root" in sys.argv else PAGE_DIR
	check = "--check" in sys.argv
	stale = stamp_all(page_dir, check)
	if check:
		for line in stale:
			print(f"stamp-web: {line}", file=sys.stderr)
		if stale:
			print("stamp-web: run site/try-reactos/tools/stamp-web.py and commit the result", file=sys.stderr)
			return 1
		print("stamp-web: the page's versions are current")
	return 0


if __name__ == "__main__":
	sys.exit(main())
