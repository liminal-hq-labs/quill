// The audio output for the AC'97 spike: an AudioWorklet that plays a ring buffer the page tops up, and counts what it plays.
// The worklet is also the capture point: it measures the samples that go to the output (count, peak, non-silent frames, underruns)
// and can keep a copy, so a run with no sound card can still prove what a speaker would have received.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const WORKLET = `
class RingPlayer extends AudioWorkletProcessor {
	constructor() {
		super();
		this.cap = 96000;
		this.l = new Float32Array(this.cap);
		this.r = new Float32Array(this.cap);
		this.rd = 0;
		this.wr = 0;
		this.n = 0;
		this.primed = false;
		this.starving = false;
		this.s = { out: 0, silent: 0, peak: 0, underrunEvents: 0, underrunFrames: 0, dropped: 0, quanta: 0 };
		this.rec = null;
		this.tick = 0;
		this.port.onmessage = (e) => {
			const m = e.data;
			if (m.t === "push") {
				const len = m.l.length;
				for (let i = 0; i < len; i++) {
					if (this.n >= this.cap) { this.s.dropped += len - i; break; }
					this.l[this.wr] = m.l[i];
					this.r[this.wr] = m.r[i];
					this.wr = (this.wr + 1) % this.cap;
					this.n++;
				}
				this.primed = true;
			} else if (m.t === "rec") {
				this.rec = m.on ? { l: [], r: [], frames: 0, max: m.max || 48000 * 60 } : null;
			} else if (m.t === "take") {
				const r = this.rec;
				this.rec = null;
				this.port.postMessage({ t: "rec", l: r ? r.l : [], r: r ? r.r : [] });
			} else if (m.t === "stats") {
				this.port.postMessage({ t: "stats", s: this.s, n: this.n });
			}
		};
	}
	process(inputs, outputs) {
		const o = outputs[0];
		const len = o[0].length;
		const ol = o[0];
		const or = o[1] || o[0];
		let got = 0;
		const have = Math.min(this.n, len);
		for (let i = 0; i < have; i++) {
			ol[i] = this.l[this.rd];
			or[i] = this.r[this.rd];
			this.rd = (this.rd + 1) % this.cap;
			got++;
		}
		this.n -= have;
		for (let i = have; i < len; i++) { ol[i] = 0; or[i] = 0; }
		if (this.primed && have < len) {
			if (!this.starving) this.s.underrunEvents++;
			this.starving = true;
			this.s.underrunFrames += len - have;
		} else if (have === len) {
			this.starving = false;
		}
		const st = this.s;
		st.quanta++;
		for (let i = 0; i < len; i++) {
			const a = Math.max(Math.abs(ol[i]), Math.abs(or[i]));
			if (a > st.peak) st.peak = a;
			if (a < 0.0001) st.silent++;
		}
		st.out += len;
		if (this.rec && this.rec.frames < this.rec.max) {
			this.rec.l.push(ol.slice(0));
			this.rec.r.push(or.slice(0));
			this.rec.frames += len;
		}
		if (++this.tick % 4 === 0) this.port.postMessage({ t: "level", n: this.n });
		return true;
	}
}
registerProcessor("ring-player", RingPlayer);
`;

export const SINK_RATE = 48000;

export class Sink {
	constructor({ targetMs = 120 } = {}) {
		this.target = Math.round((SINK_RATE * targetMs) / 1000);
		this.ctx = null;
		this.node = null;
		this.gain = null;
		this.onNeed = null; // (frames) => {l: Float32Array, r: Float32Array} or null
		this.level = 0;
		this.pending = [];
		this.pushedFrames = 0;
		this.lastStats = null;
	}

	// Call from a user gesture in a real browser; the spike's harness runs with the autoplay policy relaxed.
	async start() {
		const ctx = new AudioContext({ sampleRate: SINK_RATE, latencyHint: "interactive" });
		this.ctx = ctx;
		const url = URL.createObjectURL(new Blob([WORKLET], { type: "text/javascript" }));
		await ctx.audioWorklet.addModule(url);
		URL.revokeObjectURL(url);
		const node = new AudioWorkletNode(ctx, "ring-player", { numberOfInputs: 0, numberOfOutputs: 1, outputChannelCount: [2] });
		this.node = node;
		this.gain = ctx.createGain();
		node.connect(this.gain).connect(ctx.destination);
		node.port.onmessage = (e) => {
			const m = e.data;
			if (m.t === "level") {
				this.level = m.n;
				this.top();
			} else if (m.t === "stats") {
				this.lastStats = m;
				this.pending.splice(0).forEach((f) => f(m));
			} else if (m.t === "rec") {
				this.pending.splice(0).forEach((f) => f(m));
			}
		};
		if (ctx.state !== "running") await ctx.resume();
		return ctx.state;
	}

	top() {
		const need = this.target - this.level;
		if (need < 256 || !this.onNeed) return;
		const out = this.onNeed(need);
		if (!out) return;
		this.level += need;
		this.pushedFrames += need;
		this.node.port.postMessage({ t: "push", l: out.l, r: out.r }, [out.l.buffer, out.r.buffer]);
	}

	ask(msg) {
		return new Promise((resolve) => {
			this.pending.push(resolve);
			this.node.port.postMessage(msg);
		});
	}

	stats() {
		return this.ask({ t: "stats" });
	}

	record(on) {
		this.node.port.postMessage({ t: "rec", on });
	}

	take() {
		return this.ask({ t: "take" });
	}
}
