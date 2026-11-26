#!/usr/bin/env bash
# Build the QAGRR disk with `rl` from the pinned reactos-liminal commit: explain, install, image, parts, manifest.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: build-with-rl.sh [--work DIR] [--quill-dir DIR] [--out DIR] [--dry-run] [--check-trim]
#   --work DIR       where rl keeps downloads, disks and scratch (default ~/.cache/reactos-liminal); never inside the repository
#   --quill-dir DIR  folder with QUILL.EXE and QUILL32.EXE (the Quill builds); needed unless --dry-run
#   --out DIR        the outputs: parts/ and build-manifest.json (default WORK/dist)
#   --dry-run        explain the recipe and print the plans for install and image; run nothing
#   --check-trim     prove that rl's base trim profile plus trim-extra.txt removes exactly what guest/trim-list-1..3.txt remove
# Environment:
#   RL_REPO        where to fetch reactos-liminal from (default https://github.com/liminal-hq-labs/reactos-liminal.git)
#   RL_SRC_CACHE   where the pinned checkout goes (default ~/.cache/quill-reactos-liminal); named by the commit
#   RL_QEMU_LOCK   a lock file; when set, `rl install` and `rl image` run under `flock` on it, so two builds never start QEMU together
# Needs git, python3 and what `rl doctor` checks (QEMU, mtools, xorriso, zstd); the QUILLDESK.ICO step needs what guest/make_desk_icon.py needs.
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
while [ $# -gt 0 ]; do
	case $1 in
		--work) work=$2; shift 2 ;;
		--quill-dir) quill_dir=$2; shift 2 ;;
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
	entries "$here/../guest/trim-list-1.txt" "$here/../guest/trim-list-2.txt" "$here/../guest/trim-list-3.txt" | sort > "$old"
	entries "$rl_dir/profiles/base.txt" "$here/trim-extra.txt" | sort > "$new"
	if diff "$old" "$new"; then
		echo "base + trim-extra.txt = the old trim lists: $(wc -l < "$old") entries, entry for entry"
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

# The payload the recipe's [[files]] name: the programs and the desktop icon.
[ -f "$quill_dir/QUILL.EXE" ] && [ -f "$quill_dir/QUILL32.EXE" ] || { echo "--quill-dir needs QUILL.EXE and QUILL32.EXE" >&2; exit 1; }
mkdir -p "$payload"
cp "$quill_dir/QUILL.EXE" "$quill_dir/QUILL32.EXE" "$payload/"
python3 -I "$here/../guest/make_desk_icon.py" "$payload/QUILLDESK.ICO"

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
