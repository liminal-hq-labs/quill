#!/usr/bin/env bash
# Runs a Win16 program in Windows for Workgroups 3.11 under DOSBox-X (in Docker, on an Xvfb display) and takes a screenshot.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win16/tools/run-w31.sh PROGRAM.EXE SHOTS_PREFIX [WAIT_SECONDS]
#
# Needs, in the environment: W31_TREE, a scratch directory holding the unpacked Windows 3.11 tree (the zip the owner supplies;
# never committed), and W31_WORK, a scratch directory for this run. The program is copied to W31_WORK/d, mounted as D:, and put in
# WIN.INI as the \`run=\` line, so whatever it writes next to itself lands in W31_WORK/d. The profile comes from W31_CONF (a file name
# in win16/tools/dosbox-x/, default fast.conf). The DOSBox-X image is quill16-dosbox-x:local (debian:trixie-slim plus the dosbox-x
# package); build it with win16/tools/dosbox-x.Dockerfile. With W31_SHOT_EVERY=N a screenshot is taken every N seconds instead (SHOTS_PREFIX-001.png on). Screenshots are taken at WAIT_SECONDS/3, 2/3 and the end, as
# SHOTS_PREFIX-1.png to -3.png. W31_ARGS, when set, is the command line after the program's name. The script returns when DOSBox-X exits (the program ends Windows) or after WAIT_SECONDS.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
PROG="$1"
PREFIX="$2"
WAIT="${3:-120}"
: "${W31_TREE:?set W31_TREE to the unpacked Windows 3.11 tree}"
: "${W31_WORK:?set W31_WORK to a scratch directory}"
DISP="${W31_DISPLAY:-:97}"
CONT="${W31_CONTAINER:-quill16-dbx}"

mkdir -p "$W31_WORK/d"
find "$W31_WORK/d" -maxdepth 1 -type f \( -name '*.LOG' -o -name '*.log' \) -exec rm -f {} + 2> /dev/null || true
cp "$PROG" "$W31_WORK/d/"
name="$(basename "$PROG")"
sed -i "s|^run=.*|run=D:\\\\$name ${W31_ARGS:-}|" "$W31_TREE/WINDOWS/WIN.INI"
pgrep -f "Xvfb $DISP" > /dev/null || (Xvfb "$DISP" -screen 0 1024x768x24 > "$W31_WORK/xvfb.log" 2>&1 &)
sleep 1
docker rm -f $CONT > /dev/null 2>&1 || true
docker run -d --name $CONT --network host -v /tmp/.X11-unix:/tmp/.X11-unix -e DISPLAY="$DISP" -e SDL_AUDIODRIVER=dummy \
	-e HOME=/tmp -v "$W31_TREE":/c -v "$W31_WORK/d":/d -v "$HERE/dosbox-x":/conf quill16-dosbox-x:local \
	dosbox-x -conf "/conf/${W31_CONF:-fast.conf}" -nomenu -fastlaunch > /dev/null
if [ -n "${W31_SHOT_EVERY:-}" ]; then
	# A screenshot every W31_SHOT_EVERY seconds, as SHOTS_PREFIX-001.png and so on, until DOSBox-X exits or WAIT passes.
	t=0
	k=1
	while [ "$t" -lt "$WAIT" ] && [ "$(docker inspect -f '{{.State.Running}}' $CONT)" = true ]; do
		sleep "$W31_SHOT_EVERY"
		t=$((t + W31_SHOT_EVERY))
		DISPLAY="$DISP" import -window root "$PREFIX-$(printf %03d $k).png" 2> /dev/null || true
		k=$((k + 1))
	done
	docker rm -f $CONT > /dev/null 2>&1 || true
	exit 0
fi
third=$((WAIT / 3))
for i in 1 2 3; do
	slept=0
	while [ "$slept" -lt "$third" ]; do
		sleep 2
		slept=$((slept + 2))
		[ "$(docker inspect -f '{{.State.Running}}' $CONT)" = true ] || break
	done
	if [ "$(docker inspect -f '{{.State.Running}}' $CONT)" = true ]; then
		DISPLAY="$DISP" import -window root "$PREFIX-$i.png"
	else
		break
	fi
done
docker rm -f $CONT > /dev/null 2>&1 || true
