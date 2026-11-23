#!/usr/bin/env python3
# Writes a Win32 .res file (menu, version information, icon) from quill.json, for lld-link to put in QUILL32.EXE.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: res_pack.py quill.json out.res
#
# The menu comes from the app's own menu tree (emit-resources writes the JSON), so the native menu and the DOS menus cannot drift.
# No manifest: Windows 9x ignores one and Quill draws its own chrome (D35).
import json
import struct
import sys

RT_ICON, RT_MENU, RT_GROUP_ICON, RT_VERSION = 3, 4, 14, 16
LANG = 0x0409


def u16(*v):
    return b"".join(struct.pack("<H", x) for x in v)


def wstr(s):
    return s.encode("utf-16-le") + b"\0\0"


def pad4(b):
    return b + b"\0" * (-len(b) % 4)


def entry(rtype, rid, data):
    """One resource: the 32-byte header, then the data padded to four bytes."""
    head = struct.pack("<II", len(data), 32) + u16(0xFFFF, rtype, 0xFFFF, rid) + struct.pack("<IHHII", 0, 0x1030, LANG, 0, 0)
    return head + pad4(data)


def menu(menus):
    out = u16(0, 0)
    for mi, m in enumerate(menus):
        last_menu = mi == len(menus) - 1
        out += u16(0x10 | (0x80 if last_menu else 0)) + wstr(m["title"])
        for ii, it in enumerate(m["items"]):
            last = ii == len(m["items"]) - 1
            out += u16(0x80 if last else 0, it["id"]) + wstr(it["text"])
    return out


def block(key, value=b"", vtype=0, children=b"", vlen=None):
    """A VS_VERSIONINFO node: length, value length, type, key, padding, value, padding, children."""
    head = u16(0, len(value) if vlen is None else vlen, vtype) + wstr(key)
    head = pad4(head)
    body = head + value
    body = pad4(body) + children if children else body
    return struct.pack("<H", len(body)) + body[2:]


def version(v):
    a, b, c, d = v["file"]
    fixed = struct.pack("<IIIIIIIIIIIII", 0xFEEF04BD, 0x10000, (a << 16) | b, (c << 16) | d, (a << 16) | b, (c << 16) | d, 0x3F, 0, 0x40004, 1, 0, 0, 0)
    strings = b""
    for k, name in [("CompanyName", "company"), ("FileDescription", "description"), ("FileVersion", None), ("LegalCopyright", "copyright"),
                    ("OriginalFilename", "original"), ("ProductName", "product")]:
        val = ".".join(map(str, v["file"])) if name is None else v[name]
        strings += pad4(block(k, wstr(val), 1, vlen=len(val) + 1))
    table = block("040904B0", b"", 1, strings)
    sfi = block("StringFileInfo", b"", 1, pad4(table))
    var = block("Translation", struct.pack("<HH", 0x409, 0x4B0), 0)
    vfi = block("VarFileInfo", b"", 1, pad4(var))
    return block("VS_VERSION_INFO", fixed, 0, pad4(sfi) + vfi, vlen=len(fixed))


VGA = [(0, 0, 0), (0, 0, 170), (0, 170, 0), (0, 170, 170), (170, 0, 0), (170, 0, 170), (170, 85, 0), (170, 170, 170),
       (85, 85, 85), (85, 85, 255), (85, 255, 85), (85, 255, 255), (255, 85, 85), (255, 85, 255), (255, 255, 85), (255, 255, 255)]


def icon():
    """A 32 by 32, 16-colour icon: parchment page on ink blue with an amber nib."""
    n = 32
    px = [[1] * n for _ in range(n)]            # ink blue ground (VGA blue, index 1)
    for y in range(3, 29):
        for x in range(5, 27):
            px[y][x] = 15                       # the page
    for y in range(8, 24, 4):
        for x in range(9, 23 - (y // 4 % 2) * 4):
            px[y][x] = 0                        # lines of text
    for i in range(14):                         # the nib, amber (VGA yellow, index 14)
        for x in range(16 - (14 - i) // 2, 17 + (14 - i) // 2):
            px[18 + i // 2 + 4][x] = 14
    pal = b"".join(bytes([b, g, r, 0]) for r, g, b in VGA)
    xor = b""
    for y in range(n - 1, -1, -1):
        row = px[y]
        xor += bytes((row[x] << 4) | row[x + 1] for x in range(0, n, 2))
    andmask = b"\0" * (4 * n)
    dib = struct.pack("<IiiHHIIiiII", 40, n, 2 * n, 1, 4, 0, len(xor) + len(andmask), 0, 0, 16, 0) + pal + xor + andmask
    group = u16(0, 1, 1) + bytes([n, n, 16, 0]) + u16(1, 4) + struct.pack("<I", len(dib)) + u16(1)
    return group, dib


def main():
    spec = json.load(open(sys.argv[1], encoding="utf-8"))
    out = struct.pack("<II", 0, 32) + u16(0xFFFF, 0, 0xFFFF, 0) + struct.pack("<IHHII", 0, 0, 0, 0, 0)
    out += entry(RT_MENU, 1, menu(spec["menus"]))
    group, dib = icon()
    out += entry(RT_GROUP_ICON, 1, group) + entry(RT_ICON, 1, dib)
    out += entry(RT_VERSION, 1, version(spec["version"]))
    open(sys.argv[2], "wb").write(out)


main()
