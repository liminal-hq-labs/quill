#!/usr/bin/env python3
# Names the Try Quill bundle by its content, writes the manifest that points at it, and stamps the page's own files with a version.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I scripts/stamp-try.py --bundle TMP.jsdos --out DIR --root SITE_DIR   (scripts/build-try.sh runs this)
#        python3 -I scripts/stamp-try.py --check                                         (the page's versions are current)
#        python3 -I scripts/stamp-try.py                                                 (restamp the page)
#
# The cache-busting scheme (decisions.md, SITE-4) has three parts, and this is the build side of all of them:
#
#   1. The bundle is renamed to quill.<first 12 hex of its SHA-256>.jsdos in DIR; every other quill*.jsdos in DIR is deleted, so DIR
#      holds only what the manifest names.
#   2. DIR/try-manifest.json is written: {"bundle", "sha256", "built", "size", "assets"}. "assets" maps the vendored player's files to
#      a short hash that the page appends as ?v= (the player, its style sheet, and its emulator folder as one).
#   3. In SITE_DIR/try/index.html, every href or src written as `name?v=XXXXXXXX` gets XXXXXXXX set to the first 8 hex of the SHA-256
#      of the file it names, so a changed try.js, try.css or site.css is fetched again on the next visit. The page is committed with the
#      current values; --check (run in CI) fails if they are stale, and a build writes them.
#
# "built" is the time of the build (SOURCE_DATE_EPOCH if set). It is the only part of the manifest that changes when the bundle does not.

import datetime
import hashlib
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "site"))
REF = re.compile(r'((?:href|src)=")([^"?#]+)\?v=([0-9a-f]*)(")')


def sha256_file(path):
	h = hashlib.sha256()
	with open(path, "rb") as f:
		for chunk in iter(lambda: f.read(1 << 20), b""):
			h.update(chunk)
	return h.hexdigest()


def stamp_page(site, check):
	"""Sets the ?v= of each versioned reference in try/index.html; returns the list of stale references."""
	page = os.path.join(site, "try", "index.html")
	with open(page, encoding="utf-8") as f:
		text = f.read()
	stale = []

	def fix(m):
		target = os.path.normpath(os.path.join(os.path.dirname(page), m.group(2)))
		if not os.path.isfile(target):
			stale.append(f"{m.group(2)}: no such file")
			return m.group(0)
		want = sha256_file(target)[:8]
		if m.group(3) != want:
			stale.append(f"{m.group(2)}: ?v={m.group(3)} should be ?v={want}")
		return m.group(1) + m.group(2) + "?v=" + want + m.group(4)

	new = REF.sub(fix, text)
	if not check and new != text:
		with open(page, "w", encoding="utf-8") as f:
			f.write(new)
	return stale


def player_versions(site):
	"""A short hash for the vendored player's script, its style sheet and its emulator folder (named as the page loads them)."""
	base = os.path.join(site, "try", "js-dos")
	out = {}
	for name in ("js-dos.js", "js-dos.css"):
		out[name] = sha256_file(os.path.join(base, name))[:8]
	h = hashlib.sha256()
	emu = os.path.join(base, "emulators")
	for name in sorted(os.listdir(emu)):
		h.update(name.encode() + b"\0" + sha256_file(os.path.join(emu, name)).encode())
	out["emulators"] = h.hexdigest()[:8]
	return out


def arg(name):
	return sys.argv[sys.argv.index(name) + 1] if name in sys.argv else None


def main():
	site = os.path.normpath(arg("--root") or ROOT)
	if "--check" in sys.argv:
		stale = stamp_page(site, True)
		for s in stale:
			print(f"stamp-try: {s}", file=sys.stderr)
		if stale:
			print("stamp-try: run scripts/stamp-try.py (or scripts/build-try.sh) and commit try/index.html", file=sys.stderr)
			return 1
		print("stamp-try: the page's versions are current")
		return 0

	bundle, out = arg("--bundle"), arg("--out")
	if bundle:
		digest = sha256_file(bundle)
		name = f"quill.{digest[:12]}.jsdos"
		os.replace(bundle, os.path.join(out, name))
		# Only what the manifest names stays: an older hashed bundle, and the unhashed one this scheme replaced.
		for other in sorted(os.listdir(out)):
			if other != name and re.fullmatch(r"quill(\.[0-9a-f]{12})?\.jsdos", other):
				os.remove(os.path.join(out, other))
		epoch = int(os.environ.get("SOURCE_DATE_EPOCH", "0")) or None
		built = datetime.datetime.fromtimestamp(epoch, datetime.timezone.utc) if epoch else datetime.datetime.now(datetime.timezone.utc)
		manifest = {
			"bundle": name,
			"sha256": digest,
			"built": built.strftime("%Y-%m-%dT%H:%M:%SZ"),
			"size": os.path.getsize(os.path.join(out, name)),
			"assets": player_versions(site),
		}
		tmp = os.path.join(out, ".try-manifest.tmp")
		with open(tmp, "w", encoding="utf-8") as f:
			json.dump(manifest, f, indent="\t")
			f.write("\n")
		os.replace(tmp, os.path.join(out, "try-manifest.json"))
		print(f"build-try: {os.path.join(out, name)} ({manifest['size']} bytes, sha-256 {digest})")
	stale = stamp_page(site, False)
	for s in stale:
		print(f"stamp-try: stamped {s}")
	return 0


if __name__ == "__main__":
	sys.exit(main())
