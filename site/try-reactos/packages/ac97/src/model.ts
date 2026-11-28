// A minimal Intel ICH AC'97 audio controller with a plain AC'97 2.3 codec, as a pure register model: no emulator, no browser.
// The PCM-out channel plays; the PCM-in and mic-in channels record whatever the page feeds them (`feedCapture`, or silence), so a recording application finds a working input.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Written from the register tables in Intel's AC'97 2.3 and 82801AA (ICH) audio documents.

import { Resampler, clampRate } from "./resample.ts";

/** The mixer's I/O BAR: the codec's registers, 16 bits at even offsets. */
export const NAM_SIZE = 256;
/** The bus master's I/O BAR. */
export const NABM_SIZE = 64;

/** Status register bits of a bus-master channel. */
export const SR = { DCH: 0x01, CELV: 0x02, LVBCI: 0x04, BCIS: 0x08, FIFOE: 0x10 } as const;
/** Control register bits. */
export const CR = { RPBM: 0x01, RR: 0x02, LVBIE: 0x04, FEIE: 0x08, IOCE: 0x10 } as const;
/** Global status bits. */
export const GS = { CODEC_READY: 0x100, PIINT: 0x20, POINT: 0x40, MINT: 0x80 } as const;

/** Channel base offsets in the bus master BAR: PCM in, PCM out, mic in. */
const CHANNEL_BASE = [0x00, 0x10, 0x20] as const;
const PI = 0;
const PO = 1;
const MC = 2;
const CHANNELS = 3;
const NAM_WORDS = NAM_SIZE / 2;
const SAVED_VERSION = 1;

/** One bus-master channel: its registers and the descriptor being played. */
export interface Channel {
	bdbar: number;
	civ: number;
	lvi: number;
	sr: number;
	picb: number;
	piv: number;
	cr: number;
	addr: number;
	len: number;
	pos: number;
	loaded: boolean;
	haltedAtLast: boolean;
	ioc: boolean;
	bup: boolean;
}

/** The device's registers as they are saved beside a machine state (`getState`, `setState`). JSON-safe. */
export interface Ac97State {
	version: 1;
	nam: number[];
	globCnt: number;
	ch: Channel[];
}

export interface Ac97Counters {
	/** Stereo frames the device took from guest memory. */
	dmaFrames: number;
	/** The largest raw sample taken, 0 to 1, before volume. */
	peakRaw: number;
	buffersDone: number;
	lastValidHalts: number;
	iocs: number;
	/** Output frames that were silence while the guest was playing (the guest was slower than the sink). */
	starvedFrames: number;
	irqRaised: number;
	bdlReads: number;
	/** Stereo (or mono, for the mic channel) frames the device wrote into guest memory while recording. */
	capFrames: number;
	/** Of those, frames of silence (no microphone was feeding the card). */
	capSilentFrames: number;
	/** Recorded frames thrown away because the guest did not take its buffers for a second. */
	capDropped: number;
	capBuffers: number;
}

export interface Ac97Options {
	/** Returns `length` bytes of guest physical memory at `address`. */
	readMem(address: number, length: number): Uint8Array;
	/** Writes `bytes` into guest physical memory at `address` (the recording channels need it; playback does not). */
	writeMem?: (address: number, bytes: Uint8Array) => void;
	raiseIrq?: () => void;
	lowerIrq?: () => void;
	/** Called for each register access: `kind` is "nam-r", "nam-w", "nabm-r1", "nabm-w4" and so on. */
	trace?: ((kind: string, offset: number, value: number) => void) | null;
}

function newChannel(): Channel {
	return { bdbar: 0, civ: 0, lvi: 0, sr: SR.DCH, picb: 0, piv: 0, cr: 0, addr: 0, len: 0, pos: 0, loaded: false, haltedAtLast: false, ioc: false, bup: false };
}

function newCounters(): Ac97Counters {
	return { dmaFrames: 0, peakRaw: 0, buffersDone: 0, lastValidHalts: 0, iocs: 0, starvedFrames: 0, irqRaised: 0, bdlReads: 0, capFrames: 0, capSilentFrames: 0, capDropped: 0, capBuffers: 0 };
}

/** Whether `value` has the shape `getState` writes. A state that fails this must not reach `setState`. */
export function isAc97State(value: unknown): value is Ac97State {
	if (typeof value !== "object" || value === null) return false;
	const s = value as Partial<Ac97State>;
	if (s.version !== SAVED_VERSION) return false;
	if (!Array.isArray(s.nam) || s.nam.length !== NAM_WORDS || !s.nam.every((w) => Number.isInteger(w) && w >= 0 && w <= 0xffff)) return false;
	if (!Number.isInteger(s.globCnt)) return false;
	if (!Array.isArray(s.ch) || s.ch.length !== CHANNELS) return false;
	return s.ch.every((c) => typeof c === "object" && c !== null && Number.isInteger((c as Channel).bdbar) && Number.isInteger((c as Channel).civ) && Number.isInteger((c as Channel).lvi) && Number.isInteger((c as Channel).sr) && Number.isInteger((c as Channel).cr));
}

interface PlayPath {
	inRate: number;
	outRate: number;
	l: Resampler;
	r: Resampler;
	/** Guest samples are in the resamplers that have not come out yet. */
	tail: boolean;
	live: boolean;
}

/** The recording side: mic samples are resampled to the guest's record rate and queued until the guest's buffers can take them. */
interface CapturePath {
	queue: Float32Array;
	len: number;
	resampler: Resampler | null;
	micRate: number;
	adcRate: number;
	scratch: Float32Array;
	pcm: Int16Array;
}

/** A second of 48 kHz frames: the most a recording queues before it drops. */
const CAPTURE_QUEUE = 48000;

function newCapture(): CapturePath {
	return { queue: new Float32Array(CAPTURE_QUEUE), len: 0, resampler: null, micRate: 0, adcRate: 0, scratch: new Float32Array(0), pcm: new Int16Array(0) };
}

export class Ac97 {
	private readonly readMem: (address: number, length: number) => Uint8Array;
	private readonly raiseIrq: () => void;
	private readonly lowerIrq: () => void;
	private readonly writeMem: ((address: number, bytes: Uint8Array) => void) | null;
	private readonly trace: ((kind: string, offset: number, value: number) => void) | null;
	private nam = new Uint16Array(NAM_WORDS);
	private ch: Channel[] = [];
	private globCnt = 0;
	private irqAsserted = false;
	private play: PlayPath | null = null;
	private playScratchL = new Float32Array(0);
	private playScratchR = new Float32Array(0);
	private capture: CapturePath = newCapture();
	counters: Ac97Counters = newCounters();

	constructor({ readMem, writeMem, raiseIrq = () => {}, lowerIrq = () => {}, trace = null }: Ac97Options) {
		this.readMem = readMem;
		this.writeMem = writeMem ?? null;
		this.raiseIrq = raiseIrq;
		this.lowerIrq = lowerIrq;
		this.trace = trace;
		this.reset();
	}

	reset(): void {
		this.resetCodec();
		this.ch = Array.from({ length: CHANNELS }, newChannel);
		this.globCnt = 0;
		this.irqAsserted = false;
		this.play = null;
		this.capture = newCapture();
		this.counters = newCounters();
	}

	private word(index: number): number {
		return this.nam[index] ?? 0;
	}

	private channel(index: number): Channel {
		const c = this.ch[index];
		if (!c) throw new RangeError("no channel " + index);
		return c;
	}

	private resetCodec(): void {
		const n = this.nam;
		n.fill(0);
		n[0x00 >> 1] = 0x0140; // reset register: 18-bit DAC and ADC, headphone out, no 3D
		n[0x02 >> 1] = 0x8000; // master volume: muted, 0 dB
		n[0x04 >> 1] = 0x8000;
		n[0x06 >> 1] = 0x8000;
		n[0x0a >> 1] = 0x0000;
		n[0x0c >> 1] = 0x8008;
		n[0x0e >> 1] = 0x8008;
		n[0x10 >> 1] = 0x8808;
		n[0x12 >> 1] = 0x8808;
		n[0x14 >> 1] = 0x8808;
		n[0x16 >> 1] = 0x8808;
		n[0x18 >> 1] = 0x8808; // PCM out volume: muted, 0 dB
		n[0x1a >> 1] = 0x0000;
		n[0x1c >> 1] = 0x8000;
		n[0x1e >> 1] = 0x8000;
		n[0x20 >> 1] = 0x0000;
		n[0x28 >> 1] = 0x0001; // extended audio ID: variable rate audio
		n[0x2a >> 1] = 0x0000; // extended audio control: VRA off
		n[0x2c >> 1] = 48000; // front DAC rate
		n[0x32 >> 1] = 48000; // ADC rate
		n[0x7c >> 1] = 0x8384; // vendor ID 1 and 2: SigmaTel STAC9700
		n[0x7e >> 1] = 0x7600;
	}

	get vra(): boolean {
		return (this.word(0x2a >> 1) & 1) === 1;
	}

	/** The PCM-out sample rate the guest chose: 48 kHz unless variable rate is on and the guest wrote another. */
	get rate(): number {
		return this.vra ? this.word(0x2c >> 1) || 48000 : 48000;
	}

	/** Linear gains per side from the master and PCM-out volume registers (1.5 dB steps; PCM out has 12 dB of headroom that is not used). */
	gains(): { l: number; r: number } {
		const side = (reg: number, shift: number, unity: number): number => {
			const v = this.word(reg >> 1);
			if (v & 0x8000) return 0;
			const step = (v >> shift) & 0x1f;
			return Math.pow(10, (-(step - unity) * 1.5) / 20);
		};
		const master = (shift: number): number => side(0x02, shift, 0);
		const pcm = (shift: number): number => side(0x18, shift, 8);
		return { l: Math.min(1, master(8) * pcm(8)), r: Math.min(1, master(0) * pcm(0)) };
	}

	// --- the mixer BAR -------------------------------------------------------------------------------------------------------

	namRead(off: number, size: 1 | 2 | 4): number {
		if (size === 1) {
			const w = this.namRead(off & ~1, 2);
			return (off & 1 ? w >> 8 : w) & 0xff;
		}
		if (size === 4) return (this.namRead(off, 2) | (this.namRead(off + 2, 2) << 16)) >>> 0;
		off &= 0xfe;
		let v = this.word(off >> 1);
		if (off === 0x26) v = 0x000f | (v & 0xff00); // power-down status: ADC, DAC, analogue and reference ready
		if (off === 0x2a) v = (v & 1) | 0x0080; // extended status: VRA, and the front DAC ready
		this.log("nam-r", off, v);
		return v;
	}

	namWrite(off: number, size: 1 | 2 | 4, value: number): void {
		if (size === 1) {
			const old = this.word((off & ~1) >> 1);
			this.namWrite(off & ~1, 2, off & 1 ? (old & 0xff) | ((value & 0xff) << 8) : (old & 0xff00) | (value & 0xff));
			return;
		}
		if (size === 4) {
			this.namWrite(off, 2, value & 0xffff);
			this.namWrite(off + 2, 2, value >>> 16);
			return;
		}
		off &= 0xfe;
		value &= 0xffff;
		this.log("nam-w", off, value);
		switch (off) {
			case 0x00:
				this.resetCodec();
				return;
			case 0x28:
			case 0x7c:
			case 0x7e:
				return; // read-only
			case 0x2a:
				this.nam[off >> 1] = value & 1;
				return;
			case 0x2c:
			case 0x32:
				if (this.vra) this.nam[off >> 1] = clampRate(value);
				return;
			default:
				this.nam[off >> 1] = value;
		}
	}

	// --- the bus master BAR --------------------------------------------------------------------------------------------------

	nabmRead(off: number, size: 1 | 2 | 4): number {
		let v = 0;
		for (let i = 0; i < size; i++) v |= this.nabmRead8(off + i) << (8 * i);
		this.log("nabm-r" + size, off, v >>> 0);
		return v >>> 0;
	}

	nabmWrite(off: number, size: 1 | 2 | 4, value: number): void {
		this.log("nabm-w" + size, off, value >>> 0);
		for (let i = 0; i < size; i++) this.nabmWrite8(off + i, (value >>> (8 * i)) & 0xff);
		this.updateIrq();
	}

	private channelAt(off: number): number {
		return CHANNEL_BASE.findIndex((base) => off >= base && off < base + 0x0c);
	}

	private nabmRead8(off: number): number {
		if (off >= 0x2c && off < 0x30) return (this.globCnt >>> (8 * (off - 0x2c))) & 0xff;
		if (off >= 0x30 && off < 0x34) return (this.globSta() >>> (8 * (off - 0x30))) & 0xff;
		if (off === 0x34) return 0; // codec access semaphore: always free
		const idx = this.channelAt(off);
		if (idx < 0) return 0;
		const c = this.channel(idx);
		const reg = off - (CHANNEL_BASE[idx] ?? 0);
		switch (reg) {
			case 0:
			case 1:
			case 2:
			case 3:
				return (c.bdbar >>> (8 * reg)) & 0xff;
			case 4:
				return c.civ;
			case 5:
				return c.lvi;
			case 6:
				return c.sr & 0xff;
			case 7:
				return 0;
			case 8:
				return c.picb & 0xff;
			case 9:
				return (c.picb >> 8) & 0xff;
			case 10:
				return c.piv;
			case 11:
				return c.cr;
		}
		return 0;
	}

	private nabmWrite8(off: number, value: number): void {
		if (off >= 0x2c && off < 0x30) {
			const sh = 8 * (off - 0x2c);
			this.globCnt = ((this.globCnt & ~(0xff << sh)) | (value << sh)) >>> 0;
			return;
		}
		if (off >= 0x30 && off < 0x34) return; // global status: the write-one-to-clear bits are codec and GPI events this model never sets
		const idx = this.channelAt(off);
		if (idx < 0) return;
		const c = this.channel(idx);
		const reg = off - (CHANNEL_BASE[idx] ?? 0);
		switch (reg) {
			case 0:
			case 1:
			case 2:
			case 3: {
				const sh = 8 * reg;
				c.bdbar = (((c.bdbar & ~(0xff << sh)) | (value << sh)) & ~7) >>> 0;
				break;
			}
			case 5:
				c.lvi = value & 0x1f;
				// Halted at the last valid buffer with the run bit still set: a new last valid index resumes at the next buffer.
				if (c.haltedAtLast && c.cr & CR.RPBM) {
					c.haltedAtLast = false;
					c.sr &= ~(SR.DCH | SR.CELV);
					c.civ = (c.civ + 1) & 31;
					c.loaded = false;
				}
				break;
			case 6:
				c.sr &= ~(value & (SR.LVBCI | SR.BCIS | SR.FIFOE));
				break;
			case 11:
				this.writeCr(idx, value);
				break;
		}
	}

	private writeCr(idx: number, value: number): void {
		const c = this.channel(idx);
		if (value & CR.RR) {
			this.ch[idx] = newChannel();
			if (idx === PO) this.play = null;
			else this.capture = newCapture();
			return;
		}
		const wasRunning = (c.cr & CR.RPBM) !== 0;
		c.cr = value & (CR.RPBM | CR.LVBIE | CR.FEIE | CR.IOCE);
		if (c.cr & CR.RPBM && !wasRunning) {
			c.sr &= ~SR.DCH;
			c.haltedAtLast = false;
			c.loaded = false;
		} else if (!(c.cr & CR.RPBM) && wasRunning) {
			c.sr |= SR.DCH;
		}
	}

	private globSta(): number {
		let v: number = GS.CODEC_READY;
		const pending = (c: Channel): boolean => Boolean(c.sr & SR.BCIS && c.cr & CR.IOCE) || Boolean(c.sr & SR.LVBCI && c.cr & CR.LVBIE) || Boolean(c.sr & SR.FIFOE && c.cr & CR.FEIE);
		if (pending(this.channel(0))) v |= GS.PIINT;
		if (pending(this.channel(1))) v |= GS.POINT;
		if (pending(this.channel(2))) v |= GS.MINT;
		return v;
	}

	private irqPending(): boolean {
		return (this.globSta() & (GS.PIINT | GS.POINT | GS.MINT)) !== 0;
	}

	private updateIrq(): void {
		const pending = this.irqPending();
		if (pending && !this.irqAsserted) {
			this.irqAsserted = true;
			this.counters.irqRaised++;
			this.raiseIrq();
		} else if (!pending && this.irqAsserted) {
			this.irqAsserted = false;
			this.lowerIrq();
		}
	}

	/** Whether the device holds its interrupt line up now (the guest has not yet cleared the status that raised it). */
	get interruptAsserted(): boolean {
		return this.irqAsserted;
	}

	/** Drops the interrupt line if the device holds it (the page does this when it lets the machine go). */
	releaseIrq(): void {
		if (this.irqAsserted) {
			this.irqAsserted = false;
			this.lowerIrq();
		}
	}

	// --- DMA ----------------------------------------------------------------------------------------------------------------

	/** Whether the PCM-out channel is running (run bit set and not halted). */
	get running(): boolean {
		return this.channelRunning(PO);
	}

	private channelRunning(index: number): boolean {
		const c = this.channel(index);
		return (c.cr & CR.RPBM) !== 0 && (c.sr & SR.DCH) === 0;
	}

	/** Whether the guest is recording (the PCM-in or the mic-in channel is running). */
	get recording(): boolean {
		return this.channelRunning(PI) || this.channelRunning(MC);
	}

	/** The rate the guest records at: the codec's ADC rate when variable rate is on, otherwise 48 kHz. */
	get adcRate(): number {
		return this.vra ? clampRate(this.word(0x32 >> 1) || 48000) : 48000;
	}

	/** Linear record gains per side from the record gain register (0 to 22.5 dB in 1.5 dB steps; bit 15 mutes). */
	recordGains(): { l: number; r: number } {
		const v = this.word(0x1c >> 1);
		if (v & 0x8000) return { l: 0, r: 0 };
		const side = (step: number): number => Math.pow(10, (step * 1.5) / 20);
		return { l: side((v >> 8) & 0xf), r: side(v & 0xf) };
	}

	/** Whether the record select register chooses the microphone for each side (the other sources, line in and the mixes, are silent here). */
	private recordSources(): { l: boolean; r: boolean } {
		const v = this.word(0x1a >> 1);
		return { l: ((v >> 8) & 7) === 0, r: (v & 7) === 0 };
	}

	/**
	 * Fetches the descriptor at the current index: a 32-bit buffer address, then a word of control bits (31 = interrupt on completion,
	 * 30 = buffer underrun policy) and the buffer's length in 16-bit samples.
	 */
	private loadBuffer(c: Channel): void {
		const bytes = this.readMem((c.bdbar + c.civ * 8) >>> 0, 8);
		const dv = new DataView(bytes.buffer, bytes.byteOffset, 8);
		this.counters.bdlReads++;
		c.addr = (dv.getUint32(0, true) & ~1) >>> 0;
		const ctl = dv.getUint32(4, true);
		c.len = ctl & 0xffff;
		c.ioc = (ctl & 0x80000000) !== 0;
		c.bup = (ctl & 0x40000000) !== 0;
		c.pos = 0;
		c.picb = c.len;
		c.piv = (c.civ + 1) & 31;
		c.loaded = true;
		if (c.civ === c.lvi) c.sr |= SR.CELV;
		else c.sr &= ~SR.CELV;
	}

	private finishBuffer(c: Channel): void {
		this.counters.buffersDone++;
		if (c.ioc) {
			c.sr |= SR.BCIS;
			this.counters.iocs++;
		}
		if (c.civ === c.lvi) {
			c.sr |= SR.LVBCI | SR.DCH | SR.CELV;
			this.counters.lastValidHalts++;
			c.haltedAtLast = true;
			c.loaded = false;
		} else {
			c.civ = (c.civ + 1) & 31;
			c.loaded = false;
		}
	}

	/**
	 * Takes up to `n` stereo frames from the guest's buffers as floats in [-1, 1) (the raw sample values, before volume). Returns how many.
	 *
	 * With `hold`, a buffer that completes with an interrupt the guest has not yet taken stops the read until the guest clears it. The guest here
	 * runs at a few million instructions a second and reaches its handler late, so two completions can merge into one interrupt, and ac97.sys counts
	 * interrupts, not buffers: it then feeds stale data and the sound repeats (2.7 times as long for Critical Stop). Real-time output does not hold
	 * (a hold would slow and gap the sound a listener hears); output nobody hears does, which keeps the guest's own sound flow whole.
	 */
	readFrames(n: number, outL: Float32Array, outR: Float32Array, at = 0, hold = false): number {
		const c = this.channel(PO);
		let made = 0;
		let guard = 64;
		while (made < n && this.running && guard-- > 0) {
			if (!c.loaded) {
				if (hold && c.sr & SR.BCIS && c.cr & CR.IOCE) break;
				this.loadBuffer(c);
			}
			const frames = Math.min((c.len - c.pos) >> 1, n - made);
			if (frames > 0) {
				const bytes = this.readMem((c.addr + c.pos * 2) >>> 0, frames * 4);
				const s = new Int16Array(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + frames * 4));
				for (let i = 0; i < frames; i++) {
					const l = (s[2 * i] ?? 0) / 32768;
					const r = (s[2 * i + 1] ?? 0) / 32768;
					outL[at + made + i] = l;
					outR[at + made + i] = r;
					const m = Math.max(Math.abs(l), Math.abs(r));
					if (m > this.counters.peakRaw) this.counters.peakRaw = m;
				}
				c.pos += frames * 2;
				c.picb = c.len - c.pos;
				made += frames;
				this.counters.dmaFrames += frames;
			}
			if (c.len - c.pos < 2) {
				c.picb = 0;
				this.finishBuffer(c);
			}
		}
		this.updateIrq();
		return made;
	}

	/**
	 * Fills `outL` and `outR` with `n` frames at `outRate`: the guest's stream resampled (linear) with the codec's volume applied; silence
	 * when the guest is not playing. Returns the number of frames that came from the guest (the rest is silence). `hold` is `readFrames`'s.
	 */
	render(n: number, outRate: number, outL: Float32Array, outR: Float32Array, hold = false): number {
		const inRate = this.rate;
		let path = this.play;
		if (!path || path.inRate !== inRate || path.outRate !== outRate) {
			path = this.play = { inRate, outRate, l: new Resampler(inRate, outRate), r: new Resampler(inRate, outRate), tail: false, live: false };
		}
		const need = path.l.inputNeeded(n);
		if (need > 0) {
			if (this.running) {
				if (this.playScratchL.length < need) {
					this.playScratchL = new Float32Array(need);
					this.playScratchR = new Float32Array(need);
				}
				const got = this.readFrames(need, this.playScratchL, this.playScratchR, 0, hold);
				if (got > 0) {
					path.l.write(this.playScratchL, 0, got);
					path.r.write(this.playScratchR, 0, got);
					path.tail = true;
					path.live = true;
				}
			} else if (path.tail) {
				// The guest has stopped: let the filter's last samples out with silence behind them.
				path.l.writeSilence(need);
				path.r.writeSilence(need);
				path.tail = false;
			}
		}
		const made = path.l.read(outL, n);
		path.r.read(outR, n);
		const g = this.gains();
		for (let i = 0; i < made; i++) {
			outL[i] = (outL[i] as number) * g.l;
			outR[i] = (outR[i] as number) * g.r;
		}
		for (let i = made; i < n; i++) {
			outL[i] = 0;
			outR[i] = 0;
		}
		if (this.running && made < n && !hold) this.counters.starvedFrames += n - made;
		const real = path.live ? made : 0;
		if (!this.running && !path.tail) path.live = false;
		return real;
	}

	// --- recording ---------------------------------------------------------------------------------------------------------

	/**
	 * Feeds the microphone: `count` mono samples at `micRate` (floats in [-1, 1]). They are resampled to the guest's record rate and written into the
	 * guest's buffers as they have room. Dropped when the guest is not recording, so nothing is kept that nobody asked for.
	 */
	feedCapture(samples: Float32Array, count: number, micRate: number): void {
		const cap = this.capture;
		if (!this.recording) {
			if (cap.resampler) cap.resampler.reset();
			cap.len = 0;
			return;
		}
		const adc = this.adcRate;
		if (!cap.resampler || cap.micRate !== micRate || cap.adcRate !== adc) {
			cap.resampler = new Resampler(micRate, adc);
			cap.micRate = micRate;
			cap.adcRate = adc;
		}
		cap.resampler.write(samples, 0, count);
		const ready = cap.resampler.available();
		if (ready > 0) {
			if (cap.scratch.length < ready) cap.scratch = new Float32Array(ready);
			const made = cap.resampler.read(cap.scratch, ready);
			this.queueCapture(cap.scratch, made, false);
		}
		this.drainCapture();
	}

	/** Feeds `frames` of silence at the guest's record rate: what the guest records when no microphone is on. */
	feedSilence(frames: number): void {
		if (!this.recording || frames <= 0) return;
		const cap = this.capture;
		if (cap.scratch.length < frames) cap.scratch = new Float32Array(frames);
		cap.scratch.fill(0, 0, frames);
		this.queueCapture(cap.scratch, frames, true);
		this.drainCapture();
	}

	private queueCapture(src: Float32Array, count: number, silent: boolean): void {
		const cap = this.capture;
		let take = count;
		let from = 0;
		if (take > CAPTURE_QUEUE) {
			from = take - CAPTURE_QUEUE;
			this.counters.capDropped += from;
			take = CAPTURE_QUEUE;
		}
		const over = cap.len + take - CAPTURE_QUEUE;
		if (over > 0) {
			cap.queue.copyWithin(0, over, cap.len);
			cap.len -= over;
			this.counters.capDropped += over;
		}
		cap.queue.set(src.subarray(from, from + take), cap.len);
		cap.len += take;
		if (silent) this.counters.capSilentFrames += take;
	}

	/** Writes queued frames into the guest's current record buffer, a buffer at a time, waiting for the guest to take each buffer's interrupt. */
	private drainCapture(): void {
		const cap = this.capture;
		const index = this.channelRunning(PI) ? PI : this.channelRunning(MC) ? MC : -1;
		if (index < 0 || !this.writeMem) return;
		const c = this.channel(index);
		const mono = index === MC;
		const per = mono ? 1 : 2;
		const gain = this.recordGains();
		const source = this.recordSources();
		let head = 0;
		let guard = 64;
		while (head < cap.len && this.channelRunning(index) && guard-- > 0) {
			if (!c.loaded) {
				// The same wait as the playback path's hold: two buffers completing before one interrupt is taken merge into one.
				if (c.sr & SR.BCIS && c.cr & CR.IOCE) break;
				this.loadBuffer(c);
			}
			const frames = Math.min(Math.floor((c.len - c.pos) / per), cap.len - head);
			if (frames > 0) {
				if (cap.pcm.length < frames * per) cap.pcm = new Int16Array(frames * per);
				for (let i = 0; i < frames; i++) {
					const x = cap.queue[head + i] as number;
					const l = source.l ? Math.max(-1, Math.min(1, x * gain.l)) : 0;
					const r = source.r ? Math.max(-1, Math.min(1, x * gain.r)) : 0;
					if (mono) cap.pcm[i] = Math.round(l * 32767);
					else {
						cap.pcm[2 * i] = Math.round(l * 32767);
						cap.pcm[2 * i + 1] = Math.round(r * 32767);
					}
				}
				const bytes = new Uint8Array(cap.pcm.buffer, 0, frames * per * 2);
				this.writeMem((c.addr + c.pos * 2) >>> 0, bytes);
				c.pos += frames * per;
				c.picb = c.len - c.pos;
				head += frames;
				this.counters.capFrames += frames;
			}
			if (c.len - c.pos < per) {
				c.picb = 0;
				this.counters.capBuffers++;
				this.finishBuffer(c);
			}
		}
		if (head > 0) {
			cap.queue.copyWithin(0, head, cap.len);
			cap.len -= head;
		}
		this.updateIrq();
	}

	// --- diagnostics and the saved state ----------------------------------------------------------------------------------------

	private log(kind: string, off: number, value: number): void {
		if (this.trace) this.trace(kind, off, value);
	}

	/** The registers, for saving beside a machine state. Save while no sound plays: the channel is halted and only registers matter. */
	getState(): Ac97State {
		return { version: SAVED_VERSION, nam: Array.from(this.nam), globCnt: this.globCnt, ch: this.ch.map((c) => ({ ...c })) };
	}

	/** Restores registers saved by `getState`. Throws on a state of another shape, leaving the device as it was. */
	setState(state: Ac97State): void {
		if (!isAc97State(state)) throw new TypeError("not a saved AC'97 state");
		this.nam = Uint16Array.from(state.nam);
		this.globCnt = state.globCnt;
		this.ch = state.ch.map((c) => ({ ...newChannel(), ...c }));
		this.play = null;
		this.capture = newCapture();
		this.irqAsserted = false;
		this.updateIrq();
	}
}
