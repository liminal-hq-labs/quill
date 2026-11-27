// The frames: a push parser for `LHQ-PKT` and `LHQ-FILE` that finds them in noise and resyncs after an error, and the encoders.
// The same rules, limits and event shapes as crates/quill-hostlink, and the same vectors (tests/host-link/vectors).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { CRC_START, crcFinish, crcUpdate, hex8 } from "./crc.ts";

export const MAX_PAYLOAD = 4096;
/** The longest `LHQ-PKT` header line, its CRLF included. */
export const PKT_HEADER_MAX = 64;
/** The longest `LHQ-FILE` header line, its CRLF included. */
export const FILE_HEADER_MAX = 160;
/** The longest `LHQ-END` line, its CRLF included. */
export const TRAILER_MAX = 80;
export const FILE_NAME_MAX = 128;
/** The largest `LHQ-FILE` body, in bytes (64 MiB). */
export const FILE_MAX_SIZE = 67_108_864;
const CHANNEL_NAME_MAX = 16;

/** The five channels, in the order a sender serves them: control first, files last. */
export const PRIORITY = ["ctl", "clip", "time", "display", "file"] as const;
export type Channel = (typeof PRIORITY)[number];
/** A channel name the codec does not know is reported as `unknown`. */
export type ChannelName = Channel | "unknown";

export function channelFromName(name: string): ChannelName {
	return (PRIORITY as readonly string[]).includes(name) ? (name as Channel) : "unknown";
}

export type FrameEvent =
	| { kind: "packet"; channel: ChannelName; id: number; more: boolean; payload: Uint8Array }
	| { kind: "file-begin"; size: number; name: string }
	| { kind: "file-data"; data: Uint8Array }
	| { kind: "file-end"; sha256: string | null }
	| { kind: "error"; error: "header" | "trailer"; offset: number }
	| { kind: "error"; error: "crc"; offset: number; channel: ChannelName; id: number };

const LF = 0x0a;
const CR = 0x0d;
const SPACE = 0x20;
const PKT_PREFIX = bytesOf("LHQ-PKT ");
const FILE_PREFIX = bytesOf("LHQ-FILE");
const LHQ = bytesOf("LHQ-");

function bytesOf(text: string): Uint8Array {
	return Uint8Array.from(text, (c) => c.charCodeAt(0));
}

function startsWith(line: Uint8Array, prefix: Uint8Array, length = line.length): boolean {
	if (length < prefix.length) return false;
	for (let i = 0; i < prefix.length; i++) if (line[i] !== prefix[i]) return false;
	return true;
}

function splitOn(line: Uint8Array, byte: number, limit = Infinity): Uint8Array[] {
	const parts: Uint8Array[] = [];
	let from = 0;
	for (let i = 0; i < line.length && parts.length < limit - 1; i++) {
		if (line[i] === byte) {
			parts.push(line.subarray(from, i));
			from = i + 1;
		}
	}
	parts.push(line.subarray(from));
	return parts;
}

function text(part: Uint8Array): string {
	let out = "";
	for (const b of part) out += String.fromCharCode(b);
	return out;
}

function parseDecimal(part: Uint8Array | undefined): number | null {
	if (!part || part.length === 0 || part.length > 10) return null;
	if (part.length > 1 && part[0] === 0x30) return null;
	let n = 0;
	for (const b of part) {
		if (b < 0x30 || b > 0x39) return null;
		n = n * 10 + (b - 0x30);
	}
	return n <= 0xffffffff ? n : null;
}

function isLowerHex(part: Uint8Array, length: number): boolean {
	if (part.length !== length) return false;
	return part.every((b) => (b >= 0x30 && b <= 0x39) || (b >= 0x61 && b <= 0x66));
}

interface PktHeader {
	channel: ChannelName;
	id: number;
	more: boolean;
	length: number;
	expect: number;
	covered: number;
}

function parsePkt(line: Uint8Array): PktHeader | null {
	const parts = splitOn(line, SPACE);
	if (parts.length !== 7 || text(parts[0]!) !== "LHQ-PKT" || text(parts[1]!) !== "1") return null;
	const name = parts[2]!;
	if (name.length === 0 || name.length > CHANNEL_NAME_MAX || !name.every((b) => b >= 0x61 && b <= 0x7a)) return null;
	const id = parseDecimal(parts[3]);
	const flag = text(parts[4]!);
	const length = parseDecimal(parts[5]);
	const crcText = parts[6]!;
	if (id === null || length === null || length > MAX_PAYLOAD || (flag !== "M" && flag !== "E") || !isLowerHex(crcText, 8)) return null;
	return { channel: channelFromName(text(name)), id, more: flag === "M", length, expect: parseInt(text(crcText), 16) >>> 0, covered: line.length - 9 };
}

function parseFile(line: Uint8Array): { size: number; name: Uint8Array } | null {
	const parts = splitOn(line, SPACE, 4);
	if (parts.length !== 4 || text(parts[0]!) !== "LHQ-FILE" || text(parts[1]!) !== "1") return null;
	const size = parseDecimal(parts[2]);
	const name = parts[3]!;
	if (size === null || size > FILE_MAX_SIZE || name.length === 0 || name.length > FILE_NAME_MAX || name.some((b) => b < 0x20 || b === 0x7f)) return null;
	return { size, name };
}

type State =
	| { s: "scan" }
	| { s: "header" }
	| { s: "skip" }
	| { s: "payload"; channel: ChannelName; id: number; more: boolean; want: number; got: number; crc: number; expect: number }
	| { s: "file"; remaining: number }
	| { s: "trailer"; start: number };

/** The push parser: `push` bytes in any split, take the events they complete. */
export class FrameParser {
	#payload = new Uint8Array(MAX_PAYLOAD);
	#line = new Uint8Array(FILE_HEADER_MAX);
	#lineLen = 0;
	#state: State = { s: "scan" };
	#offset = 0;
	#frameStart = 0;
	#atLineStart = true;

	/** Bytes consumed since the parser was made or reset. */
	get offset(): number {
		return this.#offset;
	}

	/** Forgets any frame in progress (a new `hello`); the offset restarts at zero. */
	reset(): void {
		this.#lineLen = 0;
		this.#state = { s: "scan" };
		this.#offset = 0;
		this.#frameStart = 0;
		this.#atLineStart = true;
	}

	push(bytes: Uint8Array): FrameEvent[] {
		const events: FrameEvent[] = [];
		let i = 0;
		while (i < bytes.length) {
			const state = this.#state;
			if (state.s === "payload") {
				const take = Math.min(state.want - state.got, bytes.length - i);
				const chunk = bytes.subarray(i, i + take);
				this.#payload.set(chunk, state.got);
				state.crc = crcUpdate(state.crc, chunk);
				state.got += take;
				i += take;
				this.#offset += take;
				if (state.got === state.want) this.#finishPacket(state, events);
			} else if (state.s === "file") {
				const take = Math.min(state.remaining, bytes.length - i);
				events.push({ kind: "file-data", data: bytes.slice(i, i + take) });
				i += take;
				this.#offset += take;
				state.remaining -= take;
				if (state.remaining === 0) {
					this.#lineLen = 0;
					this.#state = { s: "trailer", start: this.#offset };
				}
			} else {
				this.#offset++;
				this.#byte(bytes[i]!, events);
				i++;
			}
		}
		return events;
	}

	#finishPacket(p: Extract<State, { s: "payload" }>, events: FrameEvent[]): void {
		this.#state = { s: "scan" };
		this.#atLineStart = true;
		if (crcFinish(p.crc) === p.expect) {
			events.push({ kind: "packet", channel: p.channel, id: p.id, more: p.more, payload: this.#payload.slice(0, p.want) });
		} else {
			events.push({ kind: "error", error: "crc", offset: this.#frameStart, channel: p.channel, id: p.id });
		}
	}

	#failHeader(b: number, events: FrameEvent[]): void {
		events.push({ kind: "error", error: "header", offset: this.#frameStart });
		this.#lineLen = 0;
		if (b === LF) {
			this.#state = { s: "scan" };
			this.#atLineStart = true;
		} else {
			this.#state = { s: "skip" };
		}
	}

	#byte(b: number, events: FrameEvent[]): void {
		const state = this.#state;
		switch (state.s) {
			case "scan":
				if (this.#atLineStart && b === 0x4c) {
					this.#frameStart = this.#offset - 1;
					this.#line[0] = b;
					this.#lineLen = 1;
					this.#state = { s: "header" };
				}
				this.#atLineStart = b === LF;
				return;
			case "skip":
				if (b === LF) {
					this.#state = { s: "scan" };
					this.#atLineStart = true;
				}
				return;
			case "header":
				this.#headerByte(b, events);
				return;
			case "trailer":
				this.#trailerByte(b, state.start, events);
				return;
			default:
				return;
		}
	}

	#headerByte(b: number, events: FrameEvent[]): void {
		const len = this.#lineLen;
		if (len < 4 && b !== LHQ[len]) {
			this.#lineLen = 0;
			this.#state = { s: "scan" };
			this.#atLineStart = b === LF;
			return;
		}
		if (b === LF) {
			this.#headerLine(events);
			return;
		}
		const pkt = len >= 8 && startsWith(this.#line, PKT_PREFIX, 8);
		const limit = pkt ? PKT_HEADER_MAX : FILE_HEADER_MAX;
		if (len + 1 >= limit) {
			this.#failHeader(b, events);
			return;
		}
		this.#line[len] = b;
		this.#lineLen = len + 1;
		if (this.#lineLen === 8 && !startsWith(this.#line, PKT_PREFIX, 8) && !startsWith(this.#line, FILE_PREFIX, 8)) this.#failHeader(b, events);
	}

	#headerLine(events: FrameEvent[]): void {
		const len = this.#lineLen;
		if (len === 0 || this.#line[len - 1] !== CR) {
			this.#failHeader(LF, events);
			return;
		}
		const line = this.#line.slice(0, len - 1);
		if (startsWith(line, PKT_PREFIX)) {
			const h = len + 1 > PKT_HEADER_MAX ? null : parsePkt(line);
			if (!h) {
				this.#failHeader(LF, events);
				return;
			}
			const crc = crcUpdate(CRC_START, line.subarray(0, h.covered));
			this.#lineLen = 0;
			if (h.length === 0) {
				this.#finishPacket({ s: "payload", channel: h.channel, id: h.id, more: h.more, want: 0, got: 0, crc, expect: h.expect }, events);
			} else {
				this.#state = { s: "payload", channel: h.channel, id: h.id, more: h.more, want: h.length, got: 0, crc, expect: h.expect };
			}
		} else if (startsWith(line, bytesOf("LHQ-FILE "))) {
			const f = parseFile(line);
			if (!f) {
				this.#failHeader(LF, events);
				return;
			}
			events.push({ kind: "file-begin", size: f.size, name: new TextDecoder().decode(f.name) });
			this.#lineLen = 0;
			this.#state = f.size === 0 ? { s: "trailer", start: this.#offset } : { s: "file", remaining: f.size };
		} else {
			this.#failHeader(LF, events);
		}
	}

	#trailerByte(b: number, start: number, events: FrameEvent[]): void {
		const len = this.#lineLen;
		if (b !== LF) {
			if (len + 1 >= TRAILER_MAX) {
				events.push({ kind: "error", error: "trailer", offset: start });
				this.#lineLen = 0;
				this.#state = { s: "skip" };
				return;
			}
			this.#line[len] = b;
			this.#lineLen = len + 1;
			return;
		}
		const line = text(this.#line.subarray(0, len));
		const match = /^LHQ-END(?: ([0-9a-f]{64}))?\r$/.exec(line);
		events.push(match ? { kind: "file-end", sha256: match[1] ?? null } : { kind: "error", error: "trailer", offset: start });
		this.#lineLen = 0;
		this.#state = { s: "scan" };
		this.#atLineStart = true;
	}
}

/** Why a frame could not be written. */
export class EncodeError extends Error {}

function ascii(textValue: string): Uint8Array {
	return bytesOf(textValue);
}

function concat(parts: Uint8Array[]): Uint8Array {
	const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
	let at = 0;
	for (const p of parts) {
		out.set(p, at);
		at += p.length;
	}
	return out;
}

/** One packet. The CRC covers the header up to the space before it, then the payload. */
export function encodePacket(channel: Channel, id: number, more: boolean, payload: Uint8Array): Uint8Array {
	if (payload.length > MAX_PAYLOAD) throw new EncodeError("payload over 4,096 bytes");
	if (!(PRIORITY as readonly string[]).includes(channel)) throw new EncodeError("unknown channel");
	if (!Number.isInteger(id) || id < 0 || id > 0xffffffff) throw new EncodeError("id out of range");
	const head = ascii(`LHQ-PKT 1 ${channel} ${id} ${more ? "M" : "E"} ${payload.length}`);
	const crc = crcFinish(crcUpdate(crcUpdate(CRC_START, head), payload));
	return concat([head, ascii(` ${hex8(crc)}\r\n`), payload]);
}

/** A whole message as packets of at most 4,096 bytes, `M` on all but the last; an empty body is one empty packet. */
export function packetsOf(channel: Channel, id: number, body: Uint8Array): Uint8Array[] {
	const packets: Uint8Array[] = [];
	let at = 0;
	do {
		const part = body.subarray(at, at + MAX_PAYLOAD);
		at += part.length;
		packets.push(encodePacket(channel, id, at < body.length, part));
	} while (at < body.length);
	return packets;
}

export function encodeMessage(channel: Channel, id: number, body: Uint8Array): Uint8Array {
	return concat(packetsOf(channel, id, body));
}

/** An `LHQ-FILE 1 <size> <name>` header line. */
export function encodeFileHeader(size: number, name: string): Uint8Array {
	const nameBytes = new TextEncoder().encode(name);
	if (!Number.isInteger(size) || size < 0 || size > FILE_MAX_SIZE) throw new EncodeError("file size out of range");
	if (nameBytes.length === 0 || nameBytes.length > FILE_NAME_MAX || nameBytes.some((b) => b < 0x20 || b === 0x7f)) throw new EncodeError("bad file name");
	return concat([ascii(`LHQ-FILE 1 ${size} `), nameBytes, ascii("\r\n")]);
}

/** The `LHQ-END` line, with a SHA-256 (64 lowercase hex digits) if given. */
export function encodeFileTrailer(sha256?: string): Uint8Array {
	if (sha256 !== undefined && !/^[0-9a-f]{64}$/.test(sha256)) throw new EncodeError("bad SHA-256");
	return ascii(`LHQ-END${sha256 ? " " + sha256 : ""}\r\n`);
}
