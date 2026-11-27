// Generated hand paths and the three ways a browser reports a captured pointer's movement, for the pointer's tests. All pure and seeded.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export const FRAME_MS = 1000 / 60;

// A small seeded generator (mulberry32), so a failing run can be repeated.
export function seeded(seed) {
	let state = seed >>> 0;
	return () => {
		state = (state + 0x6d2b79f5) >>> 0;
		let t = state;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

// How long a stroke lasts at a speed: long enough for several windows of travel, and no longer than a few seconds.
export function strokeMilliseconds(speed) {
	return Math.round(Math.max(250, Math.min(3000, 90 / speed)));
}

// The true hand path in CSS pixels, y down, reported every 1000 / hz milliseconds from (originX, originY): a straight line at an angle
// in degrees, a circle of radius 100 or a figure eight about 200 wide, all at `speed` CSS pixels a millisecond.
export function handPath(kind, speed, hz, origin = [0, 0]) {
	const step = 1000 / hz;
	const total = strokeMilliseconds(speed);
	const points = [];
	for (let t = 0; t <= total; t += step) {
		const travelled = speed * t;
		let x;
		let y;
		if (typeof kind === "number") {
			const radians = (kind * Math.PI) / 180;
			x = travelled * Math.cos(radians);
			y = travelled * Math.sin(radians);
		} else if (kind === "circle") {
			const phase = travelled / 100;
			x = 100 * Math.sin(phase);
			y = 100 * (1 - Math.cos(phase));
		} else {
			const phase = travelled / 100;
			x = 100 * Math.sin(phase);
			y = 50 * Math.sin(2 * phase);
		}
		points.push({ t, x: origin[0] + x, y: origin[1] + y });
	}
	return points;
}

// What the browser reports for each step of a captured pointer, as movement in CSS pixels { t, mx, my }.
// exact: the true difference, kept to the fraction.
export function reportsExact(path) {
	const out = [];
	for (let i = 1; i < path.length; i++) out.push({ t: path[i].t, mx: path[i].x - path[i - 1].x, my: path[i].y - path[i - 1].y });
	return out;
}

// Chromium: whole numbers, as the difference of rounded positions, so the remainder is kept.
export function reportsChromium(path) {
	const out = [];
	for (let i = 1; i < path.length; i++) {
		const mx = Math.round(path[i].x) - Math.round(path[i - 1].x);
		const my = Math.round(path[i].y) - Math.round(path[i - 1].y);
		out.push({ t: path[i].t, mx, my });
	}
	return out;
}

// Firefox on Wayland: each report is cut to whole device pixels on its own (the remainder is lost), and a report with both axes 0 is
// dropped (widget/gtk/nsWindowWayland.cpp). The page sees them as CSS pixels.
export function reportsFirefoxWayland(path, scale) {
	const out = [];
	for (let i = 1; i < path.length; i++) {
		const dx = Math.trunc((path[i].x - path[i - 1].x) * scale);
		const dy = Math.trunc((path[i].y - path[i - 1].y) * scale);
		if (dx || dy) out.push({ t: path[i].t, mx: dx / scale, my: dy / scale });
	}
	return out;
}

export const INPUT_MODELS = {
	exact: (path) => reportsExact(path),
	chromium: (path) => reportsChromium(path),
	firefox: (path, scale) => reportsFirefoxWayland(path, scale),
};

// A guest that moves by whole counts in its own units, clamps at its edges and reports its position in guest pixels. `units` is the
// counts it moves per guest pixel; `real` is what it really does when the page's belief (`units`) is off.
export class StandInGuest {
	constructor(width, height, real = { x: 1, y: 1 }, start = [0, 0]) {
		this.width = width;
		this.height = height;
		this.real = real;
		this.countsX = start[0] * real.x;
		this.countsY = start[1] * real.y;
		this.calls = 0;
	}
	move(dx, dy) {
		this.calls++;
		this.countsX = Math.max(0, Math.min((this.width - 1) * this.real.x, this.countsX + dx));
		this.countsY = Math.max(0, Math.min((this.height - 1) * this.real.y, this.countsY + dy));
	}
	get x() {
		return this.countsX / this.real.x;
	}
	get y() {
		return this.countsY / this.real.y;
	}
}

// Where a stroke starts so that it stays on a 640 by 480 picture: lines are centred on (320, 240), a circle starts at its top, a figure
// eight at its middle.
export function centredOrigin(kind, speed) {
	if (typeof kind === "number") {
		const length = speed * strokeMilliseconds(speed);
		const radians = (kind * Math.PI) / 180;
		return [320 - (length * Math.cos(radians)) / 2, 240 - (length * Math.sin(radians)) / 2];
	}
	return kind === "circle" ? [320, 140] : [320, 240];
}
