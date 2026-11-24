#!/bin/bash
# Builds HALPROBE.EXE and runs the graphics hardware layer's checks under headless DOSBox Staging.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/hal-probe.sh      checks (what `make dos` runs)
#
# `HALPROBE.EXE` draws one picture in VGA mode 12h and one in VBE 640 x 480 x 8, reads each back from video memory and compares the
# checksum with the same picture computed in memory, and checks the string-move `memcpy` and `memset`. It runs on the machines
# DOSBox Staging offers: `svga_s3` (VBE with a linear frame buffer: every stage passes, status 0), `svga_paradise` (what the old
# `vgaonly` name now means: no VBE, so the VBE stage reports and the status is 2, after mode 12h passed) and `vesa_nolfb` (VBE
# without a linear frame buffer: also status 2). Needs the DOS tools (dos/tools/fetch-tools.sh); exits 0 with a note when they are
# missing, as e2e.sh does.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR="${DOS_VENDOR:-$HERE/vendor}"

if [ ! -f "$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE" ] || [ -z "$(ls "$VENDOR"/dosbox/*/dosbox 2> /dev/null)" ]; then
	echo "hal-probe: DOS tools not present; skipping (run dos/tools/fetch-tools.sh)"
	exit 0
fi
fail() { echo "hal-probe: FAIL: $*" >&2; exit 1; }

(cd "$HERE" && RUSTC_BOOTSTRAP=1 cargo build --release --locked -p hal-probe)
TARGET_DIR="${CARGO_TARGET_DIR:-$HERE/target}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-hal-probe.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
DISK="$WORK/disk"
mkdir -p "$DISK"
"$HERE/tools/elf2go32.py" "$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE" "$TARGET_DIR/dos-elf/release/hal-probe" "$DISK/HALPROBE.EXE"
export DOS_TIMEOUT=120

# run MACHINE EXPECTED_STATUS OUTPUT_FILE: runs every stage on one machine.
run() {
	local rc=0
	DOS_MACHINE="$1" "$HERE/tools/dosrun.sh" "$DISK" 'HALPROBE.EXE' > "$3" || rc=$?
	[ "$rc" = "$2" ] || { cat "$3" >&2; fail "$1: exit status $rc, expected $2"; }
	if grep -q ': false$' "$3"; then cat "$3" >&2; fail "$1: a check reported false"; fi
}

run svga_s3 0 "$WORK/s3.txt"
cat "$WORK/s3.txt"
grep -q 'halprobe: 12h: read-back equals the picture: true' "$WORK/s3.txt" || fail "svga_s3: no mode 12h read-back"
grep -q 'halprobe: vbe 640x480x8: read-back equals the picture: true' "$WORK/s3.txt" || fail "svga_s3: no VBE read-back"
# The checksums are the picture's, so they are the same on every run and every machine that gets as far as the stage.
grep -q 'halprobe: 12h: checksum 0x6d7d6262,' "$WORK/s3.txt" || fail "svga_s3: the mode 12h checksum changed"
grep -q 'halprobe: vbe 640x480x8: checksum 0x8c614730,' "$WORK/s3.txt" || fail "svga_s3: the VBE checksum changed"

for machine in svga_paradise vesa_nolfb; do
	run "$machine" 2 "$WORK/$machine.txt"
	grep -q 'halprobe: 12h: read-back equals the picture: true' "$WORK/$machine.txt" || fail "$machine: no mode 12h read-back"
	grep -q 'halprobe: vbe: no ' "$WORK/$machine.txt" || fail "$machine: the VBE stage did not say it was unavailable"
	echo "hal-probe: $machine: mode 12h passes, VBE unavailable, status 2"
done
echo "hal-probe: ok"
