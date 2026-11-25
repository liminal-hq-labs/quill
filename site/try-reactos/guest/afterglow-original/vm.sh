#!/usr/bin/env bash
# Start a throwaway QEMU VM on an Afterglow-original disk copy, with its own monitor socket and pidfile in VMDIR (default ~/.cache/reactos/web/ag2-vm).
# Usage: vm.sh start IMAGE [CDISO] | vm.sh stop | vm.sh alive | vm.sh shot OUT.png | vm.sh type TEXT | vm.sh run 'cmd line' | vm.sh key KEYS | vm.sh mon
# The binary is a private symlink named ag2box and "stop" kills only the PID in VMDIR/vm.pid. Other VMs on the machine are never touched.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -eu
export VMDIR=${VMDIR:-$HOME/.cache/reactos/web/ag2-vm}
here=$(cd "$(dirname "$0")" && pwd)
vmtools=$here/../vm
mkdir -p "$VMDIR"
case "$1" in
start)
	if [ -f "$VMDIR/vm.pid" ] && kill -0 "$(cat "$VMDIR/vm.pid")" 2>/dev/null; then
		echo "VM already running (PID $(cat "$VMDIR/vm.pid"))" >&2
		exit 1
	fi
	ln -sf "$(command -v qemu-system-i386)" "$VMDIR/ag2box"
	rm -f "$VMDIR/mon.sock" "$VMDIR/vm.pid" "$VMDIR/serial.txt"
	cd=()
	[ -n "${3:-}" ] && cd=(-drive "file=$3,media=cdrom,readonly=on,index=2")
	setsid "$VMDIR/ag2box" -accel kvm -machine pc -cpu pentium3 -m 256 \
		-drive "file=$2,format=raw,if=ide,index=0,cache=writethrough" "${cd[@]}" \
		-vga std -nic none -display none -monitor "unix:$VMDIR/mon.sock,server,nowait" -pidfile "$VMDIR/vm.pid" \
		-daemonize -D "$VMDIR/vm.log" -no-reboot -serial "file:$VMDIR/serial.txt"
	;;
stop)
	if [ -f "$VMDIR/vm.pid" ]; then
		pid=$(cat "$VMDIR/vm.pid")
		kill "$pid" 2>/dev/null || true
		for _ in $(seq 1 10); do
			kill -0 "$pid" 2>/dev/null || break
			sleep 1
		done
		# A deadlocked QEMU ignores SIGTERM; this is our own PID, so finish it.
		kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null || true
		rm -f "$VMDIR/vm.pid"
	fi
	;;
alive) [ -f "$VMDIR/vm.pid" ] && kill -0 "$(cat "$VMDIR/vm.pid")" 2>/dev/null ;;
shot) "$vmtools/shot.sh" "$2" ;;
type) "$vmtools/typ.sh" "$2" ;;
run) "$vmtools/run.sh" "$2" ;;
key) echo "sendkey $2" | python3 -I "$vmtools/mon.py" >/dev/null ;;
mouse) printf 'mouse_move %s %s\n' "$2" "$3" | python3 -I "$vmtools/mon.py" >/dev/null ;;
click) printf 'mouse_button 1\nmouse_button 0\n' | python3 -I "$vmtools/mon.py" >/dev/null ;;
mon) python3 -I "$vmtools/mon.py" ;;
waitdesk)
	# Wait (up to ${2:-180} s) until the guest shows a 1024 x 768 desktop, then a few seconds more for the shell to settle.
	end=$((SECONDS + ${2:-180}))
	while [ $SECONDS -lt $end ]; do
		"$vmtools/shot.sh" "$VMDIR/wait.png" 2>/dev/null || true
		if [ -f "$VMDIR/wait.png" ] && [ "$(magick identify -format %w "$VMDIR/wait.png" 2>/dev/null)" = 1024 ]; then
			sleep "${3:-15}"
			echo "desktop after $((SECONDS - end + ${2:-180})) s"
			exit 0
		fi
		sleep 3
	done
	echo "no desktop" >&2
	exit 1
	;;
*)
	echo "usage: $0 start|stop|alive|shot|type|run|key|mouse|click|mon|waitdesk" >&2
	exit 2
	;;
esac
