// Every shared vector in tests/host-link/vectors, fed whole, a byte at a time, split at every point and split at random.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { FrameParser, type FrameEvent } from "../src/frame.ts";
import { DEFAULT_MAX_MESSAGE, Mux, type MuxOutput } from "../src/message.ts";

const DIR = join(import.meta.dir, "../../../../../tests/host-link/vectors");

interface Vector {
	name: string;
	bytes: Uint8Array;
	max: number;
	events: unknown[];
	messages: unknown[];
}

function hex(bytes: Uint8Array): string {
	return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

export function loadVectors(): Vector[] {
	return readdirSync(DIR)
		.filter((f) => f.endsWith(".json"))
		.sort()
		.map((f) => {
			const name = f.slice(0, -5);
			const doc = JSON.parse(readFileSync(join(DIR, f), "utf8"));
			return { name, bytes: new Uint8Array(readFileSync(join(DIR, name + ".bin"))), max: doc.max ?? DEFAULT_MAX_MESSAGE, events: doc.events, messages: doc.messages };
		});
}

/** What the parser and the message layer report for `bytes` fed in `chunks` (cycled), in the vectors' JSON shape. */
export function run(bytes: Uint8Array, chunks: number[], max: number): { events: unknown[]; messages: unknown[] } {
	const parser = new FrameParser();
	const mux = new Mux(max);
	const events: any[] = [];
	const messages: unknown[] = [];
	const take = (out: MuxOutput) => {
		if (!out) return;
		if ("message" in out) {
			const m = out.message;
			messages.push({ kind: "message", channel: m.channel, id: m.id, op: m.op, fields: m.fields, data_hex: hex(m.data) });
		} else messages.push({ kind: "error", code: out.error, channel: out.channel, id: out.id });
	};
	const handle = (e: FrameEvent) => {
		switch (e.kind) {
			case "packet":
				events.push({ kind: "packet", channel: e.channel, id: e.id, more: e.more, payload_hex: hex(e.payload) });
				take(mux.push(e.channel, e.id, e.more, e.payload));
				break;
			case "file-begin":
				events.push({ kind: "file-begin", size: e.size, name: e.name });
				break;
			case "file-data": {
				const last = events[events.length - 1];
				if (last?.kind === "file-data") last.data_hex += hex(e.data);
				else events.push({ kind: "file-data", data_hex: hex(e.data) });
				break;
			}
			case "file-end":
				events.push({ kind: "file-end", sha256: e.sha256 });
				break;
			case "error":
				if (e.error === "crc") {
					events.push({ kind: "error", error: "crc", offset: e.offset, channel: e.channel, id: e.id });
					take(mux.crcFailed(e.channel, e.id));
				} else events.push({ kind: "error", error: e.error, offset: e.offset });
				break;
		}
	};
	let at = 0;
	let n = 0;
	while (at < bytes.length) {
		const size = chunks.length ? Math.max(1, chunks[n++ % chunks.length]!) : bytes.length;
		for (const e of parser.push(bytes.subarray(at, at + size))) handle(e);
		at += size;
	}
	return { events, messages };
}

function same(v: Vector, how: string, chunks: number[]) {
	const got = run(v.bytes, chunks, v.max);
	expect({ how, events: got.events }, `${v.name}: events`).toEqual({ how, events: v.events });
	expect({ how, messages: got.messages }, `${v.name}: messages`).toEqual({ how, messages: v.messages });
}

describe("shared vectors", () => {
	const vectors = loadVectors();

	test("there are at least thirty", () => {
		expect(vectors.length).toBeGreaterThanOrEqual(30);
	});

	test("whole", () => {
		for (const v of vectors) same(v, "whole", []);
	});

	test("a byte at a time", () => {
		for (const v of vectors) same(v, "byte at a time", [1]);
	});

	test("split in two at every point", () => {
		for (const v of vectors) {
			const stride = Math.floor(v.bytes.length / 600) + 1;
			for (let cut = 1; cut < v.bytes.length; cut += stride) same(v, `split at ${cut}`, [cut, v.bytes.length]);
		}
	});

	test("random splits", () => {
		let seed = 0x2545f491;
		const next = () => {
			seed ^= seed << 13;
			seed ^= seed >>> 17;
			seed ^= seed << 5;
			return seed >>> 0;
		};
		for (const v of vectors) {
			for (let round = 0; round < 20; round++) same(v, `random round ${round}`, Array.from({ length: 64 }, () => (next() % 97) + 1));
		}
	});
});
