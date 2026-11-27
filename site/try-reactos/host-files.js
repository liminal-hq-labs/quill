// The seam for the files the guest sends to the page (the host file channel). Empty until that feature lands: the page calls
// attach when a machine starts and detach when it stops, so the feature adds code here and nowhere in try-reactos.js.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export function attach(emulator) {
	void emulator;
}

export function detach() {}
