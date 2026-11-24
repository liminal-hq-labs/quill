#!/usr/bin/env python3
# Turns a segment:offset from a General Protection Fault box into the function it points into, from the map `ne_pack.py --map` wrote.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/ne_addr2sym.py PROGRAM.map SEG:OFFSET [SEG:OFFSET ...]      (hexadecimal; Windows shows 0001:11D0)
import sys

entries = []
with open(sys.argv[1]) as f:
    for line in f:
        loc, size, name = line.split(None, 2)
        seg, off = (int(x, 16) for x in loc.split(":"))
        entries.append((seg, off, int(size), name.strip()))
for arg in sys.argv[2:]:
    seg, off = (int(x, 16) for x in arg.split(":"))
    best = None
    for s, o, size, name in entries:
        if s == seg and o <= off and (best is None or o > best[0]):
            best = (o, size, name)
    if best is None:
        print(f"{arg}: no symbol")
    else:
        inside = "" if off < best[0] + max(best[1], 1) else " (past its end)"
        print(f"{arg}: {best[2]} + {off - best[0]:#x}{inside}")
