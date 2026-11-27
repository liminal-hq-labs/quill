// A packed DIB (a `BITMAPINFOHEADER`, its colour table if any, the bits: what `CF_DIB` holds) to pixels and back. No DOM: the PNG side is
// the canvas's, in browser.ts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The most pixels a side may have into the guest. */
export const MAX_SIDE = 4096;

export interface Pixels {
	width: number;
	height: number;
	/** RGBA, top row first, always opaque (a picture from the guest has no usable alpha: Print Screen's is 0). */
	rgba: Uint8ClampedArray;
}

export class DibError extends Error {}

/** Decodes a DIB of 1, 4, 8, 16, 24 or 32 bits per pixel, uncompressed or with bit fields. Throws `DibError` for anything else. */
export function dibToPixels(dib: Uint8Array): Pixels {
	if (dib.length < 40) throw new DibError("too short for a bitmap header");
	const v = new DataView(dib.buffer, dib.byteOffset, dib.byteLength);
	const headerSize = v.getUint32(0, true);
	if (headerSize < 40 || headerSize > dib.length) throw new DibError("not a BITMAPINFOHEADER");
	const width = v.getInt32(4, true);
	const rawHeight = v.getInt32(8, true);
	const bpp = v.getUint16(14, true);
	const compression = v.getUint32(16, true);
	const used = v.getUint32(32, true);
	const height = Math.abs(rawHeight);
	const topDown = rawHeight < 0;
	if (width <= 0 || height === 0 || width > 16384 || height > 16384) throw new DibError("odd size " + width + " by " + rawHeight);
	if (compression !== 0 && !(compression === 3 && (bpp === 16 || bpp === 32))) throw new DibError("compression " + compression + " is not supported");
	if (![1, 4, 8, 16, 24, 32].includes(bpp)) throw new DibError(bpp + " bits per pixel is not supported");
	let at = headerSize;
	let masks: [number, number, number] = bpp === 16 ? [0x7c00, 0x03e0, 0x001f] : [0x00ff0000, 0x0000ff00, 0x000000ff];
	if (compression === 3) {
		if (headerSize === 40) {
			if (at + 12 > dib.length) throw new DibError("bit masks are cut off");
			masks = [v.getUint32(at, true), v.getUint32(at + 4, true), v.getUint32(at + 8, true)];
			at += 12;
		} else masks = [v.getUint32(40, true), v.getUint32(44, true), v.getUint32(48, true)];
	}
	const colours = bpp <= 8 ? (used || 1 << bpp) : 0;
	const palette: number[] = [];
	for (let k = 0; k < colours; k++) {
		if (at + 4 > dib.length) throw new DibError("colour table is cut off");
		palette.push((dib[at + 2]! << 16) | (dib[at + 1]! << 8) | dib[at]!);
		at += 4;
	}
	const stride = ((width * bpp + 31) >> 5) * 4;
	if (at + stride * height > dib.length) throw new DibError("the bits are cut off");
	const rgba = new Uint8ClampedArray(width * height * 4);
	const shiftOf = (mask: number) => {
		if (mask === 0) return { shift: 0, max: 0 };
		let shift = 0;
		while (((mask >>> shift) & 1) === 0) shift++;
		let bits = 0;
		while (((mask >>> (shift + bits)) & 1) === 1 && bits < 32) bits++;
		return { shift, max: 2 ** bits - 1 };
	};
	const channels = masks.map(shiftOf);
	for (let row = 0; row < height; row++) {
		const source = at + (topDown ? row : height - 1 - row) * stride;
		let out = row * width * 4;
		for (let x = 0; x < width; x++, out += 4) {
			let r: number, g: number, b: number;
			if (bpp === 32 || bpp === 16) {
				const p = source + x * (bpp / 8);
				const pixel = bpp === 32 ? v.getUint32(p, true) : v.getUint16(p, true);
				const pick = (c: { shift: number; max: number }, mask: number) => (c.max === 0 ? 0 : Math.round((((pixel & mask) >>> c.shift) * 255) / c.max));
				if (compression === 0 && bpp === 32) {
					b = dib[p]!;
					g = dib[p + 1]!;
					r = dib[p + 2]!;
				} else {
					r = pick(channels[0]!, masks[0]);
					g = pick(channels[1]!, masks[1]);
					b = pick(channels[2]!, masks[2]);
				}
			} else if (bpp === 24) {
				const p = source + x * 3;
				b = dib[p]!;
				g = dib[p + 1]!;
				r = dib[p + 2]!;
			} else {
				const perByte = 8 / bpp;
				const byte = dib[source + Math.floor(x / perByte)]!;
				const index = (byte >> (8 - bpp * ((x % perByte) + 1))) & ((1 << bpp) - 1);
				const colour = palette[index] ?? 0;
				r = (colour >> 16) & 255;
				g = (colour >> 8) & 255;
				b = colour & 255;
			}
			rgba[out] = r;
			rgba[out + 1] = g;
			rgba[out + 2] = b;
			rgba[out + 3] = 255;
		}
	}
	return { width, height, rgba };
}

/** Encodes pixels as a 24-bit bottom-up DIB, laying transparent pixels over white. */
export function pixelsToDib(width: number, height: number, rgba: Uint8ClampedArray | Uint8Array): Uint8Array {
	const stride = ((width * 24 + 31) >> 5) * 4;
	const size = stride * height;
	const out = new Uint8Array(40 + size);
	const v = new DataView(out.buffer);
	v.setUint32(0, 40, true);
	v.setInt32(4, width, true);
	v.setInt32(8, height, true);
	v.setUint16(12, 1, true);
	v.setUint16(14, 24, true);
	v.setUint32(20, size, true);
	v.setInt32(24, 2835, true);
	v.setInt32(28, 2835, true);
	for (let row = 0; row < height; row++) {
		let p = 40 + (height - 1 - row) * stride;
		let q = row * width * 4;
		for (let x = 0; x < width; x++, p += 3, q += 4) {
			const a = rgba[q + 3]! / 255;
			out[p] = Math.round(rgba[q + 2]! * a + 255 * (1 - a));
			out[p + 1] = Math.round(rgba[q + 1]! * a + 255 * (1 - a));
			out[p + 2] = Math.round(rgba[q]! * a + 255 * (1 - a));
		}
	}
	return out;
}

/** The size in bytes of a 24-bit DIB of this many pixels. */
export function dibBytes(width: number, height: number): number {
	return 40 + ((width * 24 + 31) >> 5) * 4 * height;
}
