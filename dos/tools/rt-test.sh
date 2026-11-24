#!/bin/bash
# Builds RTTEST.EXE and runs the runtime's directory, drive, rename and critical-error checks under headless DOSBox Staging.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/rt-test.sh            checks (what `make dos` runs)
#        RT_TIME=1 dos/tools/rt-test.sh  also times directory reads at a fixed 386 setting and prints the results
#        RT_PROBE=1 dos/tools/rt-test.sh also prints what DOS says about drives, attributes and odd names
#
# A prepared C: holds a TREE of folders (a folder of 600 files, one of 512, one 12 levels deep, names in lower and mixed case) that the
# host makes with Python. `RTTEST list` reads it back through find-first and find-next; the output must equal the host's own listing,
# upper-cased and sorted the same way. `RTTEST self` checks the drives (C:, D: and Z: valid, E: refused with error 0Fh, no hang),
# rename, and that the INT 24h stub installs and is taken out again. Needs the DOS tools (dos/tools/fetch-tools.sh); exits 0 with a note
# when they are missing, as e2e.sh does.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR="${DOS_VENDOR:-$HERE/vendor}"

if [ ! -f "$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE" ] || [ -z "$(ls "$VENDOR"/dosbox/*/dosbox 2> /dev/null)" ]; then
	echo "rt-test: DOS tools not present; skipping (run dos/tools/fetch-tools.sh)"
	exit 0
fi
fail() { echo "rt-test: FAIL: $*" >&2; exit 1; }

(cd "$HERE" && RUSTC_BOOTSTRAP=1 cargo build --release --locked -p rt-test)
TARGET_DIR="${CARGO_TARGET_DIR:-$HERE/target}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-rt-test.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
DISK="$WORK/disk"
mkdir -p "$DISK"
"$HERE/tools/elf2go32.py" "$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE" "$TARGET_DIR/dos-elf/release/rt-test" "$DISK/RTTEST.EXE"

# The tree, and the host's listing of it: depth first, each folder's entries sorted by upper-cased name, `PATH <DIR>` or `PATH SIZE`.
python3 -I - "$DISK" "$WORK/expected.txt" << 'PY'
import os, sys
disk, expected = sys.argv[1], sys.argv[2]
tree = os.path.join(disk, "TREE")
def put(rel, size):
    p = os.path.join(tree, rel)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "wb") as f:
        f.write(b"x" * size)
for i in range(600):
    put("BIG/F%04d.TXT" % i, i % 7)
for i in range(512):
    put("F512/n%03d.dat" % i, (i * 3) % 11)
deep = "/".join("L%d" % i for i in range(1, 13))
put(deep + "/Last.txt", 40)
put(deep + "/A.B", 1)
put(deep + "/z", 2)
# Names that sort differently by case, punctuation or digit; lower and mixed case on the host, upper case in DOS.
for n in ["a.txt", "A-B.TXT", "a_b.txt", "A1.TXT", "a~1.txt", "B.TXT", "Mixed.Doc", "ZED", "zed.1", "9LIVES.TXT", "x$y.txt", "!BANG.TXT"]:
    put("MIXED/" + n, len(n))
os.makedirs(os.path.join(tree, "EMPTY"), exist_ok=True)
os.makedirs(os.path.join(tree, "Docs", "Letters", "2025"), exist_ok=True)
put("Docs/Letters/2025/april.qld", 1234)
put("Docs/Letters/memo.qld", 7)
put("root.txt", 3)
lines = []
def walk(d, rel):
    names = sorted(os.listdir(d), key=lambda n: n.upper().encode())
    for n in names:
        p = os.path.join(d, n)
        r = rel + "\\" + n.upper()
        if os.path.isdir(p):
            lines.append(r + " <DIR>")
            walk(p, r)
        else:
            lines.append("%s %d" % (r, os.path.getsize(p)))
walk(tree, "C:\\TREE")
open(expected, "w").write("\n".join(lines) + "\n")
# A folder for the probe: names that are not what DOS expects.
probe = os.path.join(disk, "PROBE")
os.makedirs(os.path.join(probe, "sub"))
for n in ["lower.txt", "UPPER.TXT", "MiXeD.Txt", "verylongfilename.txt", "has space.txt", ".dotfile", "tw.o.dots", "ok.html"]:
    open(os.path.join(probe, n), "w").write("x")
os.chmod(os.path.join(probe, "UPPER.TXT"), 0o444)
PY

export DOS_TIMEOUT=120
RC=0
"$HERE/tools/dosrun.sh" "$DISK" 'RTTEST.EXE list C:\TREE' > "$WORK/actual.txt" || RC=$?
[ "$RC" = 0 ] || { head -20 "$WORK/actual.txt" >&2; fail "RTTEST list exited with $RC"; }
diff -u "$WORK/expected.txt" "$WORK/actual.txt" > "$WORK/list.diff" || { head -40 "$WORK/list.diff" >&2; fail "the listing differs from the host's"; }
echo "rt-test: $(wc -l < "$WORK/actual.txt") entries (folders, files in lower and mixed case, 600 in one folder) match the host's sorted listing"

RC=0
"$HERE/tools/dosrun.sh" "$DISK" 'RTTEST.EXE self' > "$WORK/self.txt" || RC=$?
cat "$WORK/self.txt"
[ "$RC" = 0 ] && ! grep -q '^FAIL' "$WORK/self.txt" || fail "RTTEST self exited with $RC"
echo "rt-test: drives, rename and the INT 24h stub behave"

if [ -n "${RT_PROBE:-}" ]; then
	"$HERE/tools/dosrun.sh" "$DISK" 'RTTEST.EXE probe C:\PROBE' || true
fi
if [ -n "${RT_TIME:-}" ]; then
	# A fixed 386: the second [cpu] section overrides the generated one (checked by the timings differing from `max`).
	DOS_CONF=$'[cpu]\ncputype = 386\ncpu_cycles = 3000\ncpu_cycles_protected = 3000' \
		"$HERE/tools/dosrun.sh" "$DISK" 'RTTEST.EXE time C:\TREE' || fail "RTTEST time failed at 386"
	if [ -n "${RT_TIME_MAX:-}" ]; then "$HERE/tools/dosrun.sh" "$DISK" 'RTTEST.EXE time C:\TREE' || true; fi
fi
echo "rt-test: ok"
