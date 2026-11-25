#!/usr/bin/env bash
# Runs the Win16 paint benchmark (QUILLB.EXE, built with W16_FEATURES=bench) on Windows 3.11 under DOSBox-X and prints the table.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [VIDEO=s3|vga] [CYCLES=N] [SCRIPT=FILE] [OUT=DIR] [KEEP=1] [NO_BUILD=1] win16/tools/bench-w31.sh [QUILLB.EXE]
#
# 1. Builds QUILLB.EXE (`W16_FEATURES=bench win16/build.sh`, into $OUT_DIR or win16/out-bench) unless one is named or NO_BUILD is set.
# 2. Copies it, the key script (tests/bench/w16-type.txt) and tests/docs/letter.qld to D:, writes QUILL.CMD with
#    `/BENCH W16-TYPE.TXT /END` (Windows 3.1's run= line takes no arguments) and starts Windows 3.11, which starts D:\QUILLB.EXE.
#    The program sends each key through its window procedure, paints it, writes D:\BENCH.LOG and ends Windows, which ends DOSBox-X.
# 3. Prints the table (win16/tools/bench_report.py) and checks it against win16/bench-budget.json. Call counts are the gate; times
#    are reported only. The counts do not depend on the machine, so they must be the same on every run and on ReactOS under WineVDM.
#
# The emulator runs at a fixed number of cycles (CYCLES, default 60000) so the times can be compared between runs; `cycles=max` is not
# repeatable. VIDEO=s3 (default) is the tree's S3 Trio64V driver at 800x600 in 64K colours and VIDEO=vga is VGA.DRV at 640x480 in 16
# colours. OUT=DIR keeps BENCH.LOG and the table there. Needs Docker with quill16-dosbox-x:local (built on first use), Xvfb and python3.
# W31_TREE names the Windows 3.11 tree (default ~/.cache/quill16-play/w31-base, only read); the tree is the owner's and never committed,
# so this is a manual test and exits 0 with a note when the tree is missing. DOSBox-X runs in a container of its own on a virtual X
# server of its own, stopped by its process id. WAIT (seconds, default 900) is how long the run may take.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
TREE="${W31_TREE:-$HOME/.cache/quill16-play/w31-base}"
IMAGE="${W31_IMAGE:-quill16-dosbox-x:local}"
NAME="${W31_BENCH_CONTAINER:-quill16-bench-w31}"
BASE="${QUILL_BENCH_DIR:-$HOME/.cache/quill16-bench}"
VIDEO="${VIDEO:-s3}"
CYCLES="${CYCLES:-60000}"
SCRIPT="${SCRIPT:-$ROOT/tests/bench/w16-type.txt}"
WAIT="${WAIT:-900}"

log() { printf '[bench-w31 %s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
fail() {
	log "FAIL: $*"
	exit 1
}

if [ ! -f "$TREE/WINDOWS/WIN.COM" ]; then
	log "no Windows 3.11 tree at $TREE; skipping (set W31_TREE)"
	exit 0
fi
case "$VIDEO" in s3 | vga) ;; *) fail "VIDEO takes s3 or vga" ;; esac
for t in docker Xvfb python3; do command -v "$t" > /dev/null || fail "$t not found"; done

EXE="${1:-}"
if [ -z "$EXE" ]; then
	OUT_DIR="${OUT_DIR:-$ROOT/win16/out-bench}"
	if [ -z "${NO_BUILD:-}" ]; then
		log "building QUILLB.EXE"
		(cd "$ROOT/win16" && OUT_DIR="$OUT_DIR" W16_FEATURES=bench ./build.sh > /dev/null) || fail "the build failed"
	fi
	EXE="$OUT_DIR/QUILLB.EXE"
fi
[ -f "$EXE" ] || fail "$EXE is missing"
if ! docker image inspect "$IMAGE" > /dev/null 2>&1; then
	docker build -q -t "$IMAGE" -f "$HERE/dosbox-x.Dockerfile" "$HERE" > /dev/null
fi

mkdir -p "$BASE"
WORK="$(mktemp -d "$BASE/run.XXXXXX")"
XPID=""
cleanup() {
	set +e
	docker rm -f "$NAME" > /dev/null 2>&1
	[ -z "$XPID" ] || { kill "$XPID" 2> /dev/null && wait "$XPID" 2> /dev/null; }
	[ -n "${KEEP:-}" ] || rm -rf "$WORK"
}
trap cleanup EXIT

# A private X server on a free display number, stopped by its own process id.
unset WAYLAND_DISPLAY XAUTHORITY
n=90
while [ -e "/tmp/.X11-unix/X$n" ] || [ -e "/tmp/.X$n-lock" ]; do n=$((n + 1)); done
Xvfb ":$n" -screen 0 1024x768x24 -ac -nolisten tcp > /dev/null 2>&1 &
XPID=$!
export DISPLAY=":$n"
for _ in $(seq 1 50); do
	[ -S "/tmp/.X11-unix/X$n" ] && break
	sleep 0.1
done
[ -S "/tmp/.X11-unix/X$n" ] || fail "the virtual X server did not start"

mkdir -p "$WORK/d" "$WORK/conf"
cp -a --reflink=auto "$TREE" "$WORK/c"
C="$WORK/c/WINDOWS"
sed -i '/^Group5=/Id' "$C/PROGMAN.INI"
cp "$EXE" "$WORK/d/QUILLB.EXE"
cp "$SCRIPT" "$WORK/d/W16-TYPE.TXT"
cp "$ROOT/tests/docs/letter.qld" "$WORK/d/LETTER.QLD"
printf '/BENCH W16-TYPE.TXT /END\r\n' > "$WORK/d/QUILL.CMD"
awk 'BEGIN { IGNORECASE = 1 } /^[ \t]*run[ \t]*=/ { printf "run=D:\\QUILLB.EXE\r\n"; done = 1; next } { print } END { if (!done) exit 3 }' "$C/WIN.INI" > "$C/WIN.INI.new" || fail "WIN.INI has no run= line"
mv "$C/WIN.INI.new" "$C/WIN.INI"
if [ "$VIDEO" = vga ]; then
	sed -i -e 's/^display\.drv=S3TRIO\.DRV/display.drv=VGA.DRV/I' -e 's/^display=VDDS3764\.386/display=*vddvga/I' \
		-e 's/^display\.drv=S3 Trio64V.*/display.drv=VGA/I' -e 's/^386grabber=.*/386grabber=VGA.3GR/I' "$C/SYSTEM.INI"
	MACHINE=vgaonly
else
	MACHINE=svga_s3
fi

cat > "$WORK/conf/run.conf" << CONF
[sdl]
output=surface
fullscreen=false
windowresolution=800x600
autolock=false
[dosbox]
machine=$MACHINE
memsize=32
[cpu]
core=normal
cputype=pentium
cycles=fixed $CYCLES
[dos]
ems=false
xms=true
umb=true
[autoexec]
@echo off
mount c /c
mount d /d
c:
set PATH=c:\windows;c:\utils
cd \windows
win /3
exit
CONF
docker rm -f "$NAME" > /dev/null 2>&1 || true
docker run -d --name "$NAME" --user "$(id -u):$(id -g)" --network host -v /tmp/.X11-unix:/tmp/.X11-unix -e DISPLAY="$DISPLAY" \
	-e SDL_AUDIODRIVER=dummy -e HOME=/tmp -v "$WORK/c:/c" -v "$WORK/d:/d" -v "$WORK/conf:/conf" --entrypoint dosbox-x \
	"$IMAGE" -conf /conf/run.conf -nomenu -fastlaunch > /dev/null
start="$(date +%s)"
log "Windows 3.11 ($VIDEO, $CYCLES cycles) is running the script"

# The program writes BENCH.LOG when the script is done and ends Windows, which ends the emulator.
while [ "$(docker inspect -f '{{.State.Running}}' "$NAME" 2> /dev/null)" = true ]; do
	[ $(($(date +%s) - start)) -lt "$WAIT" ] || fail "no end after $WAIT s (BENCH.LOG: $([ -f "$WORK/d/BENCH.LOG" ] && echo written || echo missing))"
	sleep 2
done
[ -f "$WORK/d/BENCH.LOG" ] || fail "the emulator ended without a BENCH.LOG"
log "done after $(($(date +%s) - start)) s"
if [ -n "${OUT:-}" ]; then
	mkdir -p "$OUT"
	cp "$WORK/d/BENCH.LOG" "$OUT/bench-$VIDEO.log"
fi
if [ -f "$WORK/d/STACK.LOG" ]; then log "$(tr -d '\r\n' < "$WORK/d/STACK.LOG")"; fi
if [ -n "${OUT:-}" ]; then
	python3 -I "$HERE/bench_report.py" --budget "$ROOT/win16/bench-budget.json" "$WORK/d/BENCH.LOG" | tee "$OUT/bench-$VIDEO.txt"
else
	python3 -I "$HERE/bench_report.py" --budget "$ROOT/win16/bench-budget.json" "$WORK/d/BENCH.LOG"
fi
