// The seam for the page's sound (the audio device and its controls). Empty until that feature lands: the page calls attach when a
// machine starts and detach when it stops, so the feature adds code here and nowhere in try-reactos.js.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export function attach(emulator) {
	void emulator;
}

export function detach() {}
