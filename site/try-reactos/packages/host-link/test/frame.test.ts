// The encoders, the CRC and the parser's limits, beyond the shared vectors.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import { crc32 } from "../src/crc.ts";
import { EncodeError, FILE_MAX_SIZE, FrameParser, MAX_PAYLOAD, PKT_HEADER_MAX, encodeFileHeader, encodeFileTrailer, encodeMessage, encodePacket, type FrameEvent } from "../src/frame.ts";
import { DEFAULT_MAX_MESSAGE, Assembler, FIELDS_MAX, VALUE_MAX, field, fieldNumber, parseBody, writeBody } from "../src/message.ts";
import { loadVectors } from "./vectors.test.ts";

const text = (s: string) => new TextEncoder().encode(s);

function parse(bytes: Uint8Array, chunk = bytes.length || 1): FrameEvent[] {
	const parser = new FrameParser();
	const out: FrameEvent[] = [];
	for (let at = 0; at < bytes.length; at += chunk) out.push(...parser.push(bytes.subarray(at, at + chunk)));
	return out;
}

describe("crc", () => {
	test("the standard check value", () => {
		expect(crc32(text("123456789"))).toBe(0xcbf43926);
		expect(crc32(new Uint8Array(0))).toBe(0);
	});
});

describe("encoders", () => {
	test("a packet is the worked example in the specification", () => {
		const body = text("op=ok\nseq=1\n\n");
		const frame = encodePacket("clip", 4, false, body);
		expect(new TextDecoder().decode(frame)).toBe("LHQ-PKT 1 clip 4 E 13 49529bd1\r\nop=ok\nseq=1\n\n");
	});

	test("a longest header is within the limit and a longest payload round trips", () => {
		const frame = encodePacket("display", 0xffffffff, true, new Uint8Array(MAX_PAYLOAD).fill(7));
		expect(frame.indexOf(0x0a) + 1).toBeLessThanOrEqual(PKT_HEADER_MAX);
		const [event] = parse(frame);
		expect(event).toMatchObject({ kind: "packet", channel: "display", id: 0xffffffff, more: true });
	});

	test("limits are enforced on the way out", () => {
		expect(() => encodePacket("clip", 1, false, new Uint8Array(MAX_PAYLOAD + 1))).toThrow(EncodeError);
		expect(() => encodePacket("nope" as never, 1, false, new Uint8Array(0))).toThrow(EncodeError);
		expect(() => encodePacket("clip", 2 ** 32, false, new Uint8Array(0))).toThrow(EncodeError);
		expect(() => encodeFileHeader(FILE_MAX_SIZE + 1, "x")).toThrow(EncodeError);
		expect(() => encodeFileHeader(1, "a".repeat(129))).toThrow(EncodeError);
		expect(() => encodeFileHeader(1, "bad\nname")).toThrow(EncodeError);
		expect(() => encodeFileTrailer("ABC")).toThrow(EncodeError);
	});

	test("a message over several packets splits at 4,096 and reassembles", () => {
		const data = Uint8Array.from({ length: 10_000 }, (_, i) => i % 251);
		const wire = encodeMessage("clip", 9, writeBody("data", [["seq", 1]], data));
		const events = parse(wire, 777);
		const packets = events.filter((e) => e.kind === "packet");
		expect(packets.length).toBe(3);
		const lane = new Assembler("clip");
		let got: unknown = null;
		for (const e of packets) if (e.kind === "packet") got = lane.push(e.id, e.more, e.payload) ?? got;
		expect(got).toMatchObject({ op: "data", id: 9 });
		expect((got as { data: Uint8Array }).data).toEqual(data);
	});

	test("file frames round trip", () => {
		const sha = "ab".repeat(32);
		const wire = new Uint8Array([...encodeFileHeader(5, "Café.txt"), ...text("hello"), ...encodeFileTrailer(sha)]);
		const events = parse(wire, 3);
		expect(events[0]).toEqual({ kind: "file-begin", size: 5, name: "Café.txt" });
		expect(events.at(-1)).toEqual({ kind: "file-end", sha256: sha });
	});
});

describe("bodies", () => {
	test("fields, numbers and limits", () => {
		const m = { fields: parseBody(text("op=x\nk=7\n\nrest"))!.fields };
		expect(field(m, "k")).toBe("7");
		expect(fieldNumber(m, "k")).toBe(7);
		expect(fieldNumber({ fields: [["n", "4294967296"]] }, "n")).toBeUndefined();
		expect(fieldNumber({ fields: [["n", "-1"]] }, "n")).toBeUndefined();
		expect(parseBody(text("op=\n\n"))).toBeNull();
		expect(parseBody(text("Op=x\n\n"))).toBeNull();
		expect(parseBody(text("op=x\nk=" + "v".repeat(VALUE_MAX + 1) + "\n\n"))).toBeNull();
		expect(parseBody(text("op=x\nk=" + "v".repeat(VALUE_MAX) + "\n\n"))).not.toBeNull();
		const many = "op=x\n" + Array.from({ length: FIELDS_MAX }, (_, i) => `k${i}=1\n`).join("") + "\n";
		expect(parseBody(text(many))).toBeNull();
		expect(() => writeBody("x", [["Bad", 1]])).toThrow();
	});
});

describe("limits and errors in the parser", () => {
	test("any one changed byte in a packet is never a changed packet", () => {
		const good = encodePacket("clip", 7, false, text("op=ok\nseq=7\n\n"));
		for (let at = 0; at < good.length; at++) {
			for (const delta of [1, 0x20, 0x80, 0xff]) {
				const bad = good.slice();
				bad[at]! ^= delta;
				expect(parse(bad).filter((e) => e.kind === "packet")).toEqual([]);
			}
		}
	});

	test("random input never throws and splits the same way", () => {
		let seed = 0x9e3779b9;
		const next = () => {
			seed ^= seed << 13;
			seed ^= seed >>> 17;
			seed ^= seed << 5;
			return seed >>> 0;
		};
		const corpus = loadVectors();
		const pieces = ["LHQ-PKT 1 ctl 1 E 0 ", "LHQ-FILE 1 ", "\r\n", "\n", "LHQ-END", " 4096 ", "clip", "00000000"].map(text);
		const norm = (events: FrameEvent[]) => {
			const merged: unknown[] = [];
			for (const e of events) {
				const last = merged.at(-1) as { kind?: string; data?: number[] } | undefined;
				if (e.kind === "file-data" && last?.kind === "file-data") last.data!.push(...e.data);
				else merged.push(e.kind === "file-data" ? { kind: "file-data", data: [...e.data] } : e.kind === "packet" ? { ...e, payload: [...e.payload] } : e);
			}
			return merged;
		};
		for (let round = 0; round < 10_000; round++) {
			let input: Uint8Array;
			switch (round % 4) {
				case 0:
					input = Uint8Array.from({ length: next() % 600 }, () => next() & 0xff);
					break;
				case 1: {
					input = corpus[next() % corpus.length]!.bytes.slice();
					for (let k = 0; k <= next() % 6 && input.length; k++) input[next() % input.length] = next() & 0xff;
					break;
				}
				case 2: {
					const parts: number[] = [];
					for (let k = 0; k <= next() % 4; k++) {
						const v = corpus[next() % corpus.length]!.bytes;
						parts.push(...v.subarray(0, next() % (v.length + 1)));
					}
					input = Uint8Array.from(parts);
					break;
				}
				default: {
					const parts: number[] = [];
					for (let k = 0; k < next() % 40; k++) parts.push(...pieces[next() % pieces.length]!);
					input = Uint8Array.from(parts);
				}
			}
			input = input.subarray(0, 20_000);
			expect(norm(parse(input, (next() % 50) + 1)), `round ${round}`).toEqual(norm(parse(input)));
		}
	});

	test("a truncated vector says a prefix of what the whole says", () => {
		for (const v of loadVectors()) {
			const whole = JSON.stringify(parse(v.bytes).map((e) => (e.kind === "file-data" ? null : e)).filter(Boolean), (_k, x) => (x instanceof Uint8Array ? [...x] : x));
			void whole;
			const stride = Math.floor(v.bytes.length / 200) + 1;
			const full = parse(v.bytes).filter((e) => e.kind !== "file-data");
			for (let cut = 0; cut < v.bytes.length; cut += stride) {
				const part = parse(v.bytes.subarray(0, cut)).filter((e) => e.kind !== "file-data");
				expect(JSON.stringify(full.slice(0, part.length), (_k, x) => (x instanceof Uint8Array ? [...x] : x))).toBe(JSON.stringify(part, (_k, x) => (x instanceof Uint8Array ? [...x] : x)));
			}
		}
	});

	test("a message over max is refused before the buffer grows past it", () => {
		const lane = new Assembler("clip", 100);
		expect(lane.push(1, true, new Uint8Array(60))).toBeNull();
		expect(lane.push(1, true, new Uint8Array(60))).toBe("too-large");
		expect(lane.push(1, true, text("zz"))).toBeNull();
		expect(lane.push(1, false, text("zz"))).toBeNull();
		expect(lane.push(2, false, text("op=bye\n\n"))).toMatchObject({ op: "bye" });
		expect(DEFAULT_MAX_MESSAGE).toBe(8_388_608);
	});
});
