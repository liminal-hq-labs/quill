// fat.ts against images mtools made (goldens), and against fsck.fat and mtools for the images it builds.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import { buildVolume, entryName, FatError, fatEntry, listDir, openVolume, parseMbr, readFile, statPath, type NewFile } from "../src/fat.ts";
import { bridgeDisks, fsckClean, golden, have, manifest, mtoolsRead, sha256 } from "./golden.ts";

const names = Object.keys(manifest);

describe("reading what mtools wrote", () => {
	for (const name of names) {
		const info = manifest[name]!;
		const image = golden(name);
		const volume = openVolume(image);

		test(`${name}: the partition table and the volume`, () => {
			const parts = parseMbr(image);
			expect(parts.length).toBe(1);
			expect(parts[0]!.type).toBe(info.partitionType);
			expect(parts[0]!.start).toBe(63);
			expect(volume.fatBits).toBe(name.startsWith("fat12") ? 12 : 16);
			expect(volume.label).toBe(info.label);
			expect(volume.sectorsPerCluster).toBe(1);
		});

		test(`${name}: every file reads back with its size and SHA-256`, () => {
			for (const [path, want] of Object.entries(info.files)) {
				const bytes = readFile(volume, path);
				expect(bytes.length).toBe(want.size);
				expect(sha256(bytes)).toBe(want.sha256);
			}
		});

		test(`${name}: the root lists the files, not the label, long-name pieces or the deleted file`, () => {
			const root = listDir(volume).map((e) => e.name);
			const want = Object.keys(info.files).filter((p) => !p.includes("/"));
			expect(root.sort()).toEqual([...want, "SUB"].sort());
			expect(root).not.toContain("B.BIN");
			expect(listDir(volume, "SUB").map((e) => e.name)).toEqual(["INNER.TXT"]);
			expect(listDir(volume, "sub\\").map((e) => e.name)).toEqual(["INNER.TXT"]);
			expect(statPath(volume, "SUB")?.folder).toBe(true);
		});

		test(`${name}: times come back as the day and time they were written`, () => {
			const t = statPath(volume, "ONE.BIN")!.time;
			const [date, time] = info.time.split("T") as [string, string];
			const [y, mo, d] = date.split("-").map(Number) as [number, number, number];
			const [h, mi, s] = time.split(":").map(Number) as [number, number, number];
			expect([t.getFullYear(), t.getMonth() + 1, t.getDate(), t.getHours(), t.getMinutes(), t.getSeconds()]).toEqual([y, mo, d, h, mi, s]);
		});

		test(`${name}: FRAG.BIN's chain is not contiguous and spans more than one sector of the FAT`, () => {
			const entry = statPath(volume, "FRAG.BIN")!;
			const chain: number[] = [];
			let c = entry.cluster;
			while (c < 0xfff8) {
				chain.push(c);
				c = fatEntry(volume, c);
			}
			expect(chain.length).toBe(Math.ceil(entry.size / 512));
			expect(chain.some((v, i) => i > 0 && v !== chain[i - 1]! + 1)).toBe(true);
			const fatBytes = (volume.fatBits === 12 ? 1.5 : 2) * Math.max(...chain);
			expect(fatBytes).toBeGreaterThan(512);
		});

		test(`${name}: missing files and folders are refused`, () => {
			expect(() => readFile(volume, "NOPE.TXT")).toThrow(FatError);
			expect(() => readFile(volume, "SUB")).toThrow(FatError);
			expect(() => listDir(volume, "NOPE")).toThrow(FatError);
			expect(statPath(volume, "SUB\\NOPE.TXT")).toBeNull();
		});
	}
});

describe("refusing what it cannot read", () => {
	test("an image with no partition table", () => {
		expect(() => openVolume(new Uint8Array(4096))).toThrow(FatError);
	});
	test("a volume larger than its image", () => {
		expect(() => openVolume(golden("fat12.img.gz").subarray(0, 100 * 512))).toThrow(FatError);
	});
	test("nothing panics on every truncation or a damaged boot sector", () => {
		const image = golden("fat12.img.gz");
		for (const cut of [0, 1, 511, 512, 513, 32255, 32256, 32257, 33000]) {
			try {
				const v = openVolume(image.subarray(0, cut));
				listDir(v);
			} catch (error) {
				expect(error).toBeInstanceOf(FatError);
			}
		}
		const broken = image.slice();
		broken[63 * 512 + 13] = 3;
		expect(() => openVolume(broken)).toThrow(FatError);
	});
	test("a loop in a cluster chain", () => {
		const image = golden("fat12.img.gz").slice();
		const v = openVolume(image);
		const entry = statPath(v, "A.BIN")!;
		// Point A.BIN's first cluster at itself, in every FAT copy.
		for (let copy = 0; copy < v.fats; copy++) {
			const start = v.fatStart + copy * v.sectorsPerFat * 512;
			const at = start + Math.floor((entry.cluster * 3) / 2);
			if (entry.cluster & 1) {
				image[at] = (image[at]! & 0x0f) | ((entry.cluster & 0x0f) << 4);
				image[at + 1] = (entry.cluster >> 4) & 0xff;
			} else {
				image[at] = entry.cluster & 0xff;
				image[at + 1] = (image[at + 1]! & 0xf0) | ((entry.cluster >> 8) & 0x0f);
			}
		}
		expect(() => readFile(openVolume(image), "A.BIN")).toThrow(FatError);
	});
});

describe("building a volume", () => {
	const when = new Date(2026, 11, 25, 13, 5, 8);
	const bytesOf = (n: number, salt: number): Uint8Array => Uint8Array.from({ length: n }, (_, i) => (i * 13 + salt) % 253);

	for (const name of names) {
		test(`${name}: files of every size around a cluster read back, and the old files are gone`, () => {
			const template = golden(name);
			const copy = template.slice();
			const sizes = [0, 1, 511, 512, 513, 1023, 1024, 1025, 2049];
			const files: NewFile[] = sizes.map((n, i) => ({ name: `F${i}.BIN`, bytes: bytesOf(n, i), time: when }));
			const built = buildVolume(template, files);
			expect(template).toEqual(copy);
			const v = openVolume(built);
			expect(v.label).toBe(manifest[name]!.label);
			expect(listDir(v).map((e) => e.name)).toEqual(files.map((f) => f.name));
			for (const f of files) expect(readFile(v, f.name)).toEqual(f.bytes);
			expect(statPath(v, "FRAG.BIN")).toBeNull();
			// What the template held in the data area beyond these files (the files use under 40 sectors) is zeroed.
			expect(built.subarray(v.dataStart + 40 * 512, v.dataStart + 1000 * 512).every((b) => b === 0)).toBe(true);
			const t = statPath(v, "F1.BIN")!.time;
			expect([t.getFullYear(), t.getMonth(), t.getDate(), t.getHours(), t.getMinutes(), t.getSeconds()]).toEqual([2026, 11, 25, 13, 5, 8]);
		});
	}

	test("a chain that crosses the FAT12's sector boundary (cluster 341 and 342) is read the same by this code, fsck.fat and mtools", () => {
		const big = bytesOf(345 * 512, 3);
		const files: NewFile[] = [
			{ name: "ODD.BIN", bytes: bytesOf(512, 1), time: when },
			{ name: "BIG.BIN", bytes: big, time: when },
			{ name: "AFTER.BIN", bytes: bytesOf(777, 2), time: when },
		];
		const built = buildVolume(golden("fat12.img.gz"), files);
		expect(readFile(openVolume(built), "BIG.BIN")).toEqual(big);
		if (have("fsck.fat")) expect(fsckClean(built)).toEqual({ ok: true, output: expect.any(String) });
		if (have("mcopy")) {
			expect(mtoolsRead(built, "/BIG.BIN")).toEqual(big);
			expect(mtoolsRead(built, "/AFTER.BIN")).toEqual(files[2]!.bytes);
		}
	});

	test("fsck.fat and mtools accept every image built from the goldens", () => {
		if (!have("fsck.fat") || !have("mcopy")) return;
		for (const name of names) {
			const files: NewFile[] = [
				{ name: "SETDATE.BAT", bytes: Uint8Array.from("@DATE 12-25-2026\r\n", (c) => c.charCodeAt(0)), time: when },
				{ name: "MID.DAT", bytes: bytesOf(5000, 7), time: when },
			];
			const built = buildVolume(golden(name), files);
			const result = fsckClean(built);
			expect(result.output).toContain("3 files");
			expect(result.ok).toBe(true);
			for (const f of files) expect(mtoolsRead(built, `/${f.name}`)).toEqual(f.bytes);
		}
	});

	test("the real C: and D: images take files, pass fsck.fat and read back through mtools", () => {
		const disks = bridgeDisks();
		if (!disks) return;
		const c = openVolume(disks.c);
		const d = openVolume(disks.d);
		expect([c.fatBits, c.label, c.sectorsPerCluster]).toEqual([12, "QUILLDOCS", 4]);
		expect([d.fatBits, d.label, d.sectorsPerCluster]).toEqual([16, "FROMPC", 2]);
		const files: NewFile[] = [
			{ name: "SETDATE.BAT", bytes: Uint8Array.from("@DATE 12-25-2026\r\n", (ch) => ch.charCodeAt(0)), time: when },
			{ name: "BIG.BIN", bytes: bytesOf(3_000_000, 5), time: when },
		];
		const built = buildVolume(disks.d, files);
		expect(built.length).toBe(disks.d.length);
		expect(readFile(openVolume(built), "BIG.BIN")).toEqual(files[1]!.bytes);
		if (have("fsck.fat")) expect(fsckClean(built).ok).toBe(true);
		if (have("mcopy")) expect(mtoolsRead(built, "/SETDATE.BAT")).toEqual(files[0]!.bytes);
		// Building again from what was built (as the page does on its second write) starts clean.
		const again = buildVolume(built, [files[0]!]);
		expect(listDir(openVolume(again)).map((e) => e.name)).toEqual(["SETDATE.BAT"]);
		expect(again.subarray(d.dataStart + 4096, d.dataStart + 3_000_000).every((b) => b === 0)).toBe(true);
		// An empty C: and D: are valid volumes with the label and nothing else.
		expect(listDir(c)).toEqual([]);
		expect(listDir(d)).toEqual([]);
	});

	test("refuses, building nothing, what does not fit", () => {
		const template = golden("fat12.img.gz");
		const v = openVolume(template);
		const per = v.sectorsPerCluster * 512;
		expect(() => buildVolume(template, [{ name: "HUGE.BIN", bytes: new Uint8Array((v.clusters + 1) * per), time: when }])).toThrow(/clusters/);
		expect(() => buildVolume(template, [{ name: "EXACT.BIN", bytes: new Uint8Array(v.clusters * per), time: when }])).not.toThrow();
		const many = Array.from({ length: v.rootEntries }, (_, i) => ({ name: `N${i}.TXT`, bytes: new Uint8Array(1), time: when }));
		expect(() => buildVolume(template, many)).toThrow(/root holds/);
		expect(() => buildVolume(template, many.slice(0, v.rootEntries - 1))).not.toThrow();
		expect(() => buildVolume(template, [{ name: "A.TXT", bytes: new Uint8Array(1), time: when }, { name: "A.TXT", bytes: new Uint8Array(1), time: when }])).toThrow(/twice/);
		expect(() => buildVolume(template, [{ name: "toolongname.txt", bytes: new Uint8Array(1), time: when }])).toThrow(/8.3/);
	});
});

describe("entryName", () => {
	test("pads an upper-case 8.3 name into eleven bytes", () => {
		expect(new TextDecoder().decode(entryName("README.TXT"))).toBe("README  TXT");
		expect(new TextDecoder().decode(entryName("NOEXT"))).toBe("NOEXT      ");
	});
	test("refuses anything else", () => {
		for (const bad of ["lower.txt", "WAYTOOLONG.TXT", "A.TOOL", "A B.TXT", "A.B.C", "", "A*.TXT", "É.TXT"]) expect(() => entryName(bad)).toThrow(FatError);
	});
});
