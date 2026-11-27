#!/usr/bin/env python3
# Stage a build of the demo for the page: the disk parts and the saved state under names made from their content, and the
# web-manifest.json that names them (SITE-14). Nothing is uploaded; the output folder is ignored by git.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I site/try-reactos/tools/stage-web.py DIST [--out DIR] [--v86 DIR]   stage DIST (parts/, state.l22.bin.zst, state.audio.json, build-manifest.json)
#        python3 -I site/try-reactos/tools/stage-web.py --check [--out DIR] [--v86 DIR]   verify what is staged against the manifest
#
# DIST is the folder qagrr/build-with-rl.sh and build-state.sh leave their output in. --out is the page's folder (default: the one
# above this script), where data/ and web-manifest.json go. --v86 is the folder of the emulator files (default: the page's v86/).
#
# Names follow content and nothing else:
#   data/parts-<first 12 hex of the raw image's SHA-256>/     the parts and their manifest.json, copied as they are
#   data/state-<first 12 hex of the state file's SHA-256>.l22.bin.zst
# so the same input gives the same names and the same web-manifest.json (its "built" comes from DIST/build-manifest.json, or from
# SOURCE_DATE_EPOCH). Any other parts-* folder or state-* file in data/ is removed, so data/ holds only what the manifest names.
# The state's sound card: build-state.sh leaves the card's registers beside the state as DIST/state.audio.json, and they go into the manifest as its
# "audio" object (SITE-53). A state built without the card has no such file and no "audio", and the page then boots it without a card.
# "partsDigest" is the SHA-256 of the list of every part's name and SHA-256, which is how --check notices a changed byte in a part
# (the image's own sum cannot be checked without decompressing all of it).

import datetime
import hashlib
import json
import os
import re
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PAGE_DIR = os.path.normpath(os.path.join(HERE, ".."))
MANIFEST_FILE = "web-manifest.json"
STATE_SUFFIX = ".l22.bin.zst"
AUDIO_FILE = "state.audio.json"
# The card's saved registers (packages/ac97 `Ac97State`): 128 codec words and three bus-master channels.
AUDIO_NAM_WORDS = 128
AUDIO_CHANNELS = 3
V86_VERSION_RE = re.compile(r'^version="([0-9.]+)"', re.M)


def sha256_file(path):
	h = hashlib.sha256()
	with open(path, "rb") as f:
		for block in iter(lambda: f.read(1 << 20), b""):
			h.update(block)
	return h.hexdigest()


def read_json(path):
	with open(path, encoding="utf-8") as f:
		return json.load(f)


def audio_problem(audio):
	"""Why `audio` is not the registers of a sound card (the shape packages/ac97 reads), or an empty string."""
	if not isinstance(audio, dict) or audio.get("version") != 1:
		return "it is not a version 1 object"
	nam = audio.get("nam")
	if not isinstance(nam, list) or len(nam) != AUDIO_NAM_WORDS or not all(isinstance(w, int) and 0 <= w <= 0xFFFF for w in nam):
		return f"nam must be {AUDIO_NAM_WORDS} 16-bit words"
	if not isinstance(audio.get("globCnt"), int):
		return "globCnt must be an integer"
	channels = audio.get("ch")
	if not isinstance(channels, list) or len(channels) != AUDIO_CHANNELS or not all(isinstance(c, dict) for c in channels):
		return f"ch must be {AUDIO_CHANNELS} channels"
	return ""


def parts_digest(parts_dir):
	"""SHA-256 over the sorted lines `name sha256` of every file in the parts folder (including its manifest.json)."""
	h = hashlib.sha256()
	for name in sorted(os.listdir(parts_dir)):
		h.update(f"{name} {sha256_file(os.path.join(parts_dir, name))}\n".encode())
	return h.hexdigest()


def v86_info(v86_dir):
	"""The emulator's version (from tools/fetch-v86.sh) and the first 8 hex of the SHA-256 of the files the page loads."""
	with open(os.path.join(HERE, "fetch-v86.sh"), encoding="utf-8") as f:
		match = V86_VERSION_RE.search(f.read())
	version = match.group(1) if match else ""
	return {
		"version": version,
		"libv86": sha256_file(os.path.join(v86_dir, "libv86.mjs"))[:8],
		"wasm": sha256_file(os.path.join(v86_dir, "v86.wasm"))[:8],
	}


def built_time(build):
	if build and build.get("built"):
		return build["built"]
	epoch = os.environ.get("SOURCE_DATE_EPOCH")
	when = datetime.datetime.fromtimestamp(int(epoch), datetime.timezone.utc) if epoch else datetime.datetime.now(datetime.timezone.utc)
	return when.strftime("%Y-%m-%dT%H:%M:%SZ")


def copy_if_different(source, dest):
	if os.path.isfile(dest) and os.path.getsize(dest) == os.path.getsize(source) and sha256_file(dest) == sha256_file(source):
		return
	shutil.copyfile(source, dest)


def stage(dist, out, v86_dir):
	parts_src = os.path.join(dist, "parts")
	state_src = os.path.join(dist, "state" + STATE_SUFFIX)
	for need in (os.path.join(parts_src, "manifest.json"), state_src):
		if not os.path.isfile(need):
			raise SystemExit(f"stage-web: {need} is missing")
	parts_info = read_json(os.path.join(parts_src, "manifest.json"))
	image_sha = parts_info["sha256"]
	state_sha = sha256_file(state_src)
	build_path = os.path.join(dist, "build-manifest.json")
	build = read_json(build_path) if os.path.isfile(build_path) else None
	extra = (build or {}).get("pinned", {}).get("extra", {})

	parts_name = f"data/parts-{image_sha[:12]}/"
	state_name = f"data/state-{state_sha[:12]}{STATE_SUFFIX}"
	data = os.path.join(out, "data")
	os.makedirs(os.path.join(out, parts_name), exist_ok=True)
	for name in sorted(os.listdir(parts_src)):
		copy_if_different(os.path.join(parts_src, name), os.path.join(out, parts_name, name))
	# Only what the manifest names stays.
	for name in sorted(os.listdir(os.path.join(out, parts_name))):
		if not os.path.exists(os.path.join(parts_src, name)):
			os.remove(os.path.join(out, parts_name, name))
	copy_if_different(state_src, os.path.join(out, state_name))
	for other in sorted(os.listdir(data)):
		if other not in (os.path.basename(parts_name.rstrip("/")), os.path.basename(state_name)) and re.fullmatch(r"(parts|state)-[0-9a-f]{12}(\.l22\.bin\.zst)?", other):
			path = os.path.join(data, other)
			shutil.rmtree(path) if os.path.isdir(path) else os.remove(path)

	audio_src = os.path.join(dist, AUDIO_FILE)
	audio = read_json(audio_src) if os.path.isfile(audio_src) else None
	if audio is not None and audio_problem(audio):
		raise SystemExit(f"stage-web: {audio_src} is not a sound card's saved registers: {audio_problem(audio)}")

	manifest = {
		"version": 1,
		"built": built_time(build),
		"parts": parts_name,
		"partsSha256": image_sha,
		"partsDigest": parts_digest(os.path.join(out, parts_name)),
		"imageSize": parts_info["size"],
		"chunk": parts_info["chunk"],
		"state": state_name,
		"stateSha256": state_sha,
		"stateBytes": os.path.getsize(state_src),
		"v86": v86_info(v86_dir),
		**({"audio": audio} if audio is not None else {}),
		"quill": extra.get("quill_commit", ""),
		"reactosLiminal": extra.get("reactos_liminal_commit", ""),
	}
	with open(os.path.join(out, MANIFEST_FILE), "w", encoding="utf-8") as f:
		json.dump(manifest, f, indent="\t")
		f.write("\n")
	return manifest


def check(out, v86_dir):
	"""Returns a list of problems (empty when what is staged is what the manifest says)."""
	path = os.path.join(out, MANIFEST_FILE)
	if not os.path.isfile(path):
		return [f"{path} is missing"]
	m = read_json(path)
	problems = []
	parts_dir = os.path.join(out, m["parts"])
	state = os.path.join(out, m["state"])
	if m["parts"] != f"data/parts-{m['partsSha256'][:12]}/":
		problems.append("the parts folder's name does not follow the image's SHA-256")
	if m["state"] != f"data/state-{m['stateSha256'][:12]}{STATE_SUFFIX}":
		problems.append("the state's name does not follow its SHA-256")
	if not os.path.isdir(parts_dir):
		problems.append(f"{m['parts']} is missing")
	elif parts_digest(parts_dir) != m["partsDigest"]:
		problems.append(f"the parts in {m['parts']} are not the ones the manifest was made from (a changed, missing or extra file)")
	elif read_json(os.path.join(parts_dir, "manifest.json")).get("sha256") != m["partsSha256"]:
		problems.append("the parts' own manifest.json names a different image")
	if not os.path.isfile(state):
		problems.append(f"{m['state']} is missing")
	elif os.path.getsize(state) != m["stateBytes"] or sha256_file(state) != m["stateSha256"]:
		problems.append(f"{m['state']} is not the file the manifest names (its size or SHA-256 differs)")
	if "audio" in m and audio_problem(m["audio"]):
		problems.append(f"the manifest's audio object is not a sound card's saved registers: {audio_problem(m['audio'])}")
	if os.path.isdir(v86_dir) and os.path.isfile(os.path.join(v86_dir, "libv86.mjs")):
		have = v86_info(v86_dir)
		for key in ("libv86", "wasm"):
			if have[key] != m["v86"][key]:
				problems.append(f"the emulator's {key} is {have[key]}, the manifest names {m['v86'][key]}")
	return problems


def arg(name, default):
	return os.path.normpath(sys.argv[sys.argv.index(name) + 1]) if name in sys.argv else default


def main():
	out = arg("--out", PAGE_DIR)
	v86_dir = arg("--v86", os.path.join(PAGE_DIR, "v86"))
	if "--check" in sys.argv:
		problems = check(out, v86_dir)
		for line in problems:
			print(f"stage-web: {line}", file=sys.stderr)
		if not problems:
			print("stage-web: the staged data matches web-manifest.json")
		return 1 if problems else 0
	if len(sys.argv) < 2 or sys.argv[1].startswith("--"):
		print("usage: stage-web.py DIST [--out DIR] [--v86 DIR] | --check [--out DIR] [--v86 DIR]", file=sys.stderr)
		return 2
	manifest = stage(os.path.normpath(sys.argv[1]), out, v86_dir)
	print(json.dumps({"parts": manifest["parts"], "state": manifest["state"], "built": manifest["built"]}))
	return 0


if __name__ == "__main__":
	sys.exit(main())
