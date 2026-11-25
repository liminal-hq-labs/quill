#!/usr/bin/env bash
# Installs Quill from the Windows 3.x floppy image onto Windows 3.11 under DOSBox-X, starts it from Program Manager, then uninstalls it.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [SHOTS=DIR] installer/tools/e2e-w31.sh
#
# Needs installer/out/QUILL-W31.IMG (build-disks.sh), mtools, Docker with the DOSBox-X image of win16/tools/dosbox-x.Dockerfile
# (quill16-dosbox-x:local, built on first use), Xvfb, xdotool and ImageMagick. W31_TREE names a Windows 3.11 tree (a folder holding
# WINDOWS/ and WIN.BAT; default the play harness's base, ~/.cache/quill16-play/w31-base, which is only read: it is copied to a run
# folder under QUILL_BUILD_DIR first). The tree is the owner's and is never committed, so this is a manual test; it exits 0 with a note
# when the tree is missing. DOSBox-X runs in a container of its own (quill16-install-w31, never the play harness's) on a private
# virtual X server, so nothing here reads or types into the desktop.
#
# The disk's files go on D:. Two Windows sessions, each on a fresh DOSBox-X over the same C: copy:
#
#   1. install: `win /3 D:\SETUP.EXE /PLAY ... /PAUSE=2500` steps through the wizard (each page is held for 2.5 s and the script takes a
#      screenshot of it), then Program Manager is brought up with the Quill group open and Return starts Quill from it. The host checks
#      the files against installer/dist/ and the receipt, QUILL.GRP and PROGMAN.INI, and WIN.INI's [Extensions]: `qld=` names Quill.
#   2. remove: `D:\SETUP.EXE /U C:\QUILL` steps through the uninstall; then the group, QUILL.GRP, C:\QUILL and the `qld=` line are
#      gone, and every other [Extensions] line is as it was (`txt=` and `rtf=` too: they start ticked, see INS-2).
#
# SHOTS=<dir> saves docs/media's install-w31-*.png there.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(dirname "$HERE")"
DIST="$HERE/dist"
IMG="$HERE/out/QUILL-W31.IMG"
TREE="${W31_TREE:-$HOME/.cache/quill16-play/w31-base}"
IMAGE="${W31_IMAGE:-quill16-dosbox-x:local}"
NAME="${W31_E2E_CONTAINER:-quill16-install-w31}"
# shellcheck source=installer/tools/shots.sh
. "$HERE/tools/shots.sh"

if [ ! -f "$TREE/WINDOWS/WIN.COM" ]; then
	echo "e2e-w31: no Windows 3.11 tree at $TREE; skipping (set W31_TREE)"
	exit 0
fi
for t in docker mcopy xdotool Xvfb import cmp awk; do command -v "$t" >/dev/null || { echo "e2e-w31: $t not found" >&2 && exit 1; }; done
[ -f "$IMG" ] || { echo "e2e-w31: $IMG is missing; run installer/tools/build-disks.sh" >&2 && exit 1; }
if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
	docker build -q -t "$IMAGE" -f "$ROOT/win16/tools/dosbox-x.Dockerfile" "$ROOT/win16/tools" >/dev/null
fi

BUILD="${QUILL_BUILD_DIR:-$HOME/.cache/quill16-installer-build}"
mkdir -p "$BUILD"
WORK="$(mktemp -d "$BUILD/e2e-w31.XXXXXX")"
cleanup() {
	docker rm -f "$NAME" >/dev/null 2>&1 || true
	shots_display_stop
	[ -n "${KEEP:-}" ] || rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "e2e-w31: FAIL: $*" >&2 && exit 1; }
[ -z "${SHOTS:-}" ] || { mkdir -p "$SHOTS" && SHOTS="$(cd "$SHOTS" && pwd)"; }
shots_display_start
# shot NAME: a screenshot of the emulator's window as $SHOTS/install-w31-NAME.png, and in the work folder for the checks.
shot() {
	shots_window "$WORK/$1.png"
	[ -z "${SHOTS:-}" ] || cp "$WORK/$1.png" "$SHOTS/install-w31-$1.png"
}

mkdir -p "$WORK/d" "$WORK/conf"
cp -a --reflink=auto "$TREE" "$WORK/c"
C="$WORK/c/WINDOWS"
# The tree's StartUp group opens Notepad and dials a modem: leave it out, so Windows starts quietly.
sed -i '/^Group5=/Id' "$C/PROGMAN.INI"
mcopy -s -i "$IMG" :: "$WORK/d/"
[ -f "$WORK/d/SETUP.EXE" ] && [ -f "$WORK/d/SETUP.LPK" ] || fail "the image has no SETUP.EXE and SETUP.LPK"
printf 'licence choose licence.accepted yes\n' >"$WORK/d/PLAY.TXT"
: >"$WORK/d/PLAY2.TXT"
ext() { tr -d '\r' <"$C/WIN.INI" | sed -n '/^\[Extensions\]/,/^\[/p' | grep -v '^\[' | sort; }
ext >"$WORK/ext.before"
grep -q '^txt=' "$WORK/ext.before" || fail "the tree's WIN.INI has no txt= line to see restored"
! grep -qi '^qld=' "$WORK/ext.before" || fail "the tree already has a qld= association"

# session <WIN command line>: one Windows session on a fresh DOSBox-X; the log it writes is C:\<name>.
session() {
	{
		cat <<'CONF'
[sdl]
output=surface
fullscreen=false
windowresolution=800x600
autolock=false
mouse_emulation=always
[dosbox]
machine=svga_s3
memsize=32
[cpu]
core=normal
cputype=pentium
cycles=max
[dos]
ems=false
xms=true
umb=true
[autoexec]
CONF
		printf '%s\n' '@echo off' 'mount c /c' 'mount d /d' 'c:' 'set PATH=c:\windows;c:\utils' 'cd \windows' "win /3 $1"
	} >"$WORK/conf/run.conf"
	docker rm -f "$NAME" >/dev/null 2>&1 || true
	docker run -d --name "$NAME" --user "$(id -u):$(id -g)" --network host -v /tmp/.X11-unix:/tmp/.X11-unix -e DISPLAY="$DISPLAY" \
		-e SDL_AUDIODRIVER=dummy -e HOME=/tmp -v "$WORK/c:/c" -v "$WORK/d:/d" -v "$WORK/conf:/conf" --entrypoint dosbox-x \
		"$IMAGE" -conf /conf/run.conf -nomenu -fastlaunch >/dev/null
}
stop() { docker rm -f "$NAME" >/dev/null 2>&1 || true; }

# walk <log> <prefix> [timeout]: follows a Setup log. One screenshot a page (taken 1.3 s in, while the page is held), named
# <prefix>-<page> (the log has no line for the copy page, which is over in a second or two); true when the log reaches its `exit` line.
walk() {
	local log="$WORK/c/$1" prefix="$2" limit="${3:-240}" t=0 seen=0 n
	rm -f "$log"
	while [ "$t" -lt $((limit * 2)) ]; do
		if [ -f "$log" ]; then
			n="$(grep -c '^page ' "$log" 2>/dev/null || true)"
			if [ "$n" -gt "$seen" ]; then
				sleep 1.3
				seen="$n"
				shot "$prefix-$(grep '^page ' "$log" | tr -d '\r' | sed -n "${n}p" | cut -d' ' -f2)"
			fi
			grep -q '^exit' "$log" && return 0
		fi
		sleep 0.5
		t=$((t + 1))
	done
	return 1
}

# key KEY...: keys typed into the guest, through the virtual X server's keyboard.
key() {
	local k
	for k in "$@"; do
		xdotool key "$k"
		sleep 0.5
	done
}
# program_manager: brings Program Manager up from its icon through the Task List (Ctrl+Esc, then Return), because this tree starts it
# minimized and a screenshot should show its groups.
program_manager() {
	xdotool mousemove 400 300 2>/dev/null || true
	key ctrl+Escape
	sleep 1
	key Return
	sleep 3
}

# 1. Install.
printf 'e2e-w31: installing from %s\n' "$(basename "$IMG")"
session 'D:\SETUP.EXE /LOG=C:\S.LOG /PLAY=D:\PLAY.TXT /DUMP=C:\S.TXT /PAUSE=2500'
walk S.LOG wizard 300 || fail "SETUP.EXE did not finish (see the log below)$(echo; tr -d '\r' <"$WORK/c/S.LOG" 2>/dev/null || true)"
[ "$(tail -n 1 "$WORK/c/S.LOG" | tr -d '\r')" = "exit 0" ] || fail "SETUP.EXE did not exit 0: $(tail -n 1 "$WORK/c/S.LOG" | tr -d '\r')"
sleep 4
program_manager
shot group
# Program Manager has the new group open with its last item selected ("Uninstall Quill"); two steps left is "Quill". Return starts it.
key Left Left
shot group-quill-selected
started="$(date +%s)"
key Return
# Quill's window is on the screen when more of the picture has changed than a menu or a caret would change. The screen is watched for
# up to 30 s, picture after picture; a window that closes again within a second is caught on most runs and missed on some.
changed=0
for _ in $(seq 1 90); do
	shots_window "$WORK/poll.png"
	changed="$(compare -metric AE -fuzz 5% "$WORK/group-quill-selected.png" "$WORK/poll.png" null: 2>&1 || true)"
	changed="${changed%% *}"
	if [ "${changed:-0}" -gt 20000 ] 2>/dev/null; then
		cp "$WORK/poll.png" "$WORK/quill-running.png"
		[ -z "${SHOTS:-}" ] || cp "$WORK/poll.png" "$SHOTS/install-w31-quill-running.png"
		break
	fi
done
if [ "${changed:-0}" -gt 20000 ] 2>/dev/null; then
	caught=1
else
	# A window that came and went between two pictures still leaves its mark: Quill writes its settings and its stack log in its own
	# folder when it ends, and those files are new since Return was pressed.
	caught=0
	sleep 5
	[ -f "$WORK/c/QUILL/STACK.LOG" ] && [ "$(stat -c %Y "$WORK/c/QUILL/STACK.LOG")" -ge "$started" ] \
		|| fail "Quill did not start from the Program Manager group (no window in the pictures, and no STACK.LOG written since Return)"
fi
echo "e2e-w31: Quill started from the Program Manager group ($([ "$caught" = 1 ] && echo "its window is in the picture" || echo "its window came and went between two pictures; STACK.LOG shows it ran"))"
# What Quill does next: this build ends by itself about a second after it opens (a finding for Quill, not for the installer). If it
# is still up it is closed with Alt+F4; Alt+F4 is never sent to Program Manager, where it would offer to end Windows.
sleep 6
shots_window "$WORK/after-quill.png"
changed="$(compare -metric AE -fuzz 5% "$WORK/group-quill-selected.png" "$WORK/after-quill.png" null: 2>&1 || true)"
changed="${changed%% *}"
if [ "${changed:-0}" -gt 20000 ] 2>/dev/null; then
	key alt+F4
	sleep 2
else
	echo "e2e-w31: note: Quill had closed itself again within 6 s of opening"
fi
stop

# What the install left.
APP="$WORK/c/QUILL"
for f in QUILL.EXE QUILL.HLP EN_CA.DIC EN_GB.DIC EN_US.DIC README.TXT LICENCE.TXT INSTALL.LIR UNINST.EXE UNINST.DAT \
	SAMPLES/DEMO.QLD SAMPLES/DEMO.RTF SAMPLES/PICTURES.RTF SAMPLES/PLAIN.QLD; do
	[ -f "$APP/$f" ] || { find "$WORK/c/QUILL" | sort >&2; fail "QUILL/$f was not installed"; }
done
cmp "$APP/QUILL.EXE" "$DIST/win16-x86/QUILL.EXE" || fail "QUILL.EXE differs from the staged program"
cmp "$APP/QUILL.HLP" "$DIST/help/QUILL.HLP" || fail "QUILL.HLP differs from the staged help"
for f in EN_CA.DIC EN_GB.DIC EN_US.DIC; do cmp "$APP/$f" "$DIST/dict/$f" || fail "$f differs from the staged file"; done
cmp "$APP/LICENCE.TXT" "$DIST/common/LICENCE.TXT" || fail "LICENCE.TXT differs"
n=0
while read -r kind fname size _crc; do
	[ "$kind" = F ] || continue
	[ "$(stat -c %s "$APP/${fname//\\//}" 2>/dev/null || echo -1)" = "$size" ] || fail "$fname is not the receipt's size"
	n=$((n + 1))
done < <(tr -d '\r' <"$APP/INSTALL.LIR")
echo "e2e-w31: install: $n files, byte-equal to the staged files or the receipt's sizes"
grp="$(tr -d '\r' <"$APP/INSTALL.LIR" | sed -n 's/^P //p' | head -n 1 | LC_ALL=C tr -cd '[:alnum:]' | cut -c1-8 | tr "[:lower:]" "[:upper:]")"
[ -n "$grp" ] || fail "the receipt has no P record: the install did not make a group"
[ -f "$C/$grp.GRP" ] || fail "$grp.GRP was not written"
grep -qi "$grp.GRP" "$C/PROGMAN.INI" || fail "PROGMAN.INI does not name $grp.GRP"
grep -q '^progman: added' "$WORK/c/S.LOG" || fail "the log does not say the Program Manager group was added"
echo "e2e-w31: install: Program Manager group $grp.GRP exists and PROGMAN.INI names it"
ext >"$WORK/ext.during"
grep -qi '^qld=C:\\QUILL\\QUILL.EXE \^\.qld$' "$WORK/ext.during" || { cat "$WORK/ext.during" >&2; fail "WIN.INI [Extensions] has no qld=C:\\QUILL\\QUILL.EXE ^.qld"; }
echo "e2e-w31: install: WIN.INI [Extensions] has $(grep -i '^qld=' "$WORK/ext.during")"
grep -qi '^[a-z]*=C:.QUILL' "$WORK/ext.during" && echo "e2e-w31: note: the optional RTF and TXT components start ticked, so WIN.INI also gives them to Quill: $(grep -i 'QUILL' "$WORK/ext.during" | grep -vi '^qld=' | tr '\n' ' ')"

# 2. Uninstall.
printf 'e2e-w31: uninstalling\n'
session 'D:\SETUP.EXE /U C:\QUILL /LOG=C:\U.LOG /PLAY=D:\PLAY2.TXT /PAUSE=2500'
walk U.LOG uninstall 300 || fail "SETUP.EXE /U did not finish"
[ "$(tail -n 1 "$WORK/c/U.LOG" | tr -d '\r')" = "exit 0" ] || fail "SETUP.EXE /U did not exit 0: $(tail -n 1 "$WORK/c/U.LOG" | tr -d '\r')"
sleep 4
program_manager
shot removed
stop
# The folder goes, unless the program wrote files of its own in it (settings and logs, which the uninstall leaves on purpose: INS-2).
if [ -e "$APP" ]; then
	left="$(cd "$APP" && find . -type f | sed 's|^\./||' | LC_ALL=C sort | tr '\n' ' ')"
	for f in $left; do
		case "$f" in QUILL.INI | STACK.LOG | PICS.LOG | CUSTOM.DIC | QUILL.UDC) ;; *) fail "the uninstall left C:\\QUILL\\$f behind" ;; esac
	done
	echo "e2e-w31: note: C:\\QUILL still holds only what Quill wrote itself: $left"
fi
[ ! -f "$C/$grp.GRP" ] || fail "the uninstall left $grp.GRP behind"
! grep -qi "$grp.GRP" "$C/PROGMAN.INI" || fail "PROGMAN.INI still names $grp.GRP"
grep -q '^progman: removed' "$WORK/c/U.LOG" || fail "the log does not say the Program Manager group was removed"
ext >"$WORK/ext.after"
# The uninstall gives an association back by writing its old value; one that had none is written as an empty `ext=` line, which
# Windows reads as no association. That is as good as removed, so empty lines are set aside (and said).
if grep -q '^[A-Za-z0-9]*=$' "$WORK/ext.after"; then
	echo "e2e-w31: note: the uninstall left empty lines in [Extensions] for the types that had no owner: $(grep '^[A-Za-z0-9]*=$' "$WORK/ext.after" | tr '\n' ' ')"
	grep -v '^[A-Za-z0-9]*=$' "$WORK/ext.after" >"$WORK/ext.after.lines"
	mv "$WORK/ext.after.lines" "$WORK/ext.after"
fi
! grep -qi '^qld=' "$WORK/ext.after" || fail "WIN.INI still gives .qld to a program"
diff "$WORK/ext.before" "$WORK/ext.after" >"$WORK/ext.diff" || { cat "$WORK/ext.diff" >&2; fail "WIN.INI's [Extensions] is not as it was"; }
echo "e2e-w31: uninstall: the installed files, $grp.GRP and the group are gone, and WIN.INI's [Extensions] is as it was"
echo "e2e-w31: ok"
