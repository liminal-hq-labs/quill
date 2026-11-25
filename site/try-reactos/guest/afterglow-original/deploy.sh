#!/usr/bin/env bash
# Put a built afterglow.msstyles on a disk COPY and boot it: shut the test VM down cleanly, copy the style with mtools, optionally
# import afterglow-original.reg inside the guest (apply), boot, and wait for the desktop.
# Usage: [WALLPAPER=smooth.bmp] deploy.sh IMAGE [apply]   (the style is WORKDIR/build/afterglow.msstyles, WORKDIR as in build.sh;
#        the pointers come from cursors/; WALLPAPER, a 1024 x 768 BMP, replaces the stepped wallpaper; REPATCH=1 replaces the
#        pointers in a user32.dll that an earlier run already patched)
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -eu
here=$(cd "$(dirname "$0")" && pwd)
W=${WORKDIR:-$HOME/.cache/reactos/web/afterglow-original}
export VMDIR=${VMDIR:-$HOME/.cache/reactos/web/ag2-vm}
IMG=$1
export MTOOLS_SKIP_CHECK=1
wait_down() {
	for _ in $(seq 1 60); do
		"$here/vm.sh" alive || return 0
		sleep 2
	done
	"$here/vm.sh" stop
}
if "$here/vm.sh" alive; then
	# A clean shutdown from inside the guest first (a hard stop can leave the registry hive dirty), the ACPI button as a fallback.
	"$here/vm.sh" key esc
	"$here/vm.sh" run 'shutdown -s -t 0'
	for _ in $(seq 1 30); do
		"$here/vm.sh" alive || break
		sleep 2
	done
	"$here/vm.sh" alive && echo system_powerdown | "$here/vm.sh" mon >/dev/null
	wait_down
fi
mkdir -p "$VMDIR/cd"
cp "$here/afterglow-original.reg" "$VMDIR/cd/AGORIG.REG"
cp "$here/polish.reg" "$VMDIR/cd/POLISH.REG"
cp "$here/apply.cmd" "$VMDIR/cd/APPLY.CMD"
cp "$here/apply-polish.cmd" "$VMDIR/cd/APOLISH.CMD"
xorriso -as mkisofs -quiet -o "$VMDIR/ag.iso" -V AGCD "$VMDIR/cd"
D="$IMG@@1048576"
mmd -i "$D" ::/ReactOS/Resources/Themes/Afterglow 2>/dev/null || true
mcopy -o -i "$D" "$W/build/afterglow.msstyles" ::/ReactOS/Resources/Themes/Afterglow/afterglow.msstyles
# Pointers: the files for the scheme, and the same images patched into the guest's user32.dll (the system pointers).
mmd -i "$D" ::/ReactOS/Cursors 2>/dev/null || true
mmd -i "$D" ::/ReactOS/Cursors/Afterglow 2>/dev/null || true
for f in "$here"/cursors/afterglow-*.cur "$here"/cursors/afterglow-*.ani; do
	mcopy -o -i "$D" "$f" "::/ReactOS/Cursors/Afterglow/$(basename "$f")"
done
mcopy -n -i "$D" ::/ReactOS/system32/user32.dll "$VMDIR/user32.dll"
rc=0
force=()
[ -n "${REPATCH:-}" ] && force=(--force)
python3 "$here/cursors/patch-user32.py" "${force[@]}" "$VMDIR/user32.dll" "$here/cursors" "$VMDIR/user32-ag.dll" || rc=$?
if [ $rc = 0 ]; then
	mcopy -o -i "$D" "$VMDIR/user32-ag.dll" ::/ReactOS/system32/user32.dll
elif [ $rc != 3 ]; then
	exit $rc
fi
# Smooth wallpaper (polish.reg points the desktop at it) in place of the stepped one.
if [ -n "${WALLPAPER:-}" ]; then
	mcopy -o -i "$D" "$WALLPAPER" ::/ReactOS/Web/Wallpaper/QuillSmooth.bmp
	mdel -i "$D" ::/ReactOS/Web/Wallpaper/Quill.bmp 2>/dev/null || true
fi
if [ "${2:-}" = apply ]; then
	"$here/vm.sh" start "$IMG" "$VMDIR/ag.iso"
	"$here/vm.sh" waitdesk 240 20
	"$here/vm.sh" run 'cmd /c D:\APPLY.CMD'
	wait_down
	cp "$VMDIR/serial.txt" "$VMDIR/apply-serial.txt" 2>/dev/null || true
fi
"$here/vm.sh" start "$IMG" "$VMDIR/ag.iso"
"$here/vm.sh" waitdesk 240 "${SETTLE:-15}"
