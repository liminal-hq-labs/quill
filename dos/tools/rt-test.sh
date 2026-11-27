#!/bin/bash
# Builds RTTEST.EXE and runs the runtime's directory, drive, rename and critical-error checks under headless DOSBox Staging.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/rt-test.sh            checks (what `make dos` runs)
#        RT_TIME=1 dos/tools/rt-test.sh  also times directory reads at a fixed 386 setting and prints the results
#        RT_PROBE=1 dos/tools/rt-test.sh also prints what DOS says about drives, attributes and odd names
#        RT_KEEP=1 dos/tools/rt-test.sh  keeps the scratch folder (the disk the programs ran from) and says where
#        RT_EVIDENCE=DIR dos/tools/rt-test.sh keeps the module runs' output in DIR; RT_NO_ARENA=1 skips the arena build's run
#
# A prepared C: holds a TREE of folders (a folder of 600 files, one of 512, one 12 levels deep, names in lower and mixed case) that the
# host makes with Python. `RTTEST list` reads it back through find-first and find-next; the output must equal the host's own listing,
# upper-cased and sorted the same way. `RTTEST self` checks the drives (C:, D: and Z: valid, E: refused with error 0Fh, no hang),
# rename, and that the INT 24h stub installs and is taken out again. Needs the DOS tools (dos/tools/fetch-tools.sh); exits 0 with a note
# when they are missing, as e2e.sh does.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR="$("$HERE/tools/vendor-dir.sh" "$HERE/vendor")"

if [ ! -f "$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE" ] || [ -z "$(ls "$VENDOR"/dosbox/*/dosbox 2> /dev/null)" ]; then
	echo "rt-test: DOS tools not present; skipping (run dos/tools/fetch-tools.sh)"
	exit 0
fi
fail() { echo "rt-test: FAIL: $*" >&2; exit 1; }

(cd "$HERE" && RUSTC_BOOTSTRAP=1 cargo build --release --locked -p rt-test)
TARGET_DIR="${CARGO_TARGET_DIR:-$HERE/target}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/quill-rt-test.XXXXXX")"
trap '[ -n "${RT_KEEP:-}" ] && echo "rt-test: kept $WORK" || rm -rf "$WORK"' EXIT
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

# The module loader (docs/plans/modules.md section 5.3): the test modules at three link bases and damaged copies of one, loaded by
# `RTTEST mod`; then the checks that end or endanger the program, each in a run of its own; then all of it again with the modules in
# the arena in `.bss` (`rt`'s `module-arena` feature). RT_EVIDENCE=<folder> keeps each run's output there.
BUILD="$("$HERE/tools/build-mods.sh" --test "$WORK/mods" | tee "$WORK/build-mods.txt" | sed -n 's/^build-mods: build id //p')"
[ -n "$BUILD" ] || { cat "$WORK/build-mods.txt" >&2; fail "build-mods.sh did not say the build id"; }
MODS="$DISK/MODS"
mkdir -p "$MODS" "$DISK/STALE"
cp "$WORK"/mods/*.DMD "$MODS/"
python3 -I - "$MODS" "$DISK" << 'PY'
import os, random, struct, sys, zlib
mods, disk = sys.argv[1], sys.argv[2]
good = open(os.path.join(mods, "QUILLTST.DMD"), "rb").read()
def put(name, data):
    open(os.path.join(mods, name), "wb").write(bytes(data))
def refix(data):
    data = bytearray(data)
    data[84:88] = b"\0\0\0\0"
    data[84:88] = struct.pack("<I", zlib.crc32(bytes(data)))
    return data
put("T_EMPTY.DMD", b"")
put("T_HDR.DMD", good[:50])
put("T_HALF.DMD", good[: len(good) // 2])
put("T_LAST.DMD", good[:-1])
flip = bytearray(good); flip[len(good) // 2] ^= 1; put("T_FLIP.DMD", flip)
flip = bytearray(good); flip[84] ^= 1; put("T_CRC.DMD", flip)
bad = bytearray(good); bad[0:4] = b"XMD1"; put("T_MAGIC.DMD", bad)
bad = bytearray(good); bad[0:4] = b"DMD2"; put("T_VER.DMD", bad)
random.seed(7)
junk = bytes(random.randrange(256) for _ in range(5000))
put("T_JUNK.DMD", junk)
put("T_PAD.DMD", good + b"\0" * 16)
kind = bytearray(good); kind[12:14] = struct.pack("<H", 2); put("T_KIND.DMD", refix(kind))
assert b"desk_log" in good
put("T_IMPORT.DMD", refix(good.replace(b"desk_log", b"desk_lox")))
put("T_BIGF.DMD", b"\0" * 300000)
# Beside the program (C:\): the good module under the name a picture module has. In the folder the program is run from: noise under the same name.
open(os.path.join(disk, "QUILLPIC.DMD"), "wb").write(good)
open(os.path.join(disk, "STALE", "QUILLPIC.DMD"), "wb").write(junk)
PY
keep() { [ -z "${RT_EVIDENCE:-}" ] || { mkdir -p "$RT_EVIDENCE"; cp "$1" "$RT_EVIDENCE/$2"; }; }
# modrun NAME EXE ARGS...: runs the command, keeps its output under NAME, and sets MODRC to its status.
modrun() {
	local name="$1"; shift
	MODRC=0
	"$HERE/tools/dosrun.sh" "$DISK" "$@" > "$WORK/$name.txt" || MODRC=$?
	keep "$WORK/$name.txt" "$name.txt"
}
modchecks() {
	local suffix="$1" exe="$2"
	modrun "mod$suffix" "$exe mod C:\\MODS $BUILD"
	cat "$WORK/mod$suffix.txt"
	[ "$MODRC" = 0 ] && ! grep -q '^FAIL' "$WORK/mod$suffix.txt" || fail "$exe mod exited with $MODRC"
	modrun "nomem$suffix" "$exe mod-nomem C:\\MODS $BUILD"
	cat "$WORK/nomem$suffix.txt"
	[ "$MODRC" = 0 ] && ! grep -q '^FAIL' "$WORK/nomem$suffix.txt" || fail "$exe mod-nomem exited with $MODRC"
	DOS_CWD='\STALE' modrun "find$suffix" "C:\\$exe mod-find $BUILD"
	cat "$WORK/find$suffix.txt"
	[ "$MODRC" = 0 ] && ! grep -q '^FAIL' "$WORK/find$suffix.txt" || fail "$exe mod-find exited with $MODRC"
	modrun "panic$suffix" "$exe mod-panic C:\\MODS $BUILD"
	cat "$WORK/panic$suffix.txt"
	[ "$MODRC" = 3 ] && grep -q '^RT PANIC: module: quilltst: asked to panic' "$WORK/panic$suffix.txt" || fail "$exe mod-panic did not end through the host's panic path (status $MODRC)"
}
modchecks "" RTTEST.EXE
echo "rt-test: the module loader's checks pass (heap, extended memory)"
modrun nowiden "RTTEST.EXE mod-nowiden C:\\MODS $BUILD"
cat "$WORK/nowiden.txt"
echo "rt-test: CS not widened: status $MODRC (see above: a fault or NO FAULT, recorded, not asserted)"
if [ -z "${RT_NO_ARENA:-}" ]; then
	(cd "$HERE" && RUSTC_BOOTSTRAP=1 cargo build --release --locked -p rt-test --features module-arena)
	"$HERE/tools/elf2go32.py" "$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE" "$TARGET_DIR/dos-elf/release/rt-test" "$DISK/RTTESTA.EXE"
	modchecks "-arena" RTTESTA.EXE
	echo "rt-test: the module loader's checks pass (module-arena)"
fi

if [ -n "${RT_PROBE:-}" ]; then
	"$HERE/tools/dosrun.sh" "$DISK" 'RTTEST.EXE probe C:\PROBE' || true
fi
if [ -n "${RT_TIME:-}" ]; then
	# The module loader's load time at the same fixed 386.
	DOS_CONF=$'[cpu]\ncputype = 386\ncpu_cycles = 3000\ncpu_cycles_protected = 3000' modrun time-386 "RTTEST.EXE mod-time C:\\MODS $BUILD"
	cat "$WORK/time-386.txt"
	[ "$MODRC" = 0 ] || fail "RTTEST mod-time failed at 386"
	# A fixed 386: the second [cpu] section overrides the generated one (checked by the timings differing from `max`).
	DOS_CONF=$'[cpu]\ncputype = 386\ncpu_cycles = 3000\ncpu_cycles_protected = 3000' \
		"$HERE/tools/dosrun.sh" "$DISK" 'RTTEST.EXE time C:\TREE' || fail "RTTEST time failed at 386"
	if [ -n "${RT_TIME_MAX:-}" ]; then "$HERE/tools/dosrun.sh" "$DISK" 'RTTEST.EXE time C:\TREE' || true; fi
fi
echo "rt-test: ok"
