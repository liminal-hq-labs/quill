#!/usr/bin/env python3
# Builds the FreeDOS floppy image that the browser demo boots: FreeDOS's kernel and shell, then Quill's files, on a 2.88 MB disk.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I scripts/make-freedos-image.py OUT.IMG FD_BOOT.IMG TREE
#
#   FD_BOOT.IMG  FreeDOS 1.3's boot floppy (x86BOOT.img): KERNEL.SYS, COMMAND.COM and the boot sector's code come from it unchanged
#   TREE         a folder whose contents are copied to the root of the disk (AUTOEXEC.BAT, FDCONFIG.SYS, QUILL\ ...)
#
# mtools makes the file system. The boot sector keeps the BIOS parameter block mtools wrote and takes its jump and code from FreeDOS,
# as FreeDOS's own SYS command would. The volume serial number is fixed and every file carries the same time, so the same inputs give
# the same files (folders are stamped when they are made). A floppy is used, not a hard disk, because DOSBox boots one without a
# partition table and loads FreeDOS's boot sector as a BIOS would. Needs mformat, mcopy and mmd from mtools.

import os
import subprocess
import sys

SECTOR = 512
KIB = 2880  # a 2.88 MB floppy: 36 sectors, 2 heads and 80 cylinders
SERIAL = "51495153"  # "QIAS", an arbitrary fixed volume serial number
STAMP = 1767225600  # 2026-01-01 00:00:00 UTC, the time every file carries


def main():
	out, fd_boot, tree = sys.argv[1:4]
	if os.path.exists(out):
		os.remove(out)
	env = dict(os.environ, MTOOLS_SKIP_CHECK="1", TZ="UTC")

	def run(*args):
		subprocess.run(args, check=True, env=env)

	run("mformat", "-C", "-f", str(KIB), "-i", out, "-N", SERIAL, "-v", "QUILL", "::")

	# The FreeDOS boot code replaces mtools': keep the jump and the BIOS parameter block (bytes 3 to 61) and take the rest of the sector.
	with open(fd_boot, "rb") as f:
		fd_sector = f.read(SECTOR)
	with open(out, "r+b") as f:
		ours = bytearray(f.read(SECTOR))
		ours[0:3] = fd_sector[0:3]
		ours[0x3E:0x200] = fd_sector[0x3E:0x200]
		f.seek(0)
		f.write(ours)

	# KERNEL.SYS first, so it sits at the start of the data area, then the shell, then Quill's tree.
	staging = os.path.join(os.path.dirname(os.path.abspath(out)), "freedos-sys")
	os.makedirs(staging, exist_ok=True)
	for src, name in (("::KERNEL.SYS", "KERNEL.SYS"), ("::freedos/bin/command.com", "COMMAND.COM")):
		dest = os.path.join(staging, name)
		run("mcopy", "-n", "-i", fd_boot, src, dest)
		os.utime(dest, (STAMP, STAMP))
		run("mcopy", "-m", "-i", out, dest, "::")

	for root, dirs, files in os.walk(tree):
		dirs.sort()
		rel = os.path.relpath(root, tree)
		if rel != ".":
			run("mmd", "-i", out, "::" + rel.replace(os.sep, "/"))
		for name in sorted(files):
			dest = "::" + (name if rel == "." else rel.replace(os.sep, "/") + "/" + name)
			source = os.path.join(root, name)
			os.utime(source, (STAMP, STAMP))
			run("mcopy", "-m", "-i", out, source, dest)


if __name__ == "__main__":
	main()
