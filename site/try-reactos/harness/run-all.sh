#!/usr/bin/env bash
# Run the spike A measurement set one after another (server must be up). Results land in out/results/.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -u
cd "$(dirname "$0")" || exit 1
CD=/images/quill-demo.iso
PARTS=/images/parts/spike-a/.img.zst
ST=/images/state-q32.l19.bin.zst
m() { local name="$1"; shift; echo "== $name"; node measure.ts --name "$name" "$@" --param cdrom=$CD 2>&1 | grep -v '^ \|^{\|^}' | head -8; }
m restore-chunked --param state=$ST --param disk=$PARTS --param chunk=1048576
m restore-whole-50mbit --profile 50mbit --param state=$ST
m restore-chunked-50mbit --profile 50mbit --param state=$ST --param disk=$PARTS --param chunk=1048576
m restore-whole-10mbit --profile 10mbit --param state=$ST
m restore-chunked-10mbit --profile 10mbit --param state=$ST --param disk=$PARTS --param chunk=1048576
m restore-whole-l3 --param state=/images/state-q32.l3.bin.zst
m cold-whole --app quill32
m cold-chunked --app quill32 --param disk=$PARTS --param chunk=1048576
m cold-whole-50mbit --app quill32 --profile 50mbit
m cold-chunked-50mbit --app quill32 --profile 50mbit --param disk=$PARTS --param chunk=1048576
m cold-whole-10mbit --app quill32 --profile 10mbit
m cold-whole-ram128 --app quill32 --param ram=128
m cold-whole-noacpi --app quill32 --param acpi=0
m cold-quill16 --app quill16
echo ALLDONE
