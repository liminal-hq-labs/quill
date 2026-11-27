// Credits: how many frame bytes the page may have in flight to the guest.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The window a `hello` names when it names none: 16 KiB. */
export const DEFAULT_WINDOW = 16_384;
/** The smallest window a sender accepts: one longest packet with room to spare. */
export const MIN_WINDOW = 4_352;
/** The guest sends a `credit` once it has taken at least this many bytes off the port. */
export const CREDIT_STEP = 4_096;

/** The sender's account: `outstanding` counts frame bytes sent and not yet credited back. */
export class Credit {
	#window = DEFAULT_WINDOW;
	#outstanding = 0;

	constructor(window: number = DEFAULT_WINDOW) {
		this.reset(window);
	}

	get window(): number {
		return this.#window;
	}

	get outstanding(): number {
		return this.#outstanding;
	}

	get available(): number {
		return Math.max(0, this.#window - this.#outstanding);
	}

	/** Spends `bytes` if they fit; otherwise changes nothing and returns false. */
	trySpend(bytes: number): boolean {
		if (bytes > this.available) return false;
		this.#outstanding += bytes;
		return true;
	}

	/** Takes a `credit` message. More than is outstanding is clamped, so a repeated credit cannot raise the window. */
	grant(bytes: number): void {
		this.#outstanding = Math.max(0, this.#outstanding - bytes);
	}

	/** A new `hello`: a new window and nothing outstanding (the bytes in flight went with the old state). */
	reset(window: number): void {
		this.#window = Math.max(MIN_WINDOW, Math.floor(window));
		this.#outstanding = 0;
	}
}
