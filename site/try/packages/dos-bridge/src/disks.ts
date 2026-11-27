// The only file that calls the player's file system (`ci.fsReadFile` and `ci.fsWriteFile`): reads C:, writes D:, and makes the date file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { buildVolume, openVolume, type NewFile, type Volume } from "./fat.ts";

/** The slice of js-dos's command interface that the bridge uses. */
export interface FsInterface {
	fsReadFile(path: string): Promise<Uint8Array>;
	fsWriteFile(path: string, contents: Uint8Array): Promise<unknown>;
}

/** The image names `scripts/build-try.sh` puts in the bundle, and the date file's name on D:. */
export const C_IMAGE = "c.img";
export const D_IMAGE = "d.img";
export const DATE_FILE = "SETDATE.BAT";

/** `SETDATE.BAT`'s bytes: `@DATE mm-dd-yyyy`, the visitor's local day in FreeDOS's default (US) format, with a CR LF line end. */
export function dateBatch(day: Date): Uint8Array {
	const two = (n: number): string => String(n).padStart(2, "0");
	const text = `@DATE ${two(day.getMonth() + 1)}-${two(day.getDate())}-${String(day.getFullYear()).padStart(4, "0")}\r\n`;
	return Uint8Array.from(text, (c) => c.charCodeAt(0));
}

export function dateFile(day: Date): NewFile {
	return { name: DATE_FILE, bytes: dateBatch(day), time: day };
}

/** What a write of D: did. */
export interface WriteReport {
	/** Milliseconds for reading the template, building the volume and replacing the image. */
	ms: number;
	files: number;
	bytes: number;
}

/**
 * Rebuilds D: whole from the template (D: as it stands: the volume is made again, so what was there goes) and replaces the image.
 * The page is the only writer of D:, and never reads it back for its contents.
 */
export async function writeD(fs: FsInterface, files: readonly NewFile[], live: () => boolean = () => true, now: () => number = () => performance.now()): Promise<WriteReport> {
	const started = now();
	const template = await fs.fsReadFile(D_IMAGE);
	const image = buildVolume(template, files);
	// The emulator may have been stopped while the image was read and built; then there is nothing to write to.
	if (live()) await fs.fsWriteFile(D_IMAGE, image);
	return { ms: Math.round(now() - started), files: files.length, bytes: files.reduce((n, f) => n + f.bytes.length, 0) };
}

/** Reads C: (the guest's disk) as it is in the image now. A file the guest wrote may still be missing its last sector (SYNC flushes it). */
export async function readC(fs: FsInterface): Promise<Volume> {
	return openVolume(await fs.fsReadFile(C_IMAGE));
}
