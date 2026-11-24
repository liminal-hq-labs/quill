#!/usr/bin/env python3
# Checks that every link, image, script and stylesheet in site/ resolves, that pages carry the basics, and that no file is orphaned.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I scripts/check-site.py [--external]
#
# Local references (href, src, srcset and CSS url()) must name a file under site/, and a #fragment must name an id on the target
# page. Paths starting with /quill/ are read as the Pages project path (only 404.html uses them). Every image needs an alt attribute,
# every page a lang, a title and a meta description, and headings may not skip a level. Files under site/img that nothing references
# are reported. With --external, http(s) links are fetched too (a HEAD request, then GET if HEAD is refused); without it they are
# listed only. Exits 1 on any problem.

import html.parser
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "site")
ROOT = os.path.normpath(ROOT)
PROJECT = "/quill/"


class Page(html.parser.HTMLParser):
	def __init__(self):
		super().__init__(convert_charrefs=True)
		self.refs = []  # (attribute, value, line)
		self.ids = set()
		self.imgs_without_alt = []
		self.headings = []
		self.lang = None
		self.title = False
		self.description = False
		self._in_title = False

	def handle_starttag(self, tag, attrs):
		a = dict(attrs)
		line = self.getpos()[0]
		if "id" in a:
			self.ids.add(a["id"])
		if tag == "html":
			self.lang = a.get("lang")
		if tag == "title":
			self._in_title = True
		if tag == "meta" and a.get("name") == "description" and a.get("content"):
			self.description = True
		if tag == "img" and "alt" not in a:
			self.imgs_without_alt.append(line)
		if re.fullmatch(r"h[1-6]", tag):
			self.headings.append((int(tag[1]), line))
		if tag == "link" and a.get("rel") == "canonical" and a.get("href"):
			# The canonical address is the published one; it is checked like og:url.
			self.refs.append(("og:canonical", a["href"], line))
			return
		for key in ("href", "src"):
			if a.get(key):
				self.refs.append((key, a[key], line))
		if a.get("srcset"):
			for part in a["srcset"].split(","):
				self.refs.append(("srcset", part.strip().split()[0], line))
		if tag == "meta" and a.get("property", "").startswith("og:") and a.get("content", "").startswith("http"):
			if a["property"] in ("og:image", "og:url"):
				self.refs.append((a["property"], a["content"], line))

	def handle_data(self, data):
		if self._in_title and data.strip():
			self.title = True

	def handle_endtag(self, tag):
		if tag == "title":
			self._in_title = False


def local_path(page, url):
	"""Returns the file under site/ a local URL names, or None for an external one."""
	parsed = urllib.parse.urlsplit(url)
	if parsed.scheme or parsed.netloc:
		return None
	path = urllib.parse.unquote(parsed.path)
	if path.startswith(PROJECT):
		target = os.path.join(ROOT, path[len(PROJECT):])
	elif path.startswith("/"):
		target = os.path.join(ROOT, "__outside_project__" + path)
	elif path == "":
		target = page
	else:
		target = os.path.join(os.path.dirname(page), path)
	target = os.path.normpath(target)
	if os.path.isdir(target):
		target = os.path.join(target, "index.html")
	return target


def main():
	external = "--external" in sys.argv[1:]
	problems = []
	used = set()
	parsed = {}
	pages = []
	css = []
	for base, _, files in os.walk(ROOT):
		for name in files:
			full = os.path.join(base, name)
			if name.endswith(".html"):
				pages.append(full)
			elif name.endswith(".css"):
				css.append(full)

	for page in pages:
		p = Page()
		with open(page, encoding="utf-8") as f:
			p.feed(f.read())
		parsed[page] = p

	def rel(path):
		return os.path.relpath(path, ROOT)

	externals = set()
	for page, p in parsed.items():
		where = rel(page)
		if not p.lang:
			problems.append(f"{where}: no lang on <html>")
		if not p.title:
			problems.append(f"{where}: no <title>")
		if not p.description:
			problems.append(f"{where}: no meta description")
		for line in p.imgs_without_alt:
			problems.append(f"{where}:{line}: <img> without alt")
		levels = [lvl for lvl, _ in p.headings]
		if levels and levels[0] != 1:
			problems.append(f"{where}: the first heading is not an h1")
		if levels.count(1) > 1:
			problems.append(f"{where}: more than one h1")
		for (prev, _), (lvl, line) in zip(p.headings, p.headings[1:]):
			if lvl > prev + 1:
				problems.append(f"{where}:{line}: heading jumps from h{prev} to h{lvl}")
		for key, url, line in p.refs:
			if url.startswith(("mailto:", "tel:", "data:")):
				continue
			if key.startswith("og:"):
				# Open Graph and the canonical link need absolute URLs; check that they point into the project and at a real file.
				if url.startswith("https://liminal-hq-labs.github.io/quill/"):
					target = os.path.join(ROOT, url[len("https://liminal-hq-labs.github.io/quill/"):])
					if url.endswith("/"):
						target = os.path.join(target, "index.html")
					if not os.path.isfile(target):
						problems.append(f"{where}:{line}: {key} {url} names no file in site/")
					used.add(os.path.normpath(target))
				else:
					problems.append(f"{where}:{line}: {key} {url} is outside the project site")
				continue
			target = local_path(page, url)
			if target is None:
				externals.add(url)
				continue
			if url.startswith("/") and not url.startswith(PROJECT):
				problems.append(f"{where}:{line}: {url} is root-relative and breaks under {PROJECT}")
				continue
			if url.startswith(PROJECT) and os.path.basename(page) != "404.html":
				problems.append(f"{where}:{line}: {url} is absolute; use a relative URL")
			if not os.path.isfile(target):
				problems.append(f"{where}:{line}: {url} names no file")
				continue
			used.add(target)
			fragment = urllib.parse.urlsplit(url).fragment
			if fragment and target.endswith(".html"):
				ids = parsed[target].ids if target in parsed else set()
				if fragment not in ids:
					problems.append(f"{where}:{line}: {url}: no id {fragment!r} on the target page")

	for sheet in css:
		with open(sheet, encoding="utf-8") as f:
			text = f.read()
		for url in re.findall(r"url\(\s*['\"]?([^'\")]+)['\"]?\s*\)", text):
			if url.startswith("data:"):
				continue
			target = local_path(sheet, url)
			if target is None:
				problems.append(f"{rel(sheet)}: external url({url}); the site must be self-contained")
			elif not os.path.isfile(target):
				problems.append(f"{rel(sheet)}: url({url}) names no file")
			else:
				used.add(target)

	for base, _, files in os.walk(os.path.join(ROOT, "img")):
		for name in files:
			full = os.path.normpath(os.path.join(base, name))
			if full not in used:
				problems.append(f"{rel(full)}: not referenced by any page")

	for url in sorted(externals):
		if not external:
			print(f"external (not fetched): {url}")
			continue
		try:
			req = urllib.request.Request(url, method="HEAD", headers={"User-Agent": "quill-check-site"})
			try:
				urllib.request.urlopen(req, timeout=20)
			except urllib.error.HTTPError as e:
				if e.code not in (403, 405):
					raise
				urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "quill-check-site"}), timeout=20)
			print(f"external ok: {url}")
		except Exception as e:  # noqa: BLE001 - every failure is reported the same way
			problems.append(f"external link failed: {url} ({e})")

	for problem in problems:
		print(f"check-site: {problem}", file=sys.stderr)
	if problems:
		return 1
	print(f"check-site: ok ({len(pages)} pages, {len(used)} local files referenced)")
	return 0


if __name__ == "__main__":
	sys.exit(main())
