#!/usr/bin/env bash
# Start the build VM on a disk image COPY with a CD in drive D:, daemonised, with its own monitor socket, pidfile and serial log.
# Usage: qrun.sh IMAGE CDISO   (VMDIR defaults to ~/.cache/reactos/web/p2). The binary is called vmbox so that a stray name-based kill elsewhere cannot hit it.
# ACCEL is kvm when /dev/kvm is writable, else tcg (about ten times slower; the callers' waits scale with it). Only this VMDIR's own VM is waited for.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -eu
W=${VMDIR:-$HOME/.cache/reactos/web/p2}
mkdir -p "$W"
ln -sf "$(command -v qemu-system-i386)" "$W/vmbox"
if [ -z "${ACCEL:-}" ]; then if [ -w /dev/kvm ]; then ACCEL=kvm; else ACCEL=tcg,thread=multi; fi; fi
[ "$ACCEL" != tcg ] || ACCEL=tcg,thread=multi
while [ -f "$W/vm.pid" ] && kill -0 "$(cat "$W/vm.pid")" 2>/dev/null; do sleep 1; done
rm -f "$W/mon.sock" "$W/vm.pid" "$W/serial.txt"
exec setsid "$W/vmbox" -accel "$ACCEL" -machine pc -cpu pentium3 -m 256 \
	-drive "file=$1,format=raw,if=ide,index=0,cache=writethrough" -drive "file=$2,media=cdrom,readonly=on,index=2" \
	-vga std -nic none -display none -monitor "unix:$W/mon.sock,server,nowait" -pidfile "$W/vm.pid" \
	-daemonize -D "$W/vm.log" -no-reboot -serial "file:$W/serial.txt"
