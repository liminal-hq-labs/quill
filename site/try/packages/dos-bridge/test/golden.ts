// Shared helpers for the tests: the golden images mtools made (tests/dos-bridge/), the real template that scripts/make-bridge-disks.py
// makes, and the external checkers (fsck.fat and mtools) when they are installed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gunzipSync } from "node:zlib";

export const GOLDEN_DIR = join(import.meta.dir, "../../../../../tests/dos-bridge");
const SCRIPTS = join(import.meta.dir, "../../../../../scripts");

export interface GoldenFile {
	size: number;
	sha256: string;
}
export interface GoldenInfo {
	partitionType: number;
	label: string;
	files: Record<string, GoldenFile>;
	time: string;
}

export const manifest = JSON.parse(readFileSync(join(GOLDEN_DIR, "manifest.json"), "utf8")) as Record<string, GoldenInfo>;

export function golden(name: string): Uint8Array {
	return new Uint8Array(gunzipSync(readFileSync(join(GOLDEN_DIR, name))));
}

export const sha256 = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

export function have(tool: string): boolean {
	return Bun.spawnSync(["sh", "-c", `command -v ${tool}`]).exitCode === 0;
}

/** A scratch folder, removed by `cleanup`. */
export function scratch(): { dir: string; cleanup: () => void } {
	const dir = mkdtempSync(join(tmpdir(), "dos-bridge-"));
	return { dir, cleanup: () => rmSync(dir, { recursive: true, force: true }) };
}

/** The real C: and D: images, made by scripts/make-bridge-disks.py (null where python3 or mtools is missing). */
export function bridgeDisks(): { c: Uint8Array; d: Uint8Array } | null {
	if (!have("mformat") || !have("python3")) return null;
	const s = scratch();
	try {
		const run = Bun.spawnSync(["python3", "-I", join(SCRIPTS, "make-bridge-disks.py"), s.dir]);
		if (run.exitCode !== 0) throw new Error(run.stderr.toString());
		return { c: new Uint8Array(readFileSync(join(s.dir, "c.img"))), d: new Uint8Array(readFileSync(join(s.dir, "d.img"))) };
	} finally {
		s.cleanup();
	}
}

/** Runs fsck.fat -n on the first partition of an image (the partition is cut out first, as fsck.fat reads a bare volume). */
export function fsckClean(image: Uint8Array, startSector = 63): { ok: boolean; output: string } {
	const s = scratch();
	try {
		const path = join(s.dir, "part.img");
		writeFileSync(path, image.subarray(startSector * 512));
		const run = Bun.spawnSync(["fsck.fat", "-n", path]);
		return { ok: run.exitCode === 0, output: run.stdout.toString() + run.stderr.toString() };
	} finally {
		s.cleanup();
	}
}

/** Reads a file out of an image with mcopy, from the partition at sector 63. */
export function mtoolsRead(image: Uint8Array, path: string, startSector = 63): Uint8Array {
	const s = scratch();
	try {
		const img = join(s.dir, "disk.img");
		writeFileSync(img, image);
		const out = join(s.dir, "out.bin");
		const run = Bun.spawnSync(["mcopy", "-n", "-i", `${img}@@${startSector * 512}`, `::${path}`, out], { env: { ...process.env, MTOOLS_SKIP_CHECK: "1" } });
		if (run.exitCode !== 0 || !existsSync(out)) throw new Error(`mcopy ${path}: ${run.stderr.toString()}`);
		return new Uint8Array(readFileSync(out));
	} finally {
		s.cleanup();
	}
}
