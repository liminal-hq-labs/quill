# Drives the spike's private QEMU through its QMP socket only: start, keys, typed text, the Run box and screendumps. Pixels come from
# screendump, input goes through send-key; nothing touches the host desktop.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: ql.py start | keys K... | type TEXT | run COMMAND | shot NAME | cd FILE... | hmp COMMAND | quit | status
# `hmp savevm clean` after the desktop is up and `hmp loadvm clean` between experiments restore the whole machine in about a second.
# Environment: HL_WORK (default ~/.cache/quill-host-spike), HL_SHOTS (where shots go), HL_QMP (default /tmp/hl1-qm.sock)
import json, os, socket, subprocess, sys, time

WORK = os.environ.get("HL_WORK", os.path.expanduser("~/.cache/quill-host-spike"))
SHOTS = os.environ.get("HL_SHOTS", WORK)
QMP = os.environ.get("HL_QMP", "/tmp/hl1-qm.sock")
COM1 = "/tmp/hl1-c1.sock"
COM2 = "/tmp/hl1-c2.sock"
IMAGE = os.path.expanduser("~/.cache/quill-reactos-liminal-work/images/qagrr.img")


def qmp(command, arguments=None):
	conn = socket.socket(socket.AF_UNIX)
	conn.connect(QMP)
	stream = conn.makefile("rw")
	stream.readline()
	reply = None
	for request in ({"execute": "qmp_capabilities"}, {"execute": command, "arguments": arguments or {}}):
		stream.write(json.dumps(request) + "\n")
		stream.flush()
		while True:
			reply = json.loads(stream.readline())
			if "return" in reply or "error" in reply:
				break
	conn.close()
	return reply


def keys(*names, hold=80, pause=0.12):
	qmp("send-key", {"keys": [{"type": "qcode", "data": n} for n in names], "hold-time": hold})
	time.sleep(pause)


PLAIN = {" ": "spc", "\\": "backslash", ".": "dot", "-": "minus", "/": "slash", ";": "semicolon", ",": "comma", "=": "equal", "'": "apostrophe"}
SHIFTED = {":": "semicolon", "_": "minus", "|": "backslash", ">": "dot", "<": "comma", '"': "apostrophe", "+": "equal"}


def type_text(text, pause=0.35):
	for ch in text:
		if ch.isalpha():
			keys(*(["shift", ch.lower()] if ch.isupper() else [ch]), pause=pause)
		elif ch.isdigit():
			keys(ch, pause=pause)
		elif ch in PLAIN:
			keys(PLAIN[ch], pause=pause)
		elif ch in SHIFTED:
			keys("shift", SHIFTED[ch], pause=pause)
		else:
			raise SystemExit(f"ql: cannot type {ch!r}")


def run_box(command):
	keys("meta_l", "r")
	time.sleep(1.5)
	type_text(command)
	time.sleep(0.3)
	keys("ret")


def pointer(x, y):
	"""Moves the usb-tablet to a pixel of the 1024 by 768 screen (the tablet's axes run 0 to 32767)."""
	qmp("input-send-event", {"events": [{"type": "abs", "data": {"axis": "x", "value": int(x * 32767 / 1023)}}, {"type": "abs", "data": {"axis": "y", "value": int(y * 32767 / 767)}}]})


def button(down):
	qmp("input-send-event", {"events": [{"type": "btn", "data": {"button": "left", "down": down}}]})


def drag(x1, y1, x2, y2, steps=12):
	steps = int(steps)
	pointer(x1, y1)
	time.sleep(0.4)
	button(True)
	time.sleep(0.3)
	for i in range(1, steps + 1):
		pointer(x1 + (x2 - x1) * i / steps, y1 + (y2 - y1) * i / steps)
		time.sleep(0.15)
	time.sleep(0.5)
	button(False)


def shot(name):
	ppm = os.path.join(SHOTS, name + ".ppm")
	os.makedirs(SHOTS, exist_ok=True)
	qmp("screendump", {"filename": ppm})
	subprocess.run(["magick", ppm, os.path.join(SHOTS, name + ".png")], check=True)
	os.remove(ppm)


def swap_cd(*files):
	"""Makes an ISO of the files and puts it in the guest's CD drive (E:), so a new H.EXE needs no reboot."""
	stage = os.path.join(WORK, "iso-stage")
	subprocess.run(["rm", "-rf", stage], check=True)
	os.makedirs(stage)
	for f in files:
		subprocess.run(["cp", f, stage], check=True)
	iso = os.path.join(WORK, "hl%d.iso" % int(time.time()))
	subprocess.run(["xorriso", "-as", "mkisofs", "-quiet", "-J", "-V", "HLCD", "-o", iso, stage], check=True, capture_output=True)
	print(qmp("blockdev-change-medium", {"device": "ide1-cd0", "filename": iso, "format": "raw"}))


def start():
	pidfile = os.path.join(WORK, "qemu.pid")
	for sock in (QMP, COM1, COM2):
		if os.path.exists(sock):
			os.remove(sock)
	cmd = ["qemu-system-i386", "-enable-kvm", "-machine", "pc", "-cpu", "pentium3", "-m", "512", "-vga", "std", "-display", "none", "-usb",
		"-device", "usb-tablet", "-nic", "none", "-no-reboot",
		"-drive", f"file={WORK}/disk.qcow2,format=qcow2,if=ide,index=0",
		
		"-serial", f"unix:{COM1},server=on,wait=off", "-serial", f"unix:{COM2},server=on,wait=off",
		"-qmp", f"unix:{QMP},server=on,wait=off", "-pidfile", pidfile, "-daemonize"]
	env = {k: v for k, v in os.environ.items() if k != "WAYLAND_DISPLAY"}
	env["DISPLAY"] = ":99"
	subprocess.run(cmd, check=True, env=env)
	print("qemu pid", open(pidfile).read().strip())


if __name__ == "__main__":
	cmd, args = sys.argv[1], sys.argv[2:]
	if cmd == "start":
		start()
	elif cmd == "drag":
		drag(*[float(a) for a in args])
	elif cmd == "move":
		pointer(float(args[0]), float(args[1]))
	elif cmd == "hmp":
		print(qmp("human-monitor-command", {"command-line": " ".join(args)}))
	elif cmd == "cd":
		swap_cd(*args)
	elif cmd == "keys":
		keys(*args)
	elif cmd == "type":
		type_text(" ".join(args))
	elif cmd == "run":
		run_box(" ".join(args))
	elif cmd == "shot":
		shot(args[0])
	elif cmd == "status":
		print(qmp("query-status"))
	elif cmd == "quit":
		print(qmp("quit"))
	else:
		raise SystemExit("usage")
