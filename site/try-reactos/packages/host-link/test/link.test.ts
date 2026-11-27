// The link on a fake bus: hello, credits, priorities, resync and errors.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, test } from "bun:test";
import { attach, Link, type SerialEmulator } from "../src/index.ts";
import { CRC_START, crcFinish, crcUpdate, hex8 } from "../src/crc.ts";
import { FrameParser, encodeFileHeader, encodeFileTrailer, encodeMessage, encodePacket, type Channel, type ChannelName } from "../src/frame.ts";
import { Mux, field, writeBody, type Message } from "../src/message.ts";

const text = (s: string) => new TextEncoder().encode(s);

/** The helper's side of the bus: parses what the page wrote, and writes frames back. */
class Guest {
	parser = new FrameParser();
	mux = new Mux();
	seen: Message[] = [];
	packets: { channel: ChannelName; id: number; more: boolean; length: number }[] = [];
	nextId = 0;
	reply: (bytes: Uint8Array) => void = () => {};

	take(bytes: Uint8Array): void {
		for (const e of this.parser.push(bytes)) {
			if (e.kind !== "packet") continue;
			this.packets.push({ channel: e.channel, id: e.id, more: e.more, length: e.payload.length });
			const out = this.mux.push(e.channel, e.id, e.more, e.payload);
			if (out && "message" in out) this.seen.push(out.message);
		}
	}

	send(channel: Channel, op: string, fields: [string, string | number][] = [], data = new Uint8Array(0)): void {
		this.reply(encodeMessage(channel, this.nextId++, writeBody(op, fields, data)));
	}

	hello(window = 16384, extra: [string, string | number][] = []): void {
		this.send("ctl", "hello", [["proto", 1], ["role", "guest"], ["app", "HOSTLINK/0.1.0"], ["caps", "clip,file,time,display"], ["window", window], ["max", 8_388_608], ...extra]);
	}

	ops(channel?: Channel): string[] {
		return this.seen.filter((m) => !channel || m.channel === channel).map((m) => m.op);
	}

	errors(): string[] {
		return this.seen.filter((m) => m.channel === "ctl" && m.op === "error").map((m) => field(m, "code")!);
	}
}

interface Rig {
	link: Link;
	guest: Guest;
	clock: { now: number };
}

function rig(options: { max?: number } = {}): Rig {
	const guest = new Guest();
	const clock = { now: 1000 };
	const link = new Link({ write: (b) => guest.take(b), now: () => clock.now, ...options });
	guest.reply = (bytes) => link.receive(bytes);
	return { link, guest, clock };
}

describe("hello", () => {
	test("the page says hello when it starts and the helper's answer makes it ready", () => {
		const { link, guest } = rig();
		link.start();
		expect(link.state).toBe("waiting");
		expect(guest.ops()).toEqual(["hello"]);
		expect(field(guest.seen[0]!, "role")).toBe("page");
		expect(field(guest.seen[0]!, "proto")).toBe("1");
		guest.hello();
		expect(link.state).toBe("ready");
		expect(link.stats().peer).toMatchObject({ app: "HOSTLINK/0.1.0", proto: 1, window: 16384 });
		// The answer to the page's hello is not itself answered.
		expect(guest.ops()).toEqual(["hello"]);
	});

	test("with no answer, hello is sent again every 5 s and not before", () => {
		const { link, guest, clock } = rig();
		link.start();
		clock.now += 4900;
		link.tick();
		expect(guest.ops()).toEqual(["hello"]);
		clock.now += 200;
		link.tick();
		expect(guest.ops()).toEqual(["hello", "hello"]);
		clock.now += 5000;
		link.tick();
		expect(guest.ops().length).toBe(3);
		guest.hello();
		clock.now += 20_000;
		link.tick();
		expect(guest.ops().length).toBe(3);
	});

	test("a hello the page did not ask for (a helper that started late) is answered once", () => {
		const { link, guest } = rig();
		link.start();
		guest.hello();
		guest.hello();
		expect(guest.ops()).toEqual(["hello", "hello"]);
		expect(link.state).toBe("ready");
	});

	test("a restore sends hello again, forgets the queue and resets credit", () => {
		const { link, guest } = rig();
		link.start();
		guest.hello(4352);
		link.send("file", "chunk", [["id", 1]], new Uint8Array(3000));
		link.send("file", "chunk", [["id", 1]], new Uint8Array(3000));
		expect(link.stats().queued).toBeGreaterThan(0);
		link.start();
		expect(link.stats().queued).toBe(0);
		expect(link.stats().credit.outstanding).toBe(0);
		expect(link.state).toBe("waiting");
		expect(guest.ops("ctl").filter((o) => o === "hello").length).toBe(2);
		expect(link.send("clip", "get", [["seq", 1]])).toBeNull();
	});

	test("a newer protocol gets code=version once and the link stays quiet", () => {
		const { link, guest } = rig();
		link.start();
		guest.send("ctl", "hello", [["proto", 2], ["role", "guest"], ["window", 16384]]);
		expect(link.state).toBe("version");
		guest.send("clip", "offer", [["seq", 1]]);
		guest.send("ctl", "hello", [["proto", 2], ["role", "guest"], ["window", 16384]]);
		expect(guest.errors()).toEqual(["version"]);
		expect(link.send("clip", "get")).toBeNull();
		guest.hello();
		expect(link.state).toBe("ready");
	});
});

describe("credits", () => {
	test("the page never has more than the window in flight and resumes as credit comes", () => {
		const { link, guest } = rig();
		link.start();
		guest.hello(8192);
		const before = guest.packets.length;
		link.send("file", "chunk", [["id", 1]], new Uint8Array(40_000));
		const sentFrames = () => guest.packets.slice(before);
		expect(link.stats().credit.outstanding).toBeLessThanOrEqual(8192);
		expect(link.stats().queued).toBeGreaterThan(0);
		const firstBatch = sentFrames().length;
		expect(firstBatch).toBeGreaterThan(0);
		guest.send("ctl", "credit", [["n", 4096]]);
		expect(sentFrames().length).toBeGreaterThan(firstBatch);
		for (let i = 0; i < 30; i++) {
			expect(link.stats().credit.outstanding).toBeLessThanOrEqual(8192);
			guest.send("ctl", "credit", [["n", 4096]]);
		}
		expect(link.stats().queued).toBe(0);
		expect(guest.ops("file")).toEqual(["chunk"]);
		expect(guest.seen.find((m) => m.op === "chunk")!.data.length).toBe(40_000);
	});

	test("a credit larger than what is outstanding does not raise the window", () => {
		const { link, guest } = rig();
		link.start();
		guest.hello(8192);
		guest.send("ctl", "credit", [["n", 1_000_000]]);
		expect(link.stats().credit).toEqual({ window: 8192, outstanding: 0, available: 8192 });
	});

	test("no credit for 5 s stops the sender with a stalled state, and a credit restores it", () => {
		const { link, guest, clock } = rig();
		const states: string[] = [];
		link.on("state", (s) => states.push(s));
		link.start();
		guest.hello(4352);
		link.send("file", "chunk", [["id", 1]], new Uint8Array(20_000));
		clock.now += 4000;
		link.tick();
		expect(link.state).toBe("ready");
		clock.now += 1100;
		link.tick();
		expect(link.state).toBe("stalled");
		const sent = guest.packets.length;
		link.tick();
		expect(guest.packets.length).toBe(sent);
		guest.send("ctl", "credit", [["n", 4096]]);
		expect(link.state).toBe("ready");
		expect(states).toEqual(["waiting", "ready", "stalled", "ready"]);
	});

	test("send refuses a message over the peer's max and before the helper has spoken", () => {
		const { link, guest } = rig();
		link.start();
		expect(link.send("clip", "get")).toBeNull();
		guest.hello(16384, []);
		expect(link.send("clip", "get", [["seq", 1]])).not.toBeNull();
		expect(link.send("file", "chunk", [], new Uint8Array(9_000_000))).toBeNull();
		expect(link.stats().errors["too-large-out"]).toBe(1);
	});
});

describe("priorities", () => {
	test("a file never delays a clipboard message by more than one packet", () => {
		const { link, guest } = rig();
		link.start();
		guest.hello(8192);
		link.send("file", "chunk", [["id", 1]], new Uint8Array(100_000));
		const queuedBefore = guest.packets.length;
		const clipId = link.send("clip", "get", [["seq", 1], ["fmt", "dib"]])!;
		// The credit is spent, so the clip message waits; the first credit that arrives sends it before any more file data.
		guest.send("ctl", "credit", [["n", 4096]]);
		const after = guest.packets.slice(queuedBefore);
		const at = after.findIndex((p) => p.channel === "clip");
		expect(at).toBe(0);
		expect(after[at]!.id).toBe(clipId);
	});

	test("the channels go in their order, control first and files last", () => {
		const { link, guest } = rig();
		link.start();
		guest.hello(4352);
		// One full file packet leaves too little credit for anything else, so the rest queue up behind it.
		link.send("file", "chunk", [["id", 1]], new Uint8Array(20_000));
		const filler = new Uint8Array(300);
		link.send("display", "fit", [["w", 1]], filler);
		link.send("time", "set", [["utc", 1]], filler);
		link.send("clip", "get", [["seq", 1]], filler);
		const mark = guest.packets.length;
		guest.send("ctl", "credit", [["n", 4096]]);
		guest.send("ctl", "credit", [["n", 4096]]);
		const order = guest.packets.slice(mark).map((p) => p.channel);
		expect(order.slice(0, 4)).toEqual(["clip", "time", "display", "file"]);
	});

	test("pause holds a channel and resume lets it go", () => {
		const { link, guest } = rig();
		link.start();
		guest.hello();
		guest.send("ctl", "pause", [["channel", "clip"]]);
		link.send("clip", "get", [["seq", 1]]);
		link.send("time", "set", [["utc", 1]]);
		expect(guest.ops()).toEqual(["hello", "set"]);
		guest.send("ctl", "resume", [["channel", "clip"]]);
		expect(guest.ops()).toEqual(["hello", "set", "get"]);
	});
});

describe("resync and errors", () => {
	let r: Rig;
	beforeEach(() => {
		r = rig();
		r.link.start();
		r.guest.hello();
	});

	test("noise, split anywhere, then a frame: the frame arrives", () => {
		const got: string[] = [];
		r.link.on("message", (m) => got.push(m.op));
		const wire = new Uint8Array([...text("C:\\>dir\r\nLHQ garbage\r\n"), ...encodeMessage("clip", 5, writeBody("offer", [["seq", 1]])), ...text("more noise")]);
		for (let i = 0; i < wire.length; i++) r.link.receive(wire.subarray(i, i + 1));
		expect(got).toEqual(["offer"]);
	});

	test("a bad CRC is dropped with the rest of its message, counted and answered with code=crc and the id", () => {
		const got: string[] = [];
		r.link.on("message", (m) => got.push(m.op));
		const first = encodePacket("clip", 9, true, text("op=data\nseq=1\n\nab"));
		const bad = encodePacket("clip", 9, true, text("cd"));
		bad[bad.length - 1]! ^= 1;
		const last = encodePacket("clip", 9, false, text("ef"));
		for (const p of [first, bad, last]) r.link.receive(p);
		expect(got).toEqual([]);
		expect(r.link.stats().errors.crc).toBe(1);
		const reply = r.guest.seen.find((m) => m.op === "error")!;
		expect(field(reply, "code")).toBe("crc");
		expect(field(reply, "id")).toBe("9");
		r.link.receive(encodeMessage("clip", 10, writeBody("ok", [["seq", 1]])));
		expect(got).toEqual(["ok"]);
	});

	test("a header that does not parse is skipped and counted, with no reply", () => {
		r.link.receive(text("LHQ-PKT 1 clip x E 3 00000000\r\n"));
		r.link.receive(encodeMessage("time", 1, writeBody("set", [["utc", 1]])));
		expect(r.link.stats().errors.header).toBe(1);
		expect(r.guest.errors()).toEqual([]);
	});

	test("a message over max is refused with code=too-large", () => {
		const small = rig({ max: 65_536 });
		small.link.start();
		small.guest.hello();
		small.guest.send("clip", "data", [["seq", 1]], new Uint8Array(70_000));
		expect(small.guest.errors()).toEqual(["too-large"]);
	});

	test("an unknown channel is answered once for the message, an unknown ctl op too", () => {
		const raw = (name: string, id: number, more: boolean, payload: string) => {
			const body = text(payload);
			const head = text(`LHQ-PKT 1 ${name} ${id} ${more ? "M" : "E"} ${body.length}`);
			const crc = hex8(crcFinish(crcUpdate(crcUpdate(CRC_START, head), body)));
			return new Uint8Array([...head, ...text(` ${crc}\r\n`), ...body]);
		};
		r.link.receive(raw("zzzz", 3, true, "aa"));
		r.link.receive(raw("zzzz", 3, false, "bb"));
		expect(r.guest.errors()).toEqual(["unknown"]);
		r.guest.send("ctl", "frobnicate");
		expect(r.guest.errors()).toEqual(["unknown", "unknown"]);
	});

	test("a body with no op is answered with code=malformed", () => {
		r.link.receive(encodePacket("clip", 4, false, text("seq=1\n\n")));
		expect(r.guest.errors()).toEqual(["malformed"]);
	});

	test("the peer's errors are counted and passed on", () => {
		const seen: string[] = [];
		r.link.on("peer-error", (m) => seen.push(field(m, "code")!));
		r.guest.send("ctl", "error", [["code", "crc"], ["id", 3]]);
		expect(seen).toEqual(["crc"]);
		expect(r.link.stats().peerErrors.crc).toBe(1);
	});

	test("file frames pass through as events", () => {
		const kinds: string[] = [];
		r.link.on("file", (e) => kinds.push(e.kind));
		r.link.receive(new Uint8Array([...encodeFileHeader(3, "a.txt"), ...text("abc"), ...encodeFileTrailer()]));
		expect(kinds).toEqual(["file-begin", "file-data", "file-end"]);
	});

	test("a handler that throws does not break the link", () => {
		const original = console.error;
		console.error = () => {};
		try {
			r.link.on("message", () => {
				throw new Error("boom");
			});
			r.link.receive(encodeMessage("time", 7, writeBody("set", [["utc", 1]])));
			expect(r.link.stats().errors.handler).toBe(1);
			r.link.receive(encodeMessage("time", 8, writeBody("set", [["utc", 1]])));
			expect(r.link.stats().messagesIn).toBe(3);
		} finally {
			console.error = original;
		}
	});
});

describe("statistics", () => {
	test("bytes, packets, messages and credit are counted", () => {
		const { link, guest } = rig();
		link.start();
		guest.hello();
		link.send("clip", "get", [["seq", 1]]);
		const s = link.stats();
		expect(s.messagesOut).toBe(2);
		expect(s.messagesIn).toBe(1);
		expect(s.packetsOut).toBe(2);
		expect(s.bytesOut).toBeGreaterThan(0);
		expect(s.bytesIn).toBeGreaterThan(0);
		expect(s.credit.outstanding).toBeGreaterThan(0);
	});
});

describe("attach", () => {
	function bus() {
		const listeners = new Set<(byte: number) => void>();
		const sent: Uint8Array[] = [];
		const emulator: SerialEmulator = {
			add_listener: (_e, fn) => void listeners.add(fn),
			remove_listener: (_e, fn) => void listeners.delete(fn),
			serial_send_bytes: (n, data) => {
				expect(n).toBe(0);
				sent.push(data);
			},
		};
		return { emulator, listeners, sent, emit: (bytes: Uint8Array) => bytes.forEach((b) => listeners.forEach((fn) => fn(b))) };
	}

	test("sends hello on attach, takes bytes the listener sees, and detaches cleanly", async () => {
		const b = bus();
		const host = attach(b.emulator, { tickMs: 0 });
		expect(b.sent.length).toBe(1);
		expect(host.stats().state).toBe("waiting");
		b.emit(encodeMessage("ctl", 0, writeBody("hello", [["proto", 1], ["role", "guest"], ["app", "HOSTLINK/0.1.0"], ["window", 16384]])));
		await Promise.resolve();
		expect(host.stats().state).toBe("ready");
		const id = host.send("clip", "get", [["seq", 1]]);
		expect(id).not.toBeNull();
		expect(b.sent.length).toBe(2);
		host.detach();
		expect(b.listeners.size).toBe(0);
		expect(host.stats().state).toBe("closed");
	});

	test("restart sends hello again", () => {
		const b = bus();
		const host = attach(b.emulator, { tickMs: 0 });
		host.restart();
		expect(b.sent.length).toBe(2);
		host.detach();
	});
});

describe("a held hello (SITE-91)", () => {
	test("a held start sends nothing, not even on the retry, until the guest has spoken a frame", () => {
		const { link, guest, clock } = rig();
		link.start({ hold: true });
		clock.now += 30_000;
		link.tick();
		expect(guest.ops()).toEqual([]);
		expect(link.heard).toBe(false);
		// Console noise is not a frame and does not end the hold.
		link.receive(text("login: \r\n"));
		expect(guest.ops()).toEqual([]);
		guest.hello(4352);
		expect(link.heard).toBe(true);
		expect(link.state).toBe("ready");
		// The helper's hello is the first thing the page answers; the page speaks once.
		expect(guest.ops()).toEqual(["hello"]);
	});

	test("a held start ends on the first frame of any kind, and the page's hello follows", () => {
		const { link, guest } = rig();
		link.start({ hold: true });
		guest.send("clip", "offer", [["seq", 1], ["text", 3]]);
		expect(guest.ops()).toEqual(["hello"]);
		expect(link.state).toBe("waiting");
	});

	test("cancel drops a queued message and the packets of one half written", () => {
		const { link, guest } = rig();
		link.start();
		guest.hello(4352);
		const a = link.send("clip", "part", [["seq", 1], ["fmt", "text"]], new Uint8Array(20_000))!;
		const b = link.send("clip", "set", [["seq", 2]])!;
		expect(link.stats().queued).toBeGreaterThan(3);
		link.cancel([a, b]);
		expect(link.stats().queued).toBe(0);
		// No credit has come: the queue is empty, and a later message goes on its own.
		guest.send("ctl", "credit", [["n", 5000]]);
		link.send("clip", "get", [["seq", 3], ["fmt", "text"]]);
		expect(guest.ops("clip").at(-1)).toBe("get");
	});
});
