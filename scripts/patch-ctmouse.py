#!/usr/bin/env python3
# Switches off CuteMouse's speed-dependent acceleration in the Try Quill disk's copy of CTMOUSE.EXE, so a count of motion always moves
# the pointer the same distance.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I scripts/patch-ctmouse.py CTMOUSE.EXE   (scripts/build-try.sh runs this on its copy)
#
# CuteMouse 2.1 beta 4 (GPL-2.0) scales each report of mouse motion by its size: a report of 4 counts or fewer on an axis is scaled by the
# resolution's coefficient (the /R13 of the disk's batch files), and a larger one by that coefficient times min(12, count) / 4, so 12 or
# more counts move the pointer three times as far a count as 4 or fewer. DOSBox joins the calls that arrive in one tick of its clock into
# one report, so the distance a call of the page's moves goes depends on how many others came with it: there is no switch for it, and the
# double-speed threshold of INT 33h function 0013h is not implemented (CTMOUSE.ASM, `doublespeed_13`). The page's pointer needs to know
# how far a count goes (docs/plans/pointer.md, "js-dos units"; SITE-22).
#
# The test is in the source as `cmp ax, 4` then `ja`, where AX is the report's size, in the routine that applies the sensitivity. This
# changes the 4 to 7FFFh, so no report is ever "above" it and every report takes the linear path: the scaled count with the driver's own
# carry of the fraction. Nothing else in the file changes. Quill reads the position through INT 33h as it would from any driver.
#
# The sequence is checked to occur once, so a different build of CuteMouse is refused rather than damaged.

import sys

BEFORE = bytes([0x3D, 0x04, 0x00, 0x77])  # cmp ax, 4 ; ja
AFTER = bytes([0x3D, 0xFF, 0x7F, 0x77])  # cmp ax, 7FFFh ; ja

if len(sys.argv) != 2:
	sys.exit("usage: patch-ctmouse.py CTMOUSE.EXE")
path = sys.argv[1]
with open(path, "rb") as f:
	data = bytearray(f.read())
if len(data) != 5829 or data[:2] != b"MZ":
	sys.exit(f"patch-ctmouse: {path} is not the CuteMouse 2.1 beta 4 executable ({len(data)} bytes)")
if data.count(BEFORE) != 1:
	sys.exit(f"patch-ctmouse: the speed test is in {data.count(BEFORE)} places, not 1; refusing to patch")
at = data.index(BEFORE)
data[at : at + len(BEFORE)] = AFTER
with open(path, "wb") as f:
	f.write(data)
print(f"patch-ctmouse: speed-up switched off at offset {at}")
