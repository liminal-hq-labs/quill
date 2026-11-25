#!/usr/bin/env bash
# Build the trimmed demo guest from the pristine golden disk, offline and in a build VM, with no hand steps.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: build-image.sh GOLDEN_IMG OUT_IMG QUILL_ISO DEMO_DIR WALLPAPER_BMP
#   GOLDEN_IMG      pristine ReactOS disk (read only; it is copied, never written)
#   OUT_IMG         the working copy to create (an existing file is replaced)
#   QUILL_ISO       CD with QUILL.EXE and QUILL32.EXE (the newest builds)
#   DEMO_DIR        folder with DEMO.QLD, DEMO.RTF, DEMO16.QLD, DEMO16.RTF
#   WALLPAPER_BMP   1024 x 768 BMP from make-wallpaper.sh
# Needs mtools, xorriso, QEMU with KVM, python3 and ImageMagick. VMDIR (default ~/.cache/reactos/web/p2) holds the VM's socket and serial log.
set -eu
GOLDEN=$1; OUT=$2; QISO=$3; DEMO=$4; WALL=$5
HERE=$(cd "$(dirname "$0")" && pwd)
export VMDIR=${VMDIR:-$HOME/.cache/reactos/web/p2}
export MTOOLS_SKIP_CHECK=1
mkdir -p "$VMDIR/stage" "$VMDIR/cd" "$VMDIR/lnk"
D="$OUT@@1048576"

echo "1. copy the golden disk (sparse)"
rm -f "$OUT"
cp --sparse=always "$GOLDEN" "$OUT"
chmod u+w "$OUT"

echo "2. trim, offline"
for n in 1 2 3; do "$HERE/trim.sh" "$OUT" "$HERE/trim-list-$n.txt" "$VMDIR/trim$n.log"; done

echo "3. put Quill, the demo documents and the wallpaper on the disk, and the two desktop shortcuts"
xorriso -osirrox on -indev "$QISO" -extract /QUILL.EXE "$VMDIR/stage/QUILL.EXE" -extract /QUILL32.EXE "$VMDIR/stage/QUILL32.EXE" >/dev/null 2>&1
chmod u+w "$VMDIR/stage/QUILL.EXE" "$VMDIR/stage/QUILL32.EXE"
mmd -i "$D" ::/Quill
for f in "$VMDIR/stage/QUILL.EXE" "$VMDIR/stage/QUILL32.EXE" "$DEMO/DEMO.QLD" "$DEMO/DEMO.RTF" "$DEMO/DEMO16.QLD" "$DEMO/DEMO16.RTF" "$HERE/../demo/QUILL.HLP"; do
	[ -f "$f" ] && mcopy -i "$D" "$f" "::/Quill/$(basename "$f")"
done
mcopy -i "$D" "$WALL" ::/ReactOS/Web/Wallpaper/Quill.bmp
python3 -I "$HERE/make-lnk.py" "$VMDIR/lnk/Quill.lnk" 'C:\Quill\QUILL32.EXE' --args 'C:\Quill\DEMO.QLD' --cwd 'C:\Quill' --desc 'Quill, 32-bit build, opening the demo letter'
python3 -I "$HERE/make-lnk.py" "$VMDIR/lnk/Quill16.lnk" 'C:\Quill\QUILL.EXE' --args 'C:\Quill\DEMO16.QLD' --cwd 'C:\Quill' --desc 'Quill, Windows 3.x build, through WineVDM'
U="::/Documents and Settings/Administrator/Desktop"
for f in "Applications Manager" "Command Prompt" "Read Me"; do mdel -i "$D" "$U/$f.lnk"; done
mcopy -i "$D" "$VMDIR/lnk/Quill.lnk" "$U/Quill.lnk"
mcopy -i "$D" "$VMDIR/lnk/Quill16.lnk" "$U/Quill (Windows 3.x build).lnk"

echo "4. settings CD, then apply the registry changes inside the guest and let it shut itself down"
cp "$HERE/demo-settings.reg" "$VMDIR/cd/DEMO.REG"
cp "$HERE/apply-settings.cmd" "$VMDIR/cd/APPLY.CMD"
xorriso -as mkisofs -quiet -V SETUP -o "$VMDIR/setup.iso" "$VMDIR/cd" >/dev/null 2>&1
"$HERE/vm/qrun.sh" "$OUT" "$VMDIR/setup.iso"
sleep 60   # boot to the desktop
for try in 1 2 3 4; do   # the first Run box after a boot is sometimes ignored, so send again until the script reports
	"$HERE/vm/run.sh" 'cmd /c D:\APPLY.CMD'
	sleep 15
	grep -aq "apply: start" "$VMDIR/serial.txt" && break
	echo "  Run box did not take it (try $try), sending again"
done
"$HERE/vm/wait-exit.sh" 120
grep -a "apply:" "$VMDIR/serial.txt" || { echo "settings were not applied" >&2; exit 1; }
echo "done: $(mdir -i "$D" ::/ | awk '/bytes free/ {print}')"
