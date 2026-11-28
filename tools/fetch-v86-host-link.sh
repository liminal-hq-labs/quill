#!/usr/bin/env bash
# Refreshes site/try-reactos/vendor/v86-host-link/ from one commit of the local v86-host-link repository: the page module, the guest programs and the
# Host Link Kit CD, each built from a clean checkout with that repository's own commands, then SHA256SUMS and PROVENANCE.md.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/fetch-v86-host-link.sh <commit> [repo path]     (the repo defaults to ~/source/liminal-hq-labs/v86-host-link)
#
# The repository is only read: the commit is checked out detached in its own throwaway worktree under ~/source/worktrees/v86-host-link/ (removed
# afterwards), and Cargo's output goes to ~/.cache/v86-host-link-target/ (CARGO_TARGET_DIR and WIN32_TARGET_DIR override). Quill never edits the
# vendored files: this script is the only way they change, and tools/check-v86-host-link.sh holds them to SHA256SUMS.
set -euo pipefail

commit="${1:?usage: tools/fetch-v86-host-link.sh <commit> [repo path]}"
repo="${2:-$HOME/source/liminal-hq-labs/v86-host-link}"
quill="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="$quill/site/try-reactos/vendor/v86-host-link"
targets="$HOME/.cache/v86-host-link-target"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$targets/host}"
export WIN32_TARGET_DIR="${WIN32_TARGET_DIR:-$targets/win32}"

full="$(git -C "$repo" rev-parse --verify "$commit^{commit}")"
short="${full:0:8}"
work="$HOME/source/worktrees/v86-host-link/vendor-$short"
[ ! -e "$work" ] || { echo "fetch-v86-host-link: $work exists; remove it with 'git -C $repo worktree remove --force $work' first" >&2; exit 1; }
mkdir -p "$(dirname "$work")"
git -C "$repo" worktree add --detach "$work" "$full" > /dev/null
cleanup() {
	git -C "$repo" worktree remove --force "$work" || true
	git -C "$repo" worktree prune || true
}
trap cleanup EXIT

# The product guest programs. The first three must exist; the modules a commit does not have yet are skipped. The test pieces in dist/test/ and
# HLCLIP.EXE are never copied.
required="HOSTLINK.EXE HLAUDIO.DLL HLTRAY.EXE"
optional="HLSYS.DLL HLPOWER.DLL HLLOG.DLL"

echo "fetch-v86-host-link: building $short in $work"
(
	cd "$work"
	make win32
	make kit
	(cd page && bun install --frozen-lockfile > /dev/null && bun run check:built)
)

rm -rf "$out"
mkdir -p "$out/guest" "$out/kit"
cp "$work/page/dist/host-link.js" "$out/host-link.js"
if [ -f "$work/page/dist/host-link.d.ts" ]; then cp "$work/page/dist/host-link.d.ts" "$out/host-link.d.ts"; fi
for name in $required; do cp "$work/helpers/win32/dist/$name" "$out/guest/$name"; done
for name in $optional; do
	if [ -f "$work/helpers/win32/dist/$name" ]; then cp "$work/helpers/win32/dist/$name" "$out/guest/$name"; fi
done
cp "$work/kit/dist/hostlink-kit.iso" "$out/kit/hostlink-kit.iso"

(cd "$out" && find . -type f ! -name SHA256SUMS ! -name PROVENANCE.md | LC_ALL=C sort | sed 's|^\./||' | xargs sha256sum > SHA256SUMS)

subject="$(git -C "$repo" log -1 --format=%s "$full")"
{
	cat <<TXT
# Provenance of the vendored Host Link

Everything in this folder was built from one commit of the \`v86-host-link\` repository, which is local (no remote) at \`~/source/liminal-hq-labs/v86-host-link\`, and copied here by \`tools/fetch-v86-host-link.sh\`. Quill never edits these files: a fix goes to \`v86-host-link\` first and comes back by running the script again. \`tools/check-v86-host-link.sh\` holds every file to \`SHA256SUMS\` and fails on a test-only file.

- **Commit:** \`$full\` (${subject//\`/\'})
- **Licence:** Apache-2.0 OR MIT, the repository's own
- **Built with:** \`make win32\` (the guest programs: Rust, no C compiler, no C library, a Pentium target; the budgets and the import and instruction scans are that command's checks), \`make kit\` (the Host Link Kit CD, \`kit/dist/hostlink-kit.iso\`) and \`bun run check:built\` in \`page/\` (the module is the one the repository commits, shown equal to what its package builds)

| File | Bytes | SHA-256 | What it is |
|---|---:|---|---|
TXT
	while read -r sum file; do
		what=""
		case "$file" in
		host-link.js) what="The page's end of the link, one minified ES module" ;;
		host-link.d.ts) what="Its type declarations" ;;
		guest/HOSTLINK.EXE) what="The guest helper: owns COM1, the clipboard, files, time and the screen's size, and loads the modules" ;;
		guest/HLAUDIO.DLL) what="The audio module" ;;
		guest/HLSYS.DLL) what="The system information module" ;;
		guest/HLPOWER.DLL) what="The power module" ;;
		guest/HLLOG.DLL) what="The log module" ;;
		guest/HLTRAY.EXE) what="The tray icon and the Sound card window" ;;
		kit/hostlink-kit.iso) what="The Host Link Kit CD (setup, the helpers, AUTORUN.INF)" ;;
		esac
		printf '| `%s` | %s | `%s` | %s |\n' "$file" "$(wc -c < "$out/$file")" "$sum" "$what"
	done < "$out/SHA256SUMS"
	cat <<'TXT'

The helper looks for `HL*.DLL` beside itself, so the guest programs go together into one folder (`C:\Quill\` in the QAGRR image). The test-only programs (`HLTEST*.DLL`, `HLBRK.EXE`, `HLCLIP.EXE`, `HOSTLINKW.EXE`, `HLTRAYT.EXE`, the audio probe) are never vendored.
TXT
} > "$out/PROVENANCE.md"
echo "fetch-v86-host-link: wrote $out ($short)"
