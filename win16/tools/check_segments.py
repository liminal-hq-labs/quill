#!/usr/bin/env python3
# Decodes every code segment of a packed Win16 program and fails when a relative call or jump does not land on an instruction inside
# its own segment: the safety net for the veneer pass in ne_pack.py.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/check_segments.py PROGRAM.EXE
#
# The packer rewrites every call that crosses segments to go through a veneer, so after packing every near `call`, `jmp` and conditional
# branch must stay in its own segment. This tool does not trust the packer: it reads the file, decodes each segment with objdump
# (16-bit code with 32-bit operands) and checks each branch target. Needs objdump.
import re
import struct
import subprocess
import sys
import tempfile

data = open(sys.argv[1], "rb").read()
ne = struct.unpack_from("<I", data, 0x3C)[0]
nseg = struct.unpack_from("<H", data, ne + 0x1C)[0]
seg_tab = ne + struct.unpack_from("<H", data, ne + 0x22)[0]
shift = struct.unpack_from("<H", data, ne + 0x32)[0]
data_seg = struct.unpack_from("<H", data, ne + 0x0E)[0]
bad = []
total = 0
for n in range(1, nseg + 1):
    sector, length, flags, _alloc = struct.unpack_from("<HHHH", data, seg_tab + 8 * (n - 1))
    if n == data_seg or flags & 1:
        continue
    code = data[sector << shift : (sector << shift) + length]
    with tempfile.NamedTemporaryFile(suffix=".bin") as f:
        f.write(code)
        f.flush()
        dis = subprocess.run(["objdump", "-D", "-b", "binary", "-m", "i8086", "-M", "intel", f.name], capture_output=True, text=True, check=True).stdout
    starts = set()
    branches = []
    for line in dis.splitlines():
        m = re.match(r"\s*([0-9a-f]+):\t([0-9a-f ]+)\t(.*)", line)
        if not m:
            continue
        at = int(m.group(1), 16)
        starts.add(at)
        text = m.group(3)
        m2 = re.match(r"(?:data32\s+|addr32\s+)*(calld?|jmpd?|j[a-z]+|loop[a-z]*)\s+(0x[0-9a-f]+|[0-9a-f]+)\b", text)
        if m2 and "far" not in text and "ds:" not in text and "[" not in text:
            branches.append((at, m2.group(1), int(m2.group(2), 16), text))
    for at, op, target, text in branches:
        total += 1
        if target >= length:
            bad.append(f"segment {n}: {at:04x}: {text} targets {target:#x}, past the end ({length:#x})")
        elif target not in starts:
            bad.append(f"segment {n}: {at:04x}: {text} lands in the middle of an instruction")
if bad:
    sys.exit("check_segments: " + sys.argv[1] + ":\n" + "\n".join(bad[:20]))
print(f"check_segments: ok: {sys.argv[1]}: {total} near branches in {nseg - 1} code segments all land on instructions in their own segment")
