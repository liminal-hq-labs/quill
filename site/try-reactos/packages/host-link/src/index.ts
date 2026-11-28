// The page's end of the host link: `attach(emulator)` joins a `Link` to v86's first serial port. The protocol is docs/host-link.md.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Link, type LinkEvents, type LinkOptions, type LinkStats } from "./link.ts";
import type { Channel } from "./frame.ts";

export * from "./crc.ts";
export * from "./flow.ts";
export * from "./frame.ts";
export * from "./link.ts";
export * from "./message.ts";

/** The part of v86's `V86` object the link uses. */
export interface SerialEmulator {
	add_listener(event: string, listener: (value: any) => void): void;
	remove_listener(event: string, listener: (value: any) => void): void;
	serial_send_bytes(serial: number, data: Uint8Array): void;
}

export interface AttachOptions extends Partial<Omit<LinkOptions, "write">> {
	/** How often to call `tick`, in milliseconds; 0 for never (a test drives it). */
	tickMs?: number;
	/**
	 * What the machine is. `cold`: a boot, where the page says nothing until the guest has (the `hello` is held). `restored`: a saved state whose
	 * helper is already listening, so the `hello` goes as soon as the emulator is running (v86 drops what is sent before its serial port exists).
	 * `now`: send it now (the default, for tests).
	 */
	mode?: "cold" | "restored" | "now";
}

export interface HostLink {
	readonly link: Link;
	on: Link["on"];
	send(channel: Channel, op: string, fields?: Iterable<readonly [string, string | number]>, data?: Uint8Array): number | null;
	stats(): LinkStats;
	/** Sends `hello` again and forgets everything in flight (after a restore, or a reset). */
	restart(): void;
	/** Drops the queued packets of messages sent with these ids. */
	cancel(ids: number[]): void;
	detach(): void;
}

export type { LinkEvents };

/** Attaches the link to a machine: listens on COM1, sends `hello` and keeps retrying it until a helper answers. */
export function attach(emulator: SerialEmulator, options: AttachOptions = {}): HostLink {
	const { tickMs = 1000, mode = "now", ...rest } = options;
	const link = new Link({ ...rest, write: (bytes) => emulator.serial_send_bytes(0, bytes) });
	let pending: number[] = [];
	const onByte = (byte: number): void => {
		// v86 calls once per byte; hand the link a chunk once per task, not a call per byte.
		if (pending.length === 0) queueMicrotask(flush);
		pending.push(byte);
	};
	const flush = (): void => {
		const chunk = Uint8Array.from(pending);
		pending = [];
		link.receive(chunk);
	};
	emulator.add_listener("serial0-output-byte", onByte);
	const timer = tickMs > 0 ? setInterval(() => link.tick(), tickMs) : null;
	const started = (): void => link.start();
	let onStarted: (() => void) | null = null;
	if (mode === "cold") link.start({ hold: true });
	else if (mode === "restored") {
		// Waiting: nothing is sent until the emulator has restored the state and run, when its serial port exists.
		link.start({ hold: true });
		onStarted = () => {
			emulator.remove_listener("emulator-started", onStarted!);
			if (!link.heard) link.start();
		};
		emulator.add_listener("emulator-started", onStarted);
	} else started();
	return {
		link,
		on: (event, handler) => link.on(event, handler),
		send: (channel, op, fields, data) => link.send(channel, op, fields, data),
		stats: () => link.stats(),
		restart: () => link.start(),
		cancel: (ids) => link.cancel(ids),
		detach() {
			if (onStarted) emulator.remove_listener("emulator-started", onStarted);
			emulator.remove_listener("serial0-output-byte", onByte);
			if (timer !== null) clearInterval(timer);
			link.stop();
		},
	};
}
export * from "./clip/dib.ts";
export * from "./clip/rtf.ts";
export * from "./clip/state.ts";
export * from "./clip/browser.ts";
