#!/usr/bin/env bash
# Runs Liminal Installer's `liminst` on this project, building it and its three runtimes at the pinned commit when needed.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: installer/tools/liminst.sh bin                print the path of the liminst binary (built if missing)
#        installer/tools/liminst.sh runtimes           print DOS=, WIN16= and WIN32= paths of the three SETUP.EXE runtimes
#        installer/tools/liminst.sh build -t ID ...    liminst build, with the target's runtime flag added
#        installer/tools/liminst.sh ARGS...            liminst ARGS... -C installer
#
# `installer/liminst.rev` holds the installer commit this project is built with. The checkout is, in order:
#   $LIMINAL_INSTALLER   a local checkout; a warning says when its HEAD is not the pinned commit
#   $LIMINST_HOME/<sha>  a clone made here (default ~/.cache/quill16-liminst), fetched from $LIMINST_REMOTE
# Build output stays under $LIMINST_HOME, never inside the checkout that $LIMINAL_INSTALLER names. The DOS runtime needs the
# CWSDPMI stub that its `dos/tools/fetch-tools.sh` downloads; set DOS_VENDOR to a folder that already holds it to skip that.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PIN="$(tr -d '[:space:]' <"$HERE/liminst.rev")"
HOME_DIR="${LIMINST_HOME:-$HOME/.cache/quill16-liminst}"
REMOTE="${LIMINST_REMOTE:-https://github.com/liminal-hq-labs/liminal-installer.git}"

checkout() {
	if [ -n "${LIMINAL_INSTALLER:-}" ]; then
		local head
		head="$(git -C "$LIMINAL_INSTALLER" rev-parse HEAD 2>/dev/null || echo unknown)"
		[ "$head" = "$PIN" ] || echo "liminst: $LIMINAL_INSTALLER is at $head, not the pinned $PIN" >&2
		echo "$LIMINAL_INSTALLER"
		return
	fi
	local dir="$HOME_DIR/$PIN"
	if [ ! -d "$dir/.git" ]; then
		mkdir -p "$HOME_DIR"
		git clone -q "$REMOTE" "$dir" >&2
	fi
	if ! git -C "$dir" cat-file -e "$PIN^{commit}" 2>/dev/null; then git -C "$dir" fetch -q origin >&2; fi
	git -C "$dir" checkout -q "$PIN" >&2
	echo "$dir"
}

SRC="$(checkout)"
BUILD="$HOME_DIR/build-${PIN:0:12}"

bin() {
	local exe="$BUILD/cli/release/liminst"
	if [ ! -x "$exe" ] || [ -n "$(find "$SRC/crates" -newer "$exe" -name '*.rs' -print -quit)" ]; then
		(cd "$SRC" && CARGO_TARGET_DIR="$BUILD/cli" cargo build --release --locked -p li-cli) >&2
	fi
	echo "$exe"
}

runtime() {
	case "$1" in
	dos)
		if [ ! -f "$SRC/dos/dist/SETUP.EXE" ]; then
			[ -n "${DOS_VENDOR:-}" ] || bash "$SRC/dos/tools/fetch-tools.sh" >&2
			(cd "$SRC/dos" && CARGO_TARGET_DIR="$BUILD/dos" bash build.sh) >&2
		fi
		echo "$SRC/dos/dist/SETUP.EXE"
		;;
	win16)
		if [ ! -f "$SRC/retro/win16/out/SETUP.EXE" ]; then
			(cd "$SRC/retro/win16" && CARGO_TARGET_DIR="$BUILD/win16" bash build.sh) >&2
		fi
		echo "$SRC/retro/win16/out/SETUP.EXE"
		;;
	win32)
		if [ ! -f "$SRC/win32/dist/SETUP.EXE" ]; then
			(cd "$SRC/win32" && CARGO_TARGET_DIR="$BUILD/win32" bash build.sh) >&2
		fi
		echo "$SRC/win32/dist/SETUP.EXE"
		;;
	esac
}

# The runtime flag for a target id, from the project's targets.
flag_for() {
	case "$1" in
	dos-x86) echo "--dos-runtime $(runtime dos)" ;;
	win16-x86) echo "--win16-runtime $(runtime win16)" ;;
	windows-x86) echo "--win32-runtime $(runtime win32)" ;;
	*) echo "liminst: no runtime for target $1" >&2 && return 1 ;;
	esac
}

cmd="${1:-}"
case "$cmd" in
bin) bin ;;
runtimes)
	echo "DOS=$(runtime dos)"
	echo "WIN16=$(runtime win16)"
	echo "WIN32=$(runtime win32)"
	;;
build)
	shift
	exe="$(bin)"
	target=""
	prev=""
	for a in "$@"; do
		[ "$prev" = "-t" ] || [ "$prev" = "--target" ] && target="$a"
		prev="$a"
	done
	[ -n "$target" ] || { echo "liminst: build needs -t ID" >&2 && exit 2; }
	# Always pass the runtime: without one liminst looks for a `dos/build.sh` above the project, which here is Quill's own.
	# shellcheck disable=SC2046
	exec "$exe" -C "$HERE" build "$@" $(flag_for "$target")
	;;
"")
	echo "usage: installer/tools/liminst.sh bin | runtimes | build -t ID ... | LIMINST_ARGS..." >&2
	exit 2
	;;
*) exec "$(bin)" -C "$HERE" "$@" ;;
esac
