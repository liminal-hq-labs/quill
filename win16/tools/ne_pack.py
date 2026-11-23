#!/usr/bin/env python3
# Packs a linked 32-bit x86 ELF built for the win16 target into a Win16 NE executable that Windows 3.x, 9x and Wine's krnl386 load.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/ne_pack.py [--module NAME] [--description TEXT] [--stack N] [--heap N] [--resources FILE.json] <in.elf> <out.exe>
#
# The linker has already resolved every symbol; this tool rearranges bytes and writes the tables Windows reads. It reads the ELF section
# headers, not the program headers: `.text` becomes code segment 1 (fixed, preloaded) and `.data` becomes the automatic data segment 2.
# The linker script (link.ld) puts both at offset 0 of their own segment. `.bss` is folded into `.data`.
#
# Symbols the packer reads from the ELF symbol table:
#   __w16imp_<MODULE>_<ordinal>_<Name>   a 4-byte far-call operand in `.text`: an imported-ordinal fixup (type "far address") is written for it
#   __w16exp_<Name>               the start of an exported far procedure: an entry-table entry and a resident name are written for it
#   __w16sel_code_<n>             a 16-bit word in `.text` that must hold the code segment's selector (selector fixup to segment 1)
#   __w16sel_data_<n>             a 16-bit word in `.text` that must hold the data segment's selector (selector fixup to segment 2)
#   _start                        the entry point (CS:IP in the header)
#
# Resources come from a JSON file (see scripts/build.sh and examples/*/resources.json): icons (an .ico file each, as a group with the given ID), dialogs, strings and raw data, each with an integer ID. Raw data (RT_RCDATA)
# starts with its exact length as a little-endian 32-bit number.
import json
import os
import re
import struct
import sys

MZ_STUB_MESSAGE = b"This program requires Microsoft Windows.\r\n$"
SEG_DATA = 0x0001
SEG_PRELOAD = 0x0040
SEG_RELOCS = 0x0100
SEG_RING3 = 0x0C00
NE_MULTIPLEDATA = 0x0002
NE_WINAPI = 0x0300
RES_FLAGS = 0x1030  # MOVEABLE | PURE | DISCARDABLE
RES_SHIFT = 4
FILE_SHIFT = 4
RT_ICON = 3
RT_GROUP_ICON = 14
RT_DIALOG = 5
RT_STRING = 6
RT_RCDATA = 10
CLASS_IDS = {"button": 0x80, "edit": 0x81, "static": 0x82, "listbox": 0x83, "scrollbar": 0x84, "combobox": 0x85}
WS = {
	"popup": 0x80000000,
	"child": 0x40000000,
	"visible": 0x10000000,
	"caption": 0x00C00000,
	"sysmenu": 0x00080000,
	"tabstop": 0x00010000,
	"group": 0x00020000,
	"border": 0x00800000,
	"modalframe": 0x80,
	"setfont": 0x40,
	"defpushbutton": 1,
	"pushbutton": 0,
	"center": 1,
	"left": 0,
	"right": 2,
}


class PackError(Exception):
	pass


def align(x, a):
	return (x + a - 1) & ~(a - 1)


def read_elf(data):
	if data[:4] != b"\x7fELF" or data[4] != 1 or data[5] != 1:
		raise PackError("not a 32-bit little-endian ELF file")
	e_type, e_machine, _v, e_entry, _phoff, e_shoff, _fl, _eh, _phes, _phn, e_shentsize, e_shnum, e_shstrndx = struct.unpack_from("<HHIIIIIHHHHHH", data, 16)
	if e_machine != 3:
		raise PackError("not an Intel 80386 ELF file")
	secs = []
	for i in range(e_shnum):
		name, typ, flags, addr, off, size, link, info, _al, _es = struct.unpack_from("<IIIIIIIIII", data, e_shoff + i * e_shentsize)
		secs.append({"name_off": name, "type": typ, "flags": flags, "addr": addr, "off": off, "size": size, "link": link, "info": info})
	shstr = secs[e_shstrndx]
	for s in secs:
		end = data.index(b"\0", shstr["off"] + s["name_off"])
		s["name"] = data[shstr["off"] + s["name_off"] : end].decode()
	by_name = {s["name"]: s for s in secs}
	syms = {}
	symtab = by_name.get(".symtab")
	if symtab:
		strtab = secs[symtab["link"]]
		for i in range(symtab["size"] // 16):
			st_name, st_value, _sz, _info, _other, st_shndx = struct.unpack_from("<IIIBBH", data, symtab["off"] + 16 * i)
			end = data.index(b"\0", strtab["off"] + st_name)
			nm = data[strtab["off"] + st_name : end].decode()
			if nm:
				syms[nm] = (st_value, secs[st_shndx]["name"] if st_shndx < len(secs) else "")
	return secs, by_name, syms


def section_bytes(data, sec):
	if sec is None or sec["type"] == 8:  # NOBITS
		return b"" if sec is None else bytes(sec["size"])
	return data[sec["off"] : sec["off"] + sec["size"]]


def dialog_template(d):
	"""Encodes one dialog description as a Win16 DLGTEMPLATE (words for the geometry, a class byte of 0x80 or more for stock controls)."""
	style = d.get("style")
	if style is None:
		style = WS["popup"] | WS["caption"] | WS["sysmenu"] | WS["modalframe"] | WS["setfont"]
	elif isinstance(style, list):
		style = sum(WS[s] if isinstance(s, str) else s for s in style)
	items = d.get("items", [])
	out = bytearray()
	out += struct.pack("<IB", style, len(items))
	out += struct.pack("<hhhh", d["x"], d["y"], d["cx"], d["cy"])
	out += b"\0"  # no menu
	out += b"\0"  # default class
	out += d.get("caption", "").encode("latin-1") + b"\0"
	if style & WS["setfont"]:
		out += struct.pack("<H", d.get("font_size", 8)) + d.get("font", "MS Sans Serif").encode("latin-1") + b"\0"
	for it in items:
		istyle = it.get("style", [])
		if isinstance(istyle, list):
			istyle = sum(WS[s] if isinstance(s, str) else s for s in istyle)
		out += struct.pack("<hhhhH", it["x"], it["y"], it["cx"], it["cy"], it["id"])
		out += struct.pack("<I", istyle | WS["child"] | WS["visible"])
		cls = it["class"]
		out += bytes([CLASS_IDS[cls]]) if cls in CLASS_IDS else cls.encode("latin-1") + b"\0"
		out += it.get("text", "").encode("latin-1") + b"\0"
		out += b"\0"  # no creation data
	return bytes(out)


def string_blocks(strings):
	"""Groups string IDs into the 16-string blocks an RT_STRING resource holds. Returns {block_id: bytes}."""
	blocks = {}
	for sid, text in strings.items():
		sid = int(sid)
		blocks.setdefault(sid // 16 + 1, {})[sid % 16] = text.encode("latin-1")
	res = {}
	for bid, items in blocks.items():
		b = bytearray()
		for i in range(16):
			s = items.get(i, b"")
			if len(s) > 255:
				raise PackError("string too long")
			b += bytes([len(s)]) + s
		res[bid] = bytes(b)
	return res


def icon_resources(group_id, ico_path, first_icon_id):
	"""An RT_GROUP_ICON and its RT_ICON images from an .ico file. The images are the resources byte for byte (a DIB with the doubled height,
	its palette, the XOR bits and the AND mask); the group lists them in the same 14-byte entries as a Win32 resource, the last field being the RT_ICON id."""
	with open(ico_path, "rb") as f:
		data = f.read()
	reserved, kind, count = struct.unpack_from("<HHH", data, 0)
	if (reserved, kind) != (0, 1) or count == 0:
		raise PackError(f"{ico_path}: not an icon file")
	group = struct.pack("<HHH", 0, 1, count)
	res = []
	for i in range(count):
		w, h, colours, rsv, planes, bits, size, off = struct.unpack_from("<BBBBHHII", data, 6 + 16 * i)
		if data[off : off + 4] == b"\x89PNG":
			raise PackError(f"{ico_path}: PNG images are not valid in a Win16 icon")
		group += struct.pack("<BBBBHHIH", w, h, colours, rsv, planes, bits, size, first_icon_id + i)
		res.append((RT_ICON, first_icon_id + i, data[off : off + size]))
	return [(RT_GROUP_ICON, group_id, group)] + res


def load_resources(path):
	"""Returns a sorted list of (type_id, res_id, bytes) from a JSON description."""
	if not path:
		return []
	base = os.path.dirname(os.path.abspath(path))
	with open(path, encoding="utf-8") as f:
		spec = json.load(f)
	res = []
	next_icon = 1
	for ic in spec.get("icons", []):
		items = icon_resources(ic["id"], os.path.join(base, ic["file"]), next_icon)
		next_icon += len(items) - 1
		res += items
	for d in spec.get("dialogs", []):
		res.append((RT_DIALOG, d["id"], dialog_template(d)))
	for bid, blob in string_blocks(spec.get("strings", {})).items():
		res.append((RT_STRING, bid, blob))
	for r in spec.get("rcdata", []):
		if "file" in r:
			with open(os.path.join(base, r["file"]), "rb") as f:
				blob = f.read()
		else:
			blob = r["text"].encode("latin-1")
		# NE resource lengths are in units of the alignment, so the exact size is lost; keep it in a 4-byte prefix.
		blob = struct.pack("<I", len(blob)) + blob
		res.append((RT_RCDATA, r["id"], blob))
	return sorted(res, key=lambda r: (r[0], r[1]))


def resource_table(resources, data_base):
	"""Builds the resource table. `data_base` is where the resource data starts in the file. Returns (table bytes, data bytes)."""
	types = {}
	for t, i, blob in resources:
		types.setdefault(t, []).append((i, blob))
	pos = data_base
	body = bytearray()
	data = bytearray()
	body += struct.pack("<H", RES_SHIFT)
	for t in sorted(types):
		items = types[t]
		body += struct.pack("<HHI", 0x8000 | t, len(items), 0)
		for i, blob in items:
			off = pos + len(data)
			if len(blob) > 0xFFFF << RES_SHIFT:
				raise PackError("resource too large")
			# Both the offset and the length are in units of the resource alignment (1 << RES_SHIFT), so every resource is padded to one.
			units = (len(blob) + (1 << RES_SHIFT) - 1) >> RES_SHIFT
			body += struct.pack("<HHHHHH", off >> RES_SHIFT, units, RES_FLAGS, 0x8000 | i, 0, 0)
			data += blob + bytes((units << RES_SHIFT) - len(blob))
	body += struct.pack("<H", 0)  # end of types
	body += b"\0"  # end of resource names
	return bytes(body), bytes(data)


R_386_32 = 1
R_386_PC32 = 2
R_386_PLT32 = 4
R_386_16 = 20
R_386_PC16 = 21
SLOT_SIZE = 4  # a near `jmp rel16` and a pad byte
MAX_SEG = 0xFFFF

# The code the packer writes. Both run with DS = SS = the data segment and use only 16-bit addressing (BP, BX), so the high half of ESP
# does not matter. They keep every register but EBX, ECX and EDX intact and put those back, apart from ECX in the return trampoline, which
# is free by the calling convention (and EAX and EDX carry the result).
#
# A cross-segment call is a near `call` the linker could not resolve (the targets' offsets overlap, since every segment starts at 0). The
# packer sends it to a veneer in the caller's segment: it pushes the caller's return IP and CS on a shadow stack in the data segment,
# points the return slot at a trampoline in the callee's segment and jumps there with a far `jmp`. The callee's `ret` lands on the
# trampoline, which pops the shadow stack and returns far to the caller. Arguments, the stack and every register the caller set are
# untouched.
def veneer_code(ssp, end, ret_off, target, seg):
	"""Returns (bytes, offset of the far jump's operand, ...). `target` is the offset in the callee's segment."""
	b = bytearray()
	b += bytes([0x66, 0x55, 0x89, 0xE5, 0x66, 0x53, 0x66, 0x52])  # push ebp; mov bp,sp; push ebx; push edx
	b += bytes([0x66, 0x8B, 0x1E]) + struct.pack("<H", ssp)  # mov ebx,[ssp]
	b += bytes([0x81, 0xFB]) + struct.pack("<H", end)  # cmp bx,end
	b += bytes([0x72, 0x05, 0xB8, 0x05, 0x4C, 0xCD, 0x21])  # jb ok; mov ax,4C05h; int 21h (the shadow stack is full)
	b += bytes([0x8B, 0x56, 0x04, 0x89, 0x17])  # mov dx,[bp+4]; mov [bx],dx
	b += bytes([0x8C, 0xCA, 0x89, 0x57, 0x02])  # mov dx,cs; mov [bx+2],dx
	b += bytes([0x66, 0x83, 0xC3, 0x04])  # add ebx,4
	b += bytes([0x66, 0x89, 0x1E]) + struct.pack("<H", ssp)  # mov [ssp],ebx
	b += bytes([0x66, 0xC7, 0x46, 0x04]) + struct.pack("<I", ret_off)  # mov dword [bp+4],ret trampoline
	b += bytes([0x66, 0x5A, 0x66, 0x5B, 0x66, 0x5D])  # pop edx; pop ebx; pop ebp
	b += bytes([0xEA])
	far_at = len(b)
	b += struct.pack("<HH", target, 0)
	return bytes(b), far_at


def trampoline_code(ssp):
	b = bytearray()
	b += bytes([0x66, 0x53])  # push ebx
	b += bytes([0x66, 0x8B, 0x1E]) + struct.pack("<H", ssp)  # mov ebx,[ssp]
	b += bytes([0x66, 0x83, 0xEB, 0x04])  # sub ebx,4
	b += bytes([0x66, 0x89, 0x1E]) + struct.pack("<H", ssp)  # mov [ssp],ebx
	b += bytes([0x66, 0x8B, 0x0F])  # mov ecx,[bx]
	b += bytes([0x66, 0x5B, 0x66, 0x51, 0xCB])  # pop ebx; push ecx; retf
	return bytes(b)


def read_symbols(data, secs):
	"""Every symbol: (name, value, section index, size, type)."""
	symtab = next((s for s in secs if s["type"] == 2), None)
	out = []
	if symtab:
		strtab = secs[symtab["link"]]
		for i in range(symtab["size"] // 16):
			st_name, st_value, st_size, st_info, _o, st_shndx = struct.unpack_from("<IIIBBH", data, symtab["off"] + 16 * i)
			end = data.index(b"\0", strtab["off"] + st_name)
			out.append((data[strtab["off"] + st_name : end].decode(), st_value, st_shndx, st_size, st_info & 15))
	return out


def read_relocs(data, secs, target_index):
	"""The REL entries (offset, type, symbol index) that apply to section `target_index`."""
	out = []
	for s in secs:
		if s["type"] == 9 and s["info"] == target_index:
			for i in range(s["size"] // 8):
				off, info = struct.unpack_from("<II", data, s["off"] + 8 * i)
				out.append((off, info & 0xFF, info >> 8))
	return out


def pack(elf_data, module="SETUP", description="", stack=0x2000, heap=0x1000, resources=(), expected_ver=0x030A, map_out=None):
	secs, by_name, _syms = read_elf(elf_data)
	symlist = read_symbols(elf_data, secs)
	syms = {}
	for nm, val, shndx, _sz, _t in symlist:
		if nm and shndx < len(secs):
			syms[nm] = (val, secs[shndx]["name"])
	# The code segments: `.text` alone, or `.text1`, `.text2`... one each.
	segsec = {}
	for i, sec in enumerate(secs):
		m = re.fullmatch(r"\.text(\d*)", sec["name"])
		if m:
			segsec[int(m.group(1) or 1)] = i
	if not segsec:
		raise PackError("no .text")
	nseg = max(segsec)
	if sorted(segsec) != list(range(1, nseg + 1)):
		raise PackError("code sections must be numbered from 1 without gaps")
	segno = {i: n for n, i in segsec.items()}  # section index -> segment number
	data_seg = nseg + 1
	code = {n: bytearray(section_bytes(elf_data, secs[i])) for n, i in segsec.items()}
	for n, c in code.items():
		if len(c) > MAX_SEG:
			raise PackError(f"code segment {n} is {len(c)} bytes, over 64 KB")
		if not c:
			raise PackError(f"code segment {n} is empty")
	dat = bytearray(section_bytes(elf_data, by_name.get(".data")))
	bss = by_name.get(".bss")
	if bss:
		dat += bytes(bss["size"])
	if len(dat) < 16:
		dat += bytes(16 - len(dat))
	if len(dat) + stack + heap > 0x10000:
		raise PackError(f"data {len(dat)} + stack {stack} + heap {heap} is over one 64 KB segment")
	if "_start" not in syms:
		raise PackError("no _start")
	data_index = secs.index(by_name[".data"]) if ".data" in by_name else -1

	def seg_of(sym_index):
		_n, _v, shndx, _s, _t = symlist[sym_index]
		return segno.get(shndx)

	exports_at = set()  # (segment, offset) of every exported procedure: pointers to them are not redirected
	imports = []  # (module, ordinal, segment, offset)
	exports = []  # (name, segment, offset)
	selfix = []  # (segment, offset, target segment)
	for nm, (val, secname) in syms.items():
		sn = next((n for n, i in segsec.items() if secs[i]["name"] == secname), None)
		m = re.fullmatch(r"__w16imp_([A-Z0-9]+)_(\d+)_\w+", nm)
		if m:
			imports.append((m.group(1), int(m.group(2)), sn, val))
			continue
		m = re.fullmatch(r"__w16exp_(\w+)", nm)
		if m:
			exports.append((m.group(1), sn, val))
			exports_at.add((sn, val))
			continue
		m = re.fullmatch(r"__w16sel_(code|data)_\d+", nm)
		if m:
			selfix.append((sn, val, sn if m.group(1) == "code" else data_seg))
	exports.sort()
	imports.sort(key=lambda r: (r[2], r[3]))
	mod_names = []
	for mn, _o, _s, _v in imports:
		if mn not in mod_names:
			mod_names.append(mn)
	mod_names.sort(key=lambda n: ("KERNEL", "USER", "GDI").index(n) if n in ("KERNEL", "USER", "GDI") else 99)

	# Cross-segment references, from the relocations the linker kept (--emit-relocs). The linker resolved each one with every segment
	# starting at offset 0, so the value left in the code is the target's offset in its own segment: for a call, value + site + 4.
	calls = {n: [] for n in code}  # per segment: (site, target segment, target offset)
	pointers = []  # (where, segment or None, site, target segment, target offset): a code address stored as data
	for n, i in segsec.items():
		for off, typ, symi in read_relocs(elf_data, secs, i):
			tseg = seg_of(symi)
			if typ in (R_386_PC32, R_386_PLT32):
				if tseg is None:
					raise PackError(f"segment {n}: a call or jump at {off:#x} targets {symlist[symi][0]!r}, which is not code")
				value = struct.unpack_from("<i", code[n], off)[0]
				if tseg != n:
					calls[n].append((off, tseg, (value + off + 4) & 0xFFFF, 4))
			elif typ == R_386_32:
				if tseg is not None:
					pointers.append(("code", n, off, tseg, struct.unpack_from("<I", code[n], off)[0] & 0xFFFF))
			elif typ == R_386_16:
				# A 16-bit absolute address: a data offset (`mov ecx,[offset]`) is already right; one of code has no slot to point to.
				if tseg is not None:
					raise PackError(f"segment {n}: a 16-bit absolute address of code at {off:#x}")
			elif typ == R_386_PC16:
				# A `jmp` or `jcc` with a 16-bit displacement: a tail call. It goes through a veneer like a call, which is right because
				# the caller's own return address is already on the stack.
				if tseg is not None and tseg != n:
					value = struct.unpack_from("<h", code[n], off)[0]
					calls[n].append((off, tseg, (value + off + 2) & 0xFFFF, 2))
			else:
				raise PackError(f"segment {n}: relocation type {typ} at {off:#x} is not handled")
	if data_index >= 0:
		for off, typ, symi in read_relocs(elf_data, secs, data_index):
			tseg = seg_of(symi)
			if typ == R_386_32 and tseg is not None:
				pointers.append(("data", None, off, tseg, struct.unpack_from("<I", dat, off)[0] & 0xFFFF))
			elif tseg is not None:
				raise PackError(f"data: relocation type {typ} at {off:#x} is not handled")

	need_far = any(calls.values()) or any(p for p in pointers if (p[3], p[4]) not in exports_at)
	ssp = syms.get("__w16_ssp", (None,))[0]
	shadow_end = syms.get("__w16_shadow_end", (None,))[0]
	if need_far and (ssp is None or shadow_end is None):
		raise PackError("code crosses segments but the program has no shadow stack (__w16_ssp, __w16_shadow_end)")

	# Pointers to code go through slots at the same offset in every segment, so a pointer means the same function wherever it is called.
	slot_keys = []
	for where, n, off, tseg, t in pointers:
		if (tseg, t) in exports_at:
			if where == "code" and n != tseg:
				raise PackError(f"segment {n}: the address of an exported procedure in segment {tseg} is taken at {off:#x}")
			continue
		if (tseg, t) not in slot_keys:
			slot_keys.append((tseg, t))

	# Veneers per segment, return trampolines per target segment.
	veneer_keys = {n: [] for n in code}
	for n in code:
		for _off, tseg, t, _w in calls[n]:
			if (tseg, t) not in veneer_keys[n]:
				veneer_keys[n].append((tseg, t))
		for tseg, t in slot_keys:
			if tseg != n and (tseg, t) not in veneer_keys[n]:
				veneer_keys[n].append((tseg, t))
	tramp_needed = sorted({tseg for n in code for tseg, _t in veneer_keys[n]})
	body_len = {n: len(code[n]) for n in code}
	tramp_off = {}
	for n in code:
		if n in tramp_needed:
			tramp_off[n] = len(code[n])
			code[n] += trampoline_code(ssp or 0)
	veneer_off = {n: {} for n in code}
	far_sites = {n: [] for n in code}  # (site of the far pointer, target segment, target offset)
	for n in code:
		for tseg, t in veneer_keys[n]:
			v, far_at = veneer_code(ssp, shadow_end, tramp_off[tseg], t, tseg)
			veneer_off[n][(tseg, t)] = len(code[n])
			far_sites[n].append((len(code[n]) + far_at, tseg, t))
			code[n] += v
	used = {n: len(c) for n, c in code.items()}  # the code, trampoline and veneers, before the slot table pads each segment
	slot_base = 0
	if slot_keys:
		slot_base = align(max(len(c) for c in code.values()), 16)
		if slot_base + SLOT_SIZE * len(slot_keys) > MAX_SEG:
			raise PackError(f"the slot table does not fit: segments run to {slot_base:#x}")
		for n in code:
			code[n] += b"\x90" * (slot_base - len(code[n]))  # nops, so a linear decode stays in step
			for i, (tseg, t) in enumerate(slot_keys):
				at = len(code[n])
				dest = t if tseg == n else veneer_off[n][(tseg, t)]
				code[n] += bytes([0xE9]) + struct.pack("<H", (dest - (at + 3)) & 0xFFFF) + b"\x90"
	# Patch the call sites and the pointers.
	for n in code:
		for off, tseg, t, width in calls[n]:
			dest = veneer_off[n][(tseg, t)]
			if width == 4:
				struct.pack_into("<i", code[n], off, dest - (off + 4))
			else:
				struct.pack_into("<H", code[n], off, (dest - (off + 2)) & 0xFFFF)
	for where, n, off, tseg, t in pointers:
		if (tseg, t) in exports_at:
			target = t
		else:
			target = slot_base + SLOT_SIZE * slot_keys.index((tseg, t))
		if where == "code":
			struct.pack_into("<I", code[n], off, target)
		else:
			struct.pack_into("<I", dat, off, target)

	# Relocation records per segment: 8 bytes each. A fixup that is not additive is a link in a chain: the loader reads the offset of the
	# next fixup from the location it is about to patch, and 0xFFFF ends the chain. Each site here is a chain of one.
	relocs = {n: [] for n in code}
	for mn, o, n, val in imports:
		struct.pack_into("<H", code[n], val, 0xFFFF)
		relocs[n].append((val, struct.pack("<BBHHH", 3, 1, val, mod_names.index(mn) + 1, o)))
	for n, val, target in selfix:
		struct.pack_into("<H", code[n], val, 0xFFFF)
		relocs[n].append((val, struct.pack("<BBHBBH", 2, 0, val, target, 0, 0)))
	for n in code:
		for site, tseg, t in far_sites[n]:
			struct.pack_into("<H", code[n], site, 0xFFFF)
			relocs[n].append((site, struct.pack("<BBHBBH", 3, 0, site, tseg, 0, t)))
	reloc_blob = {}
	for n in code:
		relocs[n].sort(key=lambda r: r[0])
		reloc_blob[n] = struct.pack("<H", len(relocs[n])) + b"".join(r[1] for r in relocs[n])
	code = {n: bytes(c) for n, c in code.items()}
	dat = bytes(dat)

	# Name tables.
	def name_entry(name, ordinal):
		b = name.encode("latin-1")
		return bytes([len(b)]) + b + struct.pack("<H", ordinal)

	resident = name_entry(module, 0) + b"".join(name_entry(n.upper(), i + 1) for i, (n, _s, _o) in enumerate(exports)) + b"\0"
	nonres = (name_entry(description, 0) + b"\0") if description else b"\0"

	imp_names = bytearray(b"\0")
	modref = bytearray()
	for n in mod_names:
		modref += struct.pack("<H", len(imp_names))
		imp_names += bytes([len(n)]) + n.encode()

	entry = bytearray()
	i = 0
	while i < len(exports):
		j = i
		while j < len(exports) and exports[j][1] == exports[i][1] and j - i < 255:
			j += 1
		entry += bytes([j - i, exports[i][1]])  # a bundle of fixed entries in one segment
		for _n, _s, off in exports[i:j]:
			entry += struct.pack("<BH", 0x03, off)
		i = j
	entry += b"\0\0"

	# Layout. The MZ stub is 0x80 bytes, the NE header 0x40, then the tables, each directly after the last.
	ne_off = 0x80
	seg_tab = ne_off + 0x40
	res_tab = seg_tab + 8 * (nseg + 1)
	res_blob_dummy, _ = resource_table(resources, 0) if resources else (b"", b"")
	res_len = len(res_blob_dummy)
	resname_tab = res_tab + res_len
	modref_tab = resname_tab + len(resident)
	impname_tab = modref_tab + len(modref)
	entry_tab = impname_tab + len(imp_names)
	nonres_tab = entry_tab + len(entry)
	tables_end = nonres_tab + len(nonres)
	code_off = {}
	pos = align(tables_end, 1 << FILE_SHIFT)
	for n in range(1, nseg + 1):
		code_off[n] = pos
		pos = align(pos + len(code[n]) + len(reloc_blob[n]), 1 << FILE_SHIFT)
	data_off = pos
	res_data_base = align(data_off + len(dat), 1 << RES_SHIFT)
	res_blob, res_data = resource_table(resources, res_data_base) if resources else (b"", b"")

	seg_entries = b""
	for n in range(1, nseg + 1):
		seg_entries += struct.pack("<HHHH", code_off[n] >> FILE_SHIFT, len(code[n]), SEG_PRELOAD | SEG_RELOCS | SEG_RING3, len(code[n]))
	seg_entries += struct.pack("<HHHH", data_off >> FILE_SHIFT, len(dat), SEG_DATA | SEG_PRELOAD | SEG_RING3, len(dat))

	hdr = bytearray(0x40)
	hdr[0:2] = b"NE"
	hdr[2] = 5
	hdr[3] = 1
	struct.pack_into("<HH", hdr, 4, entry_tab - ne_off, len(entry))
	struct.pack_into("<I", hdr, 8, 0)
	struct.pack_into("<HHHH", hdr, 0x0C, NE_WINAPI | NE_MULTIPLEDATA, data_seg, heap, stack)
	struct.pack_into("<HHHH", hdr, 0x14, syms["_start"][0], 1, 0, data_seg)
	struct.pack_into("<HHHH", hdr, 0x1C, nseg + 1, len(mod_names), len(nonres), seg_tab - ne_off)
	struct.pack_into("<HHHH", hdr, 0x24, res_tab - ne_off, resname_tab - ne_off, modref_tab - ne_off, impname_tab - ne_off)
	struct.pack_into("<I", hdr, 0x2C, nonres_tab)
	struct.pack_into("<HH", hdr, 0x30, 0, FILE_SHIFT)
	struct.pack_into("<H", hdr, 0x34, sum(1 for _ in resources))
	hdr[0x36] = 2  # Windows
	struct.pack_into("<H", hdr, 0x3E, expected_ver)
	if not resources:
		struct.pack_into("<H", hdr, 0x24, resname_tab - ne_off)
	# Entry offsets are measured from the NE header, as are all table offsets above.

	# The DOS header: a one-page image, a 4-paragraph header, the relocation table at 0x40 and the NE header's offset at 0x3C.
	mz = bytearray(0x80)
	struct.pack_into("<2s13H", mz, 0, b"MZ", 0x80, 1, 0, 4, 0, 0xFFFF, 0, 0xB8, 0, 0, 0, 0x40, 0)
	struct.pack_into("<I", mz, 0x3C, ne_off)
	stub = bytes([0x0E, 0x1F, 0xBA, 0x0E, 0x00, 0xB4, 0x09, 0xCD, 0x21, 0xB8, 0x01, 0x4C, 0xCD, 0x21]) + MZ_STUB_MESSAGE
	mz[0x40 : 0x40 + len(stub)] = stub

	out = bytearray()
	out += mz
	out += hdr
	out += seg_entries
	out += res_blob
	out += resident + modref + imp_names + entry + nonres
	for n in range(1, nseg + 1):
		out += bytes(code_off[n] - len(out))
		out += code[n] + reloc_blob[n]
	out += bytes(data_off - len(out))
	out += dat
	if resources:
		out += bytes(res_data_base - len(out))
		out += res_data

	# A map for `ne_addr2sym.py`: every function symbol and veneer, by segment and offset.
	lines = []
	for n in range(1, nseg + 1):
		names = {}
		for nm, val, shndx, size, typ in symlist:
			if nm and segno.get(shndx) == n and typ in (0, 2) and not nm.startswith(("$", ".")):
				names[val] = (size, nm)
		for val, (size, nm) in sorted(names.items()):
			lines.append(f"{n:04X}:{val:04X} {size:5d} {nm}")
		if n in tramp_off:
			lines.append(f"{n:04X}:{tramp_off[n]:04X} {len(trampoline_code(0)):5d} <return trampoline>")
		for (tseg, t), v in sorted(veneer_off[n].items(), key=lambda kv: kv[1]):
			lines.append(f"{n:04X}:{v:04X} {len(veneer_code(0, 0, 0, 0, 0)[0]):5d} <veneer to {tseg:04X}:{t:04X}>")
		for i, (tseg, t) in enumerate(slot_keys):
			lines.append(f"{n:04X}:{slot_base + SLOT_SIZE * i:04X} {SLOT_SIZE:5d} <slot for {tseg:04X}:{t:04X}>")
	if map_out:
		with open(map_out, "w") as f:
			f.write("\n".join(lines) + "\n")
	info = {
		"code": sum(len(c) for c in code.values()),
		"segments": [len(code[n]) for n in range(1, nseg + 1)],
		"bodies": [body_len[n] for n in range(1, nseg + 1)],
		"used": [used[n] for n in range(1, nseg + 1)],
		"veneers": sum(len(v) for v in veneer_off.values()),
		"slots": len(slot_keys),
		"cross_calls": sum(len(c) for c in calls.values()),
		"data": len(dat),
		"stack": stack,
		"heap": heap,
		"imports": len(imports),
		"exports": [n for n, _s, _o in exports],
		"modules": mod_names,
		"relocs": sum(len(r) for r in relocs.values()),
		"resources": len(resources),
		"file": len(out),
	}
	return bytes(out), info


def main():
	args = sys.argv[1:]
	opts = {"module": "SETUP", "description": "", "stack": 0x2000, "heap": 0x1000}
	res_path = None
	budgets = {}
	while args and args[0].startswith("--"):
		o = args.pop(0)
		if o == "--module":
			opts["module"] = args.pop(0)
		elif o == "--description":
			opts["description"] = args.pop(0)
		elif o in ("--stack", "--heap"):
			opts[o[2:]] = int(args.pop(0), 0)
		elif o == "--resources":
			res_path = args.pop(0)
		elif o == "--map":
			opts["map_out"] = args.pop(0)
		elif o == "--budget":
			seg, size = args.pop(0).split(":")
			budgets[int(seg)] = int(size)
		else:
			sys.exit("ne_pack: unknown option " + o)
	if len(args) != 2:
		sys.exit(__doc__)
	try:
		with open(args[0], "rb") as f:
			out, info = pack(f.read(), resources=load_resources(res_path), **opts)
	except PackError as e:
		sys.exit("ne_pack: " + str(e))
	over = [f"segment {n} uses {info['used'][n - 1]} bytes, over its budget of {b}" for n, b in sorted(budgets.items()) if n <= len(info["used"]) and info["used"][n - 1] > b]
	if over:
		sys.exit("ne_pack: " + "; ".join(over))
	with open(args[1], "wb") as f:
		f.write(out)
	segs = ""
	if len(info["segments"]) > 1:
		segs = " in " + " + ".join(str(x) for x in info["segments"]) + f", {info['cross_calls']} cross-segment calls, {info['veneers']} veneers, {info['slots']} pointer slots"
	print(f"ne_pack: {args[1]}: {info['file']} bytes (code {info['code']}{segs}, data {info['data']} + stack {info['stack']} + heap {info['heap']}, {info['imports']} imports, {info['resources']} resources)")


if __name__ == "__main__":
	main()
