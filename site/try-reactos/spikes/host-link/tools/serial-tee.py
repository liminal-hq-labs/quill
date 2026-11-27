# Connects to one of the private QEMU's serial sockets and appends everything that arrives to a file, until killed. Optionally sends a file or a
# pattern of N bytes first, and logs how long the receiving side took.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: serial-tee.py SOCKET OUTFILE
import socket, sys, time

sock = socket.socket(socket.AF_UNIX)
sock.connect(sys.argv[1])
with open(sys.argv[2], "ab", buffering=0) as out:
	while True:
		data = sock.recv(65536)
		if not data:
			break
		out.write(data)
