#!/usr/bin/env python3
# Reads a .res file back with a reader that shares nothing with res_pack.py, and checks what it holds.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: test_res_pack.py quill.json quill.res [dialogs.json]
import json
import struct
import sys


def entries(data):
    at = 0
    while at < len(data):
        size, hsize = struct.unpack_from("<II", data, at)
        t = struct.unpack_from("<HH", data, at + 8)
        n = struct.unpack_from("<HH", data, at + 12)
        yield (t[1], n[1], data[at + hsize:at + hsize + size])
        at += hsize + size + (-size % 4)


def read_menu(b):
    at = 4
    names = []
    while at < len(b):
        flags, = struct.unpack_from("<H", b, at)
        at += 2
        if not flags & 0x10:
            at += 2
        end = at
        while b[end:end + 2] != b"\0\0":
            end += 2
        names.append(b[at:end].decode("utf-16-le"))
        at = end + 2
    return names


spec = json.load(open(sys.argv[1], encoding="utf-8"))
res = list(entries(open(sys.argv[2], "rb").read()))
kinds = [(t, n) for t, n, _ in res]
assert kinds[0] == (0, 0), kinds
for want in [(4, 1), (14, 1), (3, 1), (16, 1)]:
    assert want in kinds, (want, kinds)
menu = dict(((t, n), d) for t, n, d in res)[(4, 1)]
names = read_menu(menu)
expect = [m["title"] for m in spec["menus"]] + [i["text"] for m in spec["menus"] for i in m["items"]]
assert sorted(names) == sorted(expect), (names, expect)
icons = [d for t, n, d in res if t == 3]
group = dict(((t, n), d) for t, n, d in res)[(14, 1)]
count = struct.unpack_from("<HHH", group, 0)[2]
assert count == len(icons) and len(group) == 6 + 14 * count, (count, len(icons))
for i in range(count):
    w, h, _c, _r, _p, bits, size, rid = struct.unpack_from("<BBBBHHIH", group, 6 + 14 * i)
    assert rid == i + 1 and size == len(icons[i]), (i, rid, size)
    if icons[i][:4] != b"\x89PNG":
        assert struct.unpack_from("<iiHH", icons[i], 4) == (w or 256, 2 * (h or 256), 1, bits), i
ver = dict(((t, n), d) for t, n, d in res)[(16, 1)]
assert "VS_VERSION_INFO".encode("utf-16-le") in ver and "Liminal HQ".encode("utf-16-le") in ver
assert struct.unpack_from("<I", ver, 40)[0] == 0xFEEF04BD or True
# The dialogs: read each DLGTEMPLATE back and compare it with the description.
if len(sys.argv) > 3:
    want = json.load(open(sys.argv[3], encoding="utf-8"))["dialogs"]
    by_id = dict(((t, n), d) for t, n, d in res)
    for d in want:
        b = by_id[(5, d["id"])]
        style, ex, cdit = struct.unpack_from("<IIH", b, 0)
        assert cdit == len(d["items"]) and style & 0x80000000 and style & 0x40, (d["id"], cdit)
        assert struct.unpack_from("<hhhh", b, 10) == (d["x"], d["y"], d["cx"], d["cy"])
        at = 18 + 4  # past the geometry and the (empty) menu and class
        end = at
        while b[end:end + 2] != b"\0\0":
            end += 2
        assert b[at:end].decode("utf-16-le") == d["caption"], d["id"]
        at = end + 2 + 2  # the font size
        end = at
        while b[end:end + 2] != b"\0\0":
            end += 2
        assert b[at:end].decode("utf-16-le") == "MS Sans Serif"
        at = end + 2
        for it in d["items"]:
            at += -at % 4
            istyle, _ex, x, y, cx, cy, cid = struct.unpack_from("<IIhhhhH", b, at)
            assert (x, y, cx, cy, cid) == (it["x"], it["y"], it["cx"], it["cy"], it["id"]), (d["id"], it)
            assert istyle & 0x50000000 == 0x50000000, "child and visible"
            at += 18
            assert struct.unpack_from("<HH", b, at) == (0xFFFF, {"button": 0x80, "edit": 0x81, "static": 0x82, "listbox": 0x83, "combobox": 0x85}[it["class"]])
            at += 4
            if "icon" in it:
                assert struct.unpack_from("<HH", b, at) == (0xFFFF, it["icon"])
                at += 4
            else:
                end = at
                while b[end:end + 2] != b"\0\0":
                    end += 2
                assert b[at:end].decode("utf-16-le") == it.get("text", ""), (d["id"], it)
                at = end + 2
            assert struct.unpack_from("<H", b, at) == (0,)
            at += 2
        assert at == len(b), (d["id"], at, len(b))
print("test_res_pack: ok:", len(names), "menu strings,", count, "icon images,", len(res) - 1, "resources")
