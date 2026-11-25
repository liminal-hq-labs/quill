#!/usr/bin/env bash
# Phase 2 measurements on the trimmed guest: cold boot (whole-file disk and 1 MiB parts) and restore from the saved state, at the three
# network profiles. Needs the server running, the parts under ~/.cache/reactos/web/parts/trim and the compressed state.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -u
cd "$(dirname "$0")" || exit 1
PARTS=/images/parts/trim/.img.zst
ST19=/images/state-final.l19.bin.zst
ST22=/images/state-final.l22.bin.zst
m() { local name="$1"; shift; echo "== $name  load $(cut -d' ' -f1 /proc/loadavg)"; node measure.ts --name "$name" "$@" 2>&1 | grep -v '^ \|^{\|^}' | head -8; }
m p2-cold-whole --app auto
m p2-cold-parts --app auto --param disk=$PARTS --param chunk=1048576
m p2-restore-whole-l19 --param state=$ST19
m p2-restore-whole-l22 --param state=$ST22
m p2-restore-parts-l22 --param state=$ST22 --param disk=$PARTS --param chunk=1048576
m p2-restore-whole-l22-50 --param state=$ST22 --profile 50mbit
m p2-restore-whole-l22-10 --param state=$ST22 --profile 10mbit
m p2-restore-whole-l19-50 --param state=$ST19 --profile 50mbit
m p2-cold-parts-50 --app auto --param disk=$PARTS --param chunk=1048576 --profile 50mbit
m p2-cold-whole-50 --app auto --profile 50mbit
m p2-cold-whole-10 --app auto --profile 10mbit
