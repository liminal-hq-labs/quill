#!/usr/bin/env python3
# The packed-size gate: fails when what goes on a disk is over its budget or leaves the floppy too little free space, and writes the
# per-file packed sizes and each disk's free bytes as a table.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: size-gate.py [check] [--out DIR] [--dist DIR] [--liminst PATH] [--sizes FILE] [--summary FILE]
#        size-gate.py get KEY            print one figure from installer/budgets.toml, such as disk.dos.min_free
#        size-gate.py modules            print "NAME BUDGET" for each module row
#
# Every figure it checks is in installer/budgets.toml, which build-disks.sh and dos/tools/size-report.sh read too. `check` reads
# the images that build-disks.sh wrote (installer/out/*.IMG) and the files stage.sh staged (installer/dist), and checks
#
#   - QUILLDOS.EXE against its budget (and the 480 KB stop),
#   - QUILL.EXE's data segment against its 1,024 free bytes (read from the NE header, as win16/tools/ne_pack.py counts it),
#   - every staged *.DMD against its own row, and refuses one with no row,
#   - each disk's free bytes against its floor (mdir on the image).
#
# Packed sizes come from the installer itself: each file is packed alone with `liminst lpk pack` for the disk's target (the same
# deflate and chunks the disk's SETUP.LPK uses) and charged what it adds to an empty pack. The sum of the program and its modules is
# "program bytes" (DOS-51); the disk's real SETUP.LPK is reported beside it. The table goes to stdout, into SIZES.txt after a marker
# line (the raw-size listing build-disks.sh wrote stays above it) and, when asked, into the job summary as markdown.
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.dirname(HERE)
BUDGETS = os.path.join(PROJECT, "budgets.toml")
MARKER = "== Packed sizes and budgets"
PACK_TARGET = {"dos-x86": "dos", "win16-x86": "win16", "windows-x86": "win32"}


def load_budgets(path=BUDGETS):
	with open(path, "rb") as handle:
		return tomllib.load(handle)


def parse_free(mdir_text):
	"""The free bytes in mtools' `mdir` output, which writes "424 448 bytes free"."""
	found = re.search(r"^\s*([0-9 ]*[0-9]) bytes free", mdir_text, re.MULTILINE)
	if not found:
		raise ValueError("no free-space line in the mdir output")
	return int(found.group(1).replace(" ", ""))


def win16_data_free(exe_bytes):
	"""Bytes left in the automatic data segment of an NE program: 64 KB less its data, its heap and its stack."""
	ne = struct.unpack_from("<I", exe_bytes, 0x3C)[0]
	if exe_bytes[ne : ne + 2] != b"NE":
		raise ValueError("not an NE program")
	heap, stack = struct.unpack_from("<HH", exe_bytes, ne + 0x10)
	data_segment = struct.unpack_from("<H", exe_bytes, ne + 0x0E)[0]
	table = ne + struct.unpack_from("<H", exe_bytes, ne + 0x22)[0]
	_sector, length, _flags, _alloc = struct.unpack_from("<HHHH", exe_bytes, table + 8 * (data_segment - 1))
	return 0x10000 - (length + heap + stack)


def program_bytes(packed, program, modules):
	"""DOS-51: the packed program and its modules, from a map of file name to packed size."""
	return sum(packed[name] for name in [program, *modules] if name in packed)


def judge(budgets, facts):
	"""Returns the failures. `facts` holds the measured figures; a figure that was not measured is not checked."""
	failures = []
	for name, row in budgets.get("executable", {}).items():
		size = facts.get("executable", {}).get(name)
		if size is None:
			continue
		if size > row["budget"]:
			failures.append(f"{name}.EXE is {size:,} bytes, {size - row['budget']:,} over its budget of {row['budget']:,}")
		if size > row.get("stop", size):
			failures.append(f"{name}.EXE is {size:,} bytes, past the stop at {row['stop']:,}: stop and ask")
	data_free = facts.get("win16_data_free")
	floor = budgets.get("win16", {}).get("data_segment", {}).get("min_free")
	if data_free is not None and floor is not None and data_free < floor:
		failures.append(f"QUILL.EXE's data segment has {data_free:,} bytes free, under {floor:,}")
	for name, size in facts.get("modules", {}).items():
		row = budgets.get("module", {}).get(name)
		if row is None:
			failures.append(f"{name} is staged but has no [module.\"{name}\"] budget in installer/budgets.toml")
		elif size > row["budget"]:
			failures.append(f"{name} is {size:,} bytes, {size - row['budget']:,} over its budget of {row['budget']:,}")
	for key, free in facts.get("free", {}).items():
		floor = budgets["disk"][key]["min_free"]
		if free < floor:
			failures.append(f"{budgets['disk'][key]['image']} keeps {free:,} bytes free, under {floor:,}")
	return failures


def run(command, **kwargs):
	return subprocess.run(command, check=True, capture_output=True, text=True, **kwargs).stdout


def liminst_path(given):
	if given:
		return given
	return run(["bash", os.path.join(HERE, "liminst.sh"), "bin"]).strip().splitlines()[-1]


def packed_alone(liminst, target, path, name, work):
	"""What the file adds to a pack made for `target`: the pack of it alone, less the empty pack."""
	folder = tempfile.mkdtemp(dir=work)
	shutil.copyfile(path, os.path.join(folder, name))
	out = os.path.join(folder + ".lpk")
	run([liminst, "lpk", "pack", "--target", target, "--fixed-time", "-o", out, folder])
	empty = os.path.join(work, "empty-" + target)
	if not os.path.exists(empty + ".lpk"):
		os.makedirs(empty)
		run([liminst, "lpk", "pack", "--target", target, "--fixed-time", "-o", empty + ".lpk", empty])
	return os.path.getsize(out) - os.path.getsize(empty + ".lpk")


def measure_disk(key, row, args, liminst, work):
	image = os.path.join(args["out"], row["image"])
	if not os.path.isfile(image):
		raise SystemExit(f"size-gate: {image} is missing; run installer/tools/build-disks.sh")
	free = parse_free(run(["mdir", "-i", image, "::"]))
	unpacked = os.path.join(work, key)
	os.makedirs(unpacked)
	run(["mcopy", "-i", image, "::SETUP.LPK", unpacked + "/"])
	lpk_bytes = os.path.getsize(os.path.join(unpacked, "SETUP.LPK"))
	run([liminst, "lpk", "unpack", os.path.join(unpacked, "SETUP.LPK"), "-o", os.path.join(unpacked, "pack")])
	files = []
	root = os.path.join(unpacked, "pack")
	for folder, _dirs, names in os.walk(root):
		for name in names:
			path = os.path.join(folder, name)
			rel = os.path.relpath(path, root).replace(os.sep, "/")
			if rel == "SETUP.PRJ":
				continue
			files.append((rel, os.path.getsize(path), packed_alone(liminst, PACK_TARGET[row["target"]], path, os.path.basename(rel), work)))
	files.sort()
	return {"free": free, "lpk": lpk_bytes, "files": files}


def render(budgets, facts, disks, failures):
	lines = [MARKER, "(each file packed alone with `liminst lpk pack` for the disk's target, less the empty pack's header)", ""]
	exe = facts.get("executable", {})
	for name, row in budgets.get("executable", {}).items():
		if name in exe:
			lines.append(f"{name}.EXE: {exe[name]:,} of {row['budget']:,} bytes ({row['budget'] - exe[name]:,} left; the stop is {row['stop']:,})")
	if "win16_data_free" in facts:
		floor = budgets["win16"]["data_segment"]["min_free"]
		lines.append(f"QUILL.EXE data segment: {facts['win16_data_free']:,} bytes free (at least {floor:,})")
	for name, size in facts.get("modules", {}).items():
		row = budgets.get("module", {}).get(name, {})
		lines.append(f"{name}: {size:,} bytes of {row.get('budget', 0):,}")
	for key, disk in disks.items():
		row = budgets["disk"][key]
		packed = {name: pk for name, _raw, pk in disk["files"]}
		modules = [name for name in packed if name.endswith(".DMD")]
		lines += ["", f"{row['image']} ({row['target']})", f"  {'file':<24}{'raw':>10}{'packed':>10}"]
		for name, raw, pk in disk["files"]:
			lines.append(f"  {name:<24}{raw:>10,}{pk:>10,}")
		lines.append(f"  program bytes (packed {row['program']}{' and modules' if modules else ''}): {program_bytes(packed, row['program'], modules):,}")
		lines.append(f"  SETUP.LPK on the disk: {disk['lpk']:,}")
		lines.append(f"  free on the disk: {disk['free']:,} bytes, floor {row['min_free']:,} ({disk['free'] - row['min_free']:,} to spare)")
	lines += ["", "FAIL: " + "; ".join(failures) if failures else "all budgets met"]
	return "\n".join(lines) + "\n"


def markdown(budgets, facts, disks, failures):
	lines = ["### Packed sizes and budgets", ""]
	lines += ["| Disk | Program bytes (packed) | SETUP.LPK | Free | Floor | To spare |", "|---|---:|---:|---:|---:|---:|"]
	for key, disk in disks.items():
		row = budgets["disk"][key]
		packed = {name: pk for name, _raw, pk in disk["files"]}
		modules = [name for name in packed if name.endswith(".DMD")]
		lines.append(
			f"| {row['image']} | {program_bytes(packed, row['program'], modules):,} | {disk['lpk']:,} | {disk['free']:,} | {row['min_free']:,} | {disk['free'] - row['min_free']:,} |"
		)
	for key, disk in disks.items():
		lines += ["", f"**{budgets['disk'][key]['image']}**", "", "| File | Raw | Packed |", "|---|---:|---:|"]
		lines += [f"| {name} | {raw:,} | {pk:,} |" for name, raw, pk in disk["files"]]
	exe = facts.get("executable", {})
	lines += ["", "| Executable budget | Size | Budget | Left |", "|---|---:|---:|---:|"]
	for name, row in budgets.get("executable", {}).items():
		if name in exe:
			lines.append(f"| {name}.EXE | {exe[name]:,} | {row['budget']:,} | {row['budget'] - exe[name]:,} |")
	if "win16_data_free" in facts:
		lines.append(f"| QUILL.EXE data segment free | {facts['win16_data_free']:,} | at least {budgets['win16']['data_segment']['min_free']:,} | |")
	lines += ["", ("**Failed:** " + "; ".join(failures)) if failures else "All budgets met."]
	return "\n".join(lines) + "\n"


def check(args):
	budgets = load_budgets()
	dist = args["dist"]
	facts = {"executable": {}, "modules": {}, "free": {}}
	for name, row in budgets.get("executable", {}).items():
		path = os.path.join(dist, row["file"])
		if os.path.isfile(path):
			facts["executable"][name] = os.path.getsize(path)
	win16 = os.path.join(dist, budgets["win16"]["data_segment"]["file"])
	if os.path.isfile(win16):
		with open(win16, "rb") as handle:
			facts["win16_data_free"] = win16_data_free(handle.read())
	dos_dir = os.path.join(dist, "dos-x86")
	if os.path.isdir(dos_dir):
		for name in sorted(os.listdir(dos_dir)):
			if name.upper().endswith(".DMD"):
				facts["modules"][name] = os.path.getsize(os.path.join(dos_dir, name))
	liminst = liminst_path(args["liminst"])
	disks = {}
	with tempfile.TemporaryDirectory(prefix="size-gate.", dir=os.environ.get("QUILL_BUILD_DIR") or None) as work:
		for key, row in budgets["disk"].items():
			disks[key] = measure_disk(key, row, args, liminst, work)
			facts["free"][key] = disks[key]["free"]
	failures = judge(budgets, facts)
	text = render(budgets, facts, disks, failures)
	sys.stdout.write(text)
	sizes = args["sizes"] or os.path.join(args["out"], "SIZES.txt")
	if os.path.isfile(sizes):
		with open(sizes, encoding="utf-8") as handle:
			kept = handle.read().split(MARKER)[0].rstrip("\n") + "\n\n"
	else:
		kept = ""
	with open(sizes, "w", encoding="utf-8") as handle:
		handle.write(kept + text)
	if args["summary"]:
		with open(args["summary"], "a", encoding="utf-8") as handle:
			handle.write(markdown(budgets, facts, disks, failures))
	if failures:
		for failure in failures:
			print(f"size-gate: FAIL: {failure}", file=sys.stderr)
		return 1
	print("size-gate: ok")
	return 0


def lookup(budgets, key):
	node = budgets
	for part in re.findall(r'"[^"]+"|[^.]+', key):
		node = node[part.strip('"')]
	return node


def main(argv):
	command = "check"
	if argv and argv[0] in ("check", "get", "modules"):
		command, argv = argv[0], argv[1:]
	if command == "get":
		print(lookup(load_budgets(), argv[0]))
		return 0
	if command == "modules":
		for name, row in load_budgets().get("module", {}).items():
			print(name, row["budget"])
		return 0
	args = {"out": os.path.join(PROJECT, "out"), "dist": os.path.join(PROJECT, "dist"), "liminst": None, "sizes": None, "summary": None}
	flags = {"--out": "out", "--dist": "dist", "--liminst": "liminst", "--sizes": "sizes", "--summary": "summary"}
	while argv:
		if argv[0] not in flags or len(argv) < 2:
			print("usage: size-gate.py [check] [--out DIR] [--dist DIR] [--liminst PATH] [--sizes FILE] [--summary FILE] | get KEY | modules", file=sys.stderr)
			return 2
		args[flags[argv[0]]] = argv[1]
		argv = argv[2:]
	return check(args)


if __name__ == "__main__":
	sys.exit(main(sys.argv[1:]))
