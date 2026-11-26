#!/usr/bin/env python3
# Write the build manifest for the demo image: the SHA-256 and size of every output, the pinned inputs and the time each stage took.
# Usage: build-manifest.py DIST OUT.json KEY=VALUE...
#   DIST is the folder of outputs: every file in it is listed, the parts folder as one entry with its own manifest.json.
#   Keys: iso_sha256, iso_zip_sha256, v86_version, quill_commit, quill_dir, unattend_sha256, timings (a "stage=seconds,..." list),
#   hashes (a "name=sha256,..." list of the Quill builds and demo documents), peak_disk_mb.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import hashlib, json, os, sys, time

dist, out = sys.argv[1], sys.argv[2]
kv = dict(a.split("=", 1) for a in sys.argv[3:])


def sha256(path):
	h = hashlib.sha256()
	with open(path, "rb") as f:
		for block in iter(lambda: f.read(1 << 20), b""):
			h.update(block)
	return h.hexdigest()


def pairs(text):
	return {k: v for k, v in (p.split("=", 1) for p in text.split(",") if "=" in p)} if text else {}


files = {}
parts = None
for root, _dirs, names in os.walk(dist):
	for n in sorted(names):
		p = os.path.join(root, n)
		rel = os.path.relpath(p, dist)
		if rel == os.path.basename(out):
			continue
		if rel.startswith("parts" + os.sep) and n != "manifest.json":
			parts = parts or {"files": 0, "bytes": 0}
			parts["files"] += 1
			parts["bytes"] += os.path.getsize(p)
			continue
		files[rel] = {"bytes": os.path.getsize(p), "sha256": sha256(p)}
if parts:
	parts["note"] = "one zstd file per chunk (8 MiB by default; the size is "chunk" in parts/manifest.json); the raw image's SHA-256 and the chunk list are in parts/manifest.json"
doc = {
	"built": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
	"pinned": {
		"reactos": "0.4.16 (tag 0.4.16-release), official i386 install CD",
		"reactos_iso_zip_sha256": kv.get("iso_zip_sha256", ""),
		"reactos_iso_sha256": kv.get("iso_sha256", ""),
		"unattend_inf_sha256": kv.get("unattend_sha256", ""),
		"v86": kv.get("v86_version", ""),
		"quill_commit": kv.get("quill_commit", "unknown"),
		"quill_builds": pairs(kv.get("hashes", "")),
	},
	"timings_seconds": {k: int(v) for k, v in pairs(kv.get("timings", "")).items()},
	"peak_work_disk_mb": int(kv.get("peak_disk_mb", "0") or 0),
	"outputs": files,
	"parts": parts,
}
with open(out, "w") as f:
	json.dump(doc, f, indent="\t")
	f.write("\n")
print(json.dumps({k: v["bytes"] for k, v in files.items()} | ({"parts": parts["bytes"]} if parts else {})))
