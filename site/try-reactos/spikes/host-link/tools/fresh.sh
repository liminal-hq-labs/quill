#!/usr/bin/env bash
# Boots the QAGRR image in the spike's private QEMU from a private qcow2 copy, with H.EXE on C:\, starts the two serial
# listeners (COM1 to out/com1.log, COM2 to out/com2.log) and saves a snapshot called `clean` at the desktop, so `ql.py hmp loadvm clean` brings
# the same desktop back in about a second. Stops the previous QEMU first, by its own PID file.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -euo pipefail
cd "$(dirname "$0")/.."
export HL_SHOTS=${HL_SHOTS:-/tmp/hl1-shots}
W=${HL_WORK:-$HOME/.cache/quill-host-spike}
mkdir -p "$W/out" "$HL_SHOTS"
if [ -f "$W/qemu.pid" ] && kill -0 "$(cat "$W/qemu.pid")" 2> /dev/null; then
	kill "$(cat "$W/qemu.pid")"
	sleep 2
fi
for pidfile in tee1.pid tee2.pid; do
	[ ! -f "$W/$pidfile" ] || ! kill -0 "$(cat "$W/$pidfile")" 2> /dev/null || kill "$(cat "$W/$pidfile")"
done
# The disk: a private qcow2 made from the read-only image with H.EXE and the files named in EXTRA copied onto C:\ (the partition starts at sector 2048).
# qcow2 so that savevm works; the image itself is never written.
rm -f "$W/disk.raw" "$W/disk.qcow2" "$W/overlay.qcow2" "$W/out/com1.log" "$W/out/com2.log"
cp --sparse=always "$HOME/.cache/quill-reactos-liminal-work/images/qagrr.img" "$W/disk.raw"
mcopy -i "$W/disk.raw@@1048576" probe/dist/H.EXE ::/H.EXE
for extra in ${EXTRA:-}; do mcopy -i "$W/disk.raw@@1048576" "$extra" "::/$(basename "$extra")"; done
qemu-img convert -O qcow2 "$W/disk.raw" "$W/disk.qcow2"
rm -f "$W/disk.raw"
python3 -I tools/ql.py start
(nohup python3 -I tools/serial-tee.py /tmp/hl1-c2.sock "$W/out/com2.log" > /dev/null 2>&1 & echo $! > "$W/tee2.pid")
(nohup python3 -I tools/serial-tee.py /tmp/hl1-c1.sock "$W/out/com1.log" > /dev/null 2>&1 & echo $! > "$W/tee1.pid")
sleep "${BOOT_SECONDS:-88}"
python3 -I tools/ql.py shot desktop-clean
python3 -I tools/ql.py hmp savevm clean
echo "fresh: snapshot clean saved"
