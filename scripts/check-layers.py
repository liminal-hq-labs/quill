#!/usr/bin/env python3
# Checks Quill's layer rules over every workspace: dependency direction, tools, platform crates, no_std and long-names.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

"""Run as `python3 -I scripts/check-layers.py`.

Every crate declares `[package.metadata.quill]` (layer, no_std, alloc, float). The script reads
`cargo metadata --no-deps` for the root workspace and each platform workspace and reports:

  UP     a normal dependency points to a higher layer (V1 editor -> kit, V3 base -> editor or kit)
  ORDER  a dependency points sideways against the order inside a layer
  TOOL   a normal dependency points at a host tool (V4 when a shell does it)
  PLAT   a platform crate (windows*, gtk*, crossterm, rt) is used outside a shell
  LONG   `long-names` is turned on by a crate that is not a shell
  NOSTD  a crate marked `no_std` lacks `#![no_std]`
  ALLOC  `extern crate alloc` appears in a crate not marked `alloc`
  META   a crate has no valid `[package.metadata.quill]`

Violations listed in `scripts/layer-allow.txt` are known and only reported. Any other violation fails
the run (exit 1). Entries that no longer match are listed so the file can shrink. `--strict` ignores the
allow-list, which is what the check becomes once the list is empty.
"""

import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WORKSPACES = ["Cargo.toml", "win16/Cargo.toml", "win9x/Cargo.toml", "dos/Cargo.toml", "linux/Cargo.toml", "winui/Cargo.toml"]
ALLOW = os.path.join(ROOT, "scripts", "layer-allow.txt")

LAYERS = ["base", "editor", "kit", "shell", "tool"]
RANK = {"base": 0, "editor": 1, "kit": 2, "shell": 3}

# Sideways order inside a layer: a crate may use the crates to its left. Crates not listed are not
# ordered, so a new crate does not need an edit here to land.
ORDER = {
	"base": ["quill-base", "quill-heap", "quill-text", "quill-inflate", "quill-image", "quill-settings", "quill-menu", "quill-input", "quill-mouse", "quill-dlg", "quill-spell", "quill-sysfmt"],
	"editor": ["quill-core", "quill-formats", "quill-clip", "quill-app", "quill-flow"],
	"kit": ["quill-dialogs", "quill-strike", "quill-paint", "quill-fonts", "quill-picture", "quill-chrome", "quill-grid", "quill-raster", "quill-tui"],
}

PLATFORM = re.compile(r"^(windows(-.*)?|winapi.*|gtk4?(-.*)?|libadwaita|adw|crossterm|rt)$")

# What the plan calls each class of today's violations: an editor crate using a kit crate is V1, a base crate
# using anything above it is V3, and a shell using a tool is V4.
def label(code, layer, dep_layer):
	if code == "UP" and layer == "editor":
		return "V1"
	if code == "UP" and layer == "base":
		return "V3"
	if code == "TOOL" and layer == "shell":
		return "V4"
	return ""


def metadata(manifest):
	out = subprocess.run(
		["cargo", "metadata", "--format-version", "1", "--no-deps", "--manifest-path", manifest],
		check=True,
		capture_output=True,
		text=True,
	).stdout
	return json.loads(out)


def crate_files(pkg):
	"""Every .rs file under the directories of the package's targets."""
	seen = set()
	for t in pkg["targets"]:
		d = os.path.dirname(t["src_path"])
		for base, _, files in os.walk(d):
			for f in files:
				if f.endswith(".rs"):
					seen.add(os.path.join(base, f))
	return sorted(seen)


def has_no_std(pkg):
	pat = re.compile(r"^\s*#!\[(no_std|cfg_attr\(.*\bno_std\b.*\))\]", re.M)
	for t in pkg["targets"]:
		if "lib" in t["kind"] or "bin" in t["kind"]:
			try:
				with open(t["src_path"], encoding="utf-8") as f:
					if pat.search(f.read()):
						return True
			except OSError:
				pass
	return False


def uses_alloc(pkg):
	"""`extern crate alloc;` outside a `#[cfg(test)]` item."""
	for path in crate_files(pkg):
		lines = open(path, encoding="utf-8", errors="replace").read().splitlines()
		for i, line in enumerate(lines):
			if re.match(r"^\s*extern\s+crate\s+alloc\b", line):
				prev = next((l for l in reversed(lines[:i]) if l.strip()), "")
				if "cfg(test)" not in prev:
					return True
	return False


def rank_in_layer(layer, name):
	order = ORDER.get(layer)
	if order and name in order:
		return order.index(name)
	return None


def main():
	strict = "--strict" in sys.argv[1:]
	pkgs = {}
	manifests = []
	for w in WORKSPACES:
		path = os.path.join(ROOT, w)
		if not os.path.exists(path):
			continue
		manifests.append(w)
		for p in metadata(path)["packages"]:
			pkgs[p["name"]] = p

	violations = {}

	def flag(code, crate, detail, why, tag=""):
		key = f"{code} {crate} {detail}".rstrip()
		violations[key] = (f"{tag} " if tag else "") + f"{code}: {crate} {detail}".rstrip() + f" — {why}"

	meta = {}
	for name, p in pkgs.items():
		m = ((p.get("metadata") or {}).get("quill")) or {}
		if m.get("layer") not in LAYERS or not all(isinstance(m.get(k), bool) for k in ("no_std", "alloc", "float")):
			flag("META", name, "", "needs [package.metadata.quill] with layer, no_std, alloc and float")
			continue
		meta[name] = m

	for name, m in sorted(meta.items()):
		p = pkgs[name]
		layer = m["layer"]
		if m["no_std"] and not has_no_std(p):
			flag("NOSTD", name, "", "marked no_std but has no #![no_std]")
		if not m["alloc"] and uses_alloc(p):
			flag("ALLOC", name, "", "uses `extern crate alloc` but is not marked alloc = true")
		for d in p["dependencies"]:
			if d.get("kind") == "dev":
				continue
			dep = d["name"]
			if PLATFORM.match(dep) and layer != "shell":
				flag("PLAT", name, f"-> {dep}", "a platform crate belongs in a shell")
			if "long-names" in (d.get("features") or []) and layer != "shell" and dep in meta:
				flag("LONG", name, f"-> {dep}", "only a shell may turn on long-names")
			dm = meta.get(dep)
			if dm is None:
				continue
			dl = dm["layer"]
			if dl == "tool":
				if layer != "tool":
					flag("TOOL", name, f"-> {dep}", "no normal dependency may point at a tool", label("TOOL", layer, dl))
				continue
			if layer == "tool":
				continue
			if RANK[dl] > RANK[layer]:
				flag("UP", name, f"-> {dep}", f"{layer} depends on {dl}", label("UP", layer, dl))
			elif dl == layer:
				a, b = rank_in_layer(layer, name), rank_in_layer(layer, dep)
				if a is not None and b is not None and b > a:
					flag("ORDER", name, f"-> {dep}", f"{dep} is to the right of {name} in the {layer} order")

	allowed = {}
	if not strict and os.path.exists(ALLOW):
		for line in open(ALLOW, encoding="utf-8"):
			entry = line.split("#", 1)[0].strip()
			if entry:
				allowed[" ".join(entry.split())] = True

	known = [k for k in sorted(violations) if k in allowed]
	new = [k for k in sorted(violations) if k not in allowed]
	stale = [k for k in allowed if k not in violations]

	print(f"check-layers: {len(meta)} crates in {', '.join(manifests)}")
	for k in known:
		print(f"  known: {violations[k]}")
	for k in stale:
		print(f"  stale allow-list entry (fixed, remove it): {k}")
	if new:
		print("check-layers: new violations:", file=sys.stderr)
		for k in new:
			print(f"  {violations[k]}", file=sys.stderr)
		return 1
	print(f"check-layers: ok ({len(known)} known)")
	return 0


if __name__ == "__main__":
	sys.exit(main())
