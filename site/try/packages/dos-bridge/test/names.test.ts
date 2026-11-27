// names.ts: the 8.3 rules, collisions, device names, Windows-1252 and the NAMES.TXT index.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import { encodeWin1252, namesIndex, shortName, shortNames } from "../src/names.ts";

const none = new Set<string>();
const text = (b: Uint8Array): string => Array.from(b, (c) => String.fromCharCode(c)).join("");

describe("shortName", () => {
	const table: [string, string][] = [
		["letter.rtf", "LETTER.RTF"],
		["README", "README"],
		["a", "A"],
		["Résumé.doc", "RESUME.DOC"],
		["Åsa Öberg.txt", "ASA_OB~1.TXT"],
		["my notes.txt", "MY_NOTES.TXT"],
		["My letter (draft).rtf", "MY_LET~1.RTF"],
		["a.b.c.txt", "A_B_C.TXT"],
		["archive.tar.gz", "ARCHIV~1.GZ"],
		["file.markdown", "FILE.MAR"],
		["averyverylongname.txt", "AVERYV~1.TXT"],
		["abcdefgh.txt", "ABCDEFGH.TXT"],
		["abcdefghi.txt", "ABCDEF~1.TXT"],
		["x+y=z,[1].txt", "X_Y_Z_~1.TXT"],
		["日本語.txt", "___.TXT"],
		["50% off!.txt", "50%_OFF!.TXT"],
		["!#$%&'()-@^_{}~`.x", "!#$%&'~1.X"],
		[".hidden", "FILE.HID"],
		[".", "FILE"],
		["", "FILE"],
		["...", "__"],
		["dir/sub\\name.txt", "NAME.TXT"],
		["CON", "_CON"],
		["con.txt", "_CON.TXT"],
		["Aux.rtf", "_AUX.RTF"],
		["nul", "_NUL"],
		["lpt1.prn", "_LPT1.PRN"],
		["COM9", "_COM9"],
		["COM10.txt", "COM10.TXT"],
		["clock$", "_CLOCK$"],
		["console.txt", "CONSOLE.TXT"],
	];
	for (const [long, short] of table) {
		test(`${JSON.stringify(long)} becomes ${short}`, () => {
			expect(shortName(long, none)).toBe(short);
		});
	}

	test("every result is a valid 8.3 name", () => {
		for (const [long] of table) expect(shortName(long, none)).toMatch(/^[A-Z0-9!#$%&'()\-@^_{}~`]{1,8}(\.[A-Z0-9!#$%&'()\-@^_{}~`]{1,3})?$/);
	});

	test("an exact name already taken gets ~1, and the tilde is read at each width", () => {
		expect(shortName("letter.rtf", new Set(["LETTER.RTF"]))).toBe("LETTER~1.RTF");
		expect(shortName("ab.rtf", new Set(["AB.RTF"]))).toBe("AB~1.RTF");
	});
});

describe("shortNames", () => {
	test("collisions run to ~10 and beyond, each name unique", () => {
		const longs = Array.from({ length: 12 }, (_, i) => `My long document ${i}.rtf`);
		const out = shortNames(longs).map((e) => e.short);
		expect(out.slice(0, 3)).toEqual(["MY_LON~1.RTF", "MY_LON~2.RTF", "MY_LON~3.RTF"]);
		expect(out[8]).toBe("MY_LON~9.RTF");
		expect(out[9]).toBe("MY_LO~10.RTF");
		expect(out[11]).toBe("MY_LO~12.RTF");
		expect(new Set(out).size).toBe(12);
	});
	test("names differing only in case collide, and the taken set is respected", () => {
		const out = shortNames(["Letter.RTF", "letter.rtf"], ["setdate.bat"]).map((e) => e.short);
		expect(out).toEqual(["LETTER.RTF", "LETTER~1.RTF"]);
		expect(shortNames(["setdate.bat"], ["SETDATE.BAT"])[0]!.short).toBe("SETDAT~1.BAT");
	});
	test("an empty list gives an empty list", () => {
		expect(shortNames([])).toEqual([]);
	});
});

describe("Windows-1252 and NAMES.TXT", () => {
	test("encodes the code page's own characters and uses ? for the rest", () => {
		expect(Array.from(encodeWin1252("A é ½ € — “ ”"))).toEqual([0x41, 0x20, 0xe9, 0x20, 0xbd, 0x20, 0x80, 0x20, 0x97, 0x20, 0x93, 0x20, 0x94]);
		expect(text(encodeWin1252("日本"))).toBe("??");
		expect(Array.from(encodeWin1252("\u0081\u008d"))).toEqual([0x3f, 0x3f]);
	});
	test("NAMES.TXT lists each short name with its long name, CR LF between", () => {
		const entries = shortNames(["My letter (draft).rtf", "Café.txt", "日本.txt"]);
		expect(text(namesIndex(entries))).toBe("MY_LET~1.RTF = My letter (draft).rtf\r\nCAFE.TXT = Caf\xe9.txt\r\n__.TXT = ??.txt\r\n");
	});
	test("a long name cannot break the index's lines", () => {
		const out = text(namesIndex([{ short: "A.TXT", long: "one\r\ntwo" }]));
		expect(out).toBe("A.TXT = one two\r\n");
	});
});
