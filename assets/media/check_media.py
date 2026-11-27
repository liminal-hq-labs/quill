#!/usr/bin/env python3
# Check the print masters: they match what gen_media.py draws now, and no type is under the minimum size.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
#   python3 -I assets/media/check_media.py
#
# Draws every master into a temporary folder and compares it, byte for byte, with assets/media/masters, so a change to the words, the
# version or the drawing that was not regenerated fails. Then reads each master's data-pt values against the 6 pt floor and checks the
# version appears on every floppy label, CD face and tray card. Needs only Python.
import filecmp
import os
import re
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
MASTERS = os.path.join(HERE, "masters")


def listing(folder):
	found = []
	for base, _folders, files in os.walk(folder):
		for name in files:
			found.append(os.path.relpath(os.path.join(base, name), folder))
	return sorted(found)


def main():
	problems = []
	with tempfile.TemporaryDirectory(prefix="quill-masters-") as fresh:
		subprocess.run([sys.executable, "-I", os.path.join(HERE, "gen_media.py"), fresh], check=True, stdout=subprocess.DEVNULL)
		committed, drawn = listing(MASTERS), listing(fresh)
		for name in sorted(set(committed) ^ set(drawn)):
			problems.append("%s: only in %s" % (name, "the repository" if name in committed else "a fresh run"))
		for name in sorted(set(committed) & set(drawn)):
			if not filecmp.cmp(os.path.join(MASTERS, name), os.path.join(fresh, name), shallow=False):
				problems.append("%s: differs from a fresh run of gen_media.py (regenerate and commit)" % name)
	sys.path.insert(0, HERE)
	sys.dont_write_bytecode = True
	import gen_media
	for name in listing(MASTERS):
		text = open(os.path.join(MASTERS, name), encoding="utf-8").read()
		for size in re.findall(r'data-pt="([0-9.]+)"', text):
			if float(size) < gen_media.MIN_POINTS:
				problems.append("%s: type at %s pt" % (name, size))
		needs_version = re.search(r"(floppy-(dos|w31|w95|all)|disc|tray-card)\.svg$", name)
		if needs_version and gen_media.VERSION not in " ".join(re.findall(r'data-text="([^"]*)"', text)):
			problems.append("%s: does not name version %s" % (name, gen_media.VERSION))
	if problems:
		print("\n".join(problems), file=sys.stderr)
		return 1
	print("check_media: %d masters current for Quill %s" % (len(listing(MASTERS)), gen_media.VERSION))
	return 0


if __name__ == "__main__":
	sys.exit(main())
