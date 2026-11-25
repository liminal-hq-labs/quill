#!/usr/bin/env bash
# Install ReactOS 0.4.16 onto a blank raw disk with no hand step, and leave the golden disk the rest of the pipeline starts from.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: install-golden.sh [OUT_IMG]      (default: $CI_DIR/reactos16-golden-scripted.img)
#
# How: the official install CD is copied and gets guest/unattend.inf written over \I386\UNATTEND.INF (the download itself is not
# touched). Setup reads it, takes the partitioning, format, boot loader, language and computer answers from it,
# and asks nothing. QEMU runs three times on one raw 4 GiB disk, each run ended by the guest itself (-no-reboot turns a guest restart
# into an exit): text-mode setup (copies files, writes the boot loader), the graphical second stage, and the first boot to the desktop,
# where a GuiRunOnce line writes "ci: desktop" to the serial port and powers the guest off. Screenshots from QEMU's monitor are kept
# every minute for diagnosis. A finished image is reused when this script, unattend.inf and the ISO hash are unchanged.
#
# The disk must be RAW. On a qcow2 disk setup fails with "Setup failed to install the FAT32 bootcode on the system partition".
# Needs qemu-system-i386, xorriso, mtools, python3, curl, unzip, ImageMagick (magick or convert). Uses KVM when /dev/kvm is writable.
# Environment: CI_DIR (default ~/.cache/reactos/web/ci), DL_DIR (default ~/.cache/reactos/dl), ISO (an already downloaded ISO),
# ACCEL (kvm or tcg), STAGE_TIMEOUT (seconds per run; default 1800 with KVM, 7200 without), KEEP_DISK=1 (leave the disk after a failure).
set -euo pipefail

# Pinned: the official release, as published on SourceForge.
ISO_ZIP_URL=https://downloads.sourceforge.net/project/reactos/ReactOS/0.4.16/ReactOS-0.4.16-i386.zip
ISO_ZIP_SHA256=e5851510cf5b79ef51a76b8155dd5ef7faca1155f44d0a1d41c35b77ece4ce2a
ISO_NAME=ReactOS-0.4.16-i386.iso
ISO_SHA256=6f15ca6e1edf5527b8489958d16c54f0da0e7948c305d7c89e74687c6cbf6855

HERE=$(cd "$(dirname "$0")" && pwd)
CI_DIR=${CI_DIR:-$HOME/.cache/reactos/web/ci}
DL_DIR=${DL_DIR:-$HOME/.cache/reactos/dl}
OUT=${1:-$CI_DIR/reactos16-golden-scripted.img}
VMDIR=$CI_DIR/install
export VMDIR MTOOLS_SKIP_CHECK=1
mkdir -p "$VMDIR" "$DL_DIR" "$(dirname "$OUT")"

if [ -z "${ACCEL:-}" ]; then
	if [ -w /dev/kvm ]; then ACCEL=kvm; else ACCEL=tcg; fi
fi
if [ "$ACCEL" = kvm ]; then STAGE_TIMEOUT=${STAGE_TIMEOUT:-1800}; else STAGE_TIMEOUT=${STAGE_TIMEOUT:-7200}; fi
log() { echo "[install-golden $(date +%H:%M:%S)] $*"; }
shot() { if command -v magick >/dev/null; then magick "$1" "$2"; else convert "$1" "$2"; fi; }

# 1. The install CD: reuse a local copy whose hash matches, else download the zip and check both hashes.
iso_ok() { [ -f "$1" ] && [ "$(sha256sum "$1" | cut -d' ' -f1)" = "$ISO_SHA256" ]; }
ISO_SRC=
for c in "${ISO:-}" "$DL_DIR/$ISO_NAME" "$HOME/.cache/reactos/r16u/$ISO_NAME"; do
	[ -n "$c" ] && iso_ok "$c" && { ISO_SRC=$c; break; }
done
if [ -z "$ISO_SRC" ]; then
	log "downloading $ISO_ZIP_URL"
	curl -fL --retry 5 --retry-delay 10 -o "$DL_DIR/ReactOS-0.4.16-i386.zip.part" "$ISO_ZIP_URL"
	[ "$(sha256sum "$DL_DIR/ReactOS-0.4.16-i386.zip.part" | cut -d' ' -f1)" = "$ISO_ZIP_SHA256" ] || { echo "zip hash mismatch" >&2; exit 1; }
	unzip -p "$DL_DIR/ReactOS-0.4.16-i386.zip.part" "$ISO_NAME" > "$DL_DIR/$ISO_NAME.part"
	rm -f "$DL_DIR/ReactOS-0.4.16-i386.zip.part"
	mv "$DL_DIR/$ISO_NAME.part" "$DL_DIR/$ISO_NAME"
	iso_ok "$DL_DIR/$ISO_NAME" || { echo "iso hash mismatch" >&2; exit 1; }
	ISO_SRC=$DL_DIR/$ISO_NAME
fi
log "install CD: $ISO_SRC (sha256 $ISO_SHA256)"

# Skip when nothing that shapes the install has changed.
STAMP=$( { echo "$ISO_SHA256"; sha256sum "$HERE/unattend.inf" "$HERE/install-golden.sh" "$HERE/patch-iso.py" | cut -d' ' -f1; } | sha256sum | cut -d' ' -f1)
if [ -f "$OUT" ] && [ -f "$OUT.stamp" ] && [ "$(cat "$OUT.stamp")" = "$STAMP" ]; then
	log "golden disk is current ($OUT, input hash $STAMP); nothing to do"
	exit 0
fi

# 2. The derived CD: a copy of the official one with \I386\UNATTEND.INF and the default entry of FREELDR.INI overwritten in place. Each
# file keeps its size and place (the answers are padded with a comment), so the directories, the boot record and every other file stay
# byte for byte as shipped. A rebuilt image (xorriso -map) did not work: the boot loader could not load the registry hive from it.
DERIVED=$VMDIR/install.iso
rm -f "$DERIVED"
cp --reflink=auto "$ISO_SRC" "$DERIVED"
chmod u+w "$DERIVED"
patch() {   # PATH_ON_DISC MODE ARG...
	local lba size
	read -r lba size < <(xorriso -indev "$ISO_SRC" -find "$1" -exec report_lba 2>&1 | awk -F'[ ,]+' '/^File data lba/ {print $5, $7}')
	python3 -I "$HERE/patch-iso.py" "$DERIVED" "$lba" "$size" "$2" "${@:3}"
}
patch /i386/unattend.inf inf "$HERE/unattend.inf"
# The disc boots the live environment by default; make the text-mode setup the default entry (its menu times out after three seconds).
patch /freeldr.ini default-os Setup
# This release is a debug build, so setup's own kernel starts with the kernel debugger on COM1 and breaks into it on the first exception,
# which hangs the run when nothing is attached (it did, one run in three). Start it without the debugger.
patch /i386/txtsetup.sif sub '/DEBUG /KDSERIAL /DEBUGPORT=COM1 /FIRSTCHANCE' '/NODEBUG'
log "derived CD written: unattend.inf replaced, Setup is the default boot entry"

# 3. A blank raw disk (sparse).
DISK=$OUT.part
rm -f "$DISK" "$OUT.stamp"
truncate -s 4G "$DISK"

ln -sf "$(command -v qemu-system-i386)" "$VMDIR/vmbox"   # a name that a name-based kill elsewhere cannot hit
PID=
cleanup() {
	if [ -n "$PID" ] && kill -0 "$PID" 2>/dev/null; then kill "$PID" 2>/dev/null || true; fi
	[ "${KEEP_DISK:-0}" = 1 ] || rm -f "$DISK"
}
trap cleanup EXIT

# run_stage NAME BOOT_ORDER: one QEMU run to its own exit (the guest restarts or powers off), with screenshots and a timeout.
run_stage() {
	local name=$1 order=$2 t0 now last=0 n=0
	rm -f "$VMDIR/mon.sock" "$VMDIR/vm.pid" "$VMDIR/serial-$name.txt"
	rm -rf "$VMDIR/shots/$name"
	mkdir -p "$VMDIR/shots/$name"
	local accel_args=(-accel "$ACCEL")
	[ "$ACCEL" = kvm ] || accel_args=(-accel "tcg,thread=multi")
	"$VMDIR/vmbox" "${accel_args[@]}" -machine pc -cpu pentium3 -m 768 -vga std -nic none -display none \
		-drive "file=$DISK,format=raw,if=ide,index=0,cache=writeback" \
		-drive "file=$DERIVED,media=cdrom,readonly=on,index=2" -boot "order=$order" \
		-monitor "unix:$VMDIR/mon.sock,server,nowait" -pidfile "$VMDIR/vm.pid" -D "$VMDIR/vm.log" \
		-no-reboot -serial "file:$VMDIR/serial-$name.txt" -daemonize
	PID=$(cat "$VMDIR/vm.pid")
	t0=$(date +%s)
	log "$name: started (pid $PID, $ACCEL, boot $order)"
	while kill -0 "$PID" 2>/dev/null; do
		sleep 5
		now=$(date +%s)
		if [ $((now - last)) -ge 60 ]; then
			last=$now
			n=$((n + 1))
			if echo "screendump $VMDIR/s.ppm" | python3 -I "$HERE/vm/mon.py" >/dev/null 2>&1; then
				shot "$VMDIR/s.ppm" "$VMDIR/shots/$name/$(printf %03d $n).png" || true
			fi
		fi
		if [ $((now - t0)) -gt "$STAGE_TIMEOUT" ]; then
			log "$name: timed out after ${STAGE_TIMEOUT}s"
			echo "screendump $VMDIR/s.ppm" | python3 -I "$HERE/vm/mon.py" >/dev/null 2>&1 && shot "$VMDIR/s.ppm" "$VMDIR/shots/$name/timeout.png" || true
			exit 1
		fi
	done
	PID=
	log "$name: guest ended after $(( $(date +%s) - t0 ))s"
}

D=$DISK@@1048576
# Stage 1: boot the CD; text-mode setup partitions, formats, copies files, writes the boot loader, then restarts.
run_stage text d
mdir -a -i "$D" ::/ReactOS/system32 >/dev/null 2>&1 || { log "text-mode setup left no ReactOS folder; see $VMDIR/shots/text"; exit 1; }
# An unattended install of this (debug-enabled) release writes a boot menu that starts the kernel debugger on COM1 with no menu delay.
# Put back the entry and delay an interactive install writes, so the guest boots normally and COM1 is free for the desktop report.
mtype -i "$D" ::/freeldr.ini | tr -d '\r' | sed -e 's/^DefaultOS=.*/DefaultOS=ReactOS/' -e 's/^TimeOut=.*/TimeOut=3/' | sed 's/$/\r/' > "$VMDIR/freeldr.ini"
mattrib -i "$D" -r -h -s ::/freeldr.ini 2>/dev/null || true
mcopy -i "$D" -o "$VMDIR/freeldr.ini" ::/freeldr.ini
mattrib -i "$D" +h +s ::/freeldr.ini
# Stage 2: boot the new disk; the graphical part of setup registers components, then restarts.
run_stage gui c
# Stage 3: first boot to the desktop; GuiRunOnce reports and powers off.
run_stage desktop c
grep -aq "ci: desktop" "$VMDIR/serial-desktop.txt" || { log "the desktop stage did not report; see $VMDIR/shots/desktop"; exit 1; }

# 4. Check the result and keep it.
mdir -a -i "$D" ::/ReactOS/explorer.exe >/dev/null 2>&1 || { log "explorer.exe is missing from the installed disk"; exit 1; }
mv "$DISK" "$OUT"
DISK=$OUT.gone
echo "$STAMP" > "$OUT.stamp"
rm -f "$DERIVED"
log "golden disk: $OUT ($(du -m "$OUT" | cut -f1) MB allocated); input hash $STAMP"
