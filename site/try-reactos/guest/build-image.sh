#!/usr/bin/env bash
# Build the trimmed demo guest from the pristine golden disk, offline and in a build VM, with no hand steps.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: build-image.sh GOLDEN_IMG OUT_IMG QUILL_ISO DEMO_DIR WALLPAPER_BMP
#   GOLDEN_IMG      pristine ReactOS disk (read only; it is copied, never written)
#   OUT_IMG         the working copy to create (an existing file is replaced)
#   QUILL_ISO       CD, or a folder, with QUILL.EXE and QUILL32.EXE (the newest builds)
#   DEMO_DIR        folder with DEMO.QLD, DEMO.RTF, DEMO16.QLD, DEMO16.RTF
#   WALLPAPER_BMP   1024 x 768 BMP from make-wallpaper.sh
# Optional: AFTERGLOW_DIR, the folder prepare-afterglow.sh writes (the Afterglow theme and its Quillified wallpaper); see below.
# Needs mtools, xorriso, QEMU (KVM when available; ACCEL=tcg to force the slow path), python3 and ImageMagick. VMDIR (default ~/.cache/reactos/web/p2) holds
# the VM's socket and serial log. BOOT_WAIT (seconds, default 60 with KVM and 400 without) is the wait for the guest's desktop before the first Run box.
set -eu
GOLDEN=$1; OUT=$2; QISO=$3; DEMO=$4; WALL=$5
HERE=$(cd "$(dirname "$0")" && pwd)
export VMDIR=${VMDIR:-$HOME/.cache/reactos/web/p2}
export MTOOLS_SKIP_CHECK=1
if [ -z "${BOOT_WAIT:-}" ]; then if [ -w /dev/kvm ] && [ "${ACCEL:-kvm}" = kvm ]; then BOOT_WAIT=60; else BOOT_WAIT=400; fi; fi
mkdir -p "$VMDIR/stage" "$VMDIR/cd" "$VMDIR/lnk"
rm -f "$VMDIR/stage/QUILLDOS.EXE" "$VMDIR/stage/QUILL.HLP"
D="$OUT@@1048576"

echo "1. copy the golden disk (sparse)"
rm -f "$OUT"
cp --sparse=always "$GOLDEN" "$OUT"
chmod u+w "$OUT"

echo "2. trim, offline"
for n in 1 2 3; do "$HERE/trim.sh" "$OUT" "$HERE/trim-list-$n.txt" "$VMDIR/trim$n.log"; done

echo "3. put Quill, the demo documents and the wallpaper on the disk, and the two desktop shortcuts"
if [ -d "$QISO" ]; then
	cp "$QISO/QUILL.EXE" "$QISO/QUILL32.EXE" "$VMDIR/stage/"
	for opt in QUILLDOS.EXE QUILL.HLP; do [ ! -f "$QISO/$opt" ] || cp "$QISO/$opt" "$VMDIR/stage/"; done
else
	xorriso -osirrox on -indev "$QISO" -extract /QUILL.EXE "$VMDIR/stage/QUILL.EXE" -extract /QUILL32.EXE "$VMDIR/stage/QUILL32.EXE" >/dev/null 2>&1
fi
chmod u+w "$VMDIR/stage/QUILL.EXE" "$VMDIR/stage/QUILL32.EXE"
mmd -i "$D" ::/Quill
for f in "$VMDIR/stage/QUILL.EXE" "$VMDIR/stage/QUILL32.EXE" "$DEMO/DEMO.QLD" "$DEMO/DEMO.RTF" "$DEMO/DEMO16.QLD" "$DEMO/DEMO16.RTF" "$VMDIR/stage/QUILLDOS.EXE" "$HERE/../demo/QUILL.HLP"; do
	[ -f "$f" ] && mcopy -o -i "$D" "$f" "::/Quill/$(basename "$f")"
done
# A QUILL.HLP built with the executables replaces the committed demo copy.
[ ! -f "$VMDIR/stage/QUILL.HLP" ] || mcopy -o -i "$D" "$VMDIR/stage/QUILL.HLP" ::/Quill/QUILL.HLP
# With the Afterglow theme (below) the wallpaper is the Quillified one it ships with, not WALLPAPER_BMP.
[ -n "${AFTERGLOW_DIR:-}" ] && [ -f "$AFTERGLOW_DIR/afterglow.msstyles" ] && [ -f "$AFTERGLOW_DIR/quill-wallpaper.bmp" ] && WALL=$AFTERGLOW_DIR/quill-wallpaper.bmp
mcopy -i "$D" "$WALL" ::/ReactOS/Web/Wallpaper/Quill.bmp
# The Windows 95 shortcut uses QUILLDESK.ICO, built from the repository's icon art with the detailed master at 32, 48 and 256 px, and the
# Windows 3.x shortcut QUILL31DESK.ICO, the Windows 3.1 icon's flat 16-colour art at 16, 32 and 48 px (make_desk_icon.py, --win31).
python3 -I "$HERE/make_desk_icon.py" "$VMDIR/stage/QUILLDESK.ICO"
mcopy -o -i "$D" "$VMDIR/stage/QUILLDESK.ICO" ::/Quill/QUILLDESK.ICO
python3 -I "$HERE/make_desk_icon.py" --win31 "$VMDIR/stage/QUILL31DESK.ICO"
mcopy -o -i "$D" "$VMDIR/stage/QUILL31DESK.ICO" ::/Quill/QUILL31DESK.ICO
python3 -I "$HERE/make-lnk.py" "$VMDIR/lnk/Quill.lnk" 'C:\Quill\QUILL32.EXE' --args 'C:\Quill\DEMO.QLD' --cwd 'C:\Quill' --desc 'Quill, 32-bit build, opening the demo letter' --icon 'C:\Quill\QUILLDESK.ICO'
python3 -I "$HERE/make-lnk.py" "$VMDIR/lnk/Quill16.lnk" 'C:\Quill\QUILL.EXE' --args 'C:\Quill\DEMO16.QLD' --cwd 'C:\Quill' --desc 'Quill, Windows 3.x build, opening the demo letter. Slow: WineVDM inside the emulator is emulation inside emulation' --show 1 --icon 'C:\Quill\QUILL31DESK.ICO'
U="::/Documents and Settings/Administrator/Desktop"
for f in "Applications Manager" "Command Prompt" "Read Me"; do mdel -i "$D" "$U/$f.lnk"; done
mcopy -i "$D" "$VMDIR/lnk/Quill.lnk" "$U/Quill.lnk"
mcopy -i "$D" "$VMDIR/lnk/Quill16.lnk" "$U/Quill (Windows 3.x, slow).lnk"

echo "4. settings CD, then apply the registry changes inside the guest and let it shut itself down"
cp "$HERE/demo-settings.reg" "$VMDIR/cd/DEMO.REG"
cp "$HERE/apply-settings.cmd" "$VMDIR/cd/APPLY.CMD"
# --- polish (guest/polish): registry files applied last, from the same settings CD; apply-settings.cmd calls POLISH.CMD when it is there ---
cp "$HERE/polish/apply-mouse.cmd" "$VMDIR/cd/POLISH.CMD"
for f in "$HERE"/polish/*.reg; do cp "$f" "$VMDIR/cd/PL_$(basename "$f" .reg | tr a-z A-Z).REG"; done
# --- optional: the Afterglow theme ---
# AFTERGLOW_DIR (default unset) is a folder made by prepare-afterglow.sh from the pinned liminal-hq/afterglow-themes source:
# `afterglow.msstyles`, the `*.reg` files that select it and, optionally, `quill-wallpaper.bmp` (used instead of WALLPAPER_BMP). When it is
# unset or has no `afterglow.msstyles`, this step is skipped and the demo keeps ReactOS's classic look with the Quill wallpaper.
if [ -n "${AFTERGLOW_DIR:-}" ] && [ -f "$AFTERGLOW_DIR/afterglow.msstyles" ]; then
	echo "   Afterglow theme from $AFTERGLOW_DIR"
	mmd -i "$D" ::/ReactOS/Resources/Themes/Afterglow 2>/dev/null || true
	mcopy -o -i "$D" "$AFTERGLOW_DIR/afterglow.msstyles" ::/ReactOS/Resources/Themes/Afterglow/afterglow.msstyles
	# The pointers (prepare-afterglow.sh writes cursors/ and pointers.reg; prepare-pointers.sh writes the patched user32.dll): ReactOS
	# ignores pointer schemes and loads the system pointers from user32.dll, so the patched copy replaces it on this disk only.
	if [ -d "$AFTERGLOW_DIR/cursors" ]; then
		mmd -i "$D" ::/ReactOS/Cursors 2>/dev/null || true
		mmd -i "$D" ::/ReactOS/Cursors/Afterglow 2>/dev/null || true
		for f in "$AFTERGLOW_DIR"/cursors/*; do mcopy -o -i "$D" "$f" "::/ReactOS/Cursors/Afterglow/$(basename "$f")"; done
		echo "   Afterglow pointers: $(find "$AFTERGLOW_DIR/cursors" -type f | wc -l) files"
	fi
	if [ -f "$AFTERGLOW_DIR/user32-afterglow.dll" ]; then
		mcopy -o -i "$D" "$AFTERGLOW_DIR/user32-afterglow.dll" ::/ReactOS/system32/user32.dll
		echo "   patched user32.dll installed (this disk only)"
	fi
	for f in "$AFTERGLOW_DIR"/*.reg; do
		[ -f "$f" ] && cp "$f" "$VMDIR/cd/PL_AG_$(basename "$f" .reg | tr a-z A-Z).REG"
	done
else
	echo "   no Afterglow theme (AFTERGLOW_DIR unset or empty): classic look"
fi
# --- end polish ---
xorriso -as mkisofs -quiet -V SETUP -o "$VMDIR/setup.iso" "$VMDIR/cd" >/dev/null 2>&1
"$HERE/vm/qrun.sh" "$OUT" "$VMDIR/setup.iso"
sleep "$BOOT_WAIT"   # boot to the desktop
for try in 1 2 3 4; do   # the first Run box after a boot is sometimes ignored, so send again until the script reports
	"$HERE/vm/run.sh" 'cmd /c D:\APPLY.CMD'
	sleep $((BOOT_WAIT / 4 > 15 ? BOOT_WAIT / 4 : 15))
	grep -aq "apply: start" "$VMDIR/serial.txt" && break
	echo "  Run box did not take it (try $try), sending again"
done
"$HERE/vm/wait-exit.sh" $((BOOT_WAIT * 3))
grep -a "apply:" "$VMDIR/serial.txt" || { echo "settings were not applied" >&2; exit 1; }
echo "done: $(mdir -i "$D" ::/ | awk '/bytes free/ {print}')"
