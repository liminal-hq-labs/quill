#!/usr/bin/env bash
# Installs Quill from the DOS floppy image under headless DOSBox Staging, runs it, then uninstalls it.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: installer/tools/e2e-dos.sh
#
# Needs installer/out/QUILL-DOS.IMG (build-disks.sh), mtools and the DOS tools (dos/tools/fetch-tools.sh, or DOS_VENDOR naming a
# folder that holds them); exits 0 with a note when the tools are missing, so a checkout without them is not failed. The image's
# files are copied to a folder that DOSBox mounts as C: with `dos/tools/dosrun.sh`, so `SETUP` runs from the disk's own files and
# installs into C:\QUILL on the same drive. Keys for the wizard go in with AUTOTYPE.
#
#   1. install: SETUP.EXE exits 0; every file is in C:\QUILL and byte-equal to installer/dist/; the receipt names the folder
#   2. run:     QUILLDOS.EXE opens a sample from the install folder, and File > Exit ends it with status 0; the text of the sample is
#               on a screen it drew (QUILL_DUMP)
#   3. remove:  SETUP.EXE /U exits 0 and C:\QUILL is gone
#
# SHOTS=<dir> also saves screenshots of the wizard, of Quill running and of the uninstall as <dir>/install-dos-*.png (see shots.sh):
# the run goes under its own virtual X server and DOSBox is a window on it, so the host's screen is not used. The pages are picked
# from the distinct frames of each run by their order, which the keys fix; a note says when a run shows fewer frames than expected.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(dirname "$HERE")"
DIST="$HERE/dist"
IMG="$HERE/out/QUILL-DOS.IMG"
VENDOR="${DOS_VENDOR:-$ROOT/dos/vendor}"
export DOS_VENDOR="$VENDOR"
# shellcheck source=installer/tools/shots.sh
. "$HERE/tools/shots.sh"
shots_xvfb "$0" "$@"

if [ ! -f "$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE" ] || [ -z "$(ls "$VENDOR"/dosbox/*/dosbox 2>/dev/null)" ]; then
	echo "e2e-dos: DOS tools not present; skipping (run dos/tools/fetch-tools.sh)"
	exit 0
fi
command -v mcopy >/dev/null || { echo "e2e-dos: mtools not found" >&2 && exit 1; }
[ -f "$IMG" ] || { echo "e2e-dos: $IMG is missing; run installer/tools/build-disks.sh" >&2 && exit 1; }

WORK="$(mktemp -d "${QUILL_BUILD_DIR:-$HOME/.cache/quill16-installer-build}/e2e-dos.XXXXXX")"
trap 'shots_watch_stop; rm -rf "$WORK"' EXIT
fail() { echo "e2e-dos: FAIL: $*" >&2 && exit 1; }

DISK="$WORK/disk"
mkdir -p "$DISK"
mcopy -s -i "$IMG" :: "$DISK/"
[ -f "$DISK/SETUP.EXE" ] && [ -f "$DISK/SETUP.LPK" ] || fail "the image has no SETUP.EXE and SETUP.LPK"

DOSRUN="$ROOT/dos/tools/dosrun.sh"
if [ -n "${SHOTS:-}" ]; then export DOS_X11=1; fi

# frames <phase>: the distinct frames of a run, cropped to the DOSBox window, in $WORK/<phase>/u. Prints how many.
frames() {
	local n geom f
	n="$(shots_distinct "$WORK/$1" "$WORK/$1/u")"
	[ "$n" -gt 0 ] || { echo 0 && return; }
	geom="$(shots_bounds "$WORK/$1"/u/*.png)"
	for f in "$WORK/$1"/u/*.png; do shots_crop "$geom" "$f"; done
	echo "$n"
}
# keep <phase> <index> <name>: the numbered frame, as install-dos-<name>.png.
keep() { shots_keep "$WORK/$1/u/$(printf %02d "$2").png" "install-dos-$3"; }
# middle <first> <last>: the index half way between two.
middle() { echo $(($1 + ($2 - $1) / 2)); }
export DOS_MOUNT_OPTS="-freesize 64"
export DOS_TIMEOUT=300

# 1. Install. Welcome, licence (Tab twice to the radio group, Space to accept), folder, components, ready; then, once the copy is done,
# Tab to Finish and Enter. The keys are 2 s apart and the last pair is repeated: the copy takes longer than a key on a busy host, a key
# typed during it is lost, and a pair typed after Setup exits goes to the DOS prompt and does nothing.
export DOS_ENV='LI_DUMP=D:\DUMP.TXT'
export DOS_KEEP="$WORK/keep"
export DOS_AUTOTYPE="-w 3 -p 2 enter tab tab space enter enter enter enter tab enter tab enter tab enter tab enter"
rc=0
[ -z "${SHOTS:-}" ] || shots_watch_start "$WORK/install"
"$DOSRUN" "$DISK" SETUP.EXE >"$WORK/install.out" || rc=$?
shots_watch_stop
[ "$rc" = 0 ] || fail "SETUP.EXE exited with $rc"
if [ -n "${SHOTS:-}" ]; then
	# Welcome, licence (three focus steps), accepted, folder, components, ready, the copy, done.
	n="$(frames install)"
	if [ "$n" -ge 11 ]; then
		keep install 1 welcome
		keep install 2 licence
		keep install 5 licence-accepted
		keep install 6 folder
		keep install 7 components
		keep install 8 ready
		keep install "$(middle 9 $((n - 1)))" copying
		keep install "$n" "done"
	else
		echo "e2e-dos: note: the install showed $n distinct frames, fewer than the 11 expected; no wizard shots saved"
	fi
fi

APP="$DISK/QUILL"
for f in QUILLDOS.EXE QUILLDOS.BAT EN_CA.DIC EN_GB.DIC EN_US.DIC README.TXT LICENCE.TXT INSTALL.LIR \
	SAMPLES/DEMO.QLD SAMPLES/DEMO.RTF SAMPLES/PICTURES.RTF SAMPLES/PLAIN.QLD; do
	[ -f "$APP/$f" ] || { find "$DISK" | sort >&2; fail "QUILL/$f was not installed"; }
done
cmp "$APP/QUILLDOS.EXE" "$DIST/dos-x86/QUILLDOS.EXE" || fail "QUILLDOS.EXE differs from the staged program"
for f in EN_CA.DIC EN_GB.DIC EN_US.DIC; do cmp "$APP/$f" "$DIST/dict/$f" || fail "$f differs from the staged file"; done
for f in DEMO.QLD DEMO.RTF PICTURES.RTF PLAIN.QLD; do cmp "$APP/SAMPLES/$f" "$DIST/samples/$f" || fail "SAMPLES/$f differs"; done
cmp "$APP/README.TXT" "$DIST/common/README.TXT" || fail "README.TXT differs"
grep -q 'dir=C:.QUILL' "$APP/INSTALL.LIR" || fail "the receipt does not name C:\\QUILL"
echo "e2e-dos: install: files byte-equal to the staged files, receipt names C:\\QUILL"

# 2. Run. File > Exit is F10, End, Enter, as the editing scenarios use.
rm -rf "$WORK/keep"
export DOS_ENV='QUILL_DUMP=D:\DUMP.TXT'
export DOS_AUTOTYPE="-w 3 -p 0.3 f10 end enter"
# Slower keys when screenshots are taken, so the open menu (File, then Exit highlighted) is on the screen long enough to be caught.
if [ -n "${SHOTS:-}" ]; then export DOS_AUTOTYPE="-w 4 -p 2.5 f10 end enter"; fi
rc=0
[ -z "${SHOTS:-}" ] || shots_watch_start "$WORK/run"
"$DOSRUN" "$DISK" 'QUILL\QUILLDOS.EXE QUILL\SAMPLES\DEMO.QLD' >"$WORK/run.out" || rc=$?
shots_watch_stop
[ "$rc" = 0 ] || fail "QUILLDOS.EXE exited with $rc"
[ -f "$WORK/keep/DUMP.TXT" ] || fail "QUILLDOS.EXE drew no screen"
# The sample's first line, read from the screen it drew.
grep -q -F 'Hi, this is Quill' "$WORK/keep/DUMP.TXT" || fail "the sample's text is on no screen QUILLDOS.EXE drew"
if [ -n "${SHOTS:-}" ]; then
	n="$(frames run)"
	if [ "$n" -ge 2 ]; then
		keep run 1 quill
		keep run "$n" quill-menu
	else
		echo "e2e-dos: note: Quill showed $n distinct frames; no shots saved"
	fi
fi
echo "e2e-dos: run: QUILLDOS.EXE opened SAMPLES\\DEMO.QLD and exited 0"

# 3. Uninstall: confirm, then the done page, with the same repeated keys.
rm -rf "$WORK/keep"
export DOS_ENV='LI_DUMP=D:\DUMP.TXT'
export DOS_AUTOTYPE="-w 3 -p 3 enter tab enter tab enter tab enter"
rc=0
[ -z "${SHOTS:-}" ] || shots_watch_start "$WORK/uninstall"
"$DOSRUN" "$DISK" SETUP.EXE /U >"$WORK/uninstall.out" || rc=$?
shots_watch_stop
[ "$rc" = 0 ] || fail "SETUP.EXE /U exited with $rc"
if [ -n "${SHOTS:-}" ]; then
	n="$(frames uninstall)"
	if [ "$n" -ge 2 ]; then
		keep uninstall 1 uninstall-confirm
		keep uninstall "$n" uninstall-done
	else
		echo "e2e-dos: note: the uninstall showed $n distinct frames, fewer than the 2 expected; no shots saved"
	fi
fi
[ ! -e "$APP" ] || { find "$DISK" | sort >&2; fail "the uninstall left C:\\QUILL behind"; }
echo "e2e-dos: uninstall: C:\\QUILL is gone"
echo "e2e-dos: ok"
