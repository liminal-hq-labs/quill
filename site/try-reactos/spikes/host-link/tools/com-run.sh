#!/usr/bin/env bash
# One COM1 experiment on the private QEMU: restores the `clean` snapshot, starts the probe in the guest (`c KIND STYLE N`), runs the matching host
# client against the COM1 socket and prints the guest's log lines from COM2 after it.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [OPTS=-SD] [CHUNK=256 DELAY=0.02 LINGER=20] tools/com-run.sh KIND STYLE N [WAIT_SECONDS]      KIND e|s|o, STYLE w|r|t
set -euo pipefail
cd "$(dirname "$0")/.."
export HL_SHOTS=${HL_SHOTS:-/tmp/hl1-shots}
W=${HL_WORK:-$HOME/.cache/quill-host-spike}
kind=$1 style=$2 count=$3 wait=${4:-20}
[ ! -f "$W/tee1.pid" ] || ! kill -0 "$(cat "$W/tee1.pid")" 2> /dev/null || kill "$(cat "$W/tee1.pid")"
python3 -I tools/ql.py hmp loadvm clean > /dev/null
sleep 2
n=$(wc -l < "$W/out/com2.log")
python3 -I tools/ql.py run "c:\\h.exe c $kind $style $count ${OPTS:-}"
sleep 3
case $kind in
s) python3 -I tools/com-client.py /tmp/hl1-c1.sock sink "$count" 16384 "${LINGER:-25}" "${CHUNK:-4096}" "${DELAY:-0}" ;;
e) python3 -I tools/com-client.py /tmp/hl1-c1.sock echo "$count" ;;
o) python3 -I tools/com-client.py /tmp/hl1-c1.sock source "$count" ;;
esac
sleep "$wait"
tail -n +$((n + 1)) "$W/out/com2.log" | tr -d '\r'
