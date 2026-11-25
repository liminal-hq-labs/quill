#!/usr/bin/env python3
# Write a minimal Windows shell link (.lnk): a local target path, optional arguments, working folder and description.
# Usage: make-lnk.py OUT.lnk TARGET [--args ARGS] [--cwd DIR] [--desc TEXT] [--show 1|3|7]
# The link carries no ID list, only LinkInfo with the local base path, which the Wine-derived shell32 in ReactOS reads.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import argparse, struct

p = argparse.ArgumentParser()
p.add_argument("out"); p.add_argument("target")
p.add_argument("--args", default=""); p.add_argument("--cwd", default=""); p.add_argument("--desc", default="")
p.add_argument("--show", type=int, default=1)
a = p.parse_args()

HAS_LINKINFO, HAS_NAME, HAS_WORKDIR, HAS_ARGS, HAS_ICON = 0x2, 0x4, 0x10, 0x20, 0x40
flags = HAS_LINKINFO
if a.desc: flags |= HAS_NAME
if a.cwd: flags |= HAS_WORKDIR
if a.args: flags |= HAS_ARGS
flags |= HAS_ICON  # the icon comes from the target itself, as there is no ID list to supply one

header = struct.pack("<I", 0x4C)
header += bytes.fromhex("0114020000000000C000000000000046")  # CLSID 00021401-0000-0000-C000-000000000046
header += struct.pack("<IIQQQ", flags, 0x20, 0, 0, 0)  # attributes (archive), three FILETIMEs
header += struct.pack("<IIIHHII", 0, 0, a.show, 0, 0, 0, 0)  # size, icon index, show command, hotkey, three reserved

base = a.target.encode("ascii") + b"\0"
vol_label = b"\0"
volume_id = struct.pack("<IIII", 16 + len(vol_label), 3, 0, 16) + vol_label  # DRIVE_FIXED, no serial
info_hdr_len = 28
vol_off = info_hdr_len
base_off = vol_off + len(volume_id)
suffix_off = base_off + len(base)
linkinfo = struct.pack("<IIIIIII", suffix_off + 1 + info_hdr_len - info_hdr_len, info_hdr_len, 1, vol_off, base_off, 0, suffix_off) + volume_id + base + b"\0"
linkinfo = struct.pack("<I", len(linkinfo)) + linkinfo[4:]

def counted(s):  # StringData is UTF-16 when the IsUnicode flag is set; this link is ANSI
	b = s.encode("ascii"); return struct.pack("<H", len(b)) + b

out = header + linkinfo
if a.desc: out += counted(a.desc)
if a.cwd: out += counted(a.cwd)
if a.args: out += counted(a.args)
out += counted(a.target)
out += struct.pack("<I", 0)  # terminal block
open(a.out, "wb").write(out)
