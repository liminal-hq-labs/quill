// Long names to 8.3 names, and the NAMES.TXT index that says which is which; and Windows-1252, the code page DOS Quill reads and writes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// The rules are those of docs/plans/dos-page-bridge.md, "Files into the PC": the name is folded to ASCII (accents dropped, so é gives E)
// and upper-cased; every character other than letters, digits and !#$%&'()-@^_{}~` becomes `_`; the extension is what follows the last
// dot, cut to 3; the base is cut to 8; a base that was cut, or a name that collides, becomes its first characters, `~` and a number; a
// device name gets `_` in front; an empty base is FILE.

const KEEP = /^[A-Z0-9!#$%&'()\-@^_{}~`]$/;
const DEVICES = new Set(["CON", "PRN", "AUX", "NUL", "CLOCK$", ...Array.from({ length: 9 }, (_, i) => `COM${i + 1}`), ...Array.from({ length: 9 }, (_, i) => `LPT${i + 1}`)]);

function fold(text: string): string {
	let out = "";
	// NFD splits an accented letter into the letter and its marks; the marks (U+0300 to U+036F) are dropped.
	for (const ch of text.normalize("NFD").replace(/[̀-ͯ]/g, "")) {
		const up = ch.toUpperCase();
		out += KEEP.test(up) ? up : "_";
	}
	return out;
}

export interface ShortName {
	/** `BASE.EXT`, upper case. */
	short: string;
	long: string;
}

/**
 * The 8.3 name for a long one, not in `taken` (short names already in use, upper case; the caller adds the result). A name with a
 * path is reduced to its last part.
 */
export function shortName(long: string, taken: ReadonlySet<string>): string {
	const leaf = long.split(/[\\/]/).pop() ?? "";
	const dot = leaf.lastIndexOf(".");
	let base = fold(dot > 0 ? leaf.slice(0, dot) : dot === 0 ? "" : leaf);
	const ext = fold(dot >= 0 ? leaf.slice(dot + 1) : "").slice(0, 3);
	// Leading dots and spaces carry no letters of their own; a name of nothing but `_` from them still counts as a name.
	if (base === "") base = "FILE";
	if (DEVICES.has(base)) base = `_${base}`;
	const join = (b: string): string => (ext ? `${b}.${ext}` : b);
	if (base.length <= 8 && !taken.has(join(base))) return join(base);
	for (let n = 1; n < 1_000_000; n++) {
		const tail = `~${n}`;
		const candidate = join(base.slice(0, 8 - tail.length) + tail);
		if (!taken.has(candidate)) return candidate;
	}
	throw new Error("No short name is free");
}

/** Short names for a list of long ones, in order, each unique (and none in `taken`). */
export function shortNames(longs: readonly string[], taken: Iterable<string> = []): ShortName[] {
	const used = new Set(Array.from(taken, (t) => t.toUpperCase()));
	return longs.map((long) => {
		const short = shortName(long, used);
		used.add(short);
		return { short, long };
	});
}

const WIN1252_HIGH = [
	0x20ac, 0, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021, 0x02c6, 0x2030, 0x0160, 0x2039, 0x0152, 0, 0x017d, 0,
	0, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0x02dc, 0x2122, 0x0161, 0x203a, 0x0153, 0, 0x017e, 0x0178,
];

/** Text to Windows-1252 bytes, `?` for what the code page lacks. Line ends are left as they are. */
export function encodeWin1252(text: string): Uint8Array {
	const out: number[] = [];
	for (const ch of text) {
		const code = ch.codePointAt(0) ?? 0x3f;
		if (code < 0x80 || (code >= 0xa0 && code <= 0xff)) out.push(code);
		else {
			const at = WIN1252_HIGH.indexOf(code);
			out.push(at >= 0 && code !== 0 ? 0x80 + at : 0x3f);
		}
	}
	return Uint8Array.from(out);
}

/** The text of `NAMES.TXT`: one `SHORT.EXT = long name` line each, CR LF line ends, in Windows-1252. */
export function namesIndex(entries: readonly ShortName[]): Uint8Array {
	return encodeWin1252(entries.map((e) => `${e.short} = ${e.long.replace(/[\r\n]+/g, " ")}\r\n`).join(""));
}
