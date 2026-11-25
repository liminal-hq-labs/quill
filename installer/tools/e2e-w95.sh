#!/usr/bin/env bash
# Installs Quill from the Windows 95 floppy image onto Windows 95 OSR2.5 under DOSBox-X, looks at what the install left, starts Quill
# from the Start menu, then uninstalls it in the same boot.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [SHOTS=DIR] [W95_BASE_KIND=ie4] installer/tools/e2e-w95.sh
#
# Needs installer/out/QUILL-W95.IMG (build-disks.sh), mtools, Docker, Xvfb, xdotool, ImageMagick and Liminal Installer's Windows 95 test
# bed scripts (`scripts/win95/lib.sh` and `mouse.sh`, from $LIMINAL_INSTALLER or the checkout `liminst.sh` pins). The test bed's base
# image (win95-base.img, built once from the owner's CD; never written) is looked for in $W95_BASE_DIR (default
# /mnt/games/media/win95-testbed) and linked into a test bed folder of this script's own ($QUILL_W95_HOME, default
# ~/.cache/quill16-w95), so the guest, its Xvfb display and its container (quill16-install-w95) are separate from the play harness's.
# It exits 0 with a note when the base image is missing. W95_BASE_KIND=ie4 uses the Internet Explorer 4 base instead of the plain one.
#
# One boot, one throwaway copy of the base. The disk's files and a few helper files are on D:, which the guest also writes and the host
# reads back after shutdown. WIN.INI's run= line starts D:\SETUP.EXE at logon; the wizard is stepped with Return (its default button
# is Next) and one mouse click, which accepts the licence (the wizard's radio buttons take no keys). Then, still in that boot:
#
#   1. the files in C:\Program Files\Quill are copied to D: and compared with installer/dist/
#   2. Start > Programs > Quill is opened, and Add/Remove Programs (`control appwiz.cpl`) is shown
#   3. `regedit /e` writes the `.qld` keys, its ProgID and the Uninstall entry to D:, and the host reads them back
#   4. Quill is started from the Start menu with a sample, then closed
#   5. `UNINST.EXE /U` removes the install; the keys, the folder and the Start menu entry are gone
#
# SHOTS=<dir> saves install-w95-*.png there.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="$HERE/dist"
IMG="$HERE/out/QUILL-W95.IMG"
PIN="$(tr -d '[:space:]' <"$HERE/liminst.rev")"
INSTALLER="${LIMINAL_INSTALLER:-$HOME/.cache/quill16-liminst/$PIN}"
BASE_DIR="${W95_BASE_DIR:-/mnt/games/media/win95-testbed}"
case "${W95_BASE_KIND:-plain}" in
plain) BASE_NAME=win95-base.img ;;
ie4) BASE_NAME=win95-ie4-base.img ;;
*) echo "e2e-w95: W95_BASE_KIND takes plain or ie4" >&2 && exit 2 ;;
esac

if [ ! -f "$BASE_DIR/$BASE_NAME" ]; then
	echo "e2e-w95: no Windows 95 base image at $BASE_DIR/$BASE_NAME; skipping (set W95_BASE_DIR)"
	exit 0
fi
[ -f "$INSTALLER/scripts/win95/lib.sh" ] || { echo "e2e-w95: no scripts/win95/lib.sh in $INSTALLER; set LIMINAL_INSTALLER" >&2 && exit 1; }
[ -f "$IMG" ] || { echo "e2e-w95: $IMG is missing; run installer/tools/build-disks.sh" >&2 && exit 1; }

# The test bed folder, display and container are this script's own.
export W95_HOME="${QUILL_W95_HOME:-$HOME/.cache/quill16-w95}"
export W95_CONTAINER="${QUILL_W95_CONTAINER:-quill16-install-w95}"
if [ -z "${QUILL_W95_DISPLAY:-}" ]; then
	n=97
	while [ -e "/tmp/.X11-unix/X$n" ] || [ -e "/tmp/.X$n-lock" ]; do n=$((n + 1)); done
	QUILL_W95_DISPLAY=":$n"
fi
export W95_DISPLAY="$QUILL_W95_DISPLAY"
mkdir -p "$W95_HOME"
ln -sf "$BASE_DIR/$BASE_NAME" "$W95_HOME/$BASE_NAME"
# shellcheck source=/dev/null
. "$INSTALLER/scripts/win95/lib.sh"
# shellcheck source=/dev/null
. "$INSTALLER/scripts/win95/mouse.sh"
w95_need docker Xvfb xdotool import compare sfdisk mkfs.fat mcopy mdir truncate flock md5sum cmp
w95_lock

fail() { echo "e2e-w95: FAIL: $*" >&2 && exit 1; }
RUN="$W95_HOME/runs/e2e-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RUN/stage" "$RUN/out"
[ -z "${SHOTS:-}" ] || { mkdir -p "$SHOTS" && SHOTS="$(cd "$SHOTS" && pwd)"; }
cleanup() {
	w95_stop
	w95_xvfb_stop
	[ -n "${KEEP:-}" ] || rm -rf "$RUN"
}
trap cleanup EXIT
OFF=$((W95_PART_START * 512))

# shot NAME: the emulator's window as install-w95-NAME.png in $SHOTS, and in the run folder for the checks.
shot() {
	w95_shot "$RUN/$1.png"
	[ -z "${SHOTS:-}" ] || cp "$RUN/$1.png" "$SHOTS/install-w95-$1.png"
}
# run_box TEXT: Start > Run, the command typed and Return.
run_box() {
	w95_key ctrl+Escape
	sleep 2
	w95_key r
	sleep 2
	w95_type "$1"
	sleep 1
	w95_key Return
}

# The throwaway system disk, started on D:\SETUP.EXE.
cp --sparse=always "$W95_HOME/$BASE_NAME" "$RUN/work.img"
chmod u+w "$RUN/work.img"
mcopy -s -i "$IMG" :: "$RUN/stage/"
[ -f "$RUN/stage/SETUP.EXE" ] && [ -f "$RUN/stage/SETUP.LPK" ] || fail "the image has no SETUP.EXE and SETUP.LPK"
mcopy -n -i "$RUN/work.img@@$OFF" ::/WINDOWS/WIN.INI "$RUN/WIN.INI"
W95_CMD='D:\SETUP.EXE' awk 'BEGIN { IGNORECASE = 1; cmd = ENVIRON["W95_CMD"] } /^[ \t]*run[ \t]*=/ { printf "run=%s\r\n", cmd; done = 1; next } { print } END { if (!done) exit 3 }' \
	"$RUN/WIN.INI" >"$RUN/WIN.INI.new" || fail "WIN.INI has no run= line"
mcopy -o -i "$RUN/work.img@@$OFF" "$RUN/WIN.INI.new" ::/WINDOWS/WIN.INI
w95_make_dimage "$RUN/exchange.img" "$RUN/stage"
w95_conf "$RUN/run.conf" "/w/${RUN#"$W95_HOME"/}/work.img" "/w/${RUN#"$W95_HOME"/}/exchange.img"

echo "e2e-w95: booting Windows 95 ($BASE_NAME)"
w95_start "$RUN/run.conf"
w95_wait_desktop 300 || fail "Windows 95 did not reach the desktop"
sleep 6

# What the registry holds before the install, read while Setup waits on its first page.
reg_export() { # reg_export FILE KEY
	run_box "regedit /e D:\\$1 $2"
	sleep 5
}
reg_export QLD0.REG HKEY_CLASSES_ROOT\\.qld
reg_export TXT0.REG HKEY_CLASSES_ROOT\\.txt
reg_export RTF0.REG HKEY_CLASSES_ROOT\\.rtf
sleep 2

# The Run box leaves Setup's window without the focus, and Return would go nowhere: bring it back (Alt+Esc) until its title bar is the
# active colour (navy) at a point on the first page's title bar.
for _ in 1 2 3 4; do
	w95_shot "$RUN/focus.png"
	[ "$(magick "$RUN/focus.png" -crop 1x1+200+64 -format '%[hex:u]' info: 2>/dev/null || true)" != 000080 ] || break
	w95_key alt+Escape
	sleep 2
done

# 1. The wizard. Return is Next. The licence is accepted by a click on its first radio button.
shot wizard-welcome
w95_key Return
sleep 3
shot wizard-licence
# The pointer is walked to the radio button a pixel at a time and the guest can lag behind the host's clicks on a busy machine, so the
# click waits for the pointer to arrive, and the page is looked at: Next is enabled once the licence is accepted. Three tries.
accepted=0
for _ in 1 2 3; do
	w95_mouse_capture
	w95_mouse_move 244 285
	sleep 4
	w95_mouse_xdo click 1
	sleep 2
	w95_shot "$RUN/try.png"
	# the Next button, about 80 by 24 pixels at (432, 363): its text turns from grey with a white shadow to black
	diff="$(compare -metric AE -fuzz 8% -extract 80x24+392+351 "$RUN/wizard-licence.png" "$RUN/try.png" null: 2>&1 | cut -d' ' -f1 || true)"
	if [ "${diff%%.*}" -gt 40 ] 2>/dev/null; then
		accepted=1
		break
	fi
done
[ "$accepted" = 1 ] || fail "the licence could not be accepted: Next stayed disabled after three clicks on the radio button"
shot wizard-licence-accepted
w95_key Return
sleep 3
shot wizard-folder
w95_key Return
sleep 3
shot wizard-components
w95_key Return
sleep 3
shot wizard-ready
w95_key Return
sleep 2
shot wizard-copying
w95_wait_stable 8 180 || fail "the install did not settle on a last page"
shot wizard-done
[ "$(w95_changed "$RUN/wizard-licence-accepted.png" "$RUN/wizard-done.png")" -gt 2000 ] || fail "the wizard did not get past the licence page"
w95_key Return
sleep 4
shot desktop-after-install
echo "e2e-w95: the wizard ran to its last page"

# 2. What the install left. Output goes to D:, which the host reads back after shutdown.
run_box 'command /c xcopy C:\PROGRA~1\QUILL D:\INST\ /s /e /i /q > D:\XCOPY.TXT'
sleep 8
run_box 'command /c dir /b C:\WINDOWS\STARTM~1\PROGRAMS > D:\MENU1.TXT'
sleep 5
reg_export QLD1.REG HKEY_CLASSES_ROOT\\.qld
reg_export PROGID1.REG HKEY_CLASSES_ROOT\\hq.liminal.quill.qld
reg_export UNINST1.REG HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\hq.liminal.quill
reg_export TXT1.REG HKEY_CLASSES_ROOT\\.txt
reg_export RTF1.REG HKEY_CLASSES_ROOT\\.rtf

# The Start menu: Programs, then Quill. Typing q picks the Quill folder in the Programs list, and Right opens it.
start_quill_menu() {
	w95_key ctrl+Escape
	sleep 2
	w95_key p
	sleep 2
	w95_key q
	sleep 1
	w95_key Right
	sleep 2
}
start_quill_menu
shot start-programs-quill
w95_key Escape Escape Escape
sleep 1

run_box 'control appwiz.cpl'
sleep 8
shot add-remove-programs
w95_key Escape
sleep 2
echo "e2e-w95: looked at the Start menu and Add/Remove Programs"

# 3. Quill, started from the Start menu (the first item in the Quill folder), then closed.
start_quill_menu
w95_key Return
sleep 10
shot quill-running
w95_key alt+F4
sleep 3
shot quill-closed

# 4. The uninstall, as Add/Remove Programs would run it, then what is left.
run_box '"C:\Program Files\Quill\UNINST.EXE" /U "C:\Program Files\Quill"'
sleep 8
shot uninstall-confirm
w95_key Return
sleep 2
w95_wait_stable 8 120 || fail "the uninstall did not settle on a last page"
shot uninstall-done
w95_key Return
sleep 4
run_box 'command /c dir /a C:\PROGRA~1\QUILL > D:\DIR2.TXT'
sleep 5
run_box 'command /c dir /b C:\WINDOWS\STARTM~1\PROGRAMS > D:\MENU2.TXT'
sleep 5
reg_export QLD2.REG HKEY_CLASSES_ROOT\\.qld
reg_export PROGID2.REG HKEY_CLASSES_ROOT\\hq.liminal.quill.qld
reg_export UNINST2.REG HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\hq.liminal.quill
reg_export TXT2.REG HKEY_CLASSES_ROOT\\.txt
reg_export RTF2.REG HKEY_CLASSES_ROOT\\.rtf
w95_key Return
sleep 1
start_quill_menu
shot start-programs-after
w95_key Escape Escape Escape
sleep 1
run_box 'control appwiz.cpl'
sleep 8
shot add-remove-after
w95_key Escape
sleep 2

# Shut down from the Start menu, so the disk caches flush, and read D: back.
w95_shutdown
mcopy -s -n -m -Q -i "$RUN/exchange.img@@$OFF" ::/ "$RUN/out/" 2>/dev/null || true

# What the guest wrote, read back.
out="$RUN/out"
for f in XCOPY.TXT MENU1.TXT MENU2.TXT DIR2.TXT QLD1.REG PROGID1.REG UNINST1.REG TXT0.REG TXT1.REG TXT2.REG RTF0.REG RTF1.REG RTF2.REG; do
	[ -f "$out/$f" ] || fail "the guest wrote no $f"
done
INST="$out/INST"
for f in QUILL32.EXE QUILL.HLP EN_CA.DIC EN_GB.DIC EN_US.DIC README.TXT LICENCE.TXT INSTALL.LIR UNINST.EXE UNINST.DAT \
	SAMPLES/DEMO.QLD SAMPLES/DEMO.RTF SAMPLES/PICTURES.RTF SAMPLES/PLAIN.QLD; do
	[ -f "$INST/$f" ] || { find "$INST" -type f | sort >&2; fail "Quill\\$f was not installed"; }
done
cmp "$INST/QUILL32.EXE" "$DIST/windows-x86/QUILL32.EXE" || fail "QUILL32.EXE differs from the staged program"
cmp "$INST/QUILL.HLP" "$DIST/help/QUILL.HLP" || fail "QUILL.HLP differs from the staged help"
for f in EN_CA.DIC EN_GB.DIC EN_US.DIC; do cmp "$INST/$f" "$DIST/dict/$f" || fail "$f differs from the staged file"; done
cmp "$INST/LICENCE.TXT" "$DIST/common/LICENCE.TXT" || fail "LICENCE.TXT differs"
for f in DEMO.QLD DEMO.RTF PICTURES.RTF PLAIN.QLD; do cmp "$INST/SAMPLES/$f" "$DIST/samples/$f" || fail "SAMPLES/$f differs"; done
echo "e2e-w95: install: C:\\Program Files\\Quill holds the files, byte-equal to the staged files"
reg() { tr -d '\r' <"$out/$1"; }
reg QLD1.REG | grep -qx '@="hq.liminal.quill.qld"' || fail ".qld does not name hq.liminal.quill.qld: $(reg QLD1.REG)"
reg PROGID1.REG | grep -qx '@="Quill document"' || fail "the ProgID has no name"
reg PROGID1.REG | grep -qi 'QUILL32.EXE,0"$' || fail "the ProgID's DefaultIcon is not QUILL32.EXE,0"
reg PROGID1.REG | grep -qi '^@=.*QUILL32.EXE.*%1' || fail "the ProgID's open command does not run QUILL32.EXE with %1"
echo "e2e-w95: install: HKCR .qld -> hq.liminal.quill.qld (Quill document, DefaultIcon QUILL32.EXE,0, shell\\open\\command QUILL32.EXE \"%1\")"
reg UNINST1.REG | grep -q '"DisplayName"="Quill"' || fail "the Add/Remove Programs entry has no DisplayName"
reg UNINST1.REG | grep -qi '"UninstallString"=.*UNINST.EXE' || fail "the Add/Remove Programs entry has no UninstallString"
echo "e2e-w95: install: the Uninstall key has DisplayName Quill and an UninstallString for UNINST.EXE"
grep -qi '^QUILL' "$out/MENU1.TXT" || fail "the Start menu's Programs folder has no Quill"
echo "e2e-w95: note: the optional RTF and TXT components start ticked, so .txt is now $(reg TXT1.REG | grep '^@=' | head -n 1) and .rtf $(reg RTF1.REG | grep '^@=' | head -n 1)"

# After the uninstall: the keys are gone (regedit writes no file for a key that is not there), the others are as they were, Quill is
# out of the Start menu, and the folder holds only what the program wrote itself.
for f in QLD2.REG PROGID2.REG UNINST2.REG; do
	[ ! -f "$out/$f" ] || ! reg "$f" | grep -q '^\[' || fail "$f still holds a key after the uninstall"
done
diff <(reg TXT0.REG) <(reg TXT2.REG) >/dev/null || fail ".txt is not as it was after the uninstall"
diff <(reg RTF0.REG) <(reg RTF2.REG) >/dev/null || fail ".rtf is not as it was after the uninstall"
! grep -qi '^QUILL' "$out/MENU2.TXT" || fail "the Start menu still has Quill after the uninstall"
left="$(tr -d '\r' <"$out/DIR2.TXT" | grep -ci 'QUILL32.EXE\|UNINST\|INSTALL.LIR\|\.DIC\|\.QLD' || true)"
[ "$left" = 0 ] || { reg DIR2.TXT >&2; fail "the uninstall left files of the install behind"; }
if grep -q 'Directory of' "$out/DIR2.TXT"; then
	echo "e2e-w95: note: the uninstall left the empty folder C:\\Program Files\\Quill behind"
fi
echo "e2e-w95: uninstall: the keys, the Start menu entry and the files are gone, and .txt and .rtf are as they were"
echo "e2e-w95: ok"
