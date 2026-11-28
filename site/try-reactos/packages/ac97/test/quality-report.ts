// Prints the resampler's measured quality for every standard rate pair: the worst passband deviation up to 0.45 of the lower Nyquist frequency, the
// weakest rejection of what would alias or image into the band, and THD+N of a 1 kHz tone. Run with `bun test/quality-report.ts` (not part of `bun test`).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Resampler, STANDARD_RATES } from "../src/resample.ts";

function run(inRate: number, outRate: number, input: Float32Array): Float32Array {
	const r = new Resampler(inRate, outRate);
	r.write(input, 0, input.length);
	const out = new Float32Array(Math.ceil((input.length * outRate) / inRate) + 8);
	return out.subarray(0, r.read(out, out.length));
}
const sine = (rate: number, f: number, s: number) => Float32Array.from({ length: Math.round(rate * s) }, (_, i) => 0.5 * Math.sin((2 * Math.PI * f * i) / rate));
const rms = (a: Float32Array, from: number, to: number) => Math.sqrt(a.slice(from, to).reduce((s, v) => s + v * v, 0) / (to - from));
const db = (x: number) => 20 * Math.log10(x);
function residualDb(a: Float32Array, rate: number, f: number, from: number, to: number): number {
	let ss = 0, cc = 0, sc = 0, xs = 0, xc = 0, xx = 0;
	for (let i = from; i < to; i++) {
		const s = Math.sin((2 * Math.PI * f * i) / rate), c = Math.cos((2 * Math.PI * f * i) / rate), x = a[i] as number;
		ss += s * s; cc += c * c; sc += s * c; xs += x * s; xc += x * c; xx += x * x;
	}
	const det = ss * cc - sc * sc, p = (xs * cc - xc * sc) / det, q = (xc * ss - xs * sc) / det;
	const fit = p * p * ss + q * q * cc + 2 * p * q * sc;
	return 10 * Math.log10(Math.max(1e-30, xx - fit) / fit);
}

const rows: unknown[] = [];
for (const rate of STANDARD_RATES) {
	if (rate === 48000) continue;
	for (const [inRate, outRate] of [[rate, 48000], [48000, rate]] as [number, number][]) {
		const low = Math.min(inRate, outRate);
		let flat = 0;
		for (let step = 1; step <= 9; step++) {
			const f = (0.05 * step * low) / 2;
			const input = sine(inRate, f, 1);
			const out = run(inRate, outRate, input);
			flat = Math.max(flat, Math.abs(db(rms(out, 600, out.length - 600) / rms(input, 600, input.length - 600))));
		}
		let reject = -999;
		if (outRate < inRate) {
			for (let n = 0; n <= 8; n++) {
				const f = 0.49 * outRate + (n * (inRate / 2 - 0.49 * outRate - 1)) / 8;
				const input = sine(inRate, f, 0.5);
				const out = run(inRate, outRate, input);
				reject = Math.max(reject, db(rms(out, 600, out.length - 600) / rms(input, 600, input.length - 600)));
			}
		} else {
			for (const fraction of [0.1, 0.25, 0.4]) {
				const f = (fraction * low) / 2 + 37;
				const out = run(inRate, outRate, sine(inRate, f, 0.5));
				reject = Math.max(reject, residualDb(out, outRate, f, 600, out.length - 600));
			}
		}
		const f1k = Math.min(1000, 0.2 * low);
		const out = run(inRate, outRate, sine(inRate, f1k, 1));
		rows.push({ inRate, outRate, phases: new Resampler(inRate, outRate).phases, taps: new Resampler(inRate, outRate).taps, passbandMaxDeviationDb: Math.round(flat * 10000) / 10000, worstAliasOrImageDb: Math.round(reject * 10) / 10, thdPlusNoiseDb: Math.round(residualDb(out, outRate, f1k, 600, out.length - 600) * 10) / 10 });
	}
}
for (const row of rows) console.log(JSON.stringify(row));
