# The host end of the spike's COM1 tests on the private QEMU: connects to the COM1 socket and sends or checks the probe's byte pattern
# (byte i is (i * 31 + 7) mod 256). Modes: `sink N` sends N bytes blind; `echo N` sends N bytes with a 16 KiB window and checks the echo;
# `source N` reads N bytes the guest writes and checks them. Prints bytes, seconds and bytes per second as JSON.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: com-client.py SOCKET MODE N [WINDOW] [CREDIT] — for sink: com-client.py SOCKET sink N WINDOW LINGER_SECONDS [CHUNK] [DELAY_SECONDS]
import json, select, socket, sys, time


def pattern(start, count):
	return bytes((((start + i) * 31 + 7) & 0xFF) for i in range(count))


path, mode, total = sys.argv[1], sys.argv[2], int(sys.argv[3])
window = int(sys.argv[4]) if len(sys.argv) > 4 else 16384
sock = socket.socket(socket.AF_UNIX)
sock.connect(path)
started = time.time()
result = {"mode": mode, "bytes": total}
if mode == "sink":
	sent = 0
	step = int(sys.argv[6]) if len(sys.argv) > 6 else 4096
	delay = float(sys.argv[7]) if len(sys.argv) > 7 else 0.0
	while sent < total:
		chunk = pattern(sent, min(step, total - sent))
		sock.sendall(chunk)
		sent += len(chunk)
		if delay:
			time.sleep(delay)
	result["send_seconds"] = round(time.time() - started, 3)
	# QEMU drops what it has not delivered when the client closes, so stay connected while the guest drains its end.
	time.sleep(float(sys.argv[5]) if len(sys.argv) > 5 else 25)
elif mode == "echo":
	sent = got = bad = 0
	first_bad = None
	last_progress = time.time()
	while got < total:
		if sent < total and sent - got < window:
			chunk = pattern(sent, min(4096, total - sent, window - (sent - got)))
			sock.sendall(chunk)
			sent += len(chunk)
		ready, _, _ = select.select([sock], [], [], 0.0 if sent - got < window and sent < total else 1.0)
		if ready:
			data = sock.recv(65536)
			if not data:
				break
			expected = pattern(got, len(data))
			if data != expected:
				for i, (a, b) in enumerate(zip(data, expected)):
					if a != b:
						bad += 1
						first_bad = got + i if first_bad is None else first_bad
			got += len(data)
			last_progress = time.time()
		elif time.time() - last_progress > 60:
			result["stalled_at"] = got
			break
	result.update(sent=sent, echoed=got, bad=bad, first_bad=first_bad, window=window)
	result["seconds"] = round(time.time() - started, 3)
elif mode == "source":
	got = bad = 0
	first_bad = None
	deadline = time.time() + 400
	while got < total and time.time() < deadline:
		ready, _, _ = select.select([sock], [], [], 5.0)
		if not ready:
			continue
		data = sock.recv(65536)
		if not data:
			break
		if got == 0:
			started = time.time()
		expected = pattern(got, len(data))
		if data != expected:
			for i, (a, b) in enumerate(zip(data, expected)):
				if a != b:
					bad += 1
					first_bad = got + i if first_bad is None else first_bad
		got += len(data)
	result.update(received=got, bad=bad, first_bad=first_bad, seconds=round(time.time() - started, 3))
else:
	raise SystemExit("usage")
seconds = result.get("seconds") or result.get("send_seconds") or 1
result["bytes_per_second"] = round((result.get("echoed") or result.get("received") or total) / seconds)
print(json.dumps(result))
