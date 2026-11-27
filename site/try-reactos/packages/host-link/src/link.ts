// The link proper, with no emulator in it: hello and its retry, credits, per-channel queues sent in priority order, the error
// replies and the statistics. `attach` in index.ts joins it to v86's serial port; tests join it to a fake bus.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Credit, DEFAULT_WINDOW } from "./flow.ts";
import { FrameParser, PRIORITY, packetsOf, type Channel, type FrameEvent } from "./frame.ts";
import { DEFAULT_MAX_MESSAGE, MIN_MAX_MESSAGE, Mux, field, fieldNumber, writeBody, type ErrorCode, type Message, type MuxOutput } from "./message.ts";

/** The protocol version `hello` announces. */
export const PROTO = 1;
export const APP = "host-link/0.1.0";
export const CAPS = ["clip", "file", "time", "display"];

/** `waiting`: no helper has answered. `ready`: a hello came and credit flows. `stalled`: credit has not come back for 5 s.
 * `version`: the helper speaks a protocol this side does not know. `closed`: after `stop`. */
export type LinkState = "waiting" | "ready" | "stalled" | "version" | "closed";

export interface Peer {
	app: string;
	proto: number;
	role: string;
	caps: string[];
	window: number;
	max: number;
}

export interface LinkStats {
	state: LinkState;
	peer: Peer | null;
	bytesIn: number;
	bytesOut: number;
	packetsIn: number;
	packetsOut: number;
	messagesIn: number;
	messagesOut: number;
	/** Errors this side found, by code (`crc`, `too-large`, `unknown`, `malformed`, `version`; `header` and `trailer` for lines it skipped). */
	errors: Record<string, number>;
	/** Errors the peer reported to this side, by code. */
	peerErrors: Record<string, number>;
	/** Messages abandoned because a new id began on their channel. */
	dropped: number;
	credit: { window: number; outstanding: number; available: number };
	/** Packets queued and not yet sent. */
	queued: number;
}

export interface LinkOptions {
	/** Writes bytes to the port. */
	write(bytes: Uint8Array): void;
	now?: () => number;
	app?: string;
	/** The largest message this side accepts. */
	max?: number;
	/** How long before a `hello` with no answer is sent again. */
	helloRetryMs?: number;
	/** How long without credit before the link is `stalled`. */
	creditTimeoutMs?: number;
}

export interface LinkEvents {
	/** A message on a channel other than `ctl`, or a `ctl` message the link does not handle itself. */
	message: Message;
	/** An `LHQ-FILE` frame's event: `file-begin`, `file-data` or `file-end`. */
	file: FrameEvent;
	state: LinkState;
	/** The peer reported an error (`ctl op=error`). */
	"peer-error": Message;
}

type Handler<K extends keyof LinkEvents> = (value: LinkEvents[K]) => void;

interface Pending {
	packets: Uint8Array[];
	next: number;
}

export class Link {
	readonly #write: (bytes: Uint8Array) => void;
	readonly #now: () => number;
	readonly #app: string;
	readonly #max: number;
	readonly #helloRetryMs: number;
	readonly #creditTimeoutMs: number;
	readonly #parser = new FrameParser();
	readonly #mux: Mux;
	readonly #credit = new Credit();
	readonly #queues = new Map<Channel, Pending[]>(PRIORITY.map((c) => [c, []]));
	readonly #paused = new Set<Channel>();
	readonly #handlers: { [K in keyof LinkEvents]?: Handler<K>[] } = {};
	#state: LinkState = "closed";
	#peer: Peer | null = null;
	#nextId = 0;
	#awaiting = false;
	#helloSentAt = 0;
	#stallSince: number | null = null;
	#counts = { bytesIn: 0, bytesOut: 0, packetsIn: 0, packetsOut: 0, messagesIn: 0, messagesOut: 0 };
	#errors: Record<string, number> = {};
	#peerErrors: Record<string, number> = {};
	#versionReplied = false;

	constructor(options: LinkOptions) {
		this.#write = options.write;
		this.#now = options.now ?? (() => Date.now());
		this.#app = options.app ?? APP;
		this.#max = Math.max(MIN_MAX_MESSAGE, options.max ?? DEFAULT_MAX_MESSAGE);
		this.#helloRetryMs = options.helloRetryMs ?? 5000;
		this.#creditTimeoutMs = options.creditTimeoutMs ?? 5000;
		this.#mux = new Mux(this.#max);
	}

	get state(): LinkState {
		return this.#state;
	}

	on<K extends keyof LinkEvents>(event: K, handler: Handler<K>): () => void {
		const list = (this.#handlers[event] ??= []) as Handler<K>[];
		list.push(handler);
		return () => {
			const at = list.indexOf(handler);
			if (at >= 0) list.splice(at, 1);
		};
	}

	#emit<K extends keyof LinkEvents>(event: K, value: LinkEvents[K]): void {
		for (const handler of [...((this.#handlers[event] ?? []) as Handler<K>[])]) {
			try {
				handler(value);
			} catch (error) {
				this.#count(this.#errors, "handler");
				console.error("host-link: a handler threw", error);
			}
		}
	}

	#setState(state: LinkState): void {
		if (state === this.#state) return;
		this.#state = state;
		this.#emit("state", state);
	}

	#count(into: Record<string, number>, code: string): void {
		into[code] = (into[code] ?? 0) + 1;
	}

	/** Attaches to a machine (a cold boot or a restored state): forgets everything in flight and sends `hello`. */
	start(): void {
		this.#parser.reset();
		this.#mux.reset();
		this.#peer = null;
		this.#credit.reset(DEFAULT_WINDOW);
		for (const q of this.#queues.values()) q.length = 0;
		this.#paused.clear();
		this.#stallSince = null;
		this.#versionReplied = false;
		this.#setState("waiting");
		this.#sendHello();
	}

	/** Ends the link. Nothing more is sent or taken. */
	stop(): void {
		this.#setState("closed");
		for (const q of this.#queues.values()) q.length = 0;
	}

	#sendHello(): void {
		const body = writeBody("hello", [
			["proto", PROTO],
			["role", "page"],
			["app", this.#app],
			["caps", CAPS.join(",")],
			["max", this.#max],
		]);
		// A hello is outside the credit: the page cannot know the window until the helper has said it.
		this.#writePackets(packetsOf("ctl", this.#takeId(), body));
		this.#counts.messagesOut++;
		this.#awaiting = true;
		this.#helloSentAt = this.#now();
	}

	#takeId(): number {
		const id = this.#nextId;
		this.#nextId = (this.#nextId + 1) >>> 0;
		return id;
	}

	#writePackets(packets: Uint8Array[]): void {
		for (const p of packets) {
			this.#write(p);
			this.#counts.bytesOut += p.length;
			this.#counts.packetsOut++;
		}
	}

	/** Call about once a second: sends `hello` again if nobody has answered, and notices a stall. */
	tick(): void {
		if (this.#state === "closed") return;
		const now = this.#now();
		if (this.#awaiting && now - this.#helloSentAt >= this.#helloRetryMs) this.#sendHello();
		if (this.#state === "ready" && this.#stallSince !== null && now - this.#stallSince >= this.#creditTimeoutMs) this.#setState("stalled");
	}

	/** Queues a message to the peer. Returns its id, or null when the link is not ready or the message is over the peer's max. */
	send(channel: Channel, op: string, fields: Iterable<readonly [string, string | number]> = [], data: Uint8Array = new Uint8Array(0)): number | null {
		if (!this.#peer || (this.#state !== "ready" && this.#state !== "stalled")) return null;
		return this.#enqueue(channel, op, fields, data);
	}

	#enqueue(channel: Channel, op: string, fields: Iterable<readonly [string, string | number]>, data: Uint8Array): number | null {
		const body = writeBody(op, fields, data);
		if (this.#peer && body.length > this.#peer.max) {
			this.#count(this.#errors, "too-large-out");
			return null;
		}
		if (!this.#peer) return null;
		const id = this.#takeId();
		this.#queues.get(channel)!.push({ packets: packetsOf(channel, id, body), next: 0 });
		this.#counts.messagesOut++;
		this.#pump();
		return id;
	}

	/** Sends packets in priority order, one at a time, while credit allows. A packet from a higher channel always goes before the next one from a lower. */
	#pump(): void {
		if (!this.#peer) return;
		for (;;) {
			let sent = false;
			let blocked = false;
			for (const channel of PRIORITY) {
				if (this.#paused.has(channel)) continue;
				const queue = this.#queues.get(channel)!;
				const head = queue[0];
				if (!head) continue;
				const frame = head.packets[head.next]!;
				if (!this.#credit.trySpend(frame.length)) {
					blocked = true;
					break;
				}
				this.#writePackets([frame]);
				head.next++;
				if (head.next === head.packets.length) queue.shift();
				sent = true;
				break;
			}
			if (blocked) {
				this.#stallSince ??= this.#now();
				return;
			}
			if (!sent) {
				this.#stallSince = null;
				return;
			}
		}
	}

	/** Bytes from the port, in any split. */
	receive(bytes: Uint8Array): void {
		if (this.#state === "closed") return;
		this.#counts.bytesIn += bytes.length;
		for (const event of this.#parser.push(bytes)) this.#frameEvent(event);
	}

	#frameEvent(event: FrameEvent): void {
		switch (event.kind) {
			case "packet":
				this.#counts.packetsIn++;
				this.#muxOutput(this.#mux.push(event.channel, event.id, event.more, event.payload));
				return;
			case "error":
				if (event.error !== "crc") this.#count(this.#errors, event.error);
				if (event.error === "crc") this.#muxOutput(this.#mux.crcFailed(event.channel, event.id));
				return;
			default:
				this.#emit("file", event);
		}
	}

	#muxOutput(out: MuxOutput): void {
		if (!out) return;
		if ("error" in out) {
			this.#count(this.#errors, out.error);
			this.#reply(out.error, out.id);
			return;
		}
		this.#counts.messagesIn++;
		this.#message(out.message);
	}

	#reply(code: ErrorCode, id: number, text?: string): void {
		if (!this.#peer) return;
		const fields: [string, string | number][] = [
			["code", code],
			["id", id],
		];
		if (text) fields.push(["text", text]);
		this.#enqueue("ctl", "error", fields, new Uint8Array(0));
	}

	#message(message: Message): void {
		if (message.channel === "ctl") {
			this.#control(message);
			return;
		}
		if (this.#state === "version" || this.#state === "waiting") return;
		this.#emit("message", message);
	}

	#control(message: Message): void {
		switch (message.op) {
			case "hello":
				this.#hello(message);
				return;
			case "credit": {
				const n = fieldNumber(message, "n");
				if (n === undefined || this.#state === "version") return;
				this.#credit.grant(n);
				this.#stallSince = null;
				if (this.#state === "stalled") this.#setState("ready");
				this.#pump();
				return;
			}
			case "pause":
			case "resume": {
				const name = field(message, "channel") as Channel | undefined;
				if (!name || !(PRIORITY as readonly string[]).includes(name)) return;
				if (message.op === "pause") this.#paused.add(name);
				else {
					this.#paused.delete(name);
					this.#pump();
				}
				return;
			}
			case "error":
				this.#count(this.#peerErrors, field(message, "code") ?? "unknown");
				this.#emit("peer-error", message);
				return;
			case "bye":
				this.#peer = null;
				this.#setState("waiting");
				this.#sendHello();
				return;
			default:
				if (this.#state !== "version") this.#reply("unknown", message.id);
		}
	}

	#hello(message: Message): void {
		const proto = fieldNumber(message, "proto") ?? 0;
		const peer: Peer = {
			app: field(message, "app") ?? "",
			proto,
			role: field(message, "role") ?? "",
			caps: (field(message, "caps") ?? "").split(",").filter(Boolean),
			window: fieldNumber(message, "window") ?? DEFAULT_WINDOW,
			max: Math.max(MIN_MAX_MESSAGE, fieldNumber(message, "max") ?? DEFAULT_MAX_MESSAGE),
		};
		this.#mux.reset();
		const unsolicited = !this.#awaiting;
		this.#awaiting = false;
		this.#peer = peer;
		this.#credit.reset(peer.window);
		this.#stallSince = null;
		if (proto > PROTO) {
			this.#setState("version");
			if (!this.#versionReplied) {
				this.#versionReplied = true;
				this.#reply("version", message.id, "this side speaks protocol 1");
			}
			return;
		}
		this.#versionReplied = false;
		this.#setState("ready");
		if (unsolicited) this.#sendHello();
		this.#pump();
	}

	pause(channel: Channel): void {
		this.#paused.add(channel);
	}

	resume(channel: Channel): void {
		this.#paused.delete(channel);
		this.#pump();
	}

	stats(): LinkStats {
		let queued = 0;
		for (const q of this.#queues.values()) for (const m of q) queued += m.packets.length - m.next;
		return {
			state: this.#state,
			peer: this.#peer ? { ...this.#peer, caps: [...this.#peer.caps] } : null,
			...this.#counts,
			errors: { ...this.#errors },
			peerErrors: { ...this.#peerErrors },
			dropped: this.#mux.dropped,
			credit: { window: this.#credit.window, outstanding: this.#credit.outstanding, available: this.#credit.available },
			queued,
		};
	}
}
