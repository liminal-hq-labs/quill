#!/usr/bin/env bash
# Boot a QAGRR disk headless in QEMU, double-click the "Quill (Windows 3.x, slow)" shortcut and check that the Win16 Quill window opens on the
# demo letter, from the guest's own pixels (QMP screendump; guest input only through QMP; no host desktop capture or key injection).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: check-win16.sh IMAGE [SHOTS_DIR]
#   IMAGE       the raw disk (WORK/images/qagrr.img); it is copied first and never booted in place
#   SHOTS_DIR   where the screenshots go (default ./win16-check)
# Environment: RL_QEMU_LOCK (a lock file; QEMU runs under flock on it, as in build-with-rl.sh), BOOT_SECONDS (default 100), LAUNCH_SECONDS (default 150), LOCK_SECONDS (how long to wait for QEMU's lock; default 3600),
#   SCRATCH (where the copy of the disk goes; default ~/.cache/quill-qagrr-check).
# The check is the Win16 window's right frame: the page is 1024 x 768 and the Win16 window (881 px wide, centred) reaches x = 949, past the Win32
# window's right edge at x = 944, so the pixel (949, 300) is the window grey (212, 208, 200) only while the Win16 window is on screen. A crash
# leaves the dark wallpaper there. A second pixel, in the page, must be white. Exit status 0 when both hold.
set -euo pipefail

[ $# -ge 1 ] || { echo "usage: check-win16.sh IMAGE [SHOTS_DIR]" >&2; exit 2; }
image=$1
shots=${2:-./win16-check}
scratch=${SCRATCH:-$HOME/.cache/quill-qagrr-check}
mkdir -p "$shots" "$scratch"
shots=$(cd "$shots" && pwd)
copy=$scratch/check.img
sock=$scratch/qmp.sock
rm -f "$sock"
cp --sparse=always "$image" "$copy"

locked() {
	if [ -n "${RL_QEMU_LOCK:-}" ]; then flock "$RL_QEMU_LOCK" "$@"; else "$@"; fi
}
qemu_pid=
trap '[ -z "$qemu_pid" ] || kill "$qemu_pid" 2>/dev/null || true; rm -f "$copy" "$sock"' EXIT
locked qemu-system-i386 -machine pc -cpu pentium3 -m 768 -vga std -display none -usb -device usb-tablet -nic none \
	-drive "file=$copy,format=raw,if=ide,index=0" -qmp "unix:$sock,server=on,wait=off" $( [ -w /dev/kvm ] && echo -enable-kvm ) &
qemu_pid=$!

python3 -I - "$sock" "$shots" "${BOOT_SECONDS:-100}" "${LAUNCH_SECONDS:-150}" "${LOCK_SECONDS:-3600}" <<'PY'
import json, socket, subprocess, sys, time

sock_path, shots, boot_seconds, launch_seconds, lock_seconds = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5])
WINDOW_GREY = (212, 208, 200)


def qmp(command, arguments=None):
	conn = socket.socket(socket.AF_UNIX)
	conn.connect(sock_path)
	stream = conn.makefile("rw")
	stream.readline()
	for request in ({"execute": "qmp_capabilities"}, {"execute": command, "arguments": arguments or {}}):
		stream.write(json.dumps(request) + "\n")
		stream.flush()
		while True:
			reply = json.loads(stream.readline())
			if "return" in reply or "error" in reply:
				break
	conn.close()
	return reply


def screendump(name):
	"""Takes a screendump, returns its path and a function giving the (r, g, b) at a pixel."""
	ppm = f"{shots}/{name}.ppm"
	qmp("screendump", {"filename": ppm})
	png = f"{shots}/{name}.png"
	subprocess.run(["magick", ppm, png], check=True)
	with open(ppm, "rb") as handle:
		kind, size, _maxval = handle.readline(), handle.readline(), handle.readline()
		width, height = (int(part) for part in size.split())
		data = handle.read()

	def pixel(x, y):
		at = (y * width + x) * 3
		return tuple(data[at:at + 3])

	return png, pixel


def double_click(x, y):
	absolute = [{"type": "abs", "data": {"axis": "x", "value": x * 32767 // 1023}}, {"type": "abs", "data": {"axis": "y", "value": y * 32767 // 767}}]
	qmp("input-send-event", {"events": absolute})
	time.sleep(0.3)
	for _ in range(2):
		qmp("input-send-event", {"events": [{"type": "btn", "data": {"down": True, "button": "left"}}]})
		time.sleep(0.08)
		qmp("input-send-event", {"events": [{"type": "btn", "data": {"down": False, "button": "left"}}]})
		time.sleep(0.12)


# QEMU starts only when the shared lock is free, which can take a while.
connected = False
for _ in range(lock_seconds):
	try:
		qmp("query-status")
		connected = True
		break
	except OSError:
		time.sleep(1)
if not connected:
	sys.exit("QEMU did not start within %d seconds" % lock_seconds)
time.sleep(boot_seconds)
png, pixel = screendump("1-booted")
print("booted:", png)
double_click(44, 50)
deadline = time.time() + launch_seconds
seen = False
while time.time() < deadline:
	time.sleep(10)
	png, pixel = screendump("2-win16")
	if pixel(949, 300) == WINDOW_GREY and pixel(500, 600) == (255, 255, 255):
		seen = True
		break
print("win16 window:", "open" if seen else "NOT open", png)
qmp("quit")
sys.exit(0 if seen else 1)
PY
