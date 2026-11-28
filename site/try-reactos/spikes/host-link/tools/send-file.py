# Sends a file to the guest over a serial socket of the private QEMU, to be received by `H.EXE g NAME` (on COM2), and stays connected for a while
# so that QEMU delivers it all before the connection closes.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: send-file.py SOCKET FILE [LINGER_SECONDS]
import socket, sys, time

sock = socket.socket(socket.AF_UNIX)
sock.connect(sys.argv[1])
data = open(sys.argv[2], "rb").read()
for start in range(0, len(data), 256):
	sock.sendall(data[start:start + 256])
	time.sleep(0.02)
time.sleep(float(sys.argv[3]) if len(sys.argv) > 3 else 6)
print("sent", len(data))
