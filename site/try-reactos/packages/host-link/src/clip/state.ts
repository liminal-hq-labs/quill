// The clipboard's state machine: what the guest offers, when the page may write it to the browser clipboard, and what the page sends the
// other way. No DOM: the browser's clipboard, the PNG converter and the clock come in through `ClipboardPort`, so the privacy rules are tested
// without a browser. The rules are in docs/plans/host-integration.md ("The clipboard flows", "Privacy and security").
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { LinkStats } from "../link.ts";
import type { Message } from "../message.ts";
import { field, fieldNumber } from "../message.ts";
import { dibBytes, MAX_SIDE } from "./dib.ts";
import { rtfToHtml, htmlToRtf } from "./rtf.ts";

/** Per-format limits, enforced here before data moves (the helper enforces the same ones). */
export const LIMITS = { text: 1_048_576, rtf: 4_194_304, dib: 4_194_304 } as const;
/** A write needs a user gesture no older than this (Firefox refuses after 5 s; Chromium's activation lasts as long). */
export const GESTURE_MS = 5000;
/** At 0.33 MB/s a copy out of the guest of this many bytes arrives inside the gesture window. */
export const AUTO_BYTES = 1_600_000;
/** The helper sends text up to 256 KiB and RTF up to 1 MiB with the offer; anything more waits for a `get`. */
const HELPER_AUTO = { text: 256 * 1024, rtf: 1024 * 1024, dib: 0 } as const;
/** How long a paste waits for the helper's `ok` once the data has stopped moving. */
export const PUSH_STALL_MS = 10_000;

export type Fmt = "text" | "rtf" | "dib";
const FORMATS: readonly Fmt[] = ["text", "rtf", "dib"];

/** The browser's end: only the glue implements it, tests fake it. */
export interface ClipboardPort {
	/** Writes one clipboard item whose formats may still be arriving: must be called inside the gesture. The promises resolve to what each format holds. */
	write(item: { text?: Promise<string>; html?: Promise<string>; png?: Promise<Blob> }): Promise<void>;
	/** A packed DIB as a PNG. */
	dibToPng(dib: Uint8Array): Promise<Blob>;
	/** A PNG (or any picture the browser decodes) as a 24-bit packed DIB; throws a `RangeError` with a message when it is over the pixel limit. */
	imageToDib(image: Blob): Promise<Uint8Array>;
}

/** The part of `HostLink` the clipboard uses. */
export interface ClipLink {
	on(event: "message" | "state", handler: (value: any) => void): () => void;
	send(channel: "clip" | "ctl", op: string, fields?: Iterable<readonly [string, string | number]>, data?: Uint8Array): number | null;
	cancel(ids: number[]): void;
	stats(): LinkStats;
}

/** What a paste event or a clipboard read gives the page. */
export interface HostContent {
	text?: string;
	html?: string;
	/** `text/rtf` as the browser's paste event offers it. */
	rtf?: string;
	image?: Blob;
}

export type PushResult = { ok: true; same: boolean; ms: number; bytes: number } | { ok: false; reason: "off" | "not-ready" | "too-large" | "refused" | "timeout" | "cancelled" | "empty" | "bad-image" | "busy"; detail?: string; size?: number; limit?: number };

export type Busy = "idle" | "copying" | "sending";

export interface ClipStatus {
	enabled: boolean;
	active: boolean;
	/** `off`: no link. `no-helper`: the guest never answered a restored state. `not-ready`: the helper has not spoken, or has stalled. `connected`. */
	link: "off" | "no-helper" | "not-ready" | "connected";
	busy: Busy;
	/** 0 to 1 while busy. */
	progress: number;
	/** What the visitor can copy from the guest by the button: the sizes of each format, never the content. */
	held: { seq: number; sizes: Partial<Record<Fmt, number>>; written: boolean } | null;
	note: string;
	counters: { offers: number; copiedOut: number; pastedIn: number; refused: number; skipped: number };
}

interface Deferred<T> {
	promise: Promise<T>;
	resolve(value: T): void;
	reject(error: Error): void;
	settled: boolean;
}

function defer<T>(): Deferred<T> {
	let resolve!: (value: T) => void;
	let reject!: (error: Error) => void;
	const promise = new Promise<T>((a, b) => {
		resolve = a;
		reject = b;
	});
	promise.catch(() => {});
	const d: Deferred<T> = {
		promise,
		settled: false,
		resolve(value) {
			d.settled = true;
			resolve(value);
		},
		reject(error) {
			d.settled = true;
			reject(error);
		},
	};
	return d;
}

interface Held {
	seq: number;
	source: string;
	sizes: Partial<Record<Fmt, number>>;
	have: Partial<Record<Fmt, Uint8Array>>;
	waiting: Partial<Record<Fmt, Deferred<Uint8Array>>>;
	asked: Set<Fmt>;
	written: boolean;
}

/** A 53-bit string hash (cyrb53): enough to tell two clipboards apart, never stored. */
function hashOf(bytes: Uint8Array): string {
	let h1 = 0xdeadbeef ^ bytes.length;
	let h2 = 0x41c6ce57 ^ bytes.length;
	for (const b of bytes) {
		h1 = Math.imul(h1 ^ b, 2654435761);
		h2 = Math.imul(h2 ^ b, 1597334677);
	}
	h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^ Math.imul(h2 ^ (h2 >>> 13), 3266489909);
	h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^ Math.imul(h1 ^ (h1 >>> 13), 3266489909);
	return (4294967296 * (2097151 & h2) + (h1 >>> 0)).toString(36) + ":" + bytes.length;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8");

/** Text for the guest: UTF-8 with CRLF line ends (a lone LF becomes CRLF), which is what Notepad and the Edit control want. */
export function textForGuest(text: string): Uint8Array {
	return encoder.encode(text.replace(/\r\n|\r|\n/g, "\r\n"));
}

export interface SyncOptions {
	now?: () => number;
	/** Called after any change a status line or chip could show. */
	onChange?: () => void;
	/** How long the "copied" and "pasted" notes stay. */
	noteMs?: number;
}

export class ClipboardSync {
	readonly #link: ClipLink;
	readonly #port: ClipboardPort;
	readonly #now: () => number;
	readonly #onChange: () => void;
	readonly #noteMs: number;
	readonly #off: (() => void)[] = [];
	#enabled = true;
	#active = false;
	#gestureAt = -Infinity;
	#gestureUsed = false;
	#held: Held | null = null;
	#guestHas = "";
	#note = "";
	#noteTimer: ReturnType<typeof setTimeout> | null = null;
	#busy: Busy = "idle";
	#progress = 0;
	#nextSeq = 1;
	#push: { seq: number; ids: number[]; settle: (result: PushResult) => void; lastMove: number; sent0: number; total: number } | null = null;
	#pushQueue: Promise<unknown> = Promise.resolve();
	#counters = { offers: 0, copiedOut: 0, pastedIn: 0, refused: 0, skipped: 0 };
	#linkSeen = false;
	#notFoundAfterMs: number;
	#attachedAt: number;
	#restored: boolean;
	#lastState = "";
	#copyStartBytes = 0;
	constructor(link: ClipLink, port: ClipboardPort, options: SyncOptions & { restored?: boolean } = {}) {
		this.#link = link;
		this.#port = port;
		this.#now = options.now ?? (() => Date.now());
		this.#onChange = options.onChange ?? (() => {});
		this.#noteMs = options.noteMs ?? 2000;
		this.#notFoundAfterMs = 12_000;
		this.#attachedAt = this.#now();
		this.#restored = options.restored ?? false;
		this.#off.push(link.on("message", (m: Message) => this.#message(m)));
		this.#off.push(link.on("state", (state: string) => this.#linkState(state)));
	}

	// ---- the visitor's settings and gestures ----

	get enabled(): boolean {
		return this.#enabled;
	}

	/** The visitor's Clipboard on/off. Off pauses the helper's clipboard data and ignores offers. */
	setEnabled(on: boolean): void {
		if (on === this.#enabled) return;
		this.#enabled = on;
		if (!on) {
			this.cancel();
			this.#dropHeld("Sharing is off");
		} else this.#note_("", false);
		this.#tellHelper();
		this.#changed();
	}

	/** The screen has been selected (a click or key on it): sharing starts. Until then nothing is written. */
	setActive(active: boolean): void {
		if (active === this.#active) return;
		this.#active = active;
		this.#changed();
	}

	/** The visitor did something on the screen that can copy: a key, or a pointer release. Opens the 5 s window for one automatic write. */
	gesture(): void {
		if (!this.#enabled || !this.#active) return;
		this.#gestureAt = this.#now();
		this.#gestureUsed = false;
	}

	#fresh(): boolean {
		return this.#now() - this.#gestureAt < GESTURE_MS;
	}

	#tellHelper(): void {
		const state = this.#link.stats().state;
		if (state !== "ready" && state !== "stalled") return;
		this.#link.send("ctl", this.#enabled ? "resume" : "pause", [["channel", "clip"]]);
	}

	#linkState(state: string): void {
		this.#lastState = state;
		if (state === "ready") {
			this.#linkSeen = true;
			// A new hello clears the helper's own pause: say again that sharing is off.
			if (!this.#enabled) this.#tellHelper();
			this.#guestHas = "";
		} else if (state === "waiting") {
			this.cancel();
			this.#dropHeld("");
		}
		this.#changed();
	}

	/** Forgets what is held and stops what is moving (Stop, Reset, the page hidden). Content is never kept past this. */
	clear(): void {
		this.cancel();
		this.#dropHeld("");
		this.#guestHas = "";
		this.#changed();
	}

	/** Removes every listener. */
	dispose(): void {
		this.clear();
		for (const off of this.#off) off();
		this.#off.length = 0;
		if (this.#noteTimer) clearTimeout(this.#noteTimer);
	}

	// ---- status ----

	status(): ClipStatus {
		const stats = this.#link.stats();
		let link: ClipStatus["link"] = "off";
		if (stats.state === "ready" && stats.peer && stats.peer.caps.includes("clip")) link = "connected";
		else if (stats.state === "ready") link = "no-helper";
		else if (stats.state === "stalled" || stats.state === "version") link = "not-ready";
		else if (stats.state === "waiting") link = this.#restored && this.#now() - this.#attachedAt > this.#notFoundAfterMs && !this.#linkSeen ? "no-helper" : "not-ready";
		let progress = this.#progress;
		if (this.#busy === "sending" && this.#push) {
			progress = Math.min(0.99, Math.max(0, (stats.bytesOut - this.#push.sent0) / this.#push.total));
		} else if (this.#busy === "copying" && this.#held) {
			const want = Object.entries(this.#held.waiting).reduce((sum, [fmt, d]) => sum + (d && !d.settled ? (this.#held!.sizes[fmt as Fmt] ?? 0) : 0), 0);
			const got = stats.bytesIn - this.#copyStartBytes;
			progress = want > 0 ? Math.min(0.99, Math.max(0, got / (want * 1.01 + 1))) : 0;
		}
		return {
			enabled: this.#enabled,
			active: this.#active,
			link,
			busy: this.#busy,
			progress,
			held: this.#held ? { seq: this.#held.seq, sizes: { ...this.#held.sizes }, written: this.#held.written } : null,
			note: this.#note,
			counters: { ...this.#counters },
		};
	}

	#note_(text: string, transient: boolean): void {
		this.#note = text;
		if (this.#noteTimer) clearTimeout(this.#noteTimer);
		this.#noteTimer = null;
		if (transient && text) {
			this.#noteTimer = setTimeout(() => {
				this.#note = "";
				this.#changed();
			}, this.#noteMs);
		}
		this.#changed();
	}

	#changed(): void {
		this.#onChange();
	}

	// ---- guest to page ----

	#message(m: Message): void {
		if (m.channel !== "clip") return;
		switch (m.op) {
			case "offer":
				this.#offer(m);
				return;
			case "data":
				this.#data(m);
				return;
			case "ok": {
				const seq = fieldNumber(m, "seq");
				if (this.#push && seq === this.#push.seq) this.#finishPush({ ok: true, same: false, ms: this.#now() - this.#push.lastMove, bytes: 0 });
				return;
			}
			case "refused": {
				const seq = fieldNumber(m, "seq");
				const reason = field(m, "reason") ?? "refused";
				if (this.#push && seq === this.#push.seq) {
					this.#counters.refused++;
					this.#finishPush({ ok: false, reason: "refused", detail: reason });
				}
				if (this.#held && this.#held.seq === seq) {
					for (const d of Object.values(this.#held.waiting)) if (d && !d.settled) d.reject(new Error(reason));
					this.#counters.refused++;
					if (reason !== "changed") this.#note_("The PC would not hand over its clipboard (" + reason + ")", true);
				}
				return;
			}
			default:
			// `get`, `set` and `part` are the page's to send, never to receive: nothing here reads the host clipboard whatever the guest says.
		}
	}

	#offer(m: Message): void {
		const seq = fieldNumber(m, "seq");
		if (seq === undefined) return;
		if (this.#held && this.#held.seq === seq) return;
		this.#counters.offers++;
		const source = field(m, "source") ?? "user";
		this.#guestHas = "";
		this.#dropHeld("");
		if (!this.#enabled || source !== "user") return;
		const sizes: Partial<Record<Fmt, number>> = {};
		for (const fmt of FORMATS) {
			const size = fieldNumber(m, fmt);
			if (size !== undefined && size > 0) sizes[fmt] = size;
		}
		const held: Held = { seq, source, sizes, have: {}, waiting: {}, asked: new Set(), written: false };
		this.#held = held;
		// A format over its limit is not fetched, and the visitor is told why.
		const tooBig = FORMATS.filter((f) => (sizes[f] ?? 0) > LIMITS[f]);
		for (const fmt of tooBig) delete sizes[fmt];
		if (tooBig.length > 0) this.#note_("That copy is too large to share: " + tooBig.map((f) => `${label(f)} ${mb(m, f)} (limit ${(LIMITS[f] / 1048576).toFixed(0)} MB)`).join(", "), false);
		if (Object.keys(sizes).length === 0) {
			this.#changed();
			return;
		}
		if (!this.#active) {
			this.#note_("Copied in ReactOS. Select the screen to share it, or press Copy from ReactOS.", false);
			return;
		}
		if (this.#fresh() && !this.#gestureUsed) {
			this.#gestureUsed = true;
			void this.#copyOut(true);
		} else {
			this.#counters.skipped++;
			this.#note_("The PC clipboard changed. Press Copy from ReactOS to put it on your computer.", false);
		}
	}

	#data(m: Message): void {
		const seq = fieldNumber(m, "seq");
		const fmt = field(m, "fmt") as Fmt | undefined;
		const held = this.#held;
		if (!held || held.seq !== seq || !fmt || !FORMATS.includes(fmt)) return;
		held.have[fmt] = m.data;
		const d = held.waiting[fmt];
		if (d && !d.settled) d.resolve(m.data);
		if (fmt === "text" && this.#guestHas === "") this.#guestHas = hashOf(textForGuest(decoder.decode(m.data)));
		this.#changed();
	}

	/** A format's bytes as a promise: already here, or asked for (when the helper does not send it by itself) and awaited. */
	#need(held: Held, fmt: Fmt): Promise<Uint8Array> {
		const have = held.have[fmt];
		if (have) return Promise.resolve(have);
		let d = held.waiting[fmt];
		if (!d) {
			d = defer<Uint8Array>();
			held.waiting[fmt] = d;
		}
		const size = held.sizes[fmt] ?? 0;
		if (size > HELPER_AUTO[fmt] && !held.asked.has(fmt)) {
			held.asked.add(fmt);
			if (this.#link.send("clip", "get", [["seq", held.seq], ["fmt", fmt]]) === null) d.reject(new Error("not-ready"));
		}
		return d.promise;
	}

	/** Writes the held clipboard to the browser. `auto` is the write after the visitor's own copy (inside the gesture, and limited to what fits the window). */
	async #copyOut(auto: boolean): Promise<void> {
		const held = this.#held;
		if (!held) return;
		let want = FORMATS.filter((f) => held.sizes[f] !== undefined);
		if (auto) {
			// Inside the 5 s window only what arrives in time: text first, then RTF, then the picture.
			let total = 0;
			const keep: Fmt[] = [];
			for (const fmt of ["text", "rtf", "dib"] as Fmt[]) {
				const size = held.sizes[fmt];
				if (size === undefined || held.have[fmt]) {
					if (size !== undefined) keep.push(fmt);
					continue;
				}
				if (total + size <= AUTO_BYTES || (fmt === "text" && keep.length === 0 && size <= LIMITS.text)) {
					total += size;
					keep.push(fmt);
				}
			}
			want = keep;
			if (want.length === 0) {
				this.#counters.skipped++;
				this.#note_("The PC clipboard changed. Press Copy from ReactOS to put it on your computer.", false);
				return;
			}
		}
		this.#busy = "copying";
		this.#copyStartBytes = this.#link.stats().bytesIn;
		this.#note_("", false);
		const item: Parameters<ClipboardPort["write"]>[0] = {};
		if (want.includes("text")) item.text = this.#need(held, "text").then((b) => decoder.decode(b));
		if (want.includes("rtf")) item.html = this.#need(held, "rtf").then((b) => rtfToHtml(b) || "<p></p>");
		if (want.includes("dib")) item.png = this.#need(held, "dib").then((b) => this.#port.dibToPng(b));
		if (!item.text && !item.html && !item.png) {
			this.#busy = "idle";
			return;
		}
		try {
			await this.#port.write(item);
			held.written = true;
			this.#counters.copiedOut++;
			this.#busy = "idle";
			this.#note_("Copied to your computer", true);
		} catch (error) {
			this.#busy = "idle";
			this.#counters.refused++;
			const name = (error as Error)?.name ?? "";
			if (auto || name === "NotAllowedError") this.#note_("The PC clipboard changed. Press Copy from ReactOS to put it on your computer.", false);
			else this.#note_("Could not copy to your computer: " + ((error as Error)?.message ?? "unknown"), false);
		}
	}

	/** The Copy from ReactOS button: a click is a new gesture, so the write is made now whatever the age of the last copy. */
	async copyOut(): Promise<boolean> {
		if (!this.#held || !this.#enabled) return false;
		this.#gestureUsed = true;
		await this.#copyOut(false);
		return this.#held?.written === true;
	}

	#dropHeld(note: string): void {
		const held = this.#held;
		if (held) for (const d of Object.values(held.waiting)) if (d && !d.settled) d.reject(new Error("dropped"));
		this.#held = null;
		if (this.#busy === "copying") this.#busy = "idle";
		if (note) this.#note_(note, false);
	}

	/** True while a copy out of the guest is arriving: the page does not start a paste then (half duplex, WP-HOST-1). */
	get receiving(): boolean {
		return this.#busy === "copying";
	}

	// ---- page to guest ----

	/** Sends the host clipboard to the guest, one packet in flight, and resolves when the helper has set it. Queued behind a push in progress. */
	push(content: HostContent): Promise<PushResult> {
		const run = this.#pushQueue.then(async () => {
			const result = await this.#pushNow(content);
			if (!result.ok && result.reason !== "cancelled") this.#note_(describeFailure(result), false);
			return result;
		});
		this.#pushQueue = run.catch(() => {});
		return run;
	}

	async #pushNow(content: HostContent): Promise<PushResult> {
		if (!this.#enabled) return { ok: false, reason: "off" };
		const stats0 = this.#link.stats();
		if (stats0.state !== "ready" || !stats0.peer || !stats0.peer.caps.includes("clip")) return { ok: false, reason: "not-ready" };
		const parts: Partial<Record<Fmt, Uint8Array>> = {};
		if (content.text) {
			parts.text = textForGuest(content.text);
			if (parts.text.length > LIMITS.text) return { ok: false, reason: "too-large", detail: "text", size: parts.text.length, limit: LIMITS.text };
		}
		const rtf = content.rtf ?? (content.html ? htmlToRtf(content.html) : "");
		if (rtf) {
			parts.rtf = encoder.encode(rtf);
			if (parts.rtf.length > LIMITS.rtf) {
				if (parts.text) delete parts.rtf;
				else return { ok: false, reason: "too-large", detail: "rtf", size: parts.rtf.length, limit: LIMITS.rtf };
			}
		}
		if (content.image) {
			try {
				const dib = await this.#port.imageToDib(content.image);
				if (dib.length > LIMITS.dib) return { ok: false, reason: "too-large", detail: "picture", size: dib.length, limit: LIMITS.dib };
				parts.dib = dib;
			} catch (error) {
				return { ok: false, reason: "bad-image", detail: (error as Error).message };
			}
		}
		if (!parts.text && !parts.rtf && !parts.dib) return { ok: false, reason: "empty" };
		// The guest already has it: no second copy of the same text (the helper's own write is never offered back, so this is the visitor's repeat).
		const key = hashOf((parts.text ?? parts.rtf ?? parts.dib)!);
		if (key === this.#guestHas) {
			this.#counters.skipped++;
			return { ok: true, same: true, ms: 0, bytes: 0 };
		}
		// Half duplex: a paste does not start while a copy out of the guest is arriving.
		for (let waited = 0; this.receiving && waited < 30_000; waited += 50) await new Promise((r) => setTimeout(r, 50));
		const formats = FORMATS.filter((f) => parts[f]);
		const seq = this.#nextSeq++;
		const total = formats.reduce((sum, f) => sum + parts[f]!.length, 0);
		const started = this.#now();
		return new Promise<PushResult>((settle) => {
			const ids: number[] = [];
			this.#push = { seq, ids, settle, lastMove: started, sent0: this.#link.stats().bytesOut, total: Math.ceil(total * 1.02) + 400 };
			this.#busy = "sending";
			this.#progress = 0;
			this.#note_("", false);
			const first = this.#link.send("clip", "set", [["seq", seq], ["formats", formats.join(",")]]);
			if (first === null) {
				this.#finishPush({ ok: false, reason: "not-ready" });
				return;
			}
			ids.push(first);
			for (const fmt of formats) {
				const id = this.#link.send("clip", "part", [["seq", seq], ["fmt", fmt]], parts[fmt]!);
				if (id === null) {
					this.#link.cancel(ids);
					this.#finishPush({ ok: false, reason: "too-large", detail: label(fmt), size: parts[fmt]!.length, limit: LIMITS[fmt] });
					return;
				}
				ids.push(id);
			}
			this.#pending = { key, bytes: total, started };
			this.#watchPush();
		});
	}

	#pending: { key: string; bytes: number; started: number } | null = null;
	#pushTimer: ReturnType<typeof setTimeout> | null = null;

	/** Gives up when the link has moved nothing for `PUSH_STALL_MS`. */
	#watchPush(): void {
		if (this.#pushTimer) clearTimeout(this.#pushTimer);
		const push = this.#push;
		if (!push) return;
		const last = this.#link.stats().bytesOut;
		const lastQueued = this.#link.stats().queued;
		this.#pushTimer = setTimeout(() => {
			if (this.#push !== push) return;
			const now = this.#link.stats();
			if (now.bytesOut === last && now.queued === lastQueued) {
				this.#link.cancel(push.ids);
				this.#finishPush({ ok: false, reason: "timeout" });
			} else this.#watchPush();
			this.#changed();
		}, PUSH_STALL_MS);
	}

	#finishPush(result: PushResult): void {
		const push = this.#push;
		if (!push) return;
		this.#push = null;
		if (this.#pushTimer) clearTimeout(this.#pushTimer);
		this.#pushTimer = null;
		this.#busy = "idle";
		const pending = this.#pending;
		this.#pending = null;
		if (result.ok && pending) {
			result = { ok: true, same: false, ms: this.#now() - pending.started, bytes: pending.bytes };
			this.#guestHas = pending.key;
			this.#counters.pastedIn++;
			this.#note_("Pasted into the PC", true);
		} else if (!result.ok) {
			if (result.reason !== "cancelled") this.#note_(describeFailure(result), false);
			else this.#note_("Cancelled. Nothing was changed in the PC.", true);
		}
		push.settle(result);
		this.#changed();
	}

	/** Cancels a paste in progress: the rest of its packets are not sent (the helper drops the half-set when the next message begins). */
	cancel(): void {
		const push = this.#push;
		if (!push) return;
		this.#link.cancel(push.ids);
		this.#finishPush({ ok: false, reason: "cancelled" });
	}

	get sending(): boolean {
		return this.#push !== null;
	}

	/** True when a state saved now would restore a quiet link: nothing in flight either way. */
	get quiet(): boolean {
		return this.#push === null && this.#busy === "idle" && this.#link.stats().queued === 0;
	}

	get lastLinkState(): string {
		return this.#lastState;
	}
}

function label(fmt: Fmt | string): string {
	return fmt === "dib" ? "picture" : fmt === "rtf" ? "rich text" : fmt === "text" ? "text" : fmt;
}

function mb(m: Message, fmt: Fmt): string {
	return ((fieldNumber(m, fmt) ?? 0) / 1048576).toFixed(1) + " MB";
}

/** One plain sentence for a failed paste. */
export function describeFailure(result: Extract<PushResult, { ok: false }>): string {
	const sizeText = result.size !== undefined && result.limit !== undefined ? ` (${(result.size / 1048576).toFixed(1)} MB; the limit is ${(result.limit / 1048576).toFixed(0)} MB)` : "";
	switch (result.reason) {
		case "off":
			return "Sharing is off";
		case "not-ready":
			return "ReactOS is not ready for a paste yet";
		case "too-large":
			return `That ${result.detail ?? "clipboard"} is too large to paste${sizeText}`;
		case "refused":
			return "The PC did not take the paste" + (result.detail ? ` (${result.detail})` : "");
		case "timeout":
			return "The PC stopped answering during the paste. Nothing was changed there";
		case "empty":
			return "There is nothing on your clipboard to paste";
		case "bad-image":
			return "Could not read that picture" + (result.detail ? `: ${result.detail}` : "");
		case "busy":
			return "A paste is already in progress";
		default:
			return "Could not paste";
	}
}

export { MAX_SIDE, dibBytes };
