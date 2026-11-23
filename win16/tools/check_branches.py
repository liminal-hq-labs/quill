#!/usr/bin/env python3
# Fails when a linked Win16 program holds a jump table: an indirect jump through a table of code addresses.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: check_branches.py PROGRAM.elf
#
# T1 found that a dense `match` compiled to `jmp [ebx*4+table]` sent the program into the middle of an instruction on Windows 3.11, and
# the cause is not known (docs/progress.md). The build turns jump tables off (`-min-jump-table-entries=1000`); this is the check that
# the flag held, so a table cannot sneak back in until the cause is found. Needs objdump.
import re
import subprocess
import sys

dis = subprocess.run(["objdump", "-d", "-M", "i8086,intel", sys.argv[1]], capture_output=True, text=True, check=True).stdout
bad = [l for l in dis.splitlines() if re.search(r"\bjmp\s+(DWORD PTR\s+)?\[[^\]]*\*[48]", l)]
if bad:
    sys.exit("check_branches: jump table in " + sys.argv[1] + ":\n" + "\n".join(bad[:5]))
print("check_branches: ok:", sys.argv[1], "has no jump tables")
