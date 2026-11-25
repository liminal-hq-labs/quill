#!/usr/bin/env bash
# Runs W32SPIKE.EXE on ReactOS 0.4.16 in QEMU from a private COPY of the golden disk. The program sits in C:\SPIKE and a one-line batch file in the
# All Users Startup folder starts it at logon, so no input is needed. The run is watched by the QEMU monitor's screendump only (no keys, no pointer):
# the window turns green or red (or goes away after an abrupt exit), the script asks the guest to power down and reads SPIKE.LOG back with mtools.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/w32-worker/tools/reactos-run.sh [--out DIR] [--tag NAME] [KEY=VALUE...]
# ROS_GOLDEN is the golden disk (default ~/.cache/reactos/reactos16-golden.img, never written); the copy lives in $HOME/.cache/w32spike-ros
# and is deleted at the end. ROS_WAIT is the seconds to wait for the verdict (default 1500). KVM is used when /dev/kvm is writable.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$PWD"
TAG=""
CFG=()
while [ "$#" -gt 0 ]; do
	case "$1" in
	--out) OUT="$(mkdir -p "$2" && cd "$2" && pwd)"; shift 2 ;;
	--tag) TAG="-$2"; shift 2 ;;
	*=*) CFG+=("$1"); shift ;;
	*) echo "reactos-run: unknown option $1" >&2; exit 2 ;;
	esac
done
GOLDEN="${ROS_GOLDEN:-$HOME/.cache/reactos/reactos16-golden.img}"
[ -f "$GOLDEN" ] || { echo "reactos-run: no golden disk at $GOLDEN" >&2; exit 1; }
[ -f "$HERE/dist/W32SPIKE.EXE" ] || { echo "reactos-run: build first (tools/build.sh)" >&2; exit 1; }
WORK="$HOME/.cache/w32spike-ros"
mkdir -p "$WORK"
DISK="$WORK/disk.img"
SOCK="$WORK/mon.sock"
OFF=$((2048 * 512))
STARTUP="::/Documents and Settings/All Users/Start Menu/Programs/Startup"
FOLDER="::/SPIKE"
QPID=""
cleanup() {
	[ -z "$QPID" ] || ! kill -0 "$QPID" 2> /dev/null || kill "$QPID" 2> /dev/null || true
	rm -f "$DISK" "$SOCK" "$WORK/cur.ppm" "$WORK/cur.png" "$WORK/SPIKE.CFG" "$WORK/SPIKE.BAT"
}
trap cleanup EXIT
mon() { python3 -I -c '
import socket, sys, time
s = socket.socket(socket.AF_UNIX); s.settimeout(15); s.connect(sys.argv[1]); s.recv(4096)
s.sendall((sys.argv[2] + "\n").encode()); time.sleep(0.5)
try: s.recv(4096)
except Exception: pass
' "$SOCK" "$1"; }

cp --sparse=always "$GOLDEN" "$DISK"
chmod u+w "$DISK"
hold=1
for kv in "${CFG[@]}"; do [ "$kv" != exit=abrupt ] || hold=0; done
printf '%s\r\n' "${CFG[@]}" "hold=$hold" > "$WORK/SPIKE.CFG"
printf '@C:\\SPIKE\\W32SPIKE.EXE\r\n' > "$WORK/SPIKE.BAT"
mmd -i "$DISK@@$OFF" "$FOLDER"
mcopy -o -i "$DISK@@$OFF" "$HERE/dist/W32SPIKE.EXE" "$WORK/SPIKE.CFG" "$FOLDER/"
mcopy -o -i "$DISK@@$OFF" "$WORK/SPIKE.BAT" "$STARTUP/"
accel=()
[ -w /dev/kvm ] && accel=(-enable-kvm)
qemu-system-i386 "${accel[@]}" -m 256 -smp 1 -drive "file=$DISK,format=raw,if=ide" -vga std -display none -no-reboot \
	-monitor "unix:$SOCK,server,nowait" > "$WORK/qemu.log" 2>&1 &
QPID=$!
echo "reactos-run: QEMU pid $QPID, copy $DISK"
T0=$(date +%s)
verdict=""
seen_white=0
while kill -0 "$QPID" 2> /dev/null; do
	[ $(($(date +%s) - T0)) -lt "${ROS_WAIT:-1500}" ] || break
	if [ -S "$SOCK" ]; then
		mon "screendump $WORK/cur.ppm" > /dev/null 2>&1 || true
		p="$(convert "$WORK/cur.ppm" -format '%[hex:p{320,300}]' info: 2> /dev/null || true)"
		case "$p" in
		FFFFFF) seen_white=1 ;;
		00C000) [ "$seen_white" = 0 ] || verdict=green ;;
		D00000 | 0000D0) [ "$seen_white" = 0 ] || verdict=red ;;
		"") ;;
		*) [ "$seen_white" = 0 ] || [ "$hold" = 1 ] || verdict=gone ;;
		esac
	fi
	[ -z "$verdict" ] || break
	sleep 10
done
echo "reactos-run: verdict colour: ${verdict:-none} after $(($(date +%s) - T0)) seconds"
[ ! -f "$WORK/cur.ppm" ] || convert "$WORK/cur.ppm" "$OUT/reactos$TAG.png" 2> /dev/null || true
# A clean power-down so ReactOS writes its caches; then the monitor's quit if the guest has not stopped.
mon system_powerdown > /dev/null 2>&1 || true
for _ in $(seq 1 18); do
	kill -0 "$QPID" 2> /dev/null || break
	sleep 5
done
if kill -0 "$QPID" 2> /dev/null; then
	echo "reactos-run: the guest did not power down on its own; the monitor's quit ends QEMU (the log may be missing)"
	mon quit > /dev/null 2>&1 || true
	sleep 3
fi
if mcopy -n -i "$DISK@@$OFF" "$FOLDER/SPIKE.LOG" "$OUT/reactos$TAG.log"; then
	echo "reactos-run: wrote $OUT/reactos$TAG.log (window: ${verdict:-none})"
else
	echo "reactos-run: no SPIKE.LOG on the disk copy (window: ${verdict:-none})" >&2
	exit 1
fi
