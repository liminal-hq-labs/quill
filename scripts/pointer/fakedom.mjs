// A small stand-in for the browser, enough to attach the pointer module to a stage in Node and drive it by hand.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export function fakeDom(options = {}) {
	const storageData = new Map();
	const storage = options.noStorage
		? null
		: {
				getItem: (key) => (storageData.has(key) ? storageData.get(key) : null),
				setItem: (key, value) => storageData.set(key, String(value)),
			};
	const queue = [];
	let nextFrame = 1;
	const timers = [];
	let clock = 1000;
	const win = new EventTarget();
	const doc = new EventTarget();
	const stage = new EventTarget();
	const picture = new EventTarget();
	Object.assign(win, {
		devicePixelRatio: options.dpr || 1,
		isSecureContext: !!options.secure,
		navigator: { userAgent: options.userAgent || "Test/1.0" },
		localStorage: storage,
		performance: { now: () => clock },
		requestAnimationFrame(fn) {
			queue.push({ id: nextFrame, fn });
			return nextFrame++;
		},
		cancelAnimationFrame(id) {
			const at = queue.findIndex((q) => q.id === id);
			if (at >= 0) queue.splice(at, 1);
		},
		setTimeout(fn, ms) {
			timers.push({ fn, at: clock + ms });
			return timers.length;
		},
		document: doc,
	});
	Object.assign(doc, { defaultView: win, pointerLockElement: null, visibilityState: "visible", exitPointerLock() { doc.pointerLockElement = null; doc.dispatchEvent(new Event("pointerlockchange")); } });
	const rect = options.rect || { left: 100, top: 50, width: 640, height: 480 };
	Object.assign(picture, { getBoundingClientRect: () => ({ ...rect, right: rect.left + rect.width, bottom: rect.top + rect.height }), style: {} });
	const captures = [];
	Object.assign(stage, {
		ownerDocument: doc,
		style: {},
		contains: (target) => target === stage || target === picture,
		setPointerCapture: (id) => captures.push(id),
		requestPointerLock: options.requestPointerLock || (() => Promise.reject(new Error("refused"))),
	});

	function event(type, props) {
		const e = new Event(type, { cancelable: true, bubbles: false });
		const { timeStamp, ...rest } = props || {};
		Object.assign(e, { pointerType: "mouse", pointerId: 1, buttons: 0, clientX: 0, clientY: 0, movementX: 0, movementY: 0 }, rest);
		Object.defineProperty(e, "timeStamp", { value: timeStamp === undefined ? clock : timeStamp });
		return e;
	}
	return {
		win,
		doc,
		stage,
		picture,
		storage,
		captures,
		set clock(value) {
			clock = value;
		},
		get clock() {
			return clock;
		},
		advance(ms) {
			clock += ms;
			for (const timer of timers.splice(0)) if (timer.at <= clock) timer.fn();
			else timers.push(timer);
		},
		// Runs the animation frames that are waiting (one frame's worth).
		frame() {
			clock += 1000 / 60;
			for (const q of queue.splice(0)) q.fn(clock);
		},
		fire(type, props, target = stage) {
			const e = event(type, props);
			target.dispatchEvent(e);
			return e;
		},
		lock() {
			doc.pointerLockElement = stage;
			doc.dispatchEvent(new Event("pointerlockchange"));
		},
	};
}

// The calls an adapter receives, in order.
export function recorder() {
	const log = { moves: [], buttons: [], wheel: [] };
	return {
		log,
		send: {
			move: (dx, dy) => log.moves.push([dx, dy]),
			buttons: (l, m, r) => log.buttons.push([l, m, r]),
			wheel: (n) => log.wheel.push(n),
		},
	};
}
