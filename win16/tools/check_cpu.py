#!/usr/bin/env python3
# Checks that a Win16 executable's code uses only 386 instructions.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: check_cpu.py PROGRAM.EXE
#
# LLVM emits `bswap` (a 486 instruction) for `u32::from_be_bytes` even for an i386 target, and `QUILL.EXE` is documented for a 386:
# a 386 stops it with an "Illegal Instruction" box at the first PNG header it reads. This disassembles every code segment with objdump
# (16-bit code) and fails on the instructions the 486, Pentium and Pentium Pro added: bswap, cmpxchg, xadd, cpuid, rdtsc, cmov and
# the cache and TLB instructions. Needs objdump from GNU binutils.
import os
import re
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ne_dump  # noqa: E402

NEWER = re.compile(r"(bswap|cmpxchg|cmpxchg8b|xadd|cpuid|rdtsc|rdmsr|wrmsr|cmov[a-z]+|invd|wbinvd|invlpg|movbe|popcnt|lzcnt|tzcnt)\b")


def main():
	if len(sys.argv) != 2:
		sys.exit(__doc__)
	data = open(sys.argv[1], "rb").read()
	info = ne_dump.parse(data)
	bad = []
	for s in info["segments"]:
		if s["index"] == info["header"]["autodata"] or s["flags"] & 1:
			continue
		code = data[s["file_off"] : s["file_off"] + s["length"]]
		with tempfile.NamedTemporaryFile(suffix=".bin") as f:
			f.write(code)
			f.flush()
			out = subprocess.run(["objdump", "-D", "-b", "binary", "-m", "i8086", "-M", "intel", f.name], capture_output=True, text=True, check=True).stdout
		for line in out.splitlines():
			m = re.match(r"\s*([0-9a-f]+):\t[0-9a-f ]+\t(?:data32\s+|addr32\s+)*(\S+)", line)
			if m and NEWER.match(m.group(2)):
				bad.append(f"segment {s['index']}: {int(m.group(1), 16):04x}: {m.group(2)} is not a 386 instruction")
	if bad:
		sys.exit("check_cpu: " + sys.argv[1] + ":\n" + "\n".join(bad))
	print(f"check_cpu: ok: {sys.argv[1]}: no instruction newer than the 386")


main()
