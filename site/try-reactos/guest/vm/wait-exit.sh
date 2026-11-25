#!/usr/bin/env bash
# Wait for this VMDIR's build VM to power off (the guest's own shutdown). Usage: wait-exit.sh [SECONDS]; fails on timeout.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
t=${1:-120}
W=${VMDIR:-$HOME/.cache/reactos/web/p2}
while [ -f "$W/vm.pid" ] && kill -0 "$(cat "$W/vm.pid")" 2>/dev/null; do
	sleep 1
	t=$((t - 1))
	[ $t -gt 0 ] || { echo "VM still running" >&2; exit 1; }
done
