#!/usr/bin/env python3
# Send monitor commands (one per stdin line) to the build VM, pausing between them. VMDIR holds mon.sock; D is the pause in seconds.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import os, socket, sys, time

d = float(os.environ.get("D", "0.15"))
s = socket.socket(socket.AF_UNIX)
s.connect(os.path.join(os.environ.get("VMDIR", os.path.expanduser("~/.cache/reactos/web/p2")), "mon.sock"))
s.settimeout(0.3)
try:
	s.recv(4096)
except Exception:
	pass
for line in sys.stdin:
	s.sendall(line.encode())
	time.sleep(d)
	try:
		sys.stdout.write(s.recv(65536).decode(errors="replace"))
	except Exception:
		pass
