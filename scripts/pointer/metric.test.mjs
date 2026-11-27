// Tests for the pointer's direction error.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import test from "node:test";
import assert from "node:assert/strict";

await import("../../site/js/pointer-metric.js");
const { directionError, resample, scoreTrace } = globalThis.QuillPointerMetric;

function line(angle, perFrame, frames, scale = 1) {
	const radians = (angle * Math.PI) / 180;
	return Array.from({ length: frames }, (_, i) => ({ t: i * 16.7, x: i * perFrame * Math.cos(radians) * scale, y: i * perFrame * Math.sin(radians) * scale }));
}

test("a straight output scores zero", () => {
	const input = line(27, 5, 60);
	const result = directionError(input, input);
	assert.ok(result.windows > 40);
	assert.ok(result.p95 < 1e-6);
	assert.equal(result.axisOnly, 0);
	assert.ok(Math.abs(result.pathRatio - 1) < 1e-9);
	assert.deepEqual(result.endpoint.map((v) => Math.abs(v) < 1e-9), [true, true]);
});

test("an output rotated by ten degrees scores ten", () => {
	const result = directionError(line(27, 5, 60), line(37, 5, 60));
	assert.ok(Math.abs(result.median - 10) < 1e-6);
	assert.ok(Math.abs(result.p95 - 10) < 1e-6);
});

test("a purely horizontal output of a 27 degree input scores 27 and is all one axis", () => {
	const input = line(27, 5, 60);
	const output = input.map((p) => ({ t: p.t, x: p.x, y: 0 }));
	const result = directionError(input, output);
	assert.ok(Math.abs(result.p95 - 27) < 1e-6);
	assert.equal(result.axisOnly, 1);
});

test("a frozen output scores ninety", () => {
	const input = line(27, 5, 60);
	const result = directionError(input, input.map((p) => ({ t: p.t, x: 3, y: 4 })));
	assert.equal(result.median, 90);
	assert.equal(result.p95, 90);
});

test("windows below the travel are skipped", () => {
	const short = line(10, 0.1, 50);
	assert.equal(directionError(short, short).windows, 0);
	assert.equal(directionError(short, short, { window: 6 }).windows, 0);
	assert.ok(directionError(line(10, 0.5, 100), line(10, 0.5, 100), { window: 4 }).windows > 0);
});

test("a lag of up to two frames is allowed", () => {
	const input = Array.from({ length: 80 }, (_, i) => ({ t: i * 16.7, x: 100 * Math.sin(i / 8), y: 100 * (1 - Math.cos(i / 8)) }));
	const output = input.map((_, i) => input[Math.max(0, i - 2)]);
	const result = directionError(input, output);
	assert.equal(result.lag, 2);
	assert.ok(result.p95 < 1e-6);
});

test("the smaller component must reach the axis minimum to count as one axis", () => {
	const input = line(5, 9, 40);
	const output = input.map((p) => ({ t: p.t, x: p.x, y: 0 }));
	assert.equal(directionError(input, output).axisOnly, 0);
	assert.ok(directionError(line(30, 9, 40), line(30, 9, 40).map((p) => ({ t: p.t, x: p.x, y: 0 }))).axisOnly > 0.9);
});

test("resample takes the last sample at or before each time", () => {
	const samples = [{ t: 0, x: 0, y: 0 }, { t: 10, x: 1, y: 1 }, { t: 20, x: 2, y: 2 }];
	assert.deepEqual(resample(samples, [-5, 0, 9, 10, 25]).map((s) => s.x), [0, 0, 0, 1, 2]);
});

test("scoreTrace splits strokes at pauses and scores each", () => {
	const shown = { left: 100, top: 50, w: 640, h: 480 };
	const input = [];
	const output = [];
	let model = [0, 0];
	for (let i = 0; i < 80; i++) {
		const t = i * 5 + (i >= 40 ? 1000 : 0);
		input.push({ t, type: "pointermove", pointerType: "mouse", x: 100 + i * 2, y: 50 + i, mx: 0, my: 0 });
		if (i % 3 === 0) {
			model = [i * 2, i];
			output.push({ t: t + 1, dx: 0, dy: 0, model });
		}
	}
	const trace = { mode: "follow", picture: { shown, guest: { w: 640, h: 480 } }, input, output, events: [] };
	const strokes = scoreTrace(trace);
	assert.equal(strokes.length, 2);
	assert.ok(strokes.every((s) => s.metric.windows > 0 && s.metric.p95 < 15));
});

test("a late start of the output is not scored, but an output that never moves is", () => {
	const input = line(27, 5, 60);
	const late = input.map((_, i) => input[Math.max(2, i)]);
	const result = directionError(input, late);
	assert.ok(result.p95 < 1e-6);
	assert.equal(result.axisOnly, 0);
	assert.equal(directionError(input, input.map(() => input[0])).p95, 90);
});
