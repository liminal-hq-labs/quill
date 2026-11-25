#!/usr/bin/env bash
# Type a string into the build VM through the monitor's sendkey, one key at a time (the Run box crashes Explorer if text arrives in a burst).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
W=${VMDIR:-$HOME/.cache/reactos/web/p2}
HERE=$(dirname "$0")
s="$1"
for ((i = 0; i < ${#s}; i++)); do
	c="${s:i:1}"
	case "$c" in
		' ') k=spc ;; \\) k=backslash ;; '.') k="dot" ;; ':') k=shift-semicolon ;; '/') k=slash ;; '-') k=minus ;; '_') k=shift-minus ;;
		',') k=comma ;; '>') k=shift-dot ;; '&') k=shift-7 ;; '"') k=shift-apostrophe ;; '=') k=equal ;; '(') k=shift-9 ;; ')') k=shift-0 ;;
		'%') k=shift-5 ;; ';') k=semicolon ;;
		[A-Z]) k="shift-$(echo "$c" | tr A-Z a-z)" ;;
		*) k="$c" ;;
	esac
	echo "sendkey $k"
	sleep 0.12
done | VMDIR="$W" python3 -I "$HERE/mon.py" > /dev/null
