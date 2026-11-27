// FAT12 and FAT16 behind an MBR: read the folders and files of a hard disk image, and build a fresh volume from a list of files.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Pure: bytes in, bytes out, no DOM and no emulator. Everything it reads is bounds-checked against the image, and everything it writes
// stays inside the image it is given a copy of; a volume that does not hold the files is refused with the reason and nothing is built.

export const SECTOR = 512;
// FAT12's first end-of-chain value; FAT16's is 0xFFF8 and both are returned by `fatEntry` as 0xFFF8.
const END_OF_CHAIN = 0xff8;

export class FatError extends Error {
	constructor(message: string) {
		super(message);
		this.name = "FatError";
	}
}

export interface Partition {
	/** The first sector (LBA) and the number of sectors. */
	start: number;
	sectors: number;
	/** The MBR's type byte: 01h FAT12, 04h and 06h FAT16. */
	type: number;
	active: boolean;
}

export interface Volume {
	image: Uint8Array;
	/** Byte offset of the partition's first sector. */
	base: number;
	fatBits: 12 | 16;
	bytesPerSector: number;
	sectorsPerCluster: number;
	reserved: number;
	fats: number;
	rootEntries: number;
	sectorsPerFat: number;
	totalSectors: number;
	media: number;
	/** Byte offsets, within the image, of the first FAT, the root folder and cluster 2. */
	fatStart: number;
	rootStart: number;
	dataStart: number;
	/** The number of data clusters (numbered from 2). */
	clusters: number;
	label: string;
}

export interface DirEntry {
	/** The short name as written: `NAME.EXT`, upper case. */
	name: string;
	folder: boolean;
	size: number;
	cluster: number;
	attributes: number;
	/** The entry's modification time, read as local time. */
	time: Date;
}

export interface NewFile {
	/** A short name (`NAME.EXT`, upper case, 8.3). */
	name: string;
	bytes: Uint8Array;
	time: Date;
}

const u16 = (d: Uint8Array, o: number): number => (d[o] ?? 0) | ((d[o + 1] ?? 0) << 8);
const u32 = (d: Uint8Array, o: number): number => (u16(d, o) | (u16(d, o + 2) << 16)) >>> 0;

/** The partitions of an image's master boot record (the used entries of its table). */
export function parseMbr(image: Uint8Array): Partition[] {
	if (image.length < SECTOR || image[510] !== 0x55 || image[511] !== 0xaa) throw new FatError("The image has no partition table (no 55 AA signature)");
	const found: Partition[] = [];
	for (let i = 0; i < 4; i++) {
		const at = 446 + i * 16;
		const type = image[at + 4] ?? 0;
		const sectors = u32(image, at + 12);
		if (type === 0 || sectors === 0) continue;
		found.push({ start: u32(image, at + 8), sectors, type, active: image[at] === 0x80 });
	}
	return found;
}

/** Opens the volume in a partition (the first by default). Only FAT12 and FAT16 are read; anything else is refused. */
export function openVolume(image: Uint8Array, index = 0): Volume {
	const part = parseMbr(image)[index];
	if (!part) throw new FatError("The image has no such partition");
	const base = part.start * SECTOR;
	if (base + SECTOR > image.length) throw new FatError("The partition starts beyond the end of the image");
	const bytesPerSector = u16(image, base + 11);
	const sectorsPerCluster = image[base + 13] ?? 0;
	const reserved = u16(image, base + 14);
	const fats = image[base + 16] ?? 0;
	const rootEntries = u16(image, base + 17);
	const total16 = u16(image, base + 19);
	const media = image[base + 21] ?? 0;
	const sectorsPerFat = u16(image, base + 22);
	const totalSectors = total16 !== 0 ? total16 : u32(image, base + 32);
	if (bytesPerSector !== SECTOR) throw new FatError(`The volume has ${bytesPerSector}-byte sectors; only 512 is read`);
	if (sectorsPerCluster === 0 || (sectorsPerCluster & (sectorsPerCluster - 1)) !== 0) throw new FatError("The volume's cluster size is not a power of two");
	if (reserved === 0 || fats === 0 || fats > 2 || sectorsPerFat === 0 || rootEntries === 0) throw new FatError("The volume is not FAT12 or FAT16 (the boot sector's fields are not those of one)");
	if (totalSectors === 0 || base + totalSectors * SECTOR > image.length) throw new FatError("The volume is larger than its image");
	const rootSectors = Math.ceil((rootEntries * 32) / SECTOR);
	const firstData = reserved + fats * sectorsPerFat + rootSectors;
	if (firstData >= totalSectors) throw new FatError("The volume's tables leave no room for data");
	const clusters = Math.floor((totalSectors - firstData) / sectorsPerCluster);
	// Microsoft's rule: the number of clusters, not the label in the boot sector, says which FAT it is.
	if (clusters >= 65525) throw new FatError("The volume is FAT32; only FAT12 and FAT16 are read");
	const fatBits = clusters < 4085 ? 12 : 16;
	const rootStart = base + (reserved + fats * sectorsPerFat) * SECTOR;
	const volume: Volume = {
		image,
		base,
		fatBits,
		bytesPerSector,
		sectorsPerCluster,
		reserved,
		fats,
		rootEntries,
		sectorsPerFat,
		totalSectors,
		media,
		fatStart: base + reserved * SECTOR,
		rootStart,
		dataStart: rootStart + rootSectors * SECTOR,
		clusters,
		label: "",
	};
	for (const raw of rawEntries(volume, 0)) {
		if ((raw.attributes & 0x0f) === 0x08) {
			volume.label = decode(image.subarray(raw.at, raw.at + 11)).trimEnd();
			break;
		}
	}
	return volume;
}

function decode(bytes: Uint8Array): string {
	let out = "";
	for (const b of bytes) out += String.fromCharCode(b);
	return out;
}

/** A FAT entry: the next cluster of a chain, 0 for free, or 0xFF8 and up for the end. */
export function fatEntry(v: Volume, cluster: number): number {
	if (cluster < 0 || cluster >= v.clusters + 2) throw new FatError(`Cluster ${cluster} is outside the volume`);
	if (v.fatBits === 16) {
		const word = u16(v.image, v.fatStart + cluster * 2);
		return word >= 0xfff8 ? 0xfff8 : word;
	}
	const at = v.fatStart + Math.floor((cluster * 3) / 2);
	const pair = u16(v.image, at);
	const value = cluster & 1 ? pair >> 4 : pair & 0xfff;
	// A FAT12 end-of-chain mark (0xFF8 to 0xFFF) is returned as the FAT16 one, so a caller has one value to look for.
	return value >= END_OF_CHAIN ? 0xfff8 : value;
}

function isEnd(value: number): boolean {
	return value >= 0xfff8;
}

function chainOf(v: Volume, first: number): number[] {
	const out: number[] = [];
	let cluster = first;
	while (cluster >= 2 && cluster < v.clusters + 2) {
		out.push(cluster);
		if (out.length > v.clusters) throw new FatError("A cluster chain loops");
		const next = fatEntry(v, cluster);
		if (next < 2 || isEnd(next)) break;
		cluster = next;
	}
	return out;
}

function clusterOffset(v: Volume, cluster: number): number {
	return v.dataStart + (cluster - 2) * v.sectorsPerCluster * SECTOR;
}

interface RawEntry {
	at: number;
	attributes: number;
}

function* rawEntries(v: Volume, folderCluster: number): Generator<RawEntry> {
	const spans: [number, number][] = [];
	if (folderCluster === 0) spans.push([v.rootStart, v.rootEntries * 32]);
	else for (const c of chainOf(v, folderCluster)) spans.push([clusterOffset(v, c), v.sectorsPerCluster * SECTOR]);
	for (const [start, length] of spans) {
		for (let at = start; at < start + length; at += 32) {
			const first = v.image[at] ?? 0;
			if (first === 0) return;
			if (first === 0xe5) continue;
			yield { at, attributes: v.image[at + 11] ?? 0 };
		}
	}
}

function dosTime(date: number, time: number): Date {
	const day = date & 31;
	const month = (date >> 5) & 15;
	return new Date(1980 + (date >> 9), Math.max(month, 1) - 1, Math.max(day, 1), time >> 11, (time >> 5) & 63, (time & 31) * 2);
}

/** The entries of a folder (the root when `path` is empty), without volume labels, long-name pieces, `.` and `..`. */
export function listDir(v: Volume, path = ""): DirEntry[] {
	return listCluster(v, folderClusterOf(v, path));
}

function listCluster(v: Volume, cluster: number): DirEntry[] {
	const out: DirEntry[] = [];
	for (const raw of rawEntries(v, cluster)) {
		const attributes = raw.attributes;
		if ((attributes & 0x0f) === 0x0f || (attributes & 0x08) !== 0) continue;
		const at = raw.at;
		const head = Array.from(v.image.subarray(at, at + 8));
		if (head[0] === 0x05) head[0] = 0xe5;
		const base = decode(Uint8Array.from(head)).trimEnd();
		const ext = decode(v.image.subarray(at + 8, at + 11)).trimEnd();
		const name = ext ? `${base}.${ext}` : base;
		if (name === "." || name === "..") continue;
		out.push({
			name,
			folder: (attributes & 0x10) !== 0,
			size: u32(v.image, at + 28),
			cluster: u16(v.image, at + 26),
			attributes,
			time: dosTime(u16(v.image, at + 24), u16(v.image, at + 22)),
		});
	}
	return out;
}

function splitPath(path: string): string[] {
	return path
		.split(/[\\/]/)
		.filter((p) => p !== "")
		.map((p) => p.toUpperCase());
}

function folderClusterOf(v: Volume, path: string): number {
	let cluster = 0;
	for (const part of splitPath(path)) {
		const hit = listCluster(v, cluster).find((e) => e.name === part);
		if (!hit || !hit.folder) throw new FatError(`No folder ${part}`);
		cluster = hit.cluster;
	}
	return cluster;
}

/** The entry at a path (`DIR\FILE.TXT`, either slash), or null. */
export function statPath(v: Volume, path: string): DirEntry | null {
	const parts = splitPath(path);
	const last = parts.pop();
	if (last === undefined) return null;
	let cluster = 0;
	for (const part of parts) {
		const hit = listCluster(v, cluster).find((e) => e.name === part);
		if (!hit || !hit.folder) return null;
		cluster = hit.cluster;
	}
	return listCluster(v, cluster).find((e) => e.name === last) ?? null;
}

/** A file's bytes by path; throws when it is not there or is a folder. */
export function readFile(v: Volume, path: string): Uint8Array {
	const entry = statPath(v, path);
	if (!entry || entry.folder) throw new FatError(`No file ${path}`);
	return readEntry(v, entry);
}

export function readEntry(v: Volume, entry: DirEntry): Uint8Array {
	const out = new Uint8Array(entry.size);
	if (entry.size === 0) return out;
	const per = v.sectorsPerCluster * SECTOR;
	const chain = chainOf(v, entry.cluster);
	if (chain.length * per < entry.size) throw new FatError(`${entry.name}'s cluster chain is shorter than its size`);
	let written = 0;
	for (const cluster of chain) {
		const at = clusterOffset(v, cluster);
		const take = Math.min(per, entry.size - written);
		if (take <= 0) break;
		if (at + take > v.image.length) throw new FatError("A cluster lies beyond the image");
		out.set(v.image.subarray(at, at + take), written);
		written += take;
	}
	return out;
}

function dosStamp(time: Date): { date: number; time: number } {
	const year = Math.min(Math.max(time.getFullYear(), 1980), 2107);
	return {
		date: ((year - 1980) << 9) | ((time.getMonth() + 1) << 5) | time.getDate(),
		time: (time.getHours() << 11) | (time.getMinutes() << 5) | (time.getSeconds() >> 1),
	};
}

/** `NAME.EXT` into the eleven bytes of an entry; refuses what is not an upper-case 8.3 name. */
export function entryName(name: string): Uint8Array {
	const m = /^([^.\s"*+,/:;<=>?[\\\]|]{1,8})(?:\.([^.\s"*+,/:;<=>?[\\\]|]{1,3}))?$/.exec(name);
	if (!m || name !== name.toUpperCase() || /[^\x21-\x7e]/.test(name)) throw new FatError(`${name} is not an 8.3 name`);
	const out = new Uint8Array(11).fill(0x20);
	for (let i = 0; i < m[1]!.length; i++) out[i] = m[1]!.charCodeAt(i);
	for (let i = 0; i < (m[2] ?? "").length; i++) out[8 + i] = m[2]!.charCodeAt(i);
	return out;
}

/**
 * A new image: the template with its volume made again to hold exactly these files in its root (a folder cannot be added). The
 * template's partition table, boot sector, geometry and label are kept; both FATs, the root and the data area are rewritten, so
 * what the template (or an earlier build) held is gone. Throws, building nothing, if a name is not 8.3, repeats, or the files do
 * not fit in the root or the clusters.
 */
export function buildVolume(template: Uint8Array, files: readonly NewFile[], partition = 0): Uint8Array {
	const v = openVolume(template, partition);
	const seen = new Set<string>();
	const names = files.map((f) => {
		if (seen.has(f.name)) throw new FatError(`${f.name} is listed twice`);
		seen.add(f.name);
		return entryName(f.name);
	});
	const perCluster = v.sectorsPerCluster * SECTOR;
	const needed = files.reduce((n, f) => n + Math.ceil(f.bytes.length / perCluster), 0);
	const hasLabel = v.label !== "";
	if (files.length + (hasLabel ? 1 : 0) > v.rootEntries) throw new FatError(`The root holds ${v.rootEntries - (hasLabel ? 1 : 0)} files; ${files.length} were given`);
	if (needed > v.clusters) throw new FatError(`The files need ${needed} clusters; the volume has ${v.clusters}`);
	for (const f of files) if (f.bytes.length > 0xffffffff) throw new FatError(`${f.name} is too large`);

	const out = template.slice();
	const nv: Volume = { ...v, image: out };
	// Clear everything the volume owns after its boot sector: FATs, root and data.
	const end = v.base + v.totalSectors * SECTOR;
	out.fill(0, v.fatStart, end);
	let label: Uint8Array | null = null;
	for (const raw of rawEntries(v, 0)) {
		if ((raw.attributes & 0x0f) === 0x08) {
			label = template.slice(raw.at, raw.at + 32);
			break;
		}
	}
	const setFat = (cluster: number, value: number): void => {
		for (let copy = 0; copy < v.fats; copy++) {
			const start = v.fatStart + copy * v.sectorsPerFat * SECTOR;
			if (v.fatBits === 16) {
				out[start + cluster * 2] = value & 0xff;
				out[start + cluster * 2 + 1] = (value >> 8) & 0xff;
			} else {
				const at = start + Math.floor((cluster * 3) / 2);
				if (cluster & 1) {
					out[at] = ((out[at] ?? 0) & 0x0f) | ((value & 0x0f) << 4);
					out[at + 1] = (value >> 4) & 0xff;
				} else {
					out[at] = value & 0xff;
					out[at + 1] = ((out[at + 1] ?? 0) & 0xf0) | ((value >> 8) & 0x0f);
				}
			}
		}
	};
	const eoc = v.fatBits === 16 ? 0xffff : 0xfff;
	setFat(0, v.fatBits === 16 ? 0xff00 | v.media : 0xf00 | v.media);
	setFat(1, eoc);

	let slot = v.rootStart;
	if (label) {
		out.set(label, slot);
		slot += 32;
	}
	let next = 2;
	for (const [i, f] of files.entries()) {
		const clustersNeeded = Math.ceil(f.bytes.length / perCluster);
		const first = clustersNeeded > 0 ? next : 0;
		for (let k = 0; k < clustersNeeded; k++) {
			const cluster = next + k;
			setFat(cluster, k === clustersNeeded - 1 ? eoc : cluster + 1);
			const from = k * perCluster;
			const piece = f.bytes.subarray(from, Math.min(from + perCluster, f.bytes.length));
			out.set(piece, clusterOffset(nv, cluster));
		}
		next += clustersNeeded;
		out.set(names[i]!, slot);
		out[slot + 11] = 0x20;
		const stamp = dosStamp(f.time);
		out[slot + 22] = stamp.time & 0xff;
		out[slot + 23] = stamp.time >> 8;
		for (const field of [24, 18, 16]) {
			out[slot + field] = stamp.date & 0xff;
			out[slot + field + 1] = stamp.date >> 8;
		}
		out[slot + 26] = first & 0xff;
		out[slot + 27] = (first >> 8) & 0xff;
		const size = f.bytes.length;
		out[slot + 28] = size & 0xff;
		out[slot + 29] = (size >>> 8) & 0xff;
		out[slot + 30] = (size >>> 16) & 0xff;
		out[slot + 31] = (size >>> 24) & 0xff;
		slot += 32;
	}
	return out;
}
