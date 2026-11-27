// The clipboard: DIB and RTF converters, and the state machine's privacy and flow rules against a fake link and a fake browser.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, test } from "bun:test";
import { ClipboardSync, GESTURE_MS, LIMITS, dibToPixels, htmlToRtf, pixelsToDib, rtfToHtml, textForGuest, type ClipLink, type ClipboardPort } from "../src/index.ts";
import type { LinkStats } from "../src/index.ts";
import type { Message } from "../src/message.ts";

const enc = (s: string) => new TextEncoder().encode(s);
const dec = (b: Uint8Array) => new TextDecoder().decode(b);

describe("DIB", () => {
	test("24-bit pixels survive a round trip, rows bottom-up and padded", () => {
		const rgba = new Uint8ClampedArray([255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255]);
		const dib = pixelsToDib(3, 2, rgba);
		expect(dib.length).toBe(40 + 12 * 2);
		const back = dibToPixels(dib);
		expect([back.width, back.height]).toEqual([3, 2]);
		expect(Array.from(back.rgba)).toEqual(Array.from(rgba));
	});

	test("a 32-bit picture from the guest is read opaque, whatever its alpha byte says", () => {
		const dib = new Uint8Array(40 + 4);
		const v = new DataView(dib.buffer);
		v.setUint32(0, 40, true);
		v.setInt32(4, 1, true);
		v.setInt32(8, 1, true);
		v.setUint16(12, 1, true);
		v.setUint16(14, 32, true);
		dib.set([0x30, 0x20, 0x10, 0x00], 40);
		expect(Array.from(dibToPixels(dib).rgba)).toEqual([0x10, 0x20, 0x30, 255]);
	});

	test("an 8-bit picture uses its colour table, and a top-down one is not flipped", () => {
		const dib = new Uint8Array(40 + 8 + 4 * 2);
		const v = new DataView(dib.buffer);
		v.setUint32(0, 40, true);
		v.setInt32(4, 1, true);
		v.setInt32(8, -2, true);
		v.setUint16(12, 1, true);
		v.setUint16(14, 8, true);
		v.setUint32(32, 2, true);
		dib.set([0, 0, 255, 0, 255, 0, 0, 0], 40);
		dib.set([0, 0, 0, 0, 1, 0, 0, 0], 48);
		expect(Array.from(dibToPixels(dib).rgba)).toEqual([255, 0, 0, 255, 0, 0, 255, 255]);
	});

	test("a cut-off or odd bitmap is an error, not a crash", () => {
		expect(() => dibToPixels(new Uint8Array(10))).toThrow();
		const dib = pixelsToDib(4, 4, new Uint8ClampedArray(64).fill(255));
		expect(() => dibToPixels(dib.slice(0, dib.length - 5))).toThrow();
	});
});

describe("RTF and HTML", () => {
	test("RTF becomes paragraphs with bold, italic and underline, and accents and Unicode are read", () => {
		const rtf = "{\\rtf1\\ansi{\\fonttbl{\\f0 Arial;}}\\f0\\fs28 plain {\\b bold} {\\i italic}\\par caf\\'e9 \\u8364? {\\ul under}\\par}";
		expect(rtfToHtml(rtf)).toBe("<p>plain <b>bold</b> <i>italic</i></p><p>café € <u>under</u></p>");
	});

	test("RTF text is escaped and a destination that is not text is skipped", () => {
		const rtf = "{\\rtf1{\\*\\generator Evil;}{\\info{\\title <script>}}<b onload=x> & {\\pict deadbeef}}";
		const html = rtfToHtml(rtf);
		expect(html).not.toContain("<script>");
		expect(html).not.toContain("deadbeef");
		expect(html).toContain("&lt;b onload=x&gt; &amp;");
	});

	test("something that is not RTF gives nothing", () => {
		expect(rtfToHtml("hello")).toBe("");
	});

	test("HTML becomes RTF with its formatting kept and its scripts, styles and links dropped", () => {
		const rtf = htmlToRtf('<html><head><style>p{}</style></head><body><p>Hello <b>big</b> <a href="http://evil/">link</a></p><script>alert(1)</script><p>café € <i>it</i></p></body></html>');
		expect(rtf.startsWith("{\\rtf1")).toBe(true);
		expect(rtf).toContain("Hello {\\b big} link\\par caf\\u233? \\u8364? {\\i it}");
		expect(rtf).not.toContain("alert");
		expect(rtf).not.toContain("evil");
		expect(rtf.endsWith("}")).toBe(true);
		expect(rtf.split("{").length).toBe(rtf.split("}").length);
	});

	test("text braces and backslashes are escaped in RTF", () => {
		expect(htmlToRtf("<p>a {b} \\c</p>")).toContain("a \\{b\\} \\\\c");
	});
});

describe("text for the guest", () => {
	test("a lone LF and a lone CR become CRLF, CRLF stays", () => {
		expect(dec(textForGuest("a\nb\r\nc\rd"))).toBe("a\r\nb\r\nc\r\nd");
	});
});

// ---- the state machine, against a fake link and a fake browser ----

class FakeLink implements ClipLink {
	sent: { channel: string; op: string; fields: Record<string, string>; data: Uint8Array; id: number }[] = [];
	handlers: Record<string, ((v: any) => void)[]> = { message: [], state: [] };
	state: LinkStats["state"] = "ready";
	caps = ["clip", "file", "time", "display"];
	bytesOut = 0;
	bytesIn = 0;
	queued = 0;
	nextId = 0;
	cancelled: number[] = [];
	on(event: "message" | "state", handler: (v: any) => void) {
		this.handlers[event]!.push(handler);
		return () => {};
	}
	send(channel: string, op: string, fields: Iterable<readonly [string, string | number]> = [], data = new Uint8Array(0)) {
		if (this.state !== "ready" && this.state !== "stalled") return null;
		const id = this.nextId++;
		this.sent.push({ channel, op, fields: Object.fromEntries([...fields].map(([k, v]) => [k, String(v)])), data, id });
		return id;
	}
	cancel(ids: number[]) {
		this.cancelled.push(...ids);
	}
	stats(): LinkStats {
		return { state: this.state, peer: this.state === "ready" || this.state === "stalled" ? { app: "HOSTLINK/0.1.0", proto: 1, role: "guest", caps: this.caps, window: 4352, max: 8388608 } : null, bytesIn: this.bytesIn, bytesOut: this.bytesOut, packetsIn: 0, packetsOut: 0, messagesIn: 0, messagesOut: 0, errors: {}, peerErrors: {}, dropped: 0, credit: { window: 4352, outstanding: 0, available: 4352 }, queued: this.queued, helloMs: null };
	}
	deliver(op: string, fields: Record<string, string | number>, data = new Uint8Array(0), channel = "clip") {
		const message: Message = { channel: channel as any, id: 0, op, fields: [["op", op], ...Object.entries(fields).map(([k, v]) => [k, String(v)] as [string, string])], data };
		for (const h of this.handlers.message!) h(message);
	}
	changeState(state: LinkStats["state"]) {
		this.state = state;
		for (const h of this.handlers.state!) h(state);
	}
	ops(op: string) {
		return this.sent.filter((m) => m.op === op);
	}
}

class FakePort implements ClipboardPort {
	writes: { text?: string; html?: string; png?: Blob }[] = [];
	failWith: Error | null = null;
	async write(item: { text?: Promise<string>; html?: Promise<string>; png?: Promise<Blob> }) {
		if (this.failWith) throw this.failWith;
		this.writes.push({ text: item.text ? await item.text : undefined, html: item.html ? await item.html : undefined, png: item.png ? await item.png : undefined });
	}
	async dibToPng(dib: Uint8Array) {
		return new Blob([dib as BlobPart], { type: "image/png" });
	}
	async imageToDib(image: Blob) {
		return new Uint8Array(await image.arrayBuffer());
	}
}

let link: FakeLink;
let port: FakePort;
let clock: { now: number };
let sync: ClipboardSync;
const tick = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
	link = new FakeLink();
	port = new FakePort();
	clock = { now: 100_000 };
	sync = new ClipboardSync(link, port, { now: () => clock.now, noteMs: 10 });
});

const offerText = (seq: number, text: string, extra: Record<string, string | number> = {}) => {
	link.deliver("offer", { seq, text: enc(text).length, source: "user", ...extra });
	link.deliver("data", { seq, fmt: "text" }, enc(text));
};

describe("guest to page", () => {
	test("a copy made after a click on the screen is written once, with its text exact", async () => {
		sync.setActive(true);
		sync.gesture();
		offerText(5, "Zoë café — 5 €\r\nline two");
		await tick();
		expect(port.writes.length).toBe(1);
		expect(port.writes[0]!.text).toBe("Zoë café — 5 €\r\nline two");
		expect(sync.status().counters.copiedOut).toBe(1);
		// The doubled offer (the viewer chain) and a second offer in the same gesture write nothing more.
		offerText(5, "Zoë café — 5 €\r\nline two");
		offerText(6, "another");
		await tick();
		expect(port.writes.length).toBe(1);
		expect(sync.status().counters.skipped).toBe(1);
	});

	test("rich text from the guest is written as HTML next to the text", async () => {
		sync.setActive(true);
		sync.gesture();
		const rtf = "{\\rtf1\\ansi plain {\\b bold}\\par}";
		link.deliver("offer", { seq: 1, text: 10, rtf: rtf.length, source: "user" });
		link.deliver("data", { seq: 1, fmt: "text" }, enc("plain bold"));
		link.deliver("data", { seq: 1, fmt: "rtf" }, enc(rtf));
		await tick();
		expect(port.writes[0]).toMatchObject({ text: "plain bold", html: "<p>plain <b>bold</b></p>" });
	});

	test("with no gesture in the last 5 s nothing is written, and the button does it", async () => {
		sync.setActive(true);
		sync.gesture();
		clock.now += GESTURE_MS + 1;
		offerText(1, "late");
		await tick();
		expect(port.writes.length).toBe(0);
		expect(sync.status().held).not.toBeNull();
		expect(sync.status().note).toContain("Copy from ReactOS");
		expect(await sync.copyOut()).toBe(true);
		expect(port.writes[0]!.text).toBe("late");
	});

	test("until the screen has been selected nothing is written, however fresh the key", async () => {
		sync.gesture();
		offerText(1, "early");
		await tick();
		expect(port.writes.length).toBe(0);
		expect(sync.status().held?.sizes.text).toBe(5);
	});

	test("when the browser refuses the write the visitor is offered the button", async () => {
		port.failWith = Object.assign(new Error("denied"), { name: "NotAllowedError" });
		sync.setActive(true);
		sync.gesture();
		offerText(1, "x");
		await tick();
		expect(sync.status().note).toContain("Copy from ReactOS");
		expect(sync.status().counters.refused).toBe(1);
		port.failWith = null;
		expect(await sync.copyOut()).toBe(true);
	});

	test("an offer the helper marks as its own, and an offer while sharing is off, are ignored", async () => {
		sync.setActive(true);
		sync.gesture();
		link.deliver("offer", { seq: 1, text: 4, source: "self" });
		sync.setEnabled(false);
		offerText(2, "text");
		await tick();
		expect(port.writes.length).toBe(0);
		expect(sync.status().held).toBeNull();
		expect(link.ops("pause").map((m) => m.fields.channel)).toEqual(["clip"]);
		sync.setEnabled(true);
		expect(link.ops("resume").length).toBe(1);
	});

	test("a picture is asked for and written as a PNG when it fits the window; a large one waits for the button", async () => {
		sync.setActive(true);
		sync.gesture();
		const small = pixelsToDib(8, 8, new Uint8ClampedArray(256).fill(200));
		link.deliver("offer", { seq: 1, dib: small.length, source: "user" });
		expect(link.ops("get").map((m) => m.fields)).toEqual([{ seq: "1", fmt: "dib" }]);
		link.deliver("data", { seq: 1, fmt: "dib" }, small as Uint8Array<ArrayBuffer>);
		await tick();
		expect(port.writes[0]!.png).toBeInstanceOf(Blob);

		// A full-screen Print Screen (3.1 MB) is over what the 5 s window carries: no automatic write, no `get`.
		sync.gesture();
		link.deliver("offer", { seq: 2, dib: 3_145_780, source: "user" });
		await tick();
		expect(link.ops("get").length).toBe(1);
		expect(port.writes.length).toBe(1);
		// The button asks, and writes when the data comes.
		const done = sync.copyOut();
		await tick();
		expect(link.ops("get").length).toBe(2);
		link.deliver("data", { seq: 2, fmt: "dib" }, new Uint8Array(3_145_780));
		expect(await done).toBe(true);
		expect(port.writes.length).toBe(2);
	});

	test("a copy over a limit is not fetched and the line says why", async () => {
		sync.setActive(true);
		sync.gesture();
		link.deliver("offer", { seq: 1, text: LIMITS.text + 1, source: "user" });
		await tick();
		expect(link.ops("get").length).toBe(0);
		expect(port.writes.length).toBe(0);
		expect(sync.status().note).toContain("too large");
		expect(sync.status().note).toContain("limit 1 MB");
	});

	test("text between 256 KiB and 1 MiB is asked for", async () => {
		sync.setActive(true);
		sync.gesture();
		link.deliver("offer", { seq: 1, text: 600_000, source: "user" });
		expect(link.ops("get").map((m) => m.fields.fmt)).toEqual(["text"]);
	});
});

describe("privacy", () => {
	test("nothing the guest sends makes the page read or write the browser clipboard without a gesture", async () => {
		sync.setActive(true);
		// Every message the guest may send, with no gesture at all.
		link.deliver("get", { seq: 1, fmt: "text" });
		link.deliver("set", { seq: 1, formats: "text" });
		link.deliver("part", { seq: 1, fmt: "text" }, enc("x"));
		offerText(1, "no gesture");
		link.deliver("ok", { seq: 99 });
		link.deliver("refused", { seq: 99, reason: "x" });
		await tick();
		expect(port.writes.length).toBe(0);
		// The port has no read at all: reading is the glue's, on a paste event or a click.
		expect("read" in port).toBe(false);
		// And the helper's forged `get` is never answered with the host clipboard: the page sent nothing at all.
		expect(link.sent.filter((m) => m.channel === "clip").length).toBe(0);
	});

	test("stop, reset and the page hiding forget what is held", async () => {
		sync.setActive(true);
		sync.gesture();
		clock.now += GESTURE_MS + 1;
		offerText(1, "secret");
		expect(sync.status().held).not.toBeNull();
		sync.clear();
		expect(sync.status().held).toBeNull();
		expect(await sync.copyOut()).toBe(false);
	});
});

describe("page to guest", () => {
	test("a paste sends a set, then its parts, and resolves on the helper's ok; the text has CRLF line ends", async () => {
		const done = sync.push({ text: "a\nb", html: "<p>a<br>b</p>" });
		await tick();
		expect(link.ops("set").map((m) => m.fields)).toEqual([{ seq: "1", formats: "text,rtf" }]);
		const parts = link.ops("part");
		expect(parts.map((m) => m.fields.fmt)).toEqual(["text", "rtf"]);
		expect(dec(parts[0]!.data)).toBe("a\r\nb");
		expect(dec(parts[1]!.data).startsWith("{\\rtf1")).toBe(true);
		link.deliver("ok", { seq: 1 });
		const result = await done;
		expect(result.ok).toBe(true);
		expect(sync.status().counters.pastedIn).toBe(1);
	});

	test("RTF from the browser goes as it is", async () => {
		const done = sync.push({ text: "x", rtf: "{\\rtf1 x}", html: "<p>ignored</p>" });
		await tick();
		expect(dec(link.ops("part")[1]!.data)).toBe("{\\rtf1 x}");
		link.deliver("ok", { seq: 1 });
		await done;
	});

	test("the same text again, with nothing offered in between, is not sent twice", async () => {
		const first = sync.push({ text: "same" });
		await tick();
		link.deliver("ok", { seq: 1 });
		await first;
		const second = await sync.push({ text: "same" });
		expect(second).toMatchObject({ ok: true, same: true });
		expect(link.ops("set").length).toBe(1);
		// After the guest copies something else, the host's copy is new again.
		offerText(7, "other");
		const third = sync.push({ text: "same" });
		await tick();
		expect(link.ops("set").length).toBe(2);
		link.deliver("ok", { seq: 2 });
		await third;
	});

	test("text the guest copied and the page wrote out is not pushed back in", async () => {
		sync.setActive(true);
		sync.gesture();
		offerText(3, "from the guest\r\nline");
		await tick();
		const result = await sync.push({ text: "from the guest\nline" });
		expect(result).toMatchObject({ ok: true, same: true });
		expect(link.ops("set").length).toBe(0);
	});

	test("over the limit is refused with the size, before anything is sent", async () => {
		const result = await sync.push({ text: "x".repeat(LIMITS.text + 1) });
		expect(result).toMatchObject({ ok: false, reason: "too-large", detail: "text", limit: LIMITS.text });
		expect(link.sent.length).toBe(0);
		expect(sync.status().note).toContain("too large");
	});

	test("a refusal from the helper is reported with its reason", async () => {
		const done = sync.push({ text: "x" });
		await tick();
		link.deliver("refused", { seq: 1, reason: "busy" });
		expect(await done).toMatchObject({ ok: false, reason: "refused", detail: "busy" });
	});

	test("cancel drops the rest of the packets and settles the paste", async () => {
		const done = sync.push({ text: "x".repeat(5000) });
		await tick();
		sync.cancel();
		expect(await done).toMatchObject({ ok: false, reason: "cancelled" });
		expect(link.cancelled.length).toBe(2);
		expect(sync.status().busy).toBe("idle");
	});

	test("a paste with no helper, or with sharing off, sends nothing", async () => {
		link.changeState("waiting");
		expect(await sync.push({ text: "x" })).toMatchObject({ ok: false, reason: "not-ready" });
		link.changeState("ready");
		sync.setEnabled(false);
		expect(await sync.push({ text: "x" })).toMatchObject({ ok: false, reason: "off" });
	});

	test("a picture goes as a DIB and a bad one is refused", async () => {
		const dib = pixelsToDib(2, 2, new Uint8ClampedArray(16).fill(9));
		const done = sync.push({ image: new Blob([dib as BlobPart]) });
		await tick();
		expect(link.ops("set")[0]!.fields.formats).toBe("dib");
		expect(link.ops("part")[0]!.data.length).toBe(dib.length);
		link.deliver("ok", { seq: 1 });
		await done;
		port.imageToDib = async () => {
			throw new RangeError("too many pixels");
		};
		expect(await sync.push({ image: new Blob([new Uint8Array(3)]) })).toMatchObject({ ok: false, reason: "bad-image" });
	});

	test("a restart of the link (a new hello) cancels a paste in progress", async () => {
		const done = sync.push({ text: "x".repeat(9000) });
		await tick();
		link.changeState("waiting");
		expect(await done).toMatchObject({ ok: false, reason: "cancelled" });
	});
});

describe("status", () => {
	test("connected, not ready, and no helper on a restored state", () => {
		expect(sync.status().link).toBe("connected");
		link.changeState("waiting");
		expect(sync.status().link).toBe("not-ready");
		const restored = new ClipboardSync(link, port, { now: () => clock.now, restored: true });
		expect(restored.status().link).toBe("not-ready");
		clock.now += 13_000;
		expect(restored.status().link).toBe("no-helper");
		link.state = "stalled";
		expect(sync.status().link).toBe("not-ready");
	});

	test("a link whose helper has no clipboard says so", () => {
		link.caps = ["file"];
		expect(sync.status().link).toBe("no-helper");
	});
});
