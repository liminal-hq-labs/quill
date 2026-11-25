#!/usr/bin/env bash
# The saved-state runs: restore from the state with the whole-file disk and with 1 MiB parts, at three network profiles, plus the Win16 cold boot.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -u
cd "$(dirname "$0")" || exit 1
CD=/images/quill-demo.iso
PARTS=/images/parts/spike-a/.img.zst
ST=/images/state-q32.l19.bin.zst
m() { local name="$1"; shift; echo "== $name  load $(cut -d' ' -f1 /proc/loadavg)"; node measure.ts --name "$name" "$@" --param cdrom=$CD 2>&1 | grep -v '^ \|^{\|^}' | head -8; }
m restore-whole --param state=$ST
m restore-chunked --param state=$ST --param disk=$PARTS --param chunk=1048576
m restore-whole-50mbit --profile 50mbit --param state=$ST
m restore-chunked-50mbit --profile 50mbit --param state=$ST --param disk=$PARTS --param chunk=1048576
m restore-whole-10mbit --profile 10mbit --param state=$ST
m restore-whole-l3 --param state=/images/state-q32.l3.bin.zst
m restore-whole-b --param state=$ST
m cold-chunked --app quill32 --param disk=$PARTS --param chunk=1048576
m cold-quill16 --app quill16
echo ALLDONE
