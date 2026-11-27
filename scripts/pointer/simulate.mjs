// Level 1 of the pointer's tests: the core's counts for a generated hand path, applied to a stand-in guest, scored by the metric.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { FRAME_MS, StandInGuest, centredOrigin, handPath, INPUT_MODELS } from "./paths.mjs";

await import("../../site/js/pointer.js");
await import("../../site/js/pointer-metric.js");

export const { core } = globalThis.QuillPointer;
export const { directionError } = globalThis.QuillPointerMetric;

export const GUEST = { w: 640, h: 480 };

function frameTimes(path) {
	const times = [];
	// Three frames of rest at the end, so the endpoint compares settled paths.
	for (let t = 0; t <= path[path.length - 1].t + 3 * FRAME_MS; t += FRAME_MS) times.push(t);
	return times;
}

// The last path point at or before each frame time.
function samplePath(path, times) {
	const out = [];
	let at = 0;
	for (const time of times) {
		while (at + 1 < path.length && path[at + 1].t <= time) at++;
		out.push({ t: time, x: path[at].x, y: path[at].y });
	}
	return out;
}

// Follow: once a frame the guest pixel under the hand. The stand-in guest starts somewhere the page does not know; the page parks.
export function runFollow(path, options = {}) {
	const units = options.units || { x: 1, y: 1 };
	const real = options.real || units;
	const mover = core.createMover({ w: GUEST.w, h: GUEST.h, unitsX: units.x, unitsY: units.y, maxCount: 120 });
	const guest = new StandInGuest(GUEST.w, GUEST.h, real, options.guestStart || [200, 100]);
	const times = frameTimes(path);
	const input = samplePath(path, times);
	const output = [];
	let maxCalls = 0;
	let first = true;
	for (const sample of input) {
		const calls = first ? mover.park(sample.x, sample.y) : mover.moveTo(sample.x, sample.y);
		const counted = first ? 0 : calls.length;
		first = false;
		for (const [dx, dy] of calls) guest.move(dx, dy);
		maxCalls = Math.max(maxCalls, counted);
		output.push({ t: sample.t, x: guest.x, y: guest.y });
	}
	return { input, output, maxCalls, mover, guest };
}

// Capture: the reports a browser makes, summed per frame, scaled by the gain and carried as whole counts.
export function runCapture(path, model, options = {}) {
	const units = options.units || { x: 1, y: 1 };
	const mover = core.createMover({ w: GUEST.w, h: GUEST.h, unitsX: units.x, unitsY: units.y, maxCount: 120 });
	const guest = new StandInGuest(GUEST.w, GUEST.h, units, [path[0].x, path[0].y]);
	mover.x = path[0].x;
	mover.y = path[0].y;
	const reports = INPUT_MODELS[model](path, options.dpr || 1);
	const times = frameTimes(path);
	const input = samplePath(path, times);
	const output = [];
	let at = 0;
	let maxCalls = 0;
	for (const time of times) {
		let sumX = 0;
		let sumY = 0;
		while (at < reports.length && reports[at].t <= time) {
			sumX += reports[at].mx;
			sumY += reports[at].my;
			at++;
		}
		const calls = mover.moveBy(sumX, sumY);
		for (const [dx, dy] of calls) guest.move(dx, dy);
		maxCalls = Math.max(maxCalls, calls.length);
		output.push({ t: time, x: guest.x, y: guest.y });
	}
	return { input, output, maxCalls };
}

export function score(run, window = 8) {
	return directionError(run.input, run.output, { window });
}

// The plan's pass lines for one run.
// The end point is within one guest pixel, and within half a count more where one count is more than a pixel (units under 1).
export function passes(metric, units = { x: 1, y: 1 }) {
	const toleranceX = 1 + (units.x < 1 ? 0.5 / units.x : 0);
	const toleranceY = 1 + (units.y < 1 ? 0.5 / units.y : 0);
	return metric.windows > 0 && metric.p95 <= 10 && metric.axisOnly <= 0.01 && Math.abs(metric.endpoint[0]) <= toleranceX && Math.abs(metric.endpoint[1]) <= toleranceY;
}

export { centredOrigin, handPath };
