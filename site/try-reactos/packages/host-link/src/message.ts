// Message bodies (`key=value` lines, an empty line, then binary data) and reassembling messages from packets.
// The same rules and limits as crates/quill-hostlink.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ChannelName } from "./frame.ts";

export const KEY_MAX = 32;
export const VALUE_MAX = 256;
export const FIELDS_MAX = 16;
/** A receiver's largest message when its `hello` names none: 8 MiB. */
export const DEFAULT_MAX_MESSAGE = 8_388_608;
/** The smallest `max` a `hello` may name. */
export const MIN_MAX_MESSAGE = 65_536;

export type ErrorCode = "crc" | "too-large" | "unknown" | "version" | "malformed";

/** A parsed message: its `op`, every field in order (`op` first) and the bytes after the empty line. */
export interface Message {
	channel: ChannelName;
	id: number;
	op: string;
	fields: [string, string][];
	data: Uint8Array;
}

export class BodyError extends Error {}

const LF = 0x0a;
const EQUALS = 0x3d;

function validKey(key: Uint8Array): boolean {
	return key.length > 0 && key.length <= KEY_MAX && key.every((b) => (b >= 0x61 && b <= 0x7a) || (b >= 0x30 && b <= 0x39) || b === 0x5f || b === 0x2d);
}

/** Checks a body and splits it; returns null when it is not one. */
export function parseBody(bytes: Uint8Array): { fields: [string, string][]; data: Uint8Array } | null {
	const decoder = new TextDecoder();
	const fields: [string, string][] = [];
	let at = 0;
	for (;;) {
		const end = bytes.indexOf(LF, at);
		if (end < 0) return null;
		if (end === at) return { fields, data: bytes.slice(end + 1) };
		const line = bytes.subarray(at, end);
		const eq = line.indexOf(EQUALS);
		if (eq < 0) return null;
		const key = line.subarray(0, eq);
		const value = line.subarray(eq + 1);
		if (!validKey(key) || value.length > VALUE_MAX) return null;
		const keyText = decoder.decode(key);
		if (fields.length === 0 && (keyText !== "op" || value.length === 0)) return null;
		fields.push([keyText, decoder.decode(value)]);
		if (fields.length > FIELDS_MAX) return null;
		at = end + 1;
	}
}

/** The first field named `key`. */
export function field(message: Pick<Message, "fields">, key: string): string | undefined {
	return message.fields.find(([k]) => k === key)?.[1];
}

/** A decimal field as a number (0 to 2^32 - 1), or undefined. */
export function fieldNumber(message: Pick<Message, "fields">, key: string): number | undefined {
	const value = field(message, key);
	if (value === undefined || !/^[0-9]{1,10}$/.test(value)) return undefined;
	const n = Number(value);
	return n <= 0xffffffff ? n : undefined;
}

/** Writes a body: `op=`, the fields, an empty line, then `data`. */
export function writeBody(op: string, fields: Iterable<readonly [string, string | number]> = [], data: Uint8Array = new Uint8Array(0)): Uint8Array {
	const encoder = new TextEncoder();
	const lines: Uint8Array[] = [];
	let count = 0;
	for (const [key, raw] of [["op", op] as const, ...fields]) {
		const value = encoder.encode(String(raw));
		const keyBytes = encoder.encode(key);
		if (!validKey(keyBytes) || value.length > VALUE_MAX || value.includes(LF) || (count === 0 && value.length === 0)) throw new BodyError(`bad field ${key}`);
		if (++count > FIELDS_MAX) throw new BodyError("too many fields");
		lines.push(encoder.encode(`${key}=`), value, Uint8Array.of(LF));
	}
	lines.push(Uint8Array.of(LF), data);
	const out = new Uint8Array(lines.reduce((n, l) => n + l.length, 0));
	let at = 0;
	for (const l of lines) {
		out.set(l, at);
		at += l.length;
	}
	return out;
}

/** One channel's reassembly. A packet with a new id abandons the open message without a reply; a message past `max` is refused. */
export class Assembler {
	#chunks: Uint8Array[] = [];
	#length = 0;
	#open: number | null = null;
	#poisoned: number | null = null;
	dropped = 0;

	constructor(
		readonly channel: ChannelName,
		public max: number = DEFAULT_MAX_MESSAGE,
	) {}

	reset(): void {
		this.#chunks = [];
		this.#length = 0;
		this.#open = null;
		this.#poisoned = null;
	}

	/** A packet of message `id` failed its CRC: that message, and the rest of that id, are dropped. */
	poison(id: number): void {
		if (this.#open === id) {
			this.#open = null;
			this.#chunks = [];
			this.#length = 0;
		}
		this.#poisoned = id;
	}

	/** A packet whose CRC held. Returns the message when it completes, an error code when it is dropped, or null. */
	push(id: number, more: boolean, payload: Uint8Array): Message | ErrorCode | null {
		if (this.#poisoned === id) {
			if (!more) this.#poisoned = null;
			return null;
		}
		this.#poisoned = null;
		if (this.#open !== id) {
			if (this.#open !== null) this.dropped++;
			this.#open = id;
			this.#chunks = [];
			this.#length = 0;
		}
		if (this.#length + payload.length > this.max) {
			this.#open = null;
			this.#chunks = [];
			this.#length = 0;
			if (more) this.#poisoned = id;
			return "too-large";
		}
		this.#chunks.push(payload);
		this.#length += payload.length;
		if (more) return null;
		const body = new Uint8Array(this.#length);
		let at = 0;
		for (const c of this.#chunks) {
			body.set(c, at);
			at += c.length;
		}
		this.#open = null;
		this.#chunks = [];
		this.#length = 0;
		const parsed = parseBody(body);
		if (!parsed) return "malformed";
		return { channel: this.channel, id, op: parsed.fields[0]![1], fields: parsed.fields, data: parsed.data };
	}
}

export type MuxOutput = { message: Message } | { error: ErrorCode; channel: ChannelName; id: number } | null;

/** The five channels' assemblers, plus the once-per-message `unknown` reply for a channel the codec does not know. */
export class Mux {
	#lanes = new Map<string, Assembler>();
	#unknownId: number | null = null;

	constructor(public max: number = DEFAULT_MAX_MESSAGE) {}

	#lane(channel: ChannelName): Assembler {
		let lane = this.#lanes.get(channel);
		if (!lane) {
			lane = new Assembler(channel, this.max);
			this.#lanes.set(channel, lane);
		}
		lane.max = this.max;
		return lane;
	}

	reset(): void {
		for (const lane of this.#lanes.values()) lane.reset();
		this.#unknownId = null;
	}

	get dropped(): number {
		let n = 0;
		for (const lane of this.#lanes.values()) n += lane.dropped;
		return n;
	}

	push(channel: ChannelName, id: number, more: boolean, payload: Uint8Array): MuxOutput {
		if (channel === "unknown") {
			if (this.#unknownId === id) {
				if (!more) this.#unknownId = null;
				return null;
			}
			this.#unknownId = more ? id : null;
			return { error: "unknown", channel, id };
		}
		const result = this.#lane(channel).push(id, more, payload);
		if (result === null) return null;
		if (typeof result === "string") return { error: result, channel, id };
		return { message: result };
	}

	/** A packet failed its CRC: drops its message and returns the `crc` error to reply with. */
	crcFailed(channel: ChannelName, id: number): MuxOutput {
		if (channel !== "unknown") this.#lane(channel).poison(id);
		return { error: "crc", channel, id };
	}
}
