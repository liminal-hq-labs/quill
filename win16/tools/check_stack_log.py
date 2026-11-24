#!/usr/bin/env python3
# Checks the STACK.LOG a Quill play-back leaves on Windows 3.x: the stack must stay clear of the program's data.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: check_stack_log.py STACK.LOG [MIN_MARGIN]   (default 2048 bytes)
#
# The log says `stack used N of M margin K ...`: K is how far the lowest stack pointer sampled in the editor's deepest calls stayed above
# the end of the data and the packer's shadow return stack. Below zero, the stack overwrites the program's statics.
import re
import sys

text = open(sys.argv[1], encoding="ascii", errors="replace").read()
m = re.search(r"stack used (\d+) of (\d+) margin (\d+)", text)
need = int(sys.argv[2]) if len(sys.argv) > 2 else 2048
if not m:
	sys.exit(f"check_stack_log: no stack line in {sys.argv[1]}: {text.strip()!r}")
used, size, margin = map(int, m.groups())
if margin < need:
	sys.exit(f"check_stack_log: only {margin} bytes of stack margin ({used} of {size} used); at least {need} are needed")
print(f"check_stack_log: ok: {used} of {size} bytes used, {margin} to spare")
