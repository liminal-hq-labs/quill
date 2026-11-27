#!/usr/bin/env bash
# Fails when a module is not the image Windows 95 and later load and Quill's loader expects (W9X-132).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win9x/tools/check-dll.sh FILE BASE      (BASE as hex, such as 0x61000000)
#
# Reads the file with llvm-readobj and checks: the DLL characteristic, relocations kept (no RELOCS_STRIPPED, a `.reloc` section with
# content), subsystem 4.0, section alignment 4,096 and file alignment 512, no TLS directory, the base the plan gives, and exactly two
# exports, `QuillModuleInfo` and `QuillModuleStart`. The entry point must be the assembly stub: `mov eax, 1; ret 12`, which stores nothing
# and runs nothing under the loader lock. `scan-pentium.sh` and `check-imports.sh` cover the code and the imports.
set -euo pipefail
file="${1:?usage: check-dll.sh FILE BASE}"
base="${2:?usage: check-dll.sh FILE BASE}"
command -v llvm-readobj > /dev/null || { echo "check-dll: llvm-readobj not found; skipping"; exit 0; }
fail=0
bad() {
	echo "check-dll: $file: $*" >&2
	fail=1
}
headers="$(llvm-readobj --file-headers --sections "$file")"
field() { awk -v key="$1:" '$1 == key {print $2; exit}' <<< "$headers"; }
grep -q 'IMAGE_FILE_DLL ' <<< "$headers" || bad "not a DLL"
if grep -q 'IMAGE_FILE_RELOCS_STRIPPED' <<< "$headers"; then bad "relocations are stripped"; fi
grep -q 'Name: .reloc ' <<< "$headers" || bad "no .reloc section"
[ "$(field BaseRelocationTableSize)" != "0x0" ] || bad "an empty base relocation table"
[ "$(field MajorSubsystemVersion).$(field MinorSubsystemVersion)" = "4.0" ] || bad "subsystem version is not 4.0"
[ "$(field SectionAlignment)" = "4096" ] || bad "section alignment is not 4096"
[ "$(field FileAlignment)" = "512" ] || bad "file alignment is not 512"
[ "$(field TLSTableRVA)" = "0x0" ] || bad "has a TLS directory"
want="$(printf '0x%X' "$((base))")"
have="$(field ImageBase | tr 'a-f' 'A-F' | sed 's/^0X/0x/')"
[ "$have" = "$want" ] || bad "image base is $have, not $want"
names="$(llvm-readobj --coff-exports "$file" | awk '$1 == "Name:" {print $2}' | sort | tr '\n' ' ')"
[ "$names" = "QuillModuleInfo QuillModuleStart " ] || bad "exports are [$names], not QuillModuleInfo and QuillModuleStart"
# The entry point's first eight bytes, from the file: B8 01 00 00 00 (mov eax, 1) C2 0C 00 (ret 12).
entry="$(python3 -I - "$file" << 'PY'
import struct, sys
data = open(sys.argv[1], "rb").read()
pe = struct.unpack_from("<I", data, 0x3C)[0]
sections = struct.unpack_from("<H", data, pe + 6)[0]
optional_size = struct.unpack_from("<H", data, pe + 20)[0]
entry_rva = struct.unpack_from("<I", data, pe + 24 + 16)[0]
table = pe + 24 + optional_size
for i in range(sections):
    _name, vsize, vaddr, rawsize, rawptr = struct.unpack_from("<8sIIII", data, table + 40 * i)
    if vaddr <= entry_rva < vaddr + max(vsize, rawsize):
        at = entry_rva - vaddr + rawptr
        print(data[at:at + 8].hex())
        break
PY
)"
[ "$entry" = "b801000000c20c00" ] || bad "the entry point is not 'mov eax, 1; ret 12' ($entry)"
[ "$fail" = 0 ] || exit 1
echo "check-dll: ok: $file at $want, relocatable, two exports, entry stub"
