// The pointer's measure: the direction error of a guest pointer's path against the visitor's, over short windows of travel.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// A classic script (no build step): it sets globalThis.QuillPointerMetric, and Node's tests load it with `await import(...)`.
//
// directionError(input, output, options) compares two paths sampled at the same frames, as arrays of { t, x, y } in guest pixels
// (the two arrays are the same length and index-aligned). At each frame i it walks back to the latest earlier frame j from which the
// input's path was at least `window` guest pixels long, and takes the angle between the input's displacement from j to i and the
// output's over the same stretch (90 degrees when the output did not move). Windows that end before the output's first move are
// skipped, as its start-up, unless it never moves. A window is one axis only when the input's smaller component is at least `axisMin`
// guest pixels (2: whole-pixel output can be a pixel off at each end of a window) and the output's is zero. The output may lag the
// input by up to two frames (the guest draws a frame later): each lag in `lags` is tried and the one with the lowest median wins
// (medians within half a degree are a tie, broken by the lower mean). The endpoint is the output's last frame less the input's, so a
// run should end with a few frames of rest.
(function (root) {
	"use strict";

	function pathLengths(samples) {
		const lengths = [0];
		for (let i = 1; i < samples.length; i++) lengths.push(lengths[i - 1] + Math.hypot(samples[i].x - samples[i - 1].x, samples[i].y - samples[i - 1].y));
		return lengths;
	}

	function quantile(sorted, q) {
		return sorted.length ? sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))] : NaN;
	}

	function angleBetween(ax, ay, bx, by) {
		if (bx === 0 && by === 0) return 90;
		return (Math.abs(Math.atan2(ax * by - ay * bx, ax * bx + ay * by)) * 180) / Math.PI;
	}

	function scoreLag(input, output, inputLengths, lag, windowPixels, axisMin, started) {
		const errors = [];
		let axisOnly = 0;
		const last = input.length - 1 - lag;
		for (let i = 1; i <= last; i++) {
			let j = i - 1;
			while (j > 0 && inputLengths[i] - inputLengths[j] < windowPixels) j--;
			if (inputLengths[i] - inputLengths[j] < windowPixels) continue;
			// The guest draws a frame or two after the visitor starts: a window that ends before the output has moved at all is its
			// start-up, not its direction.
			if (i + lag < started) continue;
			const ax = input[i].x - input[j].x;
			const ay = input[i].y - input[j].y;
			const bx = output[i + lag].x - output[j + lag].x;
			const by = output[i + lag].y - output[j + lag].y;
			errors.push(angleBetween(ax, ay, bx, by));
			if (Math.min(Math.abs(ax), Math.abs(ay)) >= axisMin && Math.min(Math.abs(bx), Math.abs(by)) === 0) axisOnly++;
		}
		errors.sort((a, b) => a - b);
		return { lag, errors, axisOnly, median: quantile(errors, 0.5), mean: errors.reduce((sum, e) => sum + e, 0) / Math.max(1, errors.length) };
	}

	function directionError(input, output, options) {
		const opts = Object.assign({ window: 8, lags: [0, 1, 2], axisMin: 2 }, options);
		const count = Math.min(input.length, output.length);
		const empty = { windows: 0, median: NaN, p95: NaN, axisOnly: 0, endpoint: [0, 0], pathRatio: NaN, lag: 0 };
		if (count < 2) return empty;
		const inputs = input.slice(0, count);
		const inputLengths = pathLengths(inputs);
		// The first frame at which the output has moved; none when it never does (a frozen output scores 90 degrees throughout).
		let started = 0;
		while (started < count && output[started].x === output[0].x && output[started].y === output[0].y) started++;
		if (started >= count) started = 0;
		let best = null;
		for (const lag of opts.lags) {
			if (lag >= count - 1) continue;
			const scored = scoreLag(inputs, output, inputLengths, lag, opts.window, opts.axisMin, started);
			if (!scored.errors.length) continue;
			// The lowest median wins; medians within half a degree are a tie, broken by the lower mean (the right lag also drops the
			// windows where the output has not started yet), and a tie in that too goes to the smaller lag.
			const closer = !best || scored.median < best.median - 0.5 || (scored.median <= best.median + 0.5 && scored.mean < best.mean - 1e-6);
			if (closer) best = scored;
		}
		if (!best) return empty;
		const lag = best.lag;
		const end = count - 1 - lag;
		const outputLength = pathLengths(output.slice(lag, lag + end + 1))[end];
		return {
			windows: best.errors.length,
			median: best.median,
			p95: quantile(best.errors, 0.95),
			axisOnly: best.axisOnly / best.errors.length,
			endpoint: [output[count - 1].x - inputs[count - 1].x, output[count - 1].y - inputs[count - 1].y],
			pathRatio: inputLengths[end] > 0 ? outputLength / inputLengths[end] : NaN,
			lag,
		};
	}

	// The last sample at or before each time (the first sample for a time before them all); samples are sorted by t.
	function resample(samples, times) {
		const out = [];
		let at = 0;
		for (const time of times) {
			while (at + 1 < samples.length && samples[at + 1].t <= time) at++;
			out.push({ t: time, x: samples[at].x, y: samples[at].y });
		}
		return out;
	}

	// Splits a saved pointer trace (see QuillPointer's trace) into strokes at pauses of `pause` ms in the input, and scores each
	// one: the input's positions in guest pixels (the client position in Follow, the accumulated movement in Capture) against the
	// guest pointer's modelled position, both on a frame grid of `step` ms.
	function scoreTrace(trace, options) {
		const opts = Object.assign({ window: 8, pause: 250, step: 1000 / 60, minSamples: 8 }, options);
		const shown = trace.picture.shown;
		const guest = trace.picture.guest;
		const kx = guest.w / shown.w;
		const ky = guest.h / shown.h;
		const moves = trace.input.filter((e) => e.type === "pointermove" && e.pointerType !== "touch");
		const strokes = [];
		let stroke = null;
		let cx = 0;
		let cy = 0;
		for (const e of moves) {
			if (!stroke || e.t - stroke.last >= opts.pause) {
				stroke = { first: e.t, last: e.t, mode: trace.mode, samples: [] };
				strokes.push(stroke);
				cx = 0;
				cy = 0;
			}
			stroke.last = e.t;
			if (stroke.mode === "capture") {
				cx += (e.mx || 0) * kx;
				cy += (e.my || 0) * ky;
				stroke.samples.push({ t: e.t, x: cx, y: cy });
			} else stroke.samples.push({ t: e.t, x: (e.x - shown.left) * kx, y: (e.y - shown.top) * ky });
		}
		const outputs = trace.output.map((o) => ({ t: o.t, x: o.model[0], y: o.model[1] }));
		const results = [];
		for (const s of strokes) {
			if (s.samples.length < opts.minSamples || !outputs.length) continue;
			const times = [];
			for (let t = s.first; t <= s.last + opts.step * 2; t += opts.step) times.push(t);
			const before = outputs.filter((o) => o.t <= s.first);
			const series = (before.length ? [before[before.length - 1]] : [outputs[0]]).concat(outputs.filter((o) => o.t > s.first));
			const metric = directionError(resample(s.samples, times), resample(series, times), { window: opts.window });
			results.push({ start: s.first, end: s.last, mode: s.mode, samples: s.samples.length, metric });
		}
		return results;
	}

	root.QuillPointerMetric = { directionError, resample, scoreTrace };
})(globalThis);
