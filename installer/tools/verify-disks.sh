#!/usr/bin/env bash
# Checks the disks `build-disks.sh` made: the label, a pack that verifies, every file in it byte-equal to the staged file it came
# from, and then the size gate (`size-gate.py`): the executable and module budgets, the free bytes each disk must keep and each
# file's packed size, all from installer/budgets.toml.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: installer/tools/verify-disks.sh
#
# Reads installer/out/QUILL-DOS.IMG, QUILL-W31.IMG and QUILL-W95.IMG, and installer/dist/. Each disk's SETUP.LPK is taken off the
# image with mtools, checked with `liminst lpk verify` and unpacked with `liminst lpk unpack`. The unpacked names are compared
# with `dist/`: the program, the help, the dictionaries and the samples must match the files that were staged, and the staged
# programs must match the build scripts' own output. The Windows projects must register `.QLD` (the Windows 3.x one in `WIN.INI`, the
# Windows 95 one in the registry, both at run time); reading those back from a guest is for the emulator tests. The disk's README.TXT must be Quill's, and SETUP.EXE the runtime's.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(dirname "$HERE")"
OUT="$HERE/out"
DIST="$HERE/dist"
LIMINST="$HERE/tools/liminst.sh"
WORK="$(mktemp -d "${QUILL_BUILD_DIR:-$HOME/.cache/quill16-installer-build}/verify.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

fail() { echo "verify-disks: FAIL: $*" >&2 && exit 1; }
same() { cmp -s "$1" "$2" || fail "$3: $1 differs from $2"; }

# The programs the build scripts made, which stage.sh copied.
same "$DIST/win16-x86/QUILL.EXE" "${QUILL16_EXE:-$ROOT/win16/out/QUILL.EXE}" "staged QUILL.EXE"
same "$DIST/help/QUILL.HLP" "${QUILL16_HLP:-$ROOT/win16/out/QUILL.HLP}" "staged QUILL.HLP"
same "$DIST/windows-x86/QUILL32.EXE" "${QUILL32_EXE:-$ROOT/win9x/dist/QUILL32.EXE}" "staged QUILL32.EXE"
same "$DIST/dos-x86/QUILLDOS.EXE" "${QUILLDOS_EXE:-$ROOT/dos/dist/QUILLDOS.EXE}" "staged QUILLDOS.EXE"

# The runtime each disk's SETUP.EXE must be.
eval "$(bash "$LIMINST" runtimes)"

# image, label, program in the pack, where the program was staged, runtime
disks=(
	"QUILL-DOS|QUILL DOS|QUILLDOS.EXE|dos-x86|$DOS"
	"QUILL-W31|QUILL W31|QUILL.EXE|win16-x86|$WIN16"
	"QUILL-W95|QUILL W95|QUILL32.EXE|windows-x86|$WIN32"
)
for d in "${disks[@]}"; do
	IFS='|' read -r name label prog stagedir runtime <<<"$d"
	img="$OUT/$name.IMG"
	[ -f "$img" ] || fail "$img is missing; run installer/tools/build-disks.sh"
	dir="$WORK/$name"
	mkdir -p "$dir"

	got="$(mdir -i "$img" :: | sed -n 's/^ Volume in drive : is \(.*[^ ]\) *$/\1/p')"
	[ "$got" = "$label" ] || fail "$name: the volume label is '$got', not '$label'"

	free="$(mdir -i "$img" :: | sed -n 's/^ *\([0-9 ]*[0-9]\) bytes free.*/\1/p' | tr -d ' ')"

	mcopy -i "$img" ::SETUP.LPK ::SETUP.EXE ::README.TXT "$dir/"
	same "$dir/SETUP.EXE" "$runtime" "$name SETUP.EXE"
	same "$dir/README.TXT" "$DIST/common/README.TXT" "$name README.TXT"
	bash "$LIMINST" lpk verify "$dir/SETUP.LPK" >/dev/null || fail "$name: SETUP.LPK does not verify"
	bash "$LIMINST" lpk unpack "$dir/SETUP.LPK" -o "$dir/pack" >/dev/null

	# The compiled project (SETUP.PRJ) registers the .QLD file type on the Windows builds, with the program as its opener, and on
	# DOS, which has no file types, does not.
	prj="$dir/pack/SETUP.PRJ"
	if [ "$stagedir" = dos-x86 ]; then
		! grep -a -q 'Quill document' "$prj" || fail "$name: the DOS project registers a file type"
	else
		grep -a -q 'Quill document' "$prj" && grep -a -q 'hq.liminal.quill.qld' "$prj" && grep -a -q "$prog" "$prj" ||
			fail "$name: the project does not register .QLD for $prog"
	fi

	count=0
	while IFS= read -r f; do
		rel="${f#"$dir/pack/"}"
		case "$rel" in
		SETUP.PRJ) continue ;;
		"$prog") src="$DIST/$stagedir/$prog" ;;
		QUILL.HLP) src="$DIST/help/QUILL.HLP" ;;
		README.TXT | LICENCE.TXT) src="$DIST/common/$rel" ;;
		*.DIC) src="$DIST/dict/$rel" ;;
		*.DMD) src="$DIST/$stagedir/$rel" ;;
		SAMPLES/*) src="$DIST/samples/${rel#SAMPLES/}" ;;
		*) fail "$name: the pack holds $rel, which stage.sh did not stage" ;;
		esac
		same "$f" "$src" "$name: $rel"
		count=$((count + 1))
	done < <(find "$dir/pack" -type f | sort)

	# Everything staged for the disk must be in the pack.
	help=1
	[ "$stagedir" != dos-x86 ] || help=0
	want=$(($(find "$DIST/common" "$DIST/dict" "$DIST/samples" "$DIST/$stagedir" -type f | wc -l) + help))
	[ "$count" = "$want" ] || fail "$name: $count files in the pack, $want staged"
	echo "verify-disks: $name: $count files byte-equal to the staged files, $free bytes free"
done

# The budgets: the packed sizes, the free bytes against each disk's floor, the executable and module budgets. It writes the table
# after the raw listing in SIZES.txt and, in CI, into the job summary.
gate=(python3 -I "$HERE/tools/size-gate.py" check)
[ -z "${GITHUB_STEP_SUMMARY:-}" ] || gate+=(--summary "$GITHUB_STEP_SUMMARY")
"${gate[@]}" || fail "a size budget is not met (see above)"
echo "verify-disks: ok"
