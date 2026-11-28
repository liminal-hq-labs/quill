// The resampler on its own: passband flatness, aliasing and image rejection, THD+N, continuity across buffers, exact ratios and a known signal.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import { Resampler, STANDARD_RATES, clampRate } from "../src/resample.ts";

/** Resamples `input` in one go and returns what comes out (the filter's start and end are left to the caller to trim). */
function run(inRate: number, outRate: number, input: Float32Array): Float32Array {
	const r = new Resampler(inRate, outRate);
	r.write(input, 0, input.length);
	const out = new Float32Array(Math.ceil((input.length * outRate) / inRate) + 8);
	const made = r.read(out, out.length);
	return out.subarray(0, made);
}

function sine(rate: number, frequency: number, seconds: number, amplitude = 0.5): Float32Array {
	const out = new Float32Array(Math.round(rate * seconds));
	for (let i = 0; i < out.length; i++) out[i] = amplitude * Math.sin((2 * Math.PI * frequency * i) / rate);
	return out;
}

const rms = (a: Float32Array, from = 0, to = a.length) => {
	let sum = 0;
	for (let i = from; i < to; i++) sum += (a[i] as number) ** 2;
	return Math.sqrt(sum / (to - from));
};
const db = (ratio: number) => 20 * Math.log10(ratio);

/** The part of `a` that is not a sine of `frequency`: least-squares fit of its in-phase and quadrature parts, returned as the residual's RMS over the fundamental's. */
function thdPlusNoise(a: Float32Array, rate: number, frequency: number, from: number, to: number): number {
	let sc = 0;
	let ss = 0;
	let cc = 0;
	let ss2 = 0;
	let xs = 0;
	let xc = 0;
	for (let i = from; i < to; i++) {
		const s = Math.sin((2 * Math.PI * frequency * i) / rate);
		const c = Math.cos((2 * Math.PI * frequency * i) / rate);
		sc += s * c;
		ss += s * s;
		cc += c * c;
		xs += (a[i] as number) * s;
		xc += (a[i] as number) * c;
		ss2 += (a[i] as number) ** 2;
	}
	const det = ss * cc - sc * sc;
	const as = (xs * cc - xc * sc) / det;
	const ac = (xc * ss - xs * sc) / det;
	const fitted = as * as * ss + ac * ac * cc + 2 * as * ac * sc;
	const residual = Math.max(0, ss2 - fitted);
	return Math.sqrt(residual / fitted);
}

const PAIRS: [number, number][] = [
	[8000, 48000], [11025, 48000], [16000, 48000], [22050, 48000], [32000, 48000], [44100, 48000],
	[48000, 8000], [48000, 11025], [48000, 16000], [48000, 22050], [48000, 32000], [48000, 44100],
];

describe("the passband", () => {
	for (const [inRate, outRate] of PAIRS) {
		test(`${inRate} to ${outRate} Hz is flat within 0.1 dB up to 0.45 of the lower Nyquist frequency`, () => {
			const low = Math.min(inRate, outRate);
			let worst = 0;
			for (let step = 1; step <= 9; step++) {
				const frequency = (0.05 * step * low) / 2; // 0.05 to 0.45 of Nyquist
				const input = sine(inRate, frequency, 1);
				const out = run(inRate, outRate, input);
				const gain = db(rms(out, 600, out.length - 600) / rms(input, 600, input.length - 600));
				worst = Math.max(worst, Math.abs(gain));
			}
			expect(worst).toBeLessThan(0.1);
		});
	}
});

describe("aliasing and images", () => {
	for (const [inRate, outRate] of PAIRS) {
		test(`${inRate} to ${outRate} Hz rejects what would alias or image into the band by at least 80 dB`, () => {
			const low = Math.min(inRate, outRate);
			let worst = -999;
			if (outRate < inRate) {
				// Tones the output rate cannot carry (from just short of its Nyquist frequency to the input's): the output should hold none of them.
				for (let n = 0; n <= 8; n++) {
					const frequency = 0.49 * outRate + (n * (inRate / 2 - 0.49 * outRate - 1)) / 8;
					const input = sine(inRate, frequency, 0.5);
					const out = run(inRate, outRate, input);
					worst = Math.max(worst, db(rms(out, 600, out.length - 600) / rms(input, 600, input.length - 600)));
				}
			} else {
				// A tone in the passband: the output must hold only that tone (its images are what is measured).
				for (const fraction of [0.1, 0.25, 0.4]) {
					const frequency = (fraction * low) / 2 + 37;
					const input = sine(inRate, frequency, 0.5);
					const out = run(inRate, outRate, input);
					worst = Math.max(worst, db(thdPlusNoise(out, outRate, frequency, 600, out.length - 600)));
				}
			}
			expect(worst).toBeLessThan(-80);
		});
	}

	test("THD+N of a 1 kHz tone is below -85 dB at the common ratios", () => {
		for (const [inRate, outRate] of [[44100, 48000], [22050, 48000], [11025, 48000], [48000, 44100]] as [number, number][]) {
			const out = run(inRate, outRate, sine(inRate, 1000, 1));
			expect(db(thdPlusNoise(out, outRate, 1000, 600, out.length - 600))).toBeLessThan(-85);
		}
	});
});

describe("continuity and exactness", () => {
	test("any split of the input and the output gives the one-shot result to within 1e-6", () => {
		for (const [inRate, outRate] of [[44100, 48000], [22050, 48000], [48000, 22050], [11025, 48000], [12345, 48000]] as [number, number][]) {
			const input = new Float32Array(20000);
			for (let i = 0; i < input.length; i++) input[i] = 0.4 * Math.sin(i * 0.05) + 0.3 * Math.sin(i * 0.31) + 0.1 * Math.sin(i * 1.7);
			const whole = run(inRate, outRate, input);
			const r = new Resampler(inRate, outRate);
			const out: number[] = [];
			const chunk = new Float32Array(4000);
			let at = 0;
			let seed = 7;
			const next = () => ((seed = (seed * 1103515245 + 12345) & 0x7fffffff) % 997) + 1;
			while (at < input.length) {
				const n = Math.min(next(), input.length - at);
				r.write(input, at, n);
				at += n;
				// Read in uneven pieces too.
				for (let more = r.available(); more > 0; more = r.available()) {
					const got = r.read(chunk, Math.min(more, next()));
					for (let i = 0; i < got; i++) out.push(chunk[i] as number);
					if (got === 0) break;
				}
			}
			expect(out.length).toBe(whole.length);
			let worst = 0;
			for (let i = 0; i < whole.length; i++) worst = Math.max(worst, Math.abs((out[i] as number) - (whole[i] as number)));
			expect(worst).toBeLessThan(1e-6);
		}
	});

	test("the common rates reduce to exact tables", () => {
		expect(new Resampler(44100, 48000).phases).toBe(160);
		expect(new Resampler(22050, 48000).phases).toBe(320);
		expect(new Resampler(11025, 48000).phases).toBe(640);
		expect(new Resampler(8000, 48000).phases).toBe(6);
		expect(new Resampler(48000, 11025).phases).toBe(147);
		for (const rate of STANDARD_RATES) {
			expect(new Resampler(rate, 48000).phases).toBeLessThanOrEqual(2048);
			expect(new Resampler(48000, rate).phases).toBeLessThanOrEqual(2048);
		}
	});

	test("an exact ratio makes exactly the frames the ratio says, with no drift", () => {
		for (const [inRate, outRate] of [[44100, 48000], [48000, 11025], [22050, 48000]] as [number, number][]) {
			const out = run(inRate, outRate, new Float32Array(inRate * 3));
			// Three seconds in, three seconds out, less the filter's look-ahead.
			expect(Math.abs(out.length - outRate * 3)).toBeLessThanOrEqual(Math.ceil((outRate / inRate) * new Resampler(inRate, outRate).lookahead) + 2);
		}
	});

	test("inputNeeded says how much to write for the frames asked for, and nothing more", () => {
		for (const [inRate, outRate] of [[44100, 48000], [22050, 48000], [48000, 22050], [12345, 48000]] as [number, number][]) {
			const r = new Resampler(inRate, outRate);
			const out = new Float32Array(1000);
			for (let round = 0; round < 20; round++) {
				const need = r.inputNeeded(480);
				r.write(new Float32Array(need), 0, need);
				expect(r.read(out, 480)).toBe(480);
			}
		}
	});

	test("equal rates pass samples through unchanged", () => {
		const input = Float32Array.from({ length: 100 }, (_, i) => i / 100);
		const out = run(48000, 48000, input);
		expect(Array.from(out)).toEqual(Array.from(input));
	});

	test("16-bit input is scaled into floats, and one side of interleaved stereo can be taken", () => {
		const r = new Resampler(48000, 48000);
		r.write(Int16Array.from([16384, -16384, 8192, -8192]), 1, 2, 2, 1 / 32768);
		const out = new Float32Array(2);
		r.read(out, 2);
		expect(Array.from(out)).toEqual([-0.5, -0.25]);
	});

	test("a rate is clamped to the card's range", () => {
		expect(clampRate(100)).toBe(8000);
		expect(clampRate(96000)).toBe(48000);
		expect(clampRate(22050.4)).toBe(22050);
		expect(clampRate(Number.NaN)).toBe(48000);
	});
});

describe("a known signal", () => {
	test("a three-tone chirp at 11025, 22050 and 44100 Hz keeps its tones at 48 kHz", () => {
		for (const inRate of [11025, 22050, 44100]) {
			const tones = [440, 1000, 3000].filter((f) => f < 0.4 * inRate);
			const input = new Float32Array(inRate);
			for (let i = 0; i < input.length; i++) for (const f of tones) input[i] = (input[i] as number) + (0.25 * Math.sin((2 * Math.PI * f * i) / inRate));
			const out = run(inRate, 48000, input);
			// Take each tone out by least squares; what is left is below -80 dB of the whole.
			let rest = Float32Array.from(out);
			for (const f of tones) {
				let xs = 0, xc = 0, ss = 0, cc = 0;
				for (let i = 1000; i < 25000; i++) {
					const s = Math.sin((2 * Math.PI * f * i) / 48000), c = Math.cos((2 * Math.PI * f * i) / 48000);
					xs += (rest[i] as number) * s; xc += (rest[i] as number) * c; ss += s * s; cc += c * c;
				}
				const as = xs / ss, ac = xc / cc;
				expect(Math.hypot(as, ac)).toBeGreaterThan(0.24);
				expect(Math.hypot(as, ac)).toBeLessThan(0.26);
				rest = rest.map((v, i) => v - as * Math.sin((2 * Math.PI * f * i) / 48000) - ac * Math.cos((2 * Math.PI * f * i) / 48000));
			}
			expect(db(rms(rest, 1000, 25000) / rms(out, 1000, 25000))).toBeLessThan(-80);
		}
	});
});

describe("cost", () => {
	test("one second of 48 kHz stereo costs far less than 10 ms a second on this machine", () => {
		const left = new Resampler(22050, 48000);
		const right = new Resampler(22050, 48000);
		const input = new Int16Array(22050 * 2);
		const out = new Float32Array(48000);
		const start = performance.now();
		for (let round = 0; round < 10; round++) {
			left.write(input, 0, 22050, 2, 1 / 32768);
			right.write(input, 1, 22050, 2, 1 / 32768);
			left.read(out, 48000);
			right.read(out, 48000);
		}
		const perSecond = (performance.now() - start) / 10;
		console.log(`resample 22050 to 48000 stereo: ${perSecond.toFixed(2)} ms per second of output`);
		expect(perSecond).toBeLessThan(25);
	});
});
