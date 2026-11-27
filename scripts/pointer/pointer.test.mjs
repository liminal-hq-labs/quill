// Level 1 tests for the pointer module: the core in Node, the matrix of paths through the metric, and the stage binding on a stand-in
// browser.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import test from "node:test";
import assert from "node:assert/strict";
import { GUEST, core, passes, runCapture, runFollow, score, centredOrigin, handPath } from "./simulate.mjs";
import { StandInGuest, seeded } from "./paths.mjs";
import { fakeDom, recorder } from "./fakedom.mjs";

const { attach } = globalThis.QuillPointer;

// ---------------------------------------------------------------------------------------------------------------------------------
// The matrix.

const KINDS = [0, 7, 27, 45, 63, 83, 90, "circle", "eight"];
const SPEEDS = [0.03, 0.1, 0.4, 1.5];
const RATES = [125, 1000];
const SCALES = [1, 1.25, 1.5, 2];

function describe(kind, speed, hz, scale) {
	return `${typeof kind === "number" ? kind + " degrees" : kind}, ${speed} px/ms, ${hz} Hz, scale ${scale}`;
}

test("Follow passes the plan's lines over the matrix", () => {
	const failures = [];
	let cells = 0;
	for (const kind of KINDS)
		for (const speed of SPEEDS)
			for (const hz of RATES)
				for (const units of [{ x: 1, y: 1 }, { x: 1 / 1.37, y: 1 / 1.1 }]) {
					const run = runFollow(handPath(kind, speed, hz, centredOrigin(kind, speed)), { units });
					const metric = score(run);
					cells++;
					if (!passes(metric, units) || run.maxCalls > 1) failures.push(describe(kind, speed, hz, 1) + ` units ${units.x.toFixed(2)}: p95 ${metric.p95.toFixed(1)}, axis ${metric.axisOnly}, end ${metric.endpoint}, calls ${run.maxCalls}`);
				}
	assert.ok(cells > 100);
	assert.deepEqual(failures, []);
});

test("Capture with exact and Chromium-style input passes the same lines", () => {
	const failures = [];
	for (const kind of KINDS)
		for (const speed of SPEEDS)
			for (const hz of RATES)
				for (const scale of SCALES)
					for (const model of ["exact", "chromium"]) {
						const run = runCapture(handPath(kind, speed, hz, centredOrigin(kind, speed)), model, { dpr: scale });
						const metric = score(run);
						if (!passes(metric) || run.maxCalls > 2) failures.push(`${model}: ${describe(kind, speed, hz, scale)}: p95 ${metric.p95.toFixed(1)}, axis ${metric.axisOnly}, end ${metric.endpoint}, calls ${run.maxCalls}`);
					}
	assert.deepEqual(failures, []);
});

test("Capture under Firefox-on-Wayland input FAILS the lines at 27 degrees, 0.8 px/ms, 1000 Hz, scale 1.5", () => {
	const path = handPath(27, 0.8, 1000, centredOrigin(27, 0.8));
	const metric = score(runCapture(path, "firefox", { dpr: 1.5 }));
	assert.ok(!passes(metric));
	assert.ok(Math.abs(metric.p95 - 27) < 1, `p95 ${metric.p95}`);
	assert.ok(metric.axisOnly > 0.99, `axis only ${metric.axisOnly}`);
});

test("Capture under Firefox-on-Wayland input fails at a slow speed too, where the pointer does not move", () => {
	const metric = score(runCapture(handPath(27, 0.4, 1000, centredOrigin(27, 0.4)), "firefox", { dpr: 1.5 }));
	assert.ok(!passes(metric));
	assert.ok(metric.p95 >= 89);
});

test("Follow does not suffer from the loss that fails Capture at the same cell", () => {
	const metric = score(runFollow(handPath(27, 0.8, 1000, centredOrigin(27, 0.8))));
	assert.ok(passes(metric));
	assert.ok(metric.p95 < 6.5);
});

// ---------------------------------------------------------------------------------------------------------------------------------
// The counts splitter, the remainder, the edges and the park.

test("splitCounts never exceeds the maximum and always sums to the move", () => {
	const random = seeded(1);
	for (let i = 0; i < 2000; i++) {
		const dx = Math.round((random() - 0.5) * 1000);
		const dy = Math.round((random() - 0.5) * 1000);
		const calls = core.splitCounts(dx, dy, 120);
		for (const [x, y] of calls) assert.ok(Math.abs(x) <= 120 && Math.abs(y) <= 120);
		assert.equal(calls.reduce((s, c) => s + c[0], 0), dx);
		assert.equal(calls.reduce((s, c) => s + c[1], 0), dy);
	}
	assert.deepEqual(core.splitCounts(0, 0, 120), []);
	assert.equal(core.splitCounts(120, -120, 120).length, 1);
	assert.equal(core.splitCounts(121, 0, 120).length, 2);
});

test("a million random fractional moves lose less than one count", () => {
	const random = seeded(7);
	const mover = core.createMover({ w: 1e9, h: 1e9, unitsX: 1.37, unitsY: 0.9 });
	mover.x = 5e8;
	mover.y = 5e8;
	let wantX = 0;
	let wantY = 0;
	let sentX = 0;
	let sentY = 0;
	for (let i = 0; i < 1_000_000; i++) {
		const dx = (random() - 0.5) * 3;
		const dy = (random() - 0.5) * 3;
		wantX += dx * 1.37;
		wantY += dy * 0.9;
		for (const [x, y] of mover.moveBy(dx, dy)) {
			sentX += x;
			sentY += y;
		}
	}
	assert.ok(Math.abs(sentX - wantX) < 1);
	assert.ok(Math.abs(sentY - wantY) < 1);
});

test("the model equals the guest after any path that touches an edge, with the units 3 percent off, and a move there is at most 5 percent of the picture past it", () => {
	const random = seeded(3);
	for (let run = 0; run < 200; run++) {
		const believed = { x: 1 + random() * 0.5, y: 1 + random() * 0.5 };
		const real = { x: believed.x * 1.03, y: believed.y * 0.97 };
		const mover = core.createMover({ w: GUEST.w, h: GUEST.h, unitsX: believed.x, unitsY: believed.y, maxCount: 120 });
		const guest = new StandInGuest(GUEST.w, GUEST.h, real, [random() * 639, random() * 479]);
		mover.x = guest.x;
		mover.y = guest.y;
		for (let step = 0; step < 60; step++) {
			for (const [dx, dy] of mover.moveTo(random() * 700 - 30, random() * 540 - 30)) guest.move(dx, dy);
		}
		// Touch the left edge and the bottom edge: both axes are exact again.
		for (const [dx, dy] of mover.moveTo(-5, 600)) guest.move(dx, dy);
		assert.ok(Math.abs(mover.x - guest.x) < 1e-6 && Math.abs(mover.y - guest.y) < 1e-6, `run ${run}: model ${mover.x},${mover.y} guest ${guest.x},${guest.y}`);
		assert.equal(guest.x, 0);
		assert.ok(Math.abs(guest.y - (GUEST.h - 1)) < 1e-9);
	}
});

test("the overshoot is sent once per arrival at an edge", () => {
	const mover = core.createMover({ w: 640, h: 480 });
	mover.x = 10;
	mover.y = 10;
	assert.deepEqual(mover.moveTo(0, 10), [[-42, 0]]);
	assert.deepEqual(mover.moveTo(0, 10), []);
	assert.deepEqual(mover.moveTo(30, 10), [[30, 0]]);
	assert.deepEqual(mover.moveTo(0, 10), [[-62, 0]]);
});

test("the calls say where the guest is clamped, so an emulator that joins a tick's calls can pause there", () => {
	const mover = core.createMover({ w: 640, h: 480, maxCount: 120 });
	mover.x = 10;
	mover.y = 10;
	// Arriving at an edge: the overshoot makes the last call a clamp. A move away, and a move along the edge, do not.
	assert.deepEqual(mover.moveTo(0, 10).clampAfter, [0]);
	assert.deepEqual(mover.moveTo(0, 10).clampAfter, []);
	assert.deepEqual(mover.moveTo(30, 10).clampAfter, []);
	assert.deepEqual(mover.moveBy(3, 4).clampAfter, []);
	// A park: the clamp is after the last call of the drive into the corner, before the move to the target.
	const calls = mover.park(300, 200);
	assert.ok(calls.length > 2);
	const parkCalls = calls.clampAfter[0] + 1;
	assert.ok(parkCalls >= 2 && parkCalls < calls.length);
	assert.equal(calls.clampAfter.length, 1);
});

test("a park leaves the model equal to the guest from any start, including with units that are not one", () => {
	const random = seeded(11);
	for (let run = 0; run < 300; run++) {
		const units = { x: 0.5 + random(), y: 0.5 + random() };
		const mover = core.createMover({ w: GUEST.w, h: GUEST.h, unitsX: units.x, unitsY: units.y, maxCount: 120 });
		const guest = new StandInGuest(GUEST.w, GUEST.h, units, [random() * 639, random() * 479]);
		const tx = random() * 640;
		const ty = random() * 480;
		const calls = mover.park(tx, ty);
		for (const [dx, dy] of calls) {
			assert.ok(Math.abs(dx) <= 120 && Math.abs(dy) <= 120);
			guest.move(dx, dy);
		}
		assert.ok(Math.abs(guest.x - Math.floor(tx)) <= 1 && Math.abs(guest.y - Math.floor(ty)) <= 1, `guest ${guest.x},${guest.y} target ${tx},${ty}`);
		assert.ok(Math.abs(mover.x - guest.x) <= 1 && Math.abs(mover.y - guest.y) <= 1);
	}
});

test("the whole-number detector", () => {
	const feed = (dpr, values) => {
		const detector = core.createWholeDetector({ dpr });
		for (const v of values) detector.add(v[0], v[1]);
		return detector;
	};
	const whole = Array.from({ length: 200 }, (_, i) => [(i % 3) + 1, i % 2]);
	assert.equal(feed(1.5, whole).wholeOnly, true);
	assert.equal(feed(1, whole).wholeOnly, false);
	assert.equal(feed(2, whole).wholeOnly, false);
	assert.equal(feed(1.5, whole.slice(0, 100)).wholeOnly, false);
	const withFraction = whole.slice();
	withFraction[50] = [0.5, 1];
	assert.equal(feed(1.5, withFraction).wholeOnly, false);
	const late = feed(1.5, whole);
	late.add(0.25, 0);
	assert.equal(late.wholeOnly, false);
	assert.equal(feed(1.5, Array.from({ length: 500 }, () => [0, 0])).wholeOnly, false);
});

// ---------------------------------------------------------------------------------------------------------------------------------
// The wheel, touch and the raw probe.

test("the wheel counts a notch per 50 pixels, in lines, pixels and pages", () => {
	const wheel = core.createWheel();
	assert.equal(wheel.add(100, 0, 480, 0), 2);
	assert.equal(wheel.add(1, 1, 480, 10), 0);
	assert.equal(wheel.add(3, 1, 480, 20), 1);
	assert.equal(wheel.add(-1, 2, 480, 400), -9);
	assert.equal(wheel.add(49, 0, 480, 1000), 0);
	assert.equal(wheel.add(2, 0, 480, 1010), 1);
});

test("a touchpad's 120 small deltas make a sensible number of notches", () => {
	const wheel = core.createWheel();
	let notches = 0;
	for (let i = 0; i < 120; i++) notches += wheel.add(3, 0, 480, i * 8);
	assert.equal(notches, 7);
});

test("the wheel's remainder is dropped after 300 ms without an event", () => {
	const wheel = core.createWheel();
	assert.equal(wheel.add(40, 0, 480, 0), 0);
	assert.equal(wheel.add(40, 0, 480, 301), 0);
	assert.equal(wheel.add(40, 0, 480, 400), 1);
});

test("touch: a tap clicks, a drag moves with the gain, a drag below the tap distance is a tap", () => {
	const touch = core.createTouch({});
	touch.down(1, 100, 100, 0);
	assert.deepEqual(touch.up(1, 120), [{ kind: "click", button: "left" }]);
	touch.down(1, 100, 100, 1000);
	assert.deepEqual(touch.move(1, 103, 104, 1020), [{ kind: "move", dx: 3 * 1.4, dy: 4 * 1.4 }]);
	assert.deepEqual(touch.up(1, 1100), [{ kind: "click", button: "left" }]);
	touch.down(1, 0, 0, 2000);
	const actions = touch.move(1, 50, 0, 2050);
	assert.deepEqual(actions, [{ kind: "move", dx: 70, dy: 0 }]);
	assert.deepEqual(touch.up(1, 2100), []);
});

test("touch: held still for 350 ms then dragged holds the button until the finger lifts", () => {
	const touch = core.createTouch({});
	touch.down(1, 0, 0, 0);
	const actions = touch.move(1, 30, 0, 400);
	assert.equal(actions[0].kind, "press");
	assert.equal(actions[1].kind, "move");
	assert.deepEqual(touch.up(1, 900), [{ kind: "release", button: "left" }]);
	touch.down(1, 0, 0, 1000);
	assert.equal(touch.move(1, 30, 0, 1100)[0].kind, "move");
	assert.deepEqual(touch.up(1, 1500), []);
});

test("touch: two fingers tapped together are a right click, and two dragging are not", () => {
	const touch = core.createTouch({});
	touch.down(1, 0, 0, 0);
	touch.down(2, 50, 0, 20);
	assert.deepEqual(touch.up(1, 100), []);
	assert.deepEqual(touch.up(2, 120), [{ kind: "click", button: "right" }]);
	touch.down(1, 0, 0, 1000);
	touch.down(2, 50, 0, 1010);
	assert.deepEqual(touch.move(1, 60, 0, 1100), []);
	assert.deepEqual(touch.up(1, 1200), []);
	assert.deepEqual(touch.up(2, 1210), []);
});

test("the raw probe: resolves, rejects, and no promise", async () => {
	let exited = 0;
	const doc = { exitPointerLock: () => exited++ };
	assert.deepEqual(await core.probeRaw({ ownerDocument: doc, requestPointerLock: () => Promise.resolve() }), { raw: "yes", locked: true });
	assert.deepEqual(await core.probeRaw({ ownerDocument: doc, requestPointerLock: () => Promise.reject(new Error("NotSupportedError")) }), { raw: "no" });
	assert.equal(exited, 0);
	assert.deepEqual(await core.probeRaw({ ownerDocument: doc, requestPointerLock: () => undefined }), { raw: "no", released: true });
	assert.equal(exited, 1);
	assert.deepEqual(await core.probeRaw({ ownerDocument: doc, requestPointerLock: () => { throw new Error("no"); } }), { raw: "no" });
});

test("the stored answer is believed only for the same user agent, and storage that throws is survived", () => {
	const data = new Map();
	const storage = { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => data.set(k, v) };
	const first = core.createStore(storage, "q", "Agent/1");
	assert.equal(first.getRaw(), null);
	first.setRaw("yes");
	assert.equal(first.getRaw(), "yes");
	assert.equal(core.createStore(storage, "q", "Agent/2").getRaw(), null);
	first.setMode("capture");
	first.setSpeed(1.5);
	assert.equal(first.getMode(), "capture");
	assert.equal(first.getSpeed(), 1.5);
	const broken = core.createStore({ getItem: () => { throw new Error("blocked"); }, setItem: () => { throw new Error("blocked"); } }, "q", "A");
	broken.setRaw("no");
	assert.equal(broken.getRaw(), null);
	assert.equal(core.createStore(null, "q", "A").getMode(), null);
	assert.equal(core.chooseMode("auto", null), "follow");
	assert.equal(core.chooseMode("auto", "yes"), "capture");
	assert.equal(core.chooseMode("follow", "yes"), "follow");
	assert.equal(core.chooseMode("capture", "no"), "capture");
});

// ---------------------------------------------------------------------------------------------------------------------------------
// The stage binding, on a stand-in browser.

function setup(extra = {}, domOptions = {}) {
	const dom = fakeDom(domOptions);
	const rec = recorder();
	const states = [];
	let selects = 0;
	const handle = attach({
		stage: dom.stage,
		picture: () => ({ el: dom.picture, w: 640, h: 480 }),
		units: { x: 1, y: 1 },
		send: rec.send,
		maxCount: 120,
		parkOnEnter: false,
		defaults: { mode: "auto", speed: 1 },
		storageKey: "t",
		onState: (s) => states.push(s),
		onSelect: () => selects++,
		...extra,
	});
	return { dom, rec, handle, states, selects: () => selects };
}

// Where the picture's pixel (x, y) is on the page: the picture sits at (100, 50), 640 by 480, so a pixel is a CSS pixel.
const at = (x, y) => ({ clientX: 100 + x + 0.5, clientY: 50 + y + 0.5 });

function guestAfter(rec, start = [0, 0]) {
	const guest = new StandInGuest(640, 480, { x: 1, y: 1 }, start);
	for (const [dx, dy] of rec.log.moves) guest.move(dx, dy);
	return guest;
}

test("Follow: the first move parks, then the guest pointer is under the visitor's, once a frame", () => {
	const { dom, rec } = setup({}, { secure: false });
	dom.fire("pointermove", at(300, 200));
	dom.fire("pointermove", at(310, 205));
	dom.frame();
	const guest = guestAfter(rec);
	assert.equal(Math.floor(guest.x), 310);
	assert.equal(Math.floor(guest.y), 205);
	rec.log.moves.length = 0;
	dom.fire("pointermove", at(320, 210));
	dom.fire("pointermove", at(330, 215));
	dom.frame();
	assert.equal(rec.log.moves.length, 1);
	assert.deepEqual(rec.log.moves[0], [20, 10]);
});

test("the adapter's move is told which call leaves the guest clamped", () => {
	const flags = [];
	const { dom } = setup({ send: { move: (dx, dy, clamps) => flags.push(clamps), buttons() {}, wheel: null } });
	dom.fire("pointermove", at(300, 200));
	dom.frame();
	// The first move parks: the drive into the corner ends in a clamp, then the move to the target.
	assert.ok(flags.length >= 6);
	assert.equal(flags.filter(Boolean).length, 1);
	assert.equal(flags[flags.length - 1], false);
	flags.length = 0;
	dom.fire("pointermove", at(0, 200));
	dom.frame();
	// Arriving at the left edge: the move and its overshoot are split at maxCount, and only the last call is the clamp.
	assert.ok(flags.length >= 1);
	assert.equal(flags[flags.length - 1], true);
	assert.equal(flags.filter(Boolean).length, 1);
});

test("assume puts the model where the guest is known to be, so the first move is a move and not a park", () => {
	const { dom, rec, handle } = setup({}, { secure: false });
	handle.assume(320, 240);
	dom.fire("pointermove", at(330, 245));
	dom.frame();
	assert.deepEqual(rec.log.moves, [[10, 5]]);
	handle.assume(0, 0);
	rec.log.moves.length = 0;
	dom.fire("pointermove", at(0, 0));
	dom.frame();
	assert.deepEqual(rec.log.moves, [], "already at the corner: nothing owed, and no overshoot to repeat");
});

test("Follow hides the system pointer over the picture and shows it over the letterbox", () => {
	const { dom } = setup();
	dom.fire("pointermove", at(300, 200));
	assert.equal(dom.stage.style.cursor, "none");
	dom.fire("pointermove", { clientX: 50, clientY: 100 });
	assert.equal(dom.stage.style.cursor, "");
});

test("a press lands where the visitor sees, selects once, and is held through leaving until released outside", () => {
	const { dom, rec, selects } = setup({ defaults: { mode: "follow", speed: 1, force: true } });
	dom.fire("pointermove", at(100, 100));
	dom.frame();
	dom.fire("pointerdown", { ...at(120, 110), buttons: 1 });
	assert.equal(selects(), 1);
	assert.deepEqual(rec.log.buttons, [[true, false, false]]);
	assert.equal(dom.captures.length, 1);
	assert.equal(Math.floor(guestAfter(rec).x), 120);
	dom.fire("pointerdown", { ...at(120, 110), buttons: 3 });
	assert.equal(selects(), 1);
	// Out of the picture, below it: the guest pointer stays on the bottom edge while the button is held.
	dom.fire("pointermove", { clientX: 300, clientY: 900, buttons: 3 });
	dom.frame();
	assert.equal(Math.floor(guestAfter(rec).y), 479);
	dom.fire("pointerup", { clientX: 300, clientY: 900, buttons: 0 });
	assert.deepEqual(rec.log.buttons[rec.log.buttons.length - 1], [false, false, false]);
});

test("no button is left down after lostpointercapture, blur or the page being hidden", () => {
	for (const how of ["lost", "blur", "hidden"]) {
		const { dom, rec } = setup({ defaults: { mode: "follow", speed: 1, force: true } });
		dom.fire("pointermove", at(100, 100));
		dom.fire("pointerdown", { ...at(100, 100), buttons: 5 });
		assert.deepEqual(rec.log.buttons[rec.log.buttons.length - 1], [true, true, false]);
		if (how === "lost") dom.fire("lostpointercapture", {});
		else if (how === "blur") dom.win.dispatchEvent(new Event("blur"));
		else {
			dom.doc.visibilityState = "hidden";
			dom.doc.dispatchEvent(new Event("visibilitychange"));
		}
		assert.deepEqual(rec.log.buttons[rec.log.buttons.length - 1], [false, false, false], how);
	}
});

test("left, middle and right go through, and the context menu is suppressed", () => {
	const { dom, rec } = setup({ defaults: { mode: "follow", speed: 1, force: true } });
	dom.fire("pointermove", at(10, 10));
	dom.fire("pointerdown", { ...at(10, 10), buttons: 2 });
	assert.deepEqual(rec.log.buttons[0], [false, false, true]);
	assert.equal(dom.fire("contextmenu", {}).defaultPrevented, true);
});

test("a pen follows in Capture mode too, with contact as the left button and the barrel as the right", () => {
	const { dom, rec } = setup({ defaults: { mode: "capture", speed: 1, force: true }, lockless: true });
	dom.fire("pointermove", { ...at(200, 150), pointerType: "pen" });
	dom.frame();
	assert.equal(Math.floor(guestAfter(rec).x), 200);
	dom.fire("pointerdown", { ...at(200, 150), pointerType: "pen", buttons: 1 });
	dom.fire("pointerup", { ...at(200, 150), pointerType: "pen", buttons: 0 });
	dom.fire("pointerdown", { ...at(200, 150), pointerType: "pen", buttons: 2 });
	assert.deepEqual(rec.log.buttons, [[true, false, false], [false, false, false], [false, false, true]]);
	const before = rec.log.buttons.length;
	dom.fire("pointerdown", { ...at(200, 150), pointerType: "pen", buttons: 32 });
	assert.equal(rec.log.buttons.length, before);
});

test("the raw probe on the first press: refused stays in Follow and the press goes through as a click", async () => {
	const { dom, rec, handle } = setup({}, { requestPointerLock: () => Promise.reject(new Error("NotSupportedError")) });
	dom.fire("pointermove", at(50, 50));
	dom.frame();
	dom.fire("pointerdown", { ...at(50, 50), buttons: 1 });
	dom.fire("pointerup", { ...at(50, 50), buttons: 0 });
	assert.equal(rec.log.buttons.length, 0);
	await new Promise((resolve) => setImmediate(resolve));
	assert.equal(handle.state().raw, "no");
	assert.equal(handle.state().mode, "follow");
	assert.deepEqual(rec.log.buttons[0], [true, false, false]);
	dom.advance(100);
	assert.deepEqual(rec.log.buttons[rec.log.buttons.length - 1], [false, false, false]);
	assert.ok(dom.storage.getItem("t.raw").includes("no"));
});

test("the raw probe: granted switches to Capture and the press is not sent", async () => {
	const dom = fakeDom({ requestPointerLock: () => { dom.lock(); return Promise.resolve(); } });
	const rec = recorder();
	const handle = attach({ stage: dom.stage, picture: () => ({ el: dom.picture, w: 640, h: 480 }), send: rec.send, defaults: { mode: "auto", speed: 1 }, storageKey: "t" });
	dom.fire("pointerdown", { ...at(50, 50), buttons: 1 });
	await new Promise((resolve) => setImmediate(resolve));
	assert.equal(handle.state().raw, "yes");
	assert.equal(handle.state().mode, "capture");
	assert.equal(handle.state().captured, true);
	assert.equal(rec.log.buttons.length, 0);
	dom.fire("pointermove", { movementX: 10, movementY: 4 });
	dom.frame();
	assert.deepEqual(rec.log.moves, [[10, 4]]);
});

test("a stored raw answer for another user agent is asked again", async () => {
	const dom = fakeDom({ userAgent: "B/1", requestPointerLock: () => Promise.reject(new Error("x")) });
	dom.storage.setItem("t.raw", JSON.stringify({ raw: "yes", ua: "A/1" }));
	const handle = attach({ stage: dom.stage, picture: () => ({ el: dom.picture, w: 640, h: 480 }), send: recorder().send, defaults: { mode: "auto", speed: 1 }, storageKey: "t" });
	assert.equal(handle.state().raw, "unknown");
	assert.equal(handle.state().mode, "follow");
});

test("Capture sums coalesced movement, scales by the guest over the shown size and carries the fraction", () => {
	const dom = fakeDom({ rect: { left: 0, top: 0, width: 320, height: 240 } });
	const rec = recorder();
	attach({ stage: dom.stage, picture: () => ({ el: dom.picture, w: 640, h: 480 }), send: rec.send, defaults: { mode: "capture", speed: 1.5, force: true }, lockless: true, baseGain: 1, storageKey: "t" });
	const sample = (mx, my) => ({ movementX: mx, movementY: my });
	dom.fire("pointermove", { movementX: 0, movementY: 0, getCoalescedEvents: () => [sample(0.4, 0.2), sample(0.4, 0.2), sample(0.4, 0.2)] });
	dom.frame();
	// 1.2 CSS pixels across, times 2 (guest over shown), times 1.5 = 3.6 counts, 1.8 counts down.
	assert.deepEqual(rec.log.moves, [[3, 1]]);
	dom.fire("pointermove", { movementX: 1, movementY: 1 });
	dom.frame();
	assert.deepEqual(rec.log.moves[1], [3, 3]);
});

test("Capture never sends more than two calls a frame for a long move", () => {
	const { dom, rec } = setup({ defaults: { mode: "capture", speed: 1, force: true }, lockless: true });
	dom.fire("pointermove", { movementX: 200, movementY: -90 });
	dom.frame();
	assert.ok(rec.log.moves.length <= 2);
	assert.equal(rec.log.moves.reduce((s, m) => s + m[0], 0), 200);
});

test("Capture detects whole-number movement at a fractional scale and says so in the state", () => {
	const dom = fakeDom({ dpr: 1.5 });
	const rec = recorder();
	const handle = attach({ stage: dom.stage, picture: () => ({ el: dom.picture, w: 640, h: 480 }), send: rec.send, defaults: { mode: "capture", speed: 1, force: true }, lockless: true, storageKey: "t" });
	for (let i = 0; i < 130; i++) dom.fire("pointermove", { movementX: 1, movementY: 0 });
	assert.equal(handle.state().wholeOnly, true);
});

test("the wheel accumulates to notches only after the screen is selected, and consumes the event", () => {
	const { dom, rec } = setup({ defaults: { mode: "follow", speed: 1, force: true } });
	const early = dom.fire("wheel", { deltaY: 100, deltaMode: 0 });
	assert.equal(early.defaultPrevented, false);
	assert.deepEqual(rec.log.wheel, []);
	dom.fire("pointermove", at(10, 10));
	dom.fire("pointerdown", { ...at(10, 10), buttons: 1 });
	dom.fire("pointerup", { ...at(10, 10), buttons: 0 });
	const used = dom.fire("wheel", { deltaY: 120, deltaMode: 0 });
	assert.equal(used.defaultPrevented, true);
	assert.deepEqual(rec.log.wheel, [2]);
	dom.fire("pointerdown", { clientX: 5, clientY: 5, buttons: 1 }, dom.doc);
	dom.fire("wheel", { deltaY: 120, deltaMode: 0 });
	assert.deepEqual(rec.log.wheel, [2]);
});

test("an emulator with no wheel gets none and the page scrolls", () => {
	const { dom } = setup({ defaults: { mode: "follow", speed: 1, force: true }, send: { move() {}, buttons() {}, wheel: null } });
	dom.fire("pointerdown", { ...at(10, 10), buttons: 1 });
	assert.equal(dom.fire("wheel", { deltaY: 100, deltaMode: 0 }).defaultPrevented, false);
});

test("touch moves the guest by the finger's distance, taps click and a two-finger tap is a right click", () => {
	const { dom, rec } = setup();
	dom.fire("pointerdown", { ...at(0, 0), pointerType: "touch", pointerId: 7, timeStamp: 0 });
	dom.fire("pointermove", { clientX: 160, clientY: 100, pointerType: "touch", pointerId: 7, timeStamp: 50 });
	dom.frame();
	const [moved] = rec.log.moves.splice(-1);
	assert.ok(Math.abs(moved[0] - 84) <= 1 && Math.abs(moved[1] - 70) <= 1, String(moved));
	dom.fire("pointerup", { pointerType: "touch", pointerId: 7, timeStamp: 100 });
	rec.log.buttons.length = 0;
	dom.fire("pointerdown", { ...at(0, 0), pointerType: "touch", pointerId: 8, timeStamp: 1000 });
	dom.fire("pointerup", { pointerType: "touch", pointerId: 8, timeStamp: 1100 });
	assert.deepEqual(rec.log.buttons, [[true, false, false]]);
	dom.advance(100);
	assert.deepEqual(rec.log.buttons[1], [false, false, false]);
	rec.log.buttons.length = 0;
	dom.fire("pointerdown", { ...at(0, 0), pointerType: "touch", pointerId: 9, timeStamp: 2000 });
	dom.fire("pointerdown", { ...at(40, 0), pointerType: "touch", pointerId: 10, timeStamp: 2010 });
	dom.fire("pointerup", { pointerType: "touch", pointerId: 9, timeStamp: 2100 });
	dom.fire("pointerup", { pointerType: "touch", pointerId: 10, timeStamp: 2120 });
	assert.deepEqual(rec.log.buttons, [[false, false, true]]);
});

test("setMode stores the choice, and a stored choice wins over Auto but not over the page's forced one", () => {
	const first = setup();
	first.handle.setMode("capture");
	assert.equal(first.dom.storage.getItem("t.mode"), '"capture"');
	const dom = fakeDom();
	dom.storage.setItem("t.mode", '"capture"');
	const mk = (defaults) => attach({ stage: dom.stage, picture: () => ({ el: dom.picture, w: 640, h: 480 }), send: recorder().send, defaults, storageKey: "t" });
	assert.equal(mk({ mode: "auto", speed: 1 }).state().mode, "capture");
	assert.equal(mk({ mode: "follow", speed: 1, force: true }).state().mode, "follow");
});

test("a resolution change parks again, and detach removes the listeners and releases the buttons", () => {
	let size = [640, 480];
	const dom = fakeDom();
	const rec = recorder();
	const handle = attach({ stage: dom.stage, picture: () => ({ el: dom.picture, w: size[0], h: size[1] }), send: rec.send, defaults: { mode: "follow", speed: 1, force: true }, storageKey: "t" });
	dom.fire("pointermove", at(10, 10));
	dom.frame();
	const firstCalls = rec.log.moves.length;
	assert.ok(firstCalls > 1, "the first follow parks");
	dom.fire("pointermove", at(20, 20));
	dom.frame();
	assert.equal(rec.log.moves.length, firstCalls + 1);
	size = [320, 200];
	dom.fire("pointermove", at(30, 30));
	dom.frame();
	assert.ok(rec.log.moves.length > firstCalls + 2, "a new size parks");
	dom.fire("pointerdown", { ...at(30, 30), buttons: 1 });
	handle.detach();
	assert.deepEqual(rec.log.buttons[rec.log.buttons.length - 1], [false, false, false]);
	const count = rec.log.moves.length;
	dom.fire("pointermove", at(40, 40));
	dom.frame();
	assert.equal(rec.log.moves.length, count);
});

test("the trace keeps input, output and events in the plan's shape", () => {
	const { dom, handle } = setup({ defaults: { mode: "follow", speed: 1, force: true } });
	handle.trace.start();
	dom.fire("pointermove", at(10, 10));
	dom.frame();
	const trace = handle.trace.stop();
	assert.equal(trace.version, 1);
	assert.equal(trace.input[0].type, "pointermove");
	assert.ok(trace.output.length > 0 && trace.output[0].model.length === 2);
	assert.ok(trace.events.some((e) => e.kind === "park"));
	assert.deepEqual(Object.keys(trace.picture), ["shown", "guest"]);
});
