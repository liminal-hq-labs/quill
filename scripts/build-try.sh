#!/usr/bin/env bash
# Builds the browser demo's disk bundle, site/try/quill.<hash>.jsdos: a FreeDOS 1.3 floppy that starts Quill, packed for js-dos, and the
# manifest that names it, site/try/try-manifest.json.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: scripts/build-try.sh [--build]
#
#   (no option)  use the programs that already exist: QUILLDOS_EXE (default dos/dist/QUILLDOS.EXE), QUILL_FNT and QUILL_DICT, the
#                files `--build` makes under $QUILL_TRY_DIR
#   --build      make them first: dos/build.sh (with only CWSDPMI's stub fetched), quill-fntgen for QUILL.FNT and quill-dict for the
#                dictionary. Needs the pinned Rust toolchain with rust-src
#
# FreeDOS 1.3's floppy edition is downloaded from the pinned address and checked against the pinned SHA-256 before anything is
# unpacked; only its boot floppy's kernel, shell and boot sector are used, and they are not modified. The disk image is made by
# scripts/make-freedos-image.py (needs mtools), its files are in scripts/try-disk, and the DOSBox settings are
# scripts/try-dosbox.conf. Nothing here is committed: the bundle is a build output that the Pages workflow makes. Work files go to
# $QUILL_TRY_DIR (default ~/.cache/quill16-try), not the checkout.
#
# Cache busting (SITE-4). The bundle is named by its content: quill.<first 12 hex of its SHA-256>.jsdos, so one name is only ever one
# file and no cache can hold a stale copy under it. try-manifest.json says which name is current ({"bundle", "sha256", "built",
# "size", "assets"}); the page fetches it with `cache: 'no-store'`. Every other quill.*.jsdos in the output folder is deleted, so the
# folder holds only what the manifest names. The page's own files carry a ?v= version that scripts/stamp-try.py writes into index.html,
# and the manifest's "assets" names the same for the vendored player. QUILL_TRY_OUT (default site/try) moves the output, for
# scripts/check-try.sh.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(dirname "$HERE")"
WORK="${QUILL_TRY_DIR:-$HOME/.cache/quill16-try}"
OUTDIR="${QUILL_TRY_OUT:-$ROOT/site/try}"

FD_URL=https://www.ibiblio.org/pub/micro/pc-stuff/freedos/files/distributions/1.3/official/FD13-FloppyEdition.zip
FD_SHA256=75a4e11a7fce6f124e20927b3022b4b715a2a3f7324c5f5bfea42d90d80eb072
FD_MEMBER=144m/x86BOOT.img
# CuteMouse 2.1 beta 4 (GPL-2.0), FreeDOS's mouse driver, from FreeDOS 1.3's package repository. A floppy booted in DOSBox has no INT 33h
# driver of its own (DOSBox's built-in one serves only its own DOS shell), so Quill's pointer needs one, as on real hardware.
CM_URL=https://www.ibiblio.org/pub/micro/pc-stuff/freedos/files/repositories/1.3/base/ctmouse.zip
CM_SHA256=a891124cd5b13e8141778fcae718f3b2667b0a49727ce92b782ab11a8c4bb63a
CM_MEMBER=BIN/CTMOUSE.EXE

case "${1:-}" in
"" | --build) ;;
*) echo "build-try: unknown option $1" >&2 && exit 2 ;;
esac
command -v mformat >/dev/null || { echo "build-try: mtools is needed (apt-get install mtools)" >&2 && exit 1; }
mkdir -p "$WORK/dl"

if [ "${1:-}" = --build ]; then
	bash "$ROOT/dos/tools/fetch-tools.sh" --stub-only
	(cd "$ROOT/dos" && CARGO_TARGET_DIR="$WORK/target-dos" bash build.sh)
	(cd "$ROOT" && CARGO_TARGET_DIR="$WORK/target-host" cargo run -q --release --locked -p quill-fntgen -- --set desk -o "$WORK/QUILL.FNT")
	(cd "$ROOT" && CARGO_TARGET_DIR="$WORK/target-host" cargo run -q --release --locked -p quill-dict -- "$WORK/dic")
	QUILLDOS_EXE="$ROOT/dos/dist/QUILLDOS.EXE"
fi

exe="${QUILLDOS_EXE:-$ROOT/dos/dist/QUILLDOS.EXE}"
fnt="${QUILL_FNT:-$WORK/QUILL.FNT}"
dict="${QUILL_DICT:-$WORK/dic/EN_CA.DIC}"
for f in "$exe" "$fnt" "$dict"; do
	[ -f "$f" ] || { echo "build-try: $f is missing; run scripts/build-try.sh --build" >&2 && exit 1; }
done

# FreeDOS, checked before it is unpacked.
zip="$WORK/dl/${FD_URL##*/}"
[ -f "$zip" ] || curl -fsSL --retry 5 --retry-all-errors -o "$zip" "$FD_URL"
if ! printf '%s  %s\n' "$FD_SHA256" "$zip" | sha256sum -c - >/dev/null; then
	rm -f "$zip"
	echo "build-try: checksum mismatch for ${zip##*/}; removed it" >&2
	exit 1
fi

# CuteMouse, checked the same way.
cmzip="$WORK/dl/${CM_URL##*/}"
[ -f "$cmzip" ] || curl -fsSL --retry 5 --retry-all-errors -o "$cmzip" "$CM_URL"
if ! printf '%s  %s\n' "$CM_SHA256" "$cmzip" | sha256sum -c - >/dev/null; then
	rm -f "$cmzip"
	echo "build-try: checksum mismatch for ${cmzip##*/}; removed it" >&2
	exit 1
fi
rm -rf "$WORK/cm"
mkdir -p "$WORK/cm"
python3 -I -c 'import sys,zipfile; zipfile.ZipFile(sys.argv[1]).extract(sys.argv[2], sys.argv[3])' "$cmzip" "$CM_MEMBER" "$WORK/cm"

rm -rf "$WORK/fd"
mkdir -p "$WORK/fd"
python3 -I -c 'import sys,zipfile; zipfile.ZipFile(sys.argv[1]).extract(sys.argv[2], sys.argv[3])' "$zip" "$FD_MEMBER" "$WORK/fd"
boot="$WORK/fd/$FD_MEMBER"

# The disk's files. DOS reads text with CR LF line ends.
tree="$WORK/tree"
rm -rf "$tree"
mkdir -p "$tree/QUILL/SAMPLES"
for f in AUTOEXEC.BAT QUILLG.BAT QUILL16.BAT FDCONFIG.SYS FREEDOS.TXT; do sed 's/$/\r/' "$HERE/try-disk/$f" >"$tree/$f"; done
sed 's/$/\r/' "$HERE/try-disk/QUILL.INI" >"$tree/QUILL/QUILL.INI"
sed 's/$/\r/' "$ROOT/assets/fonts/desk/OFL.txt" >"$tree/QUILL/OFL.TXT"
cp "$exe" "$tree/QUILL/QUILLDOS.EXE"
cp "$WORK/cm/$CM_MEMBER" "$tree/QUILL/CTMOUSE.EXE"
cp "$fnt" "$tree/QUILL/QUILL.FNT"
cp "$dict" "$tree/QUILL/EN_CA.DIC"
cp "$ROOT/samples/demo.qld" "$tree/QUILL/SAMPLES/DEMO.QLD"
cp "$ROOT/tests/docs/plain.qld" "$tree/QUILL/SAMPLES/PLAIN.QLD"
cp "$ROOT/samples/demo.rtf" "$tree/QUILL/SAMPLES/DEMO.RTF"
cp "$ROOT/tests/docs/pictures.rtf" "$tree/QUILL/SAMPLES/PICTURES.RTF"

python3 -I "$HERE/make-freedos-image.py" "$WORK/fl.img" "$boot" "$tree"

# The js-dos bundle is a zip: the settings in .jsdos/ and the floppy image beside them. Fixed times keep the zip repeatable, so the same
# inputs give the same bytes and so the same name.
mkdir -p "$OUTDIR"
tmp="$OUTDIR/.quill-bundle.tmp"
python3 -I - "$tmp" "$HERE/try-dosbox.conf" "$WORK/fl.img" <<'PY'
import sys, zipfile

out, conf, image = sys.argv[1:4]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
	for name, path in ((".jsdos/dosbox.conf", conf), ("fl.img", image)):
		info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
		info.compress_type = zipfile.ZIP_DEFLATED
		info.external_attr = 0o644 << 16
		with open(path, "rb") as f:
			z.writestr(info, f.read(), compresslevel=9)
PY

# Name it by content, write the manifest, and drop every other bundle (the old unhashed quill.jsdos too).
python3 -I "$HERE/stamp-try.py" --bundle "$tmp" --out "$OUTDIR" --root "$ROOT/site"
