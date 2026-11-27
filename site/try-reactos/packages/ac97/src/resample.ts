// A band-limited polyphase resampler: a Kaiser-windowed sinc low-pass cut between the two rates' Nyquist limits, evaluated at the exact fractional phase
// of every output frame from a table made once per rate pair. A pure typed module: no browser, no emulator. One instance resamples one channel; two
// instances on the same rates share their table.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The rates the card offers (the AC'97 codec's variable-rate range is 8,000 to 48,000 Hz; these are the ones applications ask for). */
export const STANDARD_RATES = [8000, 11025, 16000, 22050, 32000, 44100, 48000] as const;
export const MIN_RATE = 8000;
export const MAX_RATE = 48000;

/** Clamps a rate a guest wrote to the range the card supports, as a whole number of frames per second. */
export function clampRate(rate: number): number {
	if (!Number.isFinite(rate)) return MAX_RATE;
	return Math.min(MAX_RATE, Math.max(MIN_RATE, Math.round(rate)));
}

export interface ResamplerOptions {
	/** Taps per output phase at the lower of the two rates (default 32); a downsampler uses proportionally more input samples. */
	taps?: number;
	/** Stopband attenuation of the window, in dB (default 90). */
	attenuationDb?: number;
	/** The edge of the flat passband as a fraction of the lower rate (default 0.3, which is 0.6 of its Nyquist frequency). */
	passband?: number;
}

/** The most phases a table has. Rate pairs that reduce to more (an odd rate) use this many and pick the nearest phase. */
const MAX_PHASES = 2048;

interface Table {
	phases: number;
	taps: number;
	coefs: Float32Array;
}

const tables = new Map<string, Table>();

function gcd(a: number, b: number): number {
	while (b) [a, b] = [b, a % b];
	return a;
}

/** The zeroth-order modified Bessel function of the first kind, by its power series. */
function bessel0(x: number): number {
	let sum = 1;
	let term = 1;
	const quarter = (x * x) / 4;
	for (let k = 1; k < 60; k++) {
		term *= quarter / (k * k);
		sum += term;
		if (term < sum * 1e-17) break;
	}
	return sum;
}

function kaiserBeta(attenuationDb: number): number {
	if (attenuationDb > 50) return 0.1102 * (attenuationDb - 8.7);
	if (attenuationDb > 21) return 0.5842 * Math.pow(attenuationDb - 21, 0.4) + 0.07886 * (attenuationDb - 21);
	return 0;
}

function buildTable(inRate: number, outRate: number, phases: number, options: Required<ResamplerOptions>): Table {
	const key = [inRate, outRate, phases, options.taps, options.attenuationDb, options.passband].join(":");
	const cached = tables.get(key);
	if (cached) return cached;
	// The cutoff in cycles per input sample. Below the input rate (a downsampler) it falls with the output rate and the kernel spans more input samples.
	const scale = Math.min(1, outRate / inRate);
	const transition = (options.attenuationDb - 7.95) / (14.36 * options.taps);
	const centre = Math.min(0.495, options.passband + transition / 2);
	const cutoff = centre * scale;
	const half = Math.ceil(options.taps / (2 * scale));
	const taps = half * 2;
	const beta = kaiserBeta(options.attenuationDb);
	const norm = bessel0(beta);
	const coefs = new Float32Array(phases * taps);
	const weights = new Float64Array(taps);
	for (let phase = 0; phase < phases; phase++) {
		const fraction = phase / phases;
		let sum = 0;
		for (let j = 0; j < taps; j++) {
			const x = j - half + 1 - fraction;
			const r = x / half;
			const window = Math.abs(r) < 1 ? bessel0(beta * Math.sqrt(1 - r * r)) / norm : 0;
			const arg = 2 * cutoff * x;
			const sinc = Math.abs(arg) < 1e-12 ? 1 : Math.sin(Math.PI * arg) / (Math.PI * arg);
			const weight = 2 * cutoff * sinc * window;
			weights[j] = weight;
			sum += weight;
		}
		// Every phase sums to one, so a constant stays constant whatever the fractional position.
		for (let j = 0; j < taps; j++) coefs[phase * taps + j] = (weights[j] ?? 0) / sum;
	}
	const table = { phases, taps, coefs };
	tables.set(key, table);
	return table;
}

/** The input samples a resampler reads, as 16-bit integers or floats. */
export type Samples = Int16Array | Float32Array;

export class Resampler {
	readonly inRate: number;
	readonly outRate: number;
	/** Phases in the table (the output rate over the greatest common divisor, unless that is more than 2,048). */
	readonly phases: number;
	/** Taps per phase. */
	readonly taps: number;
	/** Input samples the filter reads ahead of an output: a stream's first output waits for this many. */
	readonly lookahead: number;
	private readonly identity: boolean;
	private readonly half: number;
	private readonly coefs: Float32Array;
	// The step between outputs in table units (an exact integer when the rates reduce to a table of at most 2,048 phases).
	private readonly step: number;
	private buf: Float32Array;
	private len: number;
	private k: number;
	private f = 0;

	constructor(inRate: number, outRate: number, options: ResamplerOptions = {}) {
		this.inRate = inRate;
		this.outRate = outRate;
		this.identity = inRate === outRate;
		const settled = { taps: options.taps ?? 32, attenuationDb: options.attenuationDb ?? 90, passband: options.passband ?? 0.3 };
		const divisor = gcd(inRate, outRate);
		const exactPhases = outRate / divisor;
		if (this.identity) {
			this.phases = 1;
			this.taps = 1;
			this.half = 0;
			this.lookahead = 0;
			this.coefs = new Float32Array([1]);
			this.step = 1;
		} else {
			this.phases = Math.min(exactPhases, MAX_PHASES);
			this.step = this.phases === exactPhases ? inRate / divisor : (inRate / outRate) * this.phases;
			const table = buildTable(inRate, outRate, this.phases, settled);
			this.taps = table.taps;
			this.half = table.taps / 2;
			this.lookahead = this.half;
			this.coefs = table.coefs;
		}
		this.buf = new Float32Array(8192);
		// The filter looks back half - 1 samples from the first one: they start as silence.
		this.len = this.identity ? 0 : this.half - 1;
		this.k = this.len;
	}

	/** Forgets everything queued and starts a new stream. */
	reset(): void {
		this.len = this.identity ? 0 : this.half - 1;
		this.buf.fill(0, 0, this.len);
		this.k = this.len;
		this.f = 0;
	}

	/** Appends `count` samples of `src` from `start`, `stride` apart (2 for one side of interleaved stereo), multiplied by `scale`. */
	write(src: Samples, start: number, count: number, stride = 1, scale = 1): void {
		if (this.len + count > this.buf.length) {
			// Grows only when a caller queues far more than a render asks for; the render paths ask for what they need.
			const bigger = new Float32Array(Math.max(this.buf.length * 2, this.len + count));
			bigger.set(this.buf.subarray(0, this.len));
			this.buf = bigger;
		}
		const buf = this.buf;
		let at = this.len;
		for (let i = 0, from = start; i < count; i++, from += stride) buf[at++] = (src[from] ?? 0) * scale;
		this.len = at;
	}

	/** Appends `count` zeros (the tail of a stream that has ended, so its last samples come out). */
	writeSilence(count: number): void {
		if (this.len + count > this.buf.length) {
			const bigger = new Float32Array(Math.max(this.buf.length * 2, this.len + count));
			bigger.set(this.buf.subarray(0, this.len));
			this.buf = bigger;
		}
		this.buf.fill(0, this.len, this.len + count);
		this.len += count;
	}

	private ahead(frames: number): number {
		// How many input samples the output `frames` further on is past the current base sample, after rounding to the table's nearest phase.
		const end = this.f + (frames - 1) * this.step;
		let ahead = Math.floor(end / this.phases);
		if (this.step !== Math.floor(this.step) && Math.floor(end - ahead * this.phases + 0.5) >= this.phases) ahead++;
		return ahead;
	}

	/** How many more input samples must be written for `frames` outputs to be ready. */
	inputNeeded(frames: number): number {
		if (frames <= 0) return 0;
		if (this.identity) return Math.max(0, frames - (this.len - this.k));
		return Math.max(0, this.k + this.ahead(frames) + this.half + 1 - this.len);
	}

	/** How many outputs can be produced now. */
	available(): number {
		if (this.identity) return this.len - this.k;
		const room = this.len - this.k - this.half - 1;
		if (room < 0) return 0;
		let n = Math.max(0, Math.floor((room * this.phases - this.f) / this.step) + 1);
		// The formula is exact for a whole-number step; for the nearest-phase case settle the last one by asking inputNeeded.
		while (this.inputNeeded(n + 1) === 0) n++;
		while (n > 0 && this.inputNeeded(n) > 0) n--;
		return n;
	}

	/** Produces up to `count` outputs into `out` from `at`, consuming input. Returns how many it made (fewer when the input runs out). */
	read(out: Float32Array, count: number, at = 0): number {
		const buf = this.buf;
		let made = 0;
		if (this.identity) {
			const n = Math.min(count, this.len - this.k);
			for (let i = 0; i < n; i++) out[at + i] = buf[this.k + i] ?? 0;
			this.k += n;
			made = n;
		} else {
			const { phases, taps, half, coefs, step, len } = this;
			const exact = step === Math.floor(step);
			let k = this.k;
			let f = this.f;
			while (made < count) {
				let phase = f;
				let base = k;
				if (!exact) {
					phase = Math.floor(f + 0.5);
					if (phase >= phases) {
						phase -= phases;
						base++;
					}
				}
				if (base + half >= len) break;
				const from = base - half + 1;
				const row = phase * taps;
				let sum = 0;
				for (let j = 0; j < taps; j++) sum += (buf[from + j] as number) * (coefs[row + j] as number);
				out[at + made++] = sum;
				f += step;
				if (f >= phases) {
					const advance = Math.floor(f / phases);
					k += advance;
					f -= advance * phases;
				}
			}
			this.k = k;
			this.f = f;
		}
		this.compact();
		return made;
	}

	/** Drops the samples the filter will not read again. */
	private compact(): void {
		const keepFrom = this.identity ? this.k : this.k - this.half + 1;
		if (keepFrom < 4096) return;
		this.buf.copyWithin(0, keepFrom, this.len);
		this.len -= keepFrom;
		this.k -= keepFrom;
	}
}
