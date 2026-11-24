#!/usr/bin/env bash
# Starts Windows for Workgroups 3.11 in a real DOSBox-X window on your desktop, on a throwaway copy of the tree, for playing by hand.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win16/tools/play-w31.sh [options] [PROGRAM.EXE...]
#   -v s3|vga    the video: s3 (default; the tree's own S3 Trio64V driver, 800x600 in 64K colours) or vga (VGA.DRV, 640x480, 16 colours)
#   -d DIR       also put the contents of this host directory on D:
#   -n           do not start anything when Windows is up (Program Manager only)
#   -a ARGS      arguments for the program that starts (a document, say: D:\LETTER.RTF)
#   -s SCALE     window size as a multiple of the guest screen (default 2 on a tall display, otherwise 1)
#   -o OUTPUT    the DOSBox-X video output: opengl (default, smooth scaling), openglnb (sharp pixels) or surface (no scaling at all)
#   -x           allow the container to reach the X server with xhost +SI:localuser:USER, taken back on exit (see docs/play.md)
#   -N           run a DOSBox-X installed on the host instead of the container; not tested here
#   -K           keep the session folder (the copied tree and D:) afterwards
#
# What happens. The Windows 3.11 tree (never committed: it comes from the zip the owner supplies, $W31_ZIP, default ~/Downloads/win31.zip) is
# unpacked once into $PLAY_HOME/w31-base, then copied for every session to $PLAY_HOME/run-<time>/c, so the base is never written. D: is a
# folder of the session, holding the Quill build outputs (QUILL.EXE from $OUT_DIR or win16/out, win9x/dist/QUILL32.EXE, dos/dist/QUILLDOS.EXE) and the sample
# documents (tests/docs), plus -d and the programs named on the command line. WIN.INI's run= line starts the first program on the command
# line, or QUILL.EXE, so Program Manager is behind it when it closes. DOSBox-X runs in the container $W31_PLAY_CONTAINER on your display
# (DISPLAY, the X11 socket and your Xauthority file are handed in). The window title says how to capture the pointer: click inside it, and
# press Ctrl+F10 to let it go. On exit the container is removed, files the guest added or changed on D: are copied to out/ in the session
# folder, and the session folder is deleted.
# Environment: OUT_DIR (where win16/build.sh put QUILL.EXE; default win16/out), PLAY_HOME (default ~/.cache/quill16-play), W31_ZIP, W31_TREE (an unpacked tree to use as the base), W31_PLAY_CONTAINER
# (default quill16-play-w31), W31_IMAGE (default quill16-dosbox-x:local, built from win16/tools/dosbox-x.Dockerfile if missing).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
PLAY_HOME="${PLAY_HOME:-$HOME/.cache/quill16-play}"
BASE="${W31_TREE:-$PLAY_HOME/w31-base}"
ZIP="${W31_ZIP:-$HOME/Downloads/win31.zip}"
IMAGE="${W31_IMAGE:-quill16-dosbox-x:local}"
CONTAINER="${W31_PLAY_CONTAINER:-quill16-play-w31}"
VIDEO=s3
XDIR=""
NOSTART=0
PARGS=""
SCALE=""
OUTPUT=opengl
XHOST=0
NATIVE=0
KEEP=0

log() { printf '[play-w31 %s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
die() {
	log "error: $*"
	exit 1
}

while getopts "v:d:na:s:o:xNK" o; do
	case "$o" in
	v) VIDEO="$OPTARG" ;;
	d) XDIR="$OPTARG" ;;
	n) NOSTART=1 ;;
	a) PARGS="$OPTARG" ;;
	s) SCALE="$OPTARG" ;;
	o) OUTPUT="$OPTARG" ;;
	x) XHOST=1 ;;
	N) NATIVE=1 ;;
	K) KEEP=1 ;;
	*) die "bad option (see the header of play-w31.sh)" ;;
	esac
done
shift $((OPTIND - 1))

case "$VIDEO" in s3 | vga) ;; *) die "-v takes s3 or vga" ;; esac
case "$OUTPUT" in surface | opengl | openglnb) ;; *) die "-o takes surface, opengl or openglnb" ;; esac
[ -z "$XDIR" ] || [ -d "$XDIR" ] || die "no such directory: $XDIR"
[ -n "${DISPLAY:-}" ] || die "DISPLAY is not set; run this from a terminal on your desktop session"
for p in "$@"; do [ -f "$p" ] || die "no such program: $p"; done
for t in xdotool awk cmp sed; do command -v "$t" > /dev/null 2>&1 || die "missing host tool: $t"; done
if [ "$NATIVE" = 1 ]; then
	command -v dosbox-x > /dev/null 2>&1 || die "missing host tool: dosbox-x"
else
	command -v docker > /dev/null 2>&1 || die "missing host tool: docker"
fi

# The base tree, unpacked once.
if [ ! -f "$BASE/WINDOWS/WIN.COM" ]; then
	[ -f "$ZIP" ] || die "no Windows 3.11 tree at $BASE and no zip at $ZIP; set W31_ZIP (the zip the owner supplies) or W31_TREE (an unpacked tree)"
	command -v unzip > /dev/null 2>&1 || die "missing host tool: unzip"
	log "unpacking $ZIP into $BASE (once)"
	mkdir -p "$BASE"
	nice -n 19 ionice -c3 unzip -q -o "$ZIP" -d "$BASE"
	[ -f "$BASE/WINDOWS/WIN.COM" ] || die "the zip does not hold WINDOWS/WIN.COM"
fi

if [ -z "$SCALE" ]; then
	SCALE=1
	geom="$(xdotool getdisplaygeometry 2> /dev/null || true)"
	[ "${geom##* }" -ge 1080 ] 2> /dev/null && SCALE=2
fi
case "$SCALE" in [1-4]) ;; *) die "-s takes 1 to 4" ;; esac

RUN="$PLAY_HOME/run-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RUN/c" "$RUN/d" "$RUN/stage" "$RUN/out"

# What goes on D:.
W16OUT="${OUT_DIR:-$REPO/win16/out}"
[ ! -f "$W16OUT/QUILL.EXE" ] || cp "$W16OUT/QUILL.EXE" "$RUN/stage/"
# Help > Contents opens QUILL.HLP from the program's own directory; win16/build.sh writes it beside QUILL.EXE.
[ ! -f "$W16OUT/QUILL.HLP" ] || cp "$W16OUT/QUILL.HLP" "$RUN/stage/"
for f in win9x/dist/QUILL32.EXE dos/dist/QUILLDOS.EXE tests/docs/letter.rtf tests/docs/letter.qld tests/docs/plain.qld; do
	[ ! -f "$REPO/$f" ] || cp "$REPO/$f" "$RUN/stage/"
done
[ -z "$XDIR" ] || cp -R "$XDIR"/. "$RUN/stage/"
for p in "$@"; do cp "$p" "$RUN/stage/"; done
if [ ! -f "$RUN/stage/QUILL.EXE" ]; then
	log "QUILL.EXE is not built (looked in $W16OUT; set OUT_DIR to where win16/build.sh wrote it). The Win16 Quill is on its own branch; build it there with: (cd win16 && ./build.sh). Nothing will start unless you name a program."
fi
cp -R "$RUN/stage/." "$RUN/d/"

if [ "$NOSTART" = 1 ]; then
	CMD=""
elif [ "$#" -gt 0 ]; then
	CMD="D:\\$(basename "$1")${PARGS:+ $PARGS}"
elif [ -f "$RUN/stage/QUILL.EXE" ]; then
	CMD="D:\\QUILL.EXE${PARGS:+ $PARGS}"
else
	CMD=""
fi

XHOST_ADDED=0
FAILED=0
NATIVE_PID=
# shellcheck disable=SC2329 # run by the EXIT trap
cleanup() {
	set +e
	docker rm -f "$CONTAINER" > /dev/null 2>&1
	[ "$XHOST_ADDED" = 1 ] && xhost -SI:localuser:"$(id -un)" > /dev/null 2>&1
	# Only what the guest added or changed on D: is worth keeping.
	for f in "$RUN"/d/*; do
		[ -f "$f" ] || continue
		if [ -f "$RUN/stage/$(basename "$f")" ] && cmp -s "$f" "$RUN/stage/$(basename "$f")"; then continue; fi
		cp "$f" "$RUN/out/"
	done
	if [ -n "$(ls -A "$RUN/out" 2> /dev/null)" ]; then
		log "stopped. Files left on D: are in $RUN/out"
		[ "$KEEP" = 1 ] || rm -rf "$RUN/c" "$RUN/d" "$RUN/stage"
	elif [ "$KEEP" = 0 ] && [ "$FAILED" = 0 ]; then
		rm -rf "$RUN"
		log "stopped. Nothing was kept"
	fi
}
trap cleanup EXIT
trap 'docker rm -f "$CONTAINER" > /dev/null 2>&1; [ -z "$NATIVE_PID" ] || kill "$NATIVE_PID" 2> /dev/null; exit 130' INT TERM HUP

log "copying the Windows 3.11 tree (the base is never written)"
nice -n 19 ionice -c3 cp -a --reflink=auto "$BASE"/. "$RUN/c/"
INI="$RUN/c/WINDOWS/WIN.INI"
[ -f "$INI" ] || die "the tree has no WINDOWS/WIN.INI"
# The program that starts: WIN.INI's run= line (CR LF line ends are kept).
W31_CMD="$CMD" awk 'BEGIN { IGNORECASE = 1; cmd = ENVIRON["W31_CMD"] } /^[ \t]*run[ \t]*=/ { printf "run=%s\r\n", cmd; done = 1; next } { print } END { if (!done) exit 3 }' "$INI" > "$INI.new" || die "WIN.INI has no run= line"
mv "$INI.new" "$INI"
# The tree's StartUp group opens Notepad and dials a modem; leave the group out so only the chosen program starts.
sed -i '/^Group5=/Id' "$RUN/c/WINDOWS/PROGMAN.INI"
if [ "$VIDEO" = vga ]; then
	SYS="$RUN/c/WINDOWS/SYSTEM.INI"
	sed -i -e 's/^display\.drv=S3TRIO\.DRV/display.drv=VGA.DRV/I' -e 's/^display=VDDS3764\.386/display=*vddvga/I' \
		-e 's/^display\.drv=S3 Trio64V.*/display.drv=VGA/I' -e 's/^386grabber=.*/386grabber=VGA.3GR/I' "$SYS"
	MACHINE=vgaonly
	GW=640
	GH=480
else
	MACHINE=svga_s3
	GW=800
	GH=600
fi
MEM=32

if [ "$NATIVE" = 1 ]; then CDIR="$RUN/c" DDIR="$RUN/d"; else CDIR=/c DDIR=/d; fi
{
	printf '[sdl]\noutput=%s\nfullscreen=false\nwindowresolution=%sx%s\nautolock=true\nmouse_emulation=locked\n\n' "$OUTPUT" $((GW * SCALE)) $((GH * SCALE))
	printf '[dosbox]\ntitle=Windows 3.11 - click to capture the mouse, Ctrl+F10 to release it\nmachine=%s\nmemsize=%s\n\n' "$MACHINE" "$MEM"
	printf '[cpu]\ncore=normal\ncputype=pentium\ncycles=max\n\n'
	printf '[dos]\nems=false\nxms=true\numb=true\n\n'
	printf '[autoexec]\n@echo off\nmount c %s\nmount d %s\nc:\nset PATH=c:\\windows;c:\\utils\ncd \\windows\nwin /3\nexit\n' "$CDIR" "$DDIR"
} > "$RUN/play.conf"
CONF="$RUN/play.conf"
[ "$NATIVE" = 1 ] || CONF=/run/play.conf

XARGS=(-e "DISPLAY=$DISPLAY" -e SDL_VIDEODRIVER=x11 -e LIBGL_ALWAYS_SOFTWARE=1 -v /tmp/.X11-unix:/tmp/.X11-unix --hostname "$(uname -n)")
XA="${XAUTHORITY:-$HOME/.Xauthority}"
[ ! -f "$XA" ] || XARGS+=(-v "$XA:/tmp/.Xauthority:ro" -e XAUTHORITY=/tmp/.Xauthority)
if [ "$XHOST" = 1 ]; then
	if xhost 2> /dev/null | grep -qi "SI:localuser:$(id -un)\$"; then
		log "xhost already allows $(id -un); leaving it alone"
	else
		xhost +SI:localuser:"$(id -un)" > /dev/null
		XHOST_ADDED=1
		log "xhost: allowed local user $(id -un) for this session; it is taken back on exit"
	fi
fi
if [ "$NATIVE" = 0 ] && ! docker image inspect "$IMAGE" > /dev/null 2>&1; then
	log "building $IMAGE"
	docker build -q -t "$IMAGE" -f "$HERE/dosbox-x.Dockerfile" "$HERE" > /dev/null
fi
[ "$NATIVE" = 1 ] || docker rm -f "$CONTAINER" > /dev/null 2>&1 || true

cat >&2 << EOF

  Windows for Workgroups 3.11 ($VIDEO video) is starting in a ${SCALE}x window. ${CMD:+It will run: $CMD}
    Click inside the window to capture the mouse; press Ctrl+F10 to release it.
    D: holds the Quill files and the sample documents. Win16 Quill is unfinished: see docs/play.md.
    Leave Windows with File > Exit Windows in Program Manager, or just close the window (or press Ctrl+C here). Nothing you do is kept.
    Stop it from another terminal: docker rm -f $CONTAINER
    No sound comes out: the emulated Sound Blaster (the tree's driver needs one) is wired to a silent audio driver.

EOF
rc=0
if [ "$NATIVE" = 1 ]; then
	SDL_AUDIODRIVER=dummy dosbox-x -conf "$CONF" -fastlaunch > "$RUN/dosbox.log" 2>&1 &
	NATIVE_PID=$!
else
	docker run --rm --name "$CONTAINER" --user "$(id -u):$(id -g)" --network host \
		"${XARGS[@]}" -e SDL_AUDIODRIVER=dummy -e HOME=/tmp -v "$RUN/c:/c" -v "$RUN/d:/d" -v "$RUN/play.conf:/run/play.conf:ro" \
		"$IMAGE" dosbox-x -conf "$CONF" -fastlaunch > "$RUN/dosbox.log" 2>&1 &
fi
wait $! || rc=$?
# 130, 137 and 143 are the window closed with a signal, docker rm or Ctrl+C: a normal way to stop.
case "$rc" in 130 | 137 | 143) rc=0 ;; esac
if [ "$rc" -ne 0 ]; then
	FAILED=1
	tail -n 15 "$RUN/dosbox.log" >&2
	log "DOSBox-X exited with status $rc (if it could not open the display, try again with -x; the log is $RUN/dosbox.log)"
fi
exit "$rc"
