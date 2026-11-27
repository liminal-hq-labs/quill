// The DOS Try Quill page's bridge to the guest: attach it to js-dos's command interface when the emulator is ready.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// C: (c.img) belongs to the guest, D: (d.img) to the page; each has one writer, so no protocol is needed. This package writes D: and
// reads C:. See docs/plans/dos-page-bridge.md and SITE-60 in docs/decisions.md.

import { dateFile, readC, writeD, type FsInterface } from "./disks.ts";
import type { NewFile, Volume } from "./fat.ts";

export { buildVolume, listDir, openVolume, parseMbr, readEntry, readFile, statPath, FatError } from "./fat.ts";
export type { DirEntry, NewFile, Partition, Volume } from "./fat.ts";
export { encodeWin1252, namesIndex, shortName, shortNames } from "./names.ts";
export type { ShortName } from "./names.ts";
export { dateBatch, dateFile, DATE_FILE } from "./disks.ts";
export type { FsInterface, WriteReport } from "./disks.ts";

export type BridgeStatus = "starting" | "on" | "failed" | "detached";

export interface BridgeState {
	status: BridgeStatus;
	/** The date written onto D:, as the batch file holds it (`mm-dd-yyyy`), once it has been. */
	date: string | null;
	/** Milliseconds the last write of D: took. */
	writeMs: number | null;
	/** Why it failed, when it did. */
	error: string | null;
}

export interface BridgeOptions {
	/** The day to put in SETDATE.BAT; the visitor's own, now, by default. */
	now?: () => Date;
	/** Called whenever the state changes. */
	onChange?: (state: BridgeState) => void;
}

export interface DosBridge {
	/** Resolves once D: holds the date file (or the write failed: see `state()`). It never rejects. */
	ready: Promise<BridgeState>;
	state(): BridgeState;
	/** Rebuilds D: from the files it is given, with the date file added. */
	write(files: readonly NewFile[]): Promise<BridgeState>;
	/** Reads C: from the image as it is now. */
	readC(): Promise<Volume>;
	/** Lets go: later writes are refused and nothing more is read. Safe twice. */
	detach(): void;
}

/** One line for the page's diagnostics: what the bridge is doing, and what it did. */
export function describe(state: BridgeState): string {
	switch (state.status) {
		case "starting":
			return "bridge: starting";
		case "on":
			return `bridge: on, D: written ${state.writeMs ?? 0} ms, date ${state.date ?? "not set"}`;
		case "failed":
			return `bridge: failed (${state.error ?? "unknown"})`;
		case "detached":
			return "bridge: off";
	}
}

export function attach(fs: FsInterface, options: BridgeOptions = {}): DosBridge {
	const now = options.now ?? ((): Date => new Date());
	let state: BridgeState = { status: "starting", date: null, writeMs: null, error: null };
	let detached = false;
	let day = "";
	const set = (next: BridgeState): BridgeState => {
		state = next;
		options.onChange?.(state);
		return state;
	};
	const write = async (files: readonly NewFile[]): Promise<BridgeState> => {
		if (detached) return state;
		const date = now();
		const stamp = dateFile(date);
		try {
			const report = await writeD(fs, [stamp, ...files.filter((f) => f.name !== stamp.name)], () => !detached);
			if (detached) return state;
			day = new TextDecoder().decode(stamp.bytes).trim().replace("@DATE ", "");
			return set({ status: "on", date: day, writeMs: report.ms, error: null });
		} catch (error) {
			if (detached) return state;
			return set({ status: "failed", date: day || null, writeMs: null, error: error instanceof Error ? error.message : String(error) });
		}
	};
	return {
		ready: write([]),
		state: () => state,
		write,
		readC: () => readC(fs),
		detach: () => {
			if (detached) return;
			detached = true;
			set({ status: "detached", date: state.date, writeMs: state.writeMs, error: null });
		},
	};
}
