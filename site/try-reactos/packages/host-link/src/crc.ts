// CRC-32 (IEEE 802.3, as zlib's): a table, an incremental update and a one-shot.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const TABLE = new Uint32Array(256);
for (let n = 0; n < 256; n++) {
	let c = n;
	for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
	TABLE[n] = c >>> 0;
}

/** The start state of a running CRC. */
export const CRC_START = 0xffffffff;

/** Folds `bytes` into a running state; `crcFinish` turns the state into the CRC. */
export function crcUpdate(state: number, bytes: Uint8Array): number {
	let c = state;
	for (let i = 0; i < bytes.length; i++) c = (TABLE[(c ^ bytes[i]!) & 0xff]! ^ (c >>> 8)) >>> 0;
	return c;
}

export function crcFinish(state: number): number {
	return (state ^ 0xffffffff) >>> 0;
}

/** The CRC-32 of one array. */
export function crc32(bytes: Uint8Array): number {
	return crcFinish(crcUpdate(CRC_START, bytes));
}

export function hex8(value: number): string {
	return (value >>> 0).toString(16).padStart(8, "0");
}
