#!/usr/bin/env bash
# Build the QAGRR disk with `rl` from the pinned reactos-liminal commit: explain, install, image, parts, manifest.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: build-with-rl.sh [--work DIR] [--quill-dir DIR | --build-quill] [--allow-stale-quill] [--out DIR] [--dry-run] [--check-trim]
#   --work DIR       where rl keeps downloads, disks and scratch (default ~/.cache/reactos-liminal); never inside the repository
#   --quill-dir DIR  folder with QUILL.EXE and QUILL32.EXE (the Quill builds); needed unless --dry-run
#   --build-quill    build QUILL.EXE (win16/build.sh) and QUILL32.EXE (win9x/build.sh) from this checkout into WORK/quill-dir and use them, so the disk never
#                    carries a program older than the source (needs the Rust toolchain the repository pins; CARGO_BUILD_JOBS applies)
#   --allow-stale-quill  accept a --quill-dir whose programs are older than the last commit that touched the Win16, Win32, core or asset sources
#   --out DIR        the outputs: parts/ and build-manifest.json (default WORK/dist)
#   --dry-run        explain the recipe and print the plans for install and image; run nothing
#   --check-trim     prove that rl's base trim profile plus trim-extra.txt removes what guest/trim-list-1..3.txt remove, less the Applications Manager and browser stub entries rl keeps
# Environment:
#   RL_REPO        where to fetch reactos-liminal from (default https://github.com/liminal-hq-labs/reactos-liminal.git)
#   RL_SRC_CACHE   where the pinned checkout goes (default ~/.cache/quill-reactos-liminal); named by the commit
#   RL_QEMU_LOCK   a lock file; when set, `rl install` and `rl image` run under `flock` on it, so two builds never start QEMU together
# Needs git, python3 and what `rl doctor` checks (QEMU, mtools, xorriso, zstd); the QUILLDESK.ICO and wallpaper steps need what guest/make_desk_icon.py and guest/quillify.py need (numpy, rsvg-convert, ImageMagick 7).
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(git -C "$here" rev-parse --show-toplevel)
recipe=$here/qagrr.toml
payload=$here/payload

work=${RL_WORK:-$HOME/.cache/reactos-liminal}
quill_dir=
out=
dry_run=0
check_trim=0
build_quill=0
allow_stale=0
while [ $# -gt 0 ]; do
	case $1 in
		--work) work=$2; shift 2 ;;
		--quill-dir) quill_dir=$2; shift 2 ;;
		--build-quill) build_quill=1; shift ;;
		--allow-stale-quill) allow_stale=1; shift ;;
		--out) out=$2; shift 2 ;;
		--dry-run) dry_run=1; shift ;;
		--check-trim) check_trim=1; shift ;;
		*) echo "unknown argument: $1" >&2; exit 2 ;;
	esac
done
out=${out:-$work/dist}
case $work in "$repo"/*) echo "--work must be outside the repository: $work" >&2; exit 2 ;; esac

pin=$(grep -v '^[[:space:]]*#' "$here/reactos-liminal.pin" | grep -E '^[0-9a-f]{40}$' | head -1)
[ -n "$pin" ] || { echo "reactos-liminal.pin holds no 40-digit commit hash" >&2; exit 1; }

# Fetch exactly the pinned commit into the cache, outside the repository, and check what was checked out.
source_cache=${RL_SRC_CACHE:-$HOME/.cache/quill-reactos-liminal}
rl_dir=$source_cache/$pin
if [ "$(git -C "$rl_dir" rev-parse HEAD 2>/dev/null || true)" != "$pin" ]; then
	rm -rf "$rl_dir"
	mkdir -p "$rl_dir"
	git -C "$rl_dir" init -q
	git -C "$rl_dir" fetch -q --depth 1 "${RL_REPO:-https://github.com/liminal-hq-labs/reactos-liminal.git}" "$pin"
	git -C "$rl_dir" checkout -q --detach FETCH_HEAD
	[ "$(git -C "$rl_dir" rev-parse HEAD)" = "$pin" ] || { echo "fetched the wrong commit" >&2; exit 1; }
fi
rl() { "$rl_dir/rl" "$@"; }
locked() {
	if [ -n "${RL_QEMU_LOCK:-}" ]; then flock "$RL_QEMU_LOCK" "$@"; else "$@"; fi
}

entries() {   # the f/d lines of a trim list, without comments and blanks
	grep -hEv '^[[:space:]]*(#|$)' "$@"
}

if [ "$check_trim" = 1 ]; then
	old=$(mktemp)
	new=$(mktemp)
	trap 'rm -f "$old" "$new"' EXIT
	# rl's base profile keeps the Applications Manager (rapps.exe and its Start menu link, RL-42) and the browser stub (iexplore.exe, RL-40),
	# which the old lists removed.
	entries "$here/../guest/trim-list-1.txt" "$here/../guest/trim-list-2.txt" "$here/../guest/trim-list-3.txt" \
		| grep -vxF -e 'f ReactOS/system32/rapps.exe' -e 'd Program Files/Internet Explorer' -e 'f Documents and Settings/Administrator/Start Menu/Programs/ReactOS Applications Manager.lnk' | sort > "$old"
	entries "$rl_dir/profiles/base.txt" "$here/trim-extra.txt" | sort > "$new"
	if diff "$old" "$new"; then
		echo "base + trim-extra.txt = the old trim lists less the three entries rl keeps (Applications Manager, browser stub): $(wc -l < "$old") entries, entry for entry"
		exit 0
	fi
	echo "base + trim-extra.txt differs from the old trim lists" >&2
	exit 1
fi

echo "== rl explain"
rl explain "$recipe" > /dev/null
rl explain "$recipe" | python3 -I -c 'import json, sys; recipe = json.load(sys.stdin)["recipe"]; print(json.dumps({key: recipe[key] for key in ("recipe", "install", "trim", "settings")}, indent=2)); print(len(recipe["files"]), "files,", len(recipe["shortcuts"]), "shortcuts")'

if [ "$dry_run" = 1 ]; then
	echo "== rl install --dry-run"
	rl install "$recipe" --dry-run --work "$work"
	echo "== rl image --dry-run"
	rl image "$recipe" --dry-run --work "$work"
	exit 0
fi

if [ "$build_quill" = 1 ]; then
	[ -z "$quill_dir" ] || { echo "--build-quill and --quill-dir exclude each other" >&2; exit 2; }
	quill_dir=$work/quill-dir
	echo "== build QUILL.EXE and QUILL32.EXE into $quill_dir"
	mkdir -p "$quill_dir"
	OUT_DIR="$quill_dir" bash "$repo/win16/build.sh"
	# win9x/build.sh writes win9x/dist/QUILL32.EXE; the Win16 build's other programs and map files stay out of the payload.
	bash "$repo/win9x/build.sh"
	cp "$repo/win9x/dist/QUILL32.EXE" "$quill_dir/QUILL32.EXE"
fi

# A program older than its sources is the mistake this guards against: a folder cached from an earlier build made a disk whose Windows 3.x
# Quill crashed on a bug that main had since fixed.
if [ "$allow_stale" = 0 ] && [ -f "$quill_dir/QUILL.EXE" ] && [ -f "$quill_dir/QUILL32.EXE" ]; then
	newest=$(git -C "$repo" log -1 --format=%ct -- win16 win9x crates assets Cargo.lock 2>/dev/null || echo 0)
	for name in QUILL.EXE QUILL32.EXE; do
		if [ "$(stat -c %Y "$quill_dir/$name")" -lt "${newest:-0}" ]; then
			echo "$quill_dir/$name is older than the last commit that touched the sources; rebuild it (--build-quill) or pass --allow-stale-quill" >&2
			exit 1
		fi
	done
fi

# The payload the recipe's [[files]] name: the programs, the desktop icon and the wallpaper.
if [ ! -f "$quill_dir/QUILL.EXE" ] || [ ! -f "$quill_dir/QUILL32.EXE" ]; then echo "--quill-dir needs QUILL.EXE and QUILL32.EXE" >&2; exit 1; fi
mkdir -p "$payload"
cp "$quill_dir/QUILL.EXE" "$quill_dir/QUILL32.EXE" "$payload/"
python3 -I "$here/../guest/make_desk_icon.py" "$payload/QUILLDESK.ICO"
python3 -I "$here/../guest/make_desk_icon.py" --win31 "$payload/QUILL31DESK.ICO"

# The wallpaper: the Afterglow scene the recipe pins, with the Quill emblem on its sun, cut to 1024 x 768 with Afterglow's own seeded dither.
# `rl fetch` puts the pinned source in WORK/afterglow/COMMIT; Quill renders its overlay itself and the recipe lists the BMP in [[files]].
echo "== rl fetch"
rl fetch "$recipe" --work "$work"
read -r afterglow_commit scene < <(rl explain "$recipe" | python3 -I -c 'import json, sys; afterglow = json.load(sys.stdin)["recipe"]["inputs"]["afterglow"]; print(afterglow["commit"], afterglow["wallpaper"])')
python3 -I "$here/../guest/quillify.py" "$work/afterglow/$afterglow_commit/assets/wallpapers/afterglow-$scene.svg" "$payload/quill-wallpaper.bmp" --dither 1

echo "== rl install"
locked "$rl_dir/rl" install "$recipe" --work "$work"
echo "== rl image"
locked "$rl_dir/rl" image "$recipe" --work "$work"
image=$work/images/qagrr.img

echo "== rl parts"
rl parts "$recipe" "$image" --out "$out/parts" --work "$work"

echo "== rl manifest"
inputs=()
for name in QUILL.EXE QUILL32.EXE; do inputs+=(--input "$name=$(sha256sum "$payload/$name" | cut -d' ' -f1)"); done
rl manifest "$recipe" "$out" "${inputs[@]}" \
	--extra "quill_commit=${QUILL_COMMIT:-unknown}" --extra "reactos_liminal_commit=$pin"
echo "done: $out"
