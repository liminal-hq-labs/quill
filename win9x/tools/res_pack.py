#!/usr/bin/env python3
# Writes a Win32 .res file (menu, version information, icon) from quill.json, for lld-link to put in QUILL32.EXE.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: res_pack.py quill.json out.res [icon.ico [dialogs.json]]
#
# The menu comes from the app's own menu tree (emit-resources writes the JSON), so the native menu and the DOS menus cannot drift.
# The icon is assets/icon/QUILL.ICO unless a third argument names another: every image in it becomes an RT_ICON (ids 1 to N) and the
# RT_GROUP_ICON with id 1 lists them, so LoadIcon(instance, 1) gets the size and depth the display wants.
# No manifest: Windows 9x ignores one and Quill draws its own chrome (D35).
import json
import os
import struct
import sys

RT_ICON, RT_MENU, RT_DIALOG, RT_GROUP_ICON, RT_VERSION = 3, 4, 5, 14, 16
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


# Window, dialog and control style bits by name (the same names the Win16 packer reads from assets/dialogs/dialogs.json).
STYLE = {
    "popup": 0x80000000, "child": 0x40000000, "visible": 0x10000000, "caption": 0x00C00000, "sysmenu": 0x00080000,
    "tabstop": 0x00010000, "group": 0x00020000, "border": 0x00800000, "vscroll": 0x00200000, "modalframe": 0x80, "setfont": 0x40,
    "center": 0x800, "pushbutton": 0, "defpushbutton": 1, "autocheckbox": 3, "autoradiobutton": 9, "groupbox": 7, "icon": 3,
    "autohscroll": 0x80, "dropdown": 2, "dropdownlist": 3, "notify": 1,
}
CLASS = {"button": 0x80, "edit": 0x81, "static": 0x82, "listbox": 0x83, "scrollbar": 0x84, "combobox": 0x85}


def style_bits(v):
    return sum(STYLE[x] if isinstance(x, str) else x for x in v)


def dialog(d):
    """A DLGTEMPLATE (the 32-bit form, strings in UTF-16) for one dialog description."""
    style = style_bits(d.get("style", ["popup", "caption", "sysmenu", "modalframe", "setfont"]))
    items = d.get("items", [])
    out = struct.pack("<IIH", style, 0, len(items)) + struct.pack("<hhhh", d["x"], d["y"], d["cx"], d["cy"])
    out += u16(0, 0) + wstr(d.get("caption", ""))
    if style & STYLE["setfont"]:
        out += u16(d.get("font_size", 8)) + wstr(d.get("font", "MS Sans Serif"))
    for it in items:
        out = pad4(out)
        istyle = style_bits(it.get("style", [])) | STYLE["child"] | STYLE["visible"]
        out += struct.pack("<IIhhhhH", istyle, 0, it["x"], it["y"], it["cx"], it["cy"], it["id"])
        out += u16(0xFFFF, CLASS[it["class"]])
        out += u16(0xFFFF, it["icon"]) if "icon" in it else wstr(it.get("text", ""))
        out += u16(0)
    return out


def menu_items(items, last_level):
    """The entries of one pop-up: a separator is an item with no id and no text, a submenu a pop-up that holds its own entries."""
    out = b""
    for ii, it in enumerate(items):
        last = 0x80 if ii == len(items) - 1 else 0
        if it.get("separator"):
            out += u16(0x800 | last, 0) + wstr("")
        elif "items" in it:
            out += u16(0x10 | last) + wstr(it["text"]) + menu_items(it["items"], True)
        else:
            out += u16(last, it["id"]) + wstr(it["text"])
    return out


def menu(menus):
    out = u16(0, 0)
    for mi, m in enumerate(menus):
        last_menu = mi == len(menus) - 1
        out += u16(0x10 | (0x80 if last_menu else 0)) + wstr(m["title"])
        out += menu_items(m["items"], True)
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


def icon(path):
    """The group and one RT_ICON per image of an .ico file: the file's images are the resources, byte for byte."""
    data = open(path, "rb").read()
    reserved, kind, count = struct.unpack_from("<HHH", data, 0)
    assert (reserved, kind) == (0, 1) and count > 0, path
    group = u16(0, 1, count)
    images = []
    for i in range(count):
        w, h, colours, res, planes, bits, size, off = struct.unpack_from("<BBBBHHII", data, 6 + 16 * i)
        images.append(data[off:off + size])
        group += bytes([w, h, colours, res]) + u16(planes, bits) + struct.pack("<IH", size, i + 1)
    return group, images


def main():
    spec = json.load(open(sys.argv[1], encoding="utf-8"))
    out = struct.pack("<II", 0, 32) + u16(0xFFFF, 0, 0xFFFF, 0) + struct.pack("<IHHII", 0, 0, 0, 0, 0)
    out += entry(RT_MENU, 1, menu(spec["menus"]))
    path = sys.argv[3] if len(sys.argv) > 3 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "assets", "icon", "QUILL.ICO")
    group, images = icon(path)
    out += entry(RT_GROUP_ICON, 1, group)
    for i, image in enumerate(images):
        out += entry(RT_ICON, i + 1, image)
    out += entry(RT_VERSION, 1, version(spec["version"]))
    if len(sys.argv) > 4:
        for d in json.load(open(sys.argv[4], encoding="utf-8"))["dialogs"]:
            out += entry(RT_DIALOG, d["id"], dialog(d))
    open(sys.argv[2], "wb").write(out)


main()
