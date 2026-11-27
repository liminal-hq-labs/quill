// A minimal Intel ICH AC'97 audio controller with a plain AC'97 2.3 codec, as a pure register model (no emulator, no browser).
// Only the PCM-out channel plays; PCM-in and mic-in answer their registers and never run.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Written from the register tables in Intel's AC'97 2.3 and 82801AA (ICH) audio documents.

export const NAM_SIZE = 256; // the mixer's I/O BAR (the codec's registers, 16 bits at even offsets)
export const NABM_SIZE = 64; // the bus master's I/O BAR

// Status register bits of a bus-master channel.
export const SR = { DCH: 0x01, CELV: 0x02, LVBCI: 0x04, BCIS: 0x08, FIFOE: 0x10 };
// Control register bits.
export const CR = { RPBM: 0x01, RR: 0x02, LVBIE: 0x04, FEIE: 0x08, IOCE: 0x10 };
// Global status bits.
export const GS = { CODEC_READY: 0x100, PIINT: 0x20, POINT: 0x40, MINT: 0x80 };

const CHANNEL_BASE = [0x00, 0x10, 0x20]; // PCM in, PCM out, mic in
const PO = 1;

export class Ac97 {
	// readMem(address, length) returns a Uint8Array of guest physical memory; raiseIrq() and lowerIrq() drive the PCI interrupt.
	constructor({ readMem, raiseIrq = () => {}, lowerIrq = () => {}, trace = null }) {
		this.readMem = readMem;
		this.raiseIrq = raiseIrq;
		this.lowerIrq = lowerIrq;
		this.trace = trace;
		this.reset();
	}

	reset() {
		this.nam = new Uint16Array(NAM_SIZE / 2);
		this.resetCodec();
		this.ch = [0, 1, 2].map(() => this.newChannel());
		this.globCnt = 0;
		this.irqAsserted = false;
		this.carry = { l: new Float32Array(0), r: new Float32Array(0), frac: 0 };
		this.counters = { dmaFrames: 0, peakRaw: 0, buffersDone: 0, lastValidHalts: 0, iocs: 0, starvedFrames: 0, irqRaised: 0, bdlReads: 0 };
	}

	newChannel() {
		return { bdbar: 0, civ: 0, lvi: 0, sr: SR.DCH, picb: 0, piv: 0, cr: 0, addr: 0, len: 0, pos: 0, loaded: false };
	}

	resetCodec() {
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

	get vra() {
		return (this.nam[0x2a >> 1] & 1) === 1;
	}

	// The PCM-out sample rate the guest chose (48 kHz unless variable rate is on and the guest wrote another).
	get rate() {
		return this.vra ? this.nam[0x2c >> 1] || 48000 : 48000;
	}

	// Linear gains per side from the master and PCM-out volume registers (1.5 dB steps; PCM out has 12 dB of headroom that is not used).
	gains() {
		const side = (reg, shift, unity) => {
			const v = this.nam[reg >> 1];
			if (v & 0x8000) return 0;
			const step = (v >> shift) & 0x1f;
			return Math.pow(10, (-(step - unity) * 1.5) / 20);
		};
		const m = (s) => side(0x02, s, 0);
		const p = (s) => side(0x18, s, 8);
		return { l: Math.min(1, m(8) * p(8)), r: Math.min(1, m(0) * p(0)) };
	}

	// --- the mixer BAR -------------------------------------------------------------------------------------------------------

	namRead(off, size) {
		if (size === 1) {
			const w = this.namRead(off & ~1, 2);
			return (off & 1 ? w >> 8 : w) & 0xff;
		}
		if (size === 4) return (this.namRead(off, 2) | (this.namRead(off + 2, 2) << 16)) >>> 0;
		off &= 0xfe;
		let v = this.nam[off >> 1];
		if (off === 0x26) v = 0x000f | (v & 0xff00); // power-down status: ADC, DAC, analogue and reference ready
		if (off === 0x2a) v = (v & 1) | 0x0080; // extended status: VRA, and the front DAC ready
		this.log("nam-r", off, v);
		return v;
	}

	namWrite(off, size, value) {
		if (size === 1) {
			const old = this.nam[(off & ~1) >> 1];
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
				if (this.vra) this.nam[off >> 1] = value;
				return;
			default:
				this.nam[off >> 1] = value;
		}
	}

	// --- the bus master BAR --------------------------------------------------------------------------------------------------

	nabmRead(off, size) {
		let v = 0;
		for (let i = 0; i < size; i++) v |= this.nabmRead8(off + i) << (8 * i);
		this.log("nabm-r" + size, off, v >>> 0);
		return v >>> 0;
	}

	nabmWrite(off, size, value) {
		this.log("nabm-w" + size, off, value >>> 0);
		for (let i = 0; i < size; i++) this.nabmWrite8(off + i, (value >>> (8 * i)) & 0xff);
		this.updateIrq();
	}

	nabmRead8(off) {
		if (off >= 0x2c && off < 0x30) return (this.globCnt >>> (8 * (off - 0x2c))) & 0xff;
		if (off >= 0x30 && off < 0x34) return (this.globSta() >>> (8 * (off - 0x30))) & 0xff;
		if (off === 0x34) return 0; // codec access semaphore: always free
		const idx = CHANNEL_BASE.findIndex((b) => off >= b && off < b + 0x0c);
		if (idx < 0) return 0;
		const c = this.ch[idx];
		switch (off - CHANNEL_BASE[idx]) {
			case 0: case 1: case 2: case 3:
				return (c.bdbar >>> (8 * (off - CHANNEL_BASE[idx]))) & 0xff;
			case 4: return c.civ;
			case 5: return c.lvi;
			case 6: return c.sr & 0xff;
			case 7: return 0;
			case 8: return c.picb & 0xff;
			case 9: return (c.picb >> 8) & 0xff;
			case 10: return c.piv;
			case 11: return c.cr;
		}
		return 0;
	}

	nabmWrite8(off, value) {
		if (off >= 0x2c && off < 0x30) {
			const sh = 8 * (off - 0x2c);
			this.globCnt = ((this.globCnt & ~(0xff << sh)) | (value << sh)) >>> 0;
			return;
		}
		if (off >= 0x30 && off < 0x34) return; // global status: the write-one-to-clear bits are codec and GPI events this model never sets
		const idx = CHANNEL_BASE.findIndex((b) => off >= b && off < b + 0x0c);
		if (idx < 0) return;
		const c = this.ch[idx];
		switch (off - CHANNEL_BASE[idx]) {
			case 0: case 1: case 2: case 3: {
				const sh = 8 * (off - CHANNEL_BASE[idx]);
				c.bdbar = (((c.bdbar & ~(0xff << sh)) | (value << sh)) & ~7) >>> 0;
				break;
			}
			case 5:
				c.lvi = value & 0x1f;
				// Halted at the last valid buffer with the run bit still set: a new last valid index resumes at the next buffer.
				if (idx === PO && c.haltedAtLast && (c.cr & CR.RPBM)) {
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

	writeCr(idx, value) {
		const c = this.ch[idx];
		if (value & CR.RR) {
			Object.assign(c, this.newChannel(), { cr: 0 });
			if (idx === PO) this.carry = { l: new Float32Array(0), r: new Float32Array(0), frac: 0 };
			return;
		}
		const wasRunning = (c.cr & CR.RPBM) !== 0;
		c.cr = value & (CR.RPBM | CR.LVBIE | CR.FEIE | CR.IOCE);
		if ((c.cr & CR.RPBM) && !wasRunning) {
			c.sr &= ~SR.DCH;
			c.haltedAtLast = false;
			c.loaded = false;
		} else if (!(c.cr & CR.RPBM) && wasRunning) {
			c.sr |= SR.DCH;
		}
	}

	globSta() {
		let v = GS.CODEC_READY;
		const pending = (c) => ((c.sr & SR.BCIS) && (c.cr & CR.IOCE)) || ((c.sr & SR.LVBCI) && (c.cr & CR.LVBIE)) || ((c.sr & SR.FIFOE) && (c.cr & CR.FEIE));
		if (pending(this.ch[0])) v |= GS.PIINT;
		if (pending(this.ch[1])) v |= GS.POINT;
		if (pending(this.ch[2])) v |= GS.MINT;
		return v;
	}

	irqPending() {
		return (this.globSta() & (GS.PIINT | GS.POINT | GS.MINT)) !== 0;
	}

	updateIrq() {
		const p = this.irqPending();
		if (p && !this.irqAsserted) {
			this.irqAsserted = true;
			this.counters.irqRaised++;
			this.raiseIrq();
		} else if (!p && this.irqAsserted) {
			this.irqAsserted = false;
			this.lowerIrq();
		}
	}

	// --- DMA ----------------------------------------------------------------------------------------------------------------

	get running() {
		const c = this.ch[PO];
		return (c.cr & CR.RPBM) !== 0 && (c.sr & SR.DCH) === 0;
	}

	// Fetch the descriptor at the current index: 32-bit buffer address, then a word of control bits (31 = interrupt on completion, 30 = buffer underrun policy)
	// and the buffer's length in 16-bit samples.
	loadBuffer(c) {
		const bytes = this.readMem((c.bdbar + c.civ * 8) >>> 0, 8);
		const dv = new DataView(bytes.buffer, bytes.byteOffset, 8);
		this.counters.bdlReads++;
		c.addr = dv.getUint32(0, true) & ~1;
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

	finishBuffer(c) {
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

	// Take up to n stereo frames from the guest's buffers as floats in [-1, 1) (the raw sample values, before volume). Returns how many were produced.
	readFrames(n, outL, outR, at = 0) {
		const c = this.ch[PO];
		let made = 0;
		let guard = 64;
		while (made < n && this.running && guard-- > 0) {
			if (!c.loaded) this.loadBuffer(c);
			const frames = Math.min((c.len - c.pos) >> 1, n - made);
			if (frames > 0) {
				const b = this.readMem((c.addr + c.pos * 2) >>> 0, frames * 4);
				const s = new Int16Array(b.buffer.slice(b.byteOffset, b.byteOffset + frames * 4));
				for (let i = 0; i < frames; i++) {
					const l = s[2 * i] / 32768;
					const r = s[2 * i + 1] / 32768;
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

	// Fill outL and outR with n frames at outRate: the guest's stream resampled (linear) with the codec's volume applied; silence when the guest is not playing.
	// Returns the number of frames that came from the guest (the rest is silence).
	render(n, outRate, outL, outR) {
		const cr = this.carry;
		const step = this.rate / outRate;
		const g = this.gains();
		let real = 0;
		const need = Math.ceil(cr.frac + (n - 1) * step) + 2;
		if (cr.l.length < need && this.running) {
			const extra = need - cr.l.length;
			const l = new Float32Array(extra);
			const r = new Float32Array(extra);
			const got = this.readFrames(extra, l, r);
			if (got > 0) {
				const nl = new Float32Array(cr.l.length + got);
				const nr = new Float32Array(cr.l.length + got);
				nl.set(cr.l);
				nr.set(cr.r);
				nl.set(l.subarray(0, got), cr.l.length);
				nr.set(r.subarray(0, got), cr.l.length);
				cr.l = nl;
				cr.r = nr;
			}
		}
		let pos = cr.frac;
		for (let i = 0; i < n; i++) {
			const k = Math.floor(pos);
			if (k + 1 < cr.l.length) {
				const f = pos - k;
				outL[i] = (cr.l[k] * (1 - f) + cr.l[k + 1] * f) * g.l;
				outR[i] = (cr.r[k] * (1 - f) + cr.r[k + 1] * f) * g.r;
				real++;
				pos += step;
			} else {
				outL[i] = 0;
				outR[i] = 0;
				if (this.running) this.counters.starvedFrames++;
			}
		}
		const used = Math.min(Math.floor(pos), cr.l.length);
		cr.l = cr.l.slice(used);
		cr.r = cr.r.slice(used);
		cr.frac = pos - used;
		return real;
	}

	// --- diagnostics and the saved state ----------------------------------------------------------------------------------------

	log(kind, off, value) {
		if (this.trace) this.trace(kind, off, value);
	}

	getState() {
		return {
			nam: Array.from(this.nam),
			globCnt: this.globCnt,
			ch: this.ch.map((c) => ({ ...c })),
		};
	}

	setState(s) {
		this.nam = Uint16Array.from(s.nam);
		this.globCnt = s.globCnt;
		this.ch = s.ch.map((c) => ({ ...this.newChannel(), ...c }));
		this.carry = { l: new Float32Array(0), r: new Float32Array(0), frac: 0 };
		this.irqAsserted = false;
		this.updateIrq();
	}
}
