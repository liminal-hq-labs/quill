#!/usr/bin/env bash
# Build the whole ReactOS demo image from the official install CD to the final artifacts, with no hand step and nothing large in git.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: guest/build-all.sh            (from anywhere in the repository)
#
# Stages, each timed: install-golden.sh (unattended ReactOS install onto a raw disk) -> build-image.sh (trim, install Quill, apply the
# settings in a build VM) -> zero-free.py (zero free clusters, punch holes) -> tools/make-parts.mjs (1 MiB zstd parts) -> harness/serve.ts
# and harness/build-state.ts in headless Chromium (boot in v86, save the machine state) -> zstd -22 --ultra.
#
# Outputs, in $DIST (default $WORK/dist): parts/ (manifest.json and the zstd parts of the disk), state.l22.bin.zst (the v86 saved state)
# and build-manifest.json (SHA-256 and size of every output, the pinned versions, the stage timings, the peak disk use).
#
# Environment:
#   WORK         where every large file goes (default ~/.cache/reactos/web/ci/work); never inside the repository
#   DIST         the outputs (default $WORK/dist)
#   QUILL_DIR    folder with QUILL.EXE and QUILL32.EXE, and optionally DEMO.QLD, DEMO.RTF, DEMO16.QLD, DEMO16.RTF; without it the builds are
#                taken from QUILL_ISO (default ~/.cache/reactos/quill-latest.iso) and the demo documents from site/try-reactos/demo/
#   QUILL_COMMIT the commit the Quill builds came from, recorded in the manifest (default: unknown)
#   GOLDEN_IMG   skip the install and start from this golden disk (read only)
#   CI_DIR, DL_DIR, ACCEL, STAGE_TIMEOUT   as install-golden.sh; PORT (default 18086) for the harness server; CHROMIUM (default /usr/bin/chromium)
#   AFTERGLOW    1 (default) builds the Afterglow theme from the pinned source (afterglow.pin; prepare-afterglow.sh needs the network, numpy,
#                llvm-rc, lld-link, rsvg-convert and ImageMagick); 0 keeps ReactOS's classic look. AFTERGLOW_DIR (a folder made by
#                prepare-afterglow.sh) is used as it is instead of building
#   BOOT_WAIT    as build-image.sh; STATE_TIMEOUT seconds for the state build (default 1500; 7200 without KVM)
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
TRY=$(cd "$HERE/.." && pwd)
REPO=$(git -C "$HERE" rev-parse --show-toplevel)
CI_DIR=${CI_DIR:-$HOME/.cache/reactos/web/ci}
DL_DIR=${DL_DIR:-$HOME/.cache/reactos/dl}
WORK=${WORK:-$CI_DIR/work}
DIST=${DIST:-$WORK/dist}
PORT=${PORT:-18086}
export CI_DIR DL_DIR CHROMIUM=${CHROMIUM:-/usr/bin/chromium} MTOOLS_SKIP_CHECK=1
if [ -z "${ACCEL:-}" ]; then if [ -w /dev/kvm ]; then ACCEL=kvm; else ACCEL=tcg; fi; fi
export ACCEL
if [ "$ACCEL" = kvm ]; then STATE_TIMEOUT=${STATE_TIMEOUT:-1500}; else STATE_TIMEOUT=${STATE_TIMEOUT:-7200}; fi

case "$WORK" in "$REPO"/*) echo "WORK must be outside the repository: $WORK" >&2; exit 2 ;; esac
mkdir -p "$WORK/images" "$WORK/vm" "$WORK/quill" "$DIST"
T0=$(date +%s)
log() { echo "[build-all $(date +%H:%M:%S)] $*"; }
TIMINGS=
PEAK=0
stage_end() {   # NAME START
	local s=$(( $(date +%s) - $2 )) used
	TIMINGS="$TIMINGS${TIMINGS:+,}$1=$s"
	used=$(du -sm "$CI_DIR" "$DL_DIR" | awk '{n += $1} END {print n}')   # allocated MB: the images are sparse
	case "$WORK" in "$CI_DIR"/*) ;; *) used=$((used + $(du -sm "$WORK" | cut -f1))) ;; esac
	[ "$used" -le "$PEAK" ] || PEAK=$used
	log "$1: ${s}s (work dir now $used MB allocated)"
}

# 0. Inputs: the Quill builds, the demo documents, the wallpaper.
S=$(date +%s)
if [ -n "${QUILL_DIR:-}" ]; then
	QSRC=$QUILL_DIR
else
	QISO=${QUILL_ISO:-$HOME/.cache/reactos/quill-latest.iso}
	rm -rf "$WORK/quill"; mkdir -p "$WORK/quill"
	xorriso -osirrox on -indev "$QISO" -extract /QUILL.EXE "$WORK/quill/QUILL.EXE" -extract /QUILL32.EXE "$WORK/quill/QUILL32.EXE" >/dev/null 2>&1
	chmod u+w "$WORK/quill/QUILL.EXE" "$WORK/quill/QUILL32.EXE"
	QSRC=$WORK/quill
fi
QUILL_COMMIT=${QUILL_COMMIT:-unknown}
[ -f "$QSRC/QUILL.EXE" ] && [ -f "$QSRC/QUILL32.EXE" ] || { echo "QUILL.EXE and QUILL32.EXE are needed in $QSRC" >&2; exit 1; }
DEMO=$WORK/demo-cd
rm -rf "$DEMO"; mkdir -p "$DEMO"
for pair in DEMO.QLD:demo.qld DEMO.RTF:demo.rtf DEMO16.QLD:demo16.qld DEMO16.RTF:demo16.rtf; do
	up=${pair%%:*}; lo=${pair##*:}
	if [ -f "$QSRC/$up" ]; then cp "$QSRC/$up" "$DEMO/$up"; else cp "$TRY/demo/$lo" "$DEMO/$up"; fi
done
WALL=$WORK/quill-1024x768.bmp
bash "$HERE/make-wallpaper.sh" "$HERE/quill-wallpaper.svg" "$WALL"
AGDIR=
if [ -n "${AFTERGLOW_DIR:-}" ]; then
	AGDIR=$AFTERGLOW_DIR
elif [ "${AFTERGLOW:-1}" != 0 ]; then
	AGDIR=$WORK/afterglow
	bash "$HERE/prepare-afterglow.sh" "$AGDIR"
fi
HASHES=
for f in "$QSRC/QUILL.EXE" "$QSRC/QUILL32.EXE" "$DEMO"/DEMO*.*; do HASHES="$HASHES${HASHES:+,}$(basename "$f")=$(sha256sum "$f" | cut -d' ' -f1)"; done
[ -z "$AGDIR" ] || HASHES="$HASHES,afterglow.msstyles=$(sha256sum "$AGDIR/afterglow.msstyles" | cut -d' ' -f1)"
stage_end inputs "$S"

# 1. The golden disk.
S=$(date +%s)
if [ -n "${GOLDEN_IMG:-}" ]; then
	GOLDEN=$GOLDEN_IMG
else
	GOLDEN=$CI_DIR/reactos16-golden-scripted.img
	bash "$HERE/install-golden.sh" "$GOLDEN"
fi
stage_end install-golden "$S"

# 2. Trim, put Quill and the settings on it, in a build VM.
S=$(date +%s)
IMG=$WORK/images/reactos16-trim.img
VMDIR=$WORK/vm AFTERGLOW_DIR=$AGDIR bash "$HERE/build-image.sh" "$GOLDEN" "$IMG" "$QSRC" "$DEMO" "$WALL"
stage_end build-image "$S"

# 3. Zero the free space, make the file sparse again.
S=$(date +%s)
python3 -I "$HERE/zero-free.py" "$IMG"
fallocate --dig-holes "$IMG"
stage_end zero-free "$S"

# 4. Parts.
S=$(date +%s)
RUNTIME=node
command -v bun >/dev/null && RUNTIME=bun
rm -rf "$DIST/parts"
$RUNTIME "$TRY/tools/make-parts.mjs" "$IMG" "$DIST/parts" 1048576 19
stage_end make-parts "$S"

# 5. The saved state: serve the page and the image, boot in headless Chromium, save once Quill shows the demo letter.
S=$(date +%s)
if [ ! -d "$TRY/harness/node_modules/playwright-core" ]; then
	if command -v bun >/dev/null; then (cd "$TRY/harness" && bun install --frozen-lockfile)
	else (cd "$TRY/harness" && npm install --no-save --no-package-lock --no-audit --no-fund playwright-core@1.65.0); fi
fi
# v86 is not committed; fetch it (pinned and checked) when it is not there.
[ -f "$TRY/v86/v86.wasm" ] && [ -f "$TRY/v86/libv86.mjs" ] && [ -f "$TRY/v86/bios/seabios.bin" ] || bash "$TRY/tools/fetch-v86.sh"
SPID=
cleanup() { [ -z "$SPID" ] || kill "$SPID" 2>/dev/null || true; }
trap cleanup EXIT
(cd "$TRY/harness" && PORT=$PORT IMAGES_DIR=$WORK/images LOG_FILE=$WORK/requests.json exec $RUNTIME serve.ts) >"$WORK/serve.log" 2>&1 &
SPID=$!
for _ in $(seq 1 60); do curl -fs "http://127.0.0.1:$PORT/__stats" >/dev/null 2>&1 && break; sleep 1; done
curl -fs "http://127.0.0.1:$PORT/__stats" >/dev/null || { echo "the harness server did not start; see $WORK/serve.log" >&2; exit 1; }
rm -f "$WORK/images/state-ci.bin"
(cd "$TRY/harness" && BASE=http://127.0.0.1:$PORT timeout "$STATE_TIMEOUT" $RUNTIME build-state.ts --name state-ci)
kill "$SPID" 2>/dev/null || true; SPID=
stage_end build-state "$S"

# 6. Compress the state.
S=$(date +%s)
zstd -22 --ultra -T0 -f -q "$WORK/images/state-ci.bin" -o "$DIST/state.l22.bin.zst"
stage_end zstd-state "$S"

# 7. The manifest.
V86=$(grep -o 'package/v86) [0-9.]*' "$TRY/v86/PROVENANCE.md" | head -1 | cut -d' ' -f2)
read -r ZIPSHA ISOSHA < <(sed -n -e 's/^ISO_ZIP_SHA256=//p' -e 's/^ISO_SHA256=//p' "$HERE/install-golden.sh" | tr '\n' ' '; echo)
python3 -I "$TRY/tools/build-manifest.py" "$DIST" "$DIST/build-manifest.json" \
	"iso_zip_sha256=$ZIPSHA" "iso_sha256=$ISOSHA" "v86_version=$V86" "quill_commit=$QUILL_COMMIT" \
	"unattend_sha256=$(sha256sum "$HERE/unattend.inf" | cut -d' ' -f1)" "hashes=$HASHES" \
	"timings=$TIMINGS,total=$(( $(date +%s) - T0 ))" "peak_disk_mb=$PEAK"
log "done in $(( $(date +%s) - T0 ))s; peak work dir $PEAK MB; outputs in $DIST"
