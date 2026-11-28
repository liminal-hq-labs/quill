// The pointer for Quill's web pages: the guest pointer follows the visitor's pointer, and capture is an option.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// A classic script (no build step) that sets globalThis.QuillPointer, loaded before a page's own script. Two parts:
//
//   QuillPointer.core    pure functions and small state machines with no DOM (Node's tests load them): the mover (follow and capture
//                        counts, remainder, clamping, edge resync, parking), the counts splitter, the whole-number detector, the wheel
//                        accumulator, the touch gesture machine, the raw-capture probe and the stored answers.
//   QuillPointer.attach  binds the core to a stage element and an emulator adapter (see docs/plans/pointer.md, "One module, two adapters").
//
// The adapter's `send.wheel(notches)` is signed like the wheel's deltaY: positive is down.
//
// The guest has a relative mouse that clamps at its edges, so the module keeps a model of where the guest pointer is, in guest pixels.
// Follow moves the guest to the guest pixel under the visitor's pointer, worked out from the event's clientX and clientY (fractional
// where the browser has them) once an animation frame; Capture sums the lock's movement. Whole counts go out; the page's model
// advances by what was sent, so what a frame could not send is still owed next frame.
(function (root) {
	"use strict";

	const OVERSHOOT_PIXELS = 16;
	const MAX_COUNT = 120;
	const FINGER_GAIN = 1.4;
	const WHEEL_PIXELS_PER_NOTCH = 50;
	const TRACE_MILLISECONDS = 20000;
	const WHOLE_NUMBER_SAMPLES = 128;

	function clamp(value, low, high) {
		return Math.max(low, Math.min(high, value));
	}

	// A list of calls that also says after which of them the guest is clamped at an edge. The list is kept out of the array's own values, so
	// a list of calls still equals the plain one.
	function withClamps(calls, clampAfter) {
		Object.defineProperty(calls, "clampAfter", { value: clampAfter, enumerable: false, writable: true });
		return calls;
	}

	// Cuts a move into calls of at most `max` counts on each axis, in proportion, so a diagonal stays diagonal; the calls add up
	// to the move exactly. dx and dy are whole numbers.
	function splitCounts(dx, dy, max) {
		const limit = max || MAX_COUNT;
		const pieces = Math.max(1, Math.ceil(Math.max(Math.abs(dx), Math.abs(dy)) / limit));
		const calls = [];
		for (let k = 0; k < pieces; k++) {
			const sx = Math.round((dx * (k + 1)) / pieces) - Math.round((dx * k) / pieces);
			const sy = Math.round((dy * (k + 1)) / pieces) - Math.round((dy * k) / pieces);
			if (sx || sy) calls.push([sx, sy]);
		}
		return calls;
	}

	// The model of the guest pointer. `unitsX` and `unitsY` are guest counts per guest pixel. The model's position is in guest pixels
	// (fractional when the units are), clamped to 0 ... w-1 and 0 ... h-1 as the guest clamps.
	function createMover(options) {
		const mover = {
			w: options.w,
			h: options.h,
			unitsX: options.unitsX || 1,
			unitsY: options.unitsY || 1,
			maxCount: options.maxCount || MAX_COUNT,
			overshoot: options.overshoot,
			x: 0,
			y: 0,
			remX: 0,
			remY: 0,
			edgeX: 0,
			edgeY: 0,
		};

		// How far past an edge a move goes: at least 16 guest pixels, and 5 percent of the picture, so units that are off by up to 5
		// percent still reach the edge from the far side.
		function overshootX() {
			return mover.overshoot === undefined ? Math.max(OVERSHOOT_PIXELS, Math.ceil(mover.w * 0.05)) : mover.overshoot;
		}
		function overshootY() {
			return mover.overshoot === undefined ? Math.max(OVERSHOOT_PIXELS, Math.ceil(mover.h * 0.05)) : mover.overshoot;
		}

		// The calls end in a clamp when the guest is driven past an edge (an overshoot, or a park): `calls.clampAfter` lists the indexes of the
		// calls after which the guest has clamped. An emulator that joins the calls of one tick into one report must be given a pause there
		// (the adapter's `send.move` gets the flag as its third argument), or the clamp would never happen between the call and the next.
		function advance(countX, countY, clamps) {
			mover.x = clamp(mover.x + countX / mover.unitsX, 0, mover.w - 1);
			mover.y = clamp(mover.y + countY / mover.unitsY, 0, mover.h - 1);
			const calls = splitCounts(countX, countY, mover.maxCount);
			return withClamps(calls, clamps && calls.length ? [calls.length - 1] : []);
		}

		// Follow: the guest pixel under (px, py), given in fractional guest pixels. The difference between the model and the target is
		// what is owed, so what a frame cannot send whole is sent in a later one. On arriving at an edge, a further `overshoot` pixels
		// of counts go outward on that axis: the guest and the model both clamp, and that axis is exact again.
		mover.moveTo = function (px, py) {
			const tx = clamp(Math.floor(px), 0, mover.w - 1);
			const ty = clamp(Math.floor(py), 0, mover.h - 1);
			const ex = tx <= 0 ? -1 : tx >= mover.w - 1 ? 1 : 0;
			const ey = ty <= 0 ? -1 : ty >= mover.h - 1 ? 1 : 0;
			let countX = Math.round((tx - mover.x) * mover.unitsX);
			let countY = Math.round((ty - mover.y) * mover.unitsY);
			const arrivedX = !!ex && ex !== mover.edgeX;
			const arrivedY = !!ey && ey !== mover.edgeY;
			if (arrivedX) countX += ex * Math.round(overshootX() * mover.unitsX);
			if (arrivedY) countY += ey * Math.round(overshootY() * mover.unitsY);
			mover.edgeX = ex;
			mover.edgeY = ey;
			mover.remX = 0;
			mover.remY = 0;
			return advance(countX, countY, arrivedX || arrivedY);
		};

		// Capture: a move in guest pixels. The fraction of a count is carried to the next call.
		mover.moveBy = function (dx, dy) {
			const rawX = dx * mover.unitsX + mover.remX;
			const rawY = dy * mover.unitsY + mover.remY;
			const countX = Math.trunc(rawX);
			const countY = Math.trunc(rawY);
			mover.remX = rawX - countX;
			mover.remY = rawY - countY;
			mover.edgeX = 0;
			mover.edgeY = 0;
			return advance(countX, countY);
		};

		// Drives the guest pointer into the corner nearest the target (the screen's size plus the overshoot, split), so its position is
		// known, then on to the target.
		mover.park = function (px, py) {
			const cornerX = px < (mover.w - 1) / 2 ? 0 : mover.w - 1;
			const cornerY = py < (mover.h - 1) / 2 ? 0 : mover.h - 1;
			const dirX = cornerX === 0 ? -1 : 1;
			const dirY = cornerY === 0 ? -1 : 1;
			const calls = advance(dirX * Math.round((mover.w + overshootX()) * mover.unitsX), dirY * Math.round((mover.h + overshootY()) * mover.unitsY), true);
			mover.x = cornerX;
			mover.y = cornerY;
			mover.remX = 0;
			mover.remY = 0;
			mover.edgeX = dirX;
			mover.edgeY = dirY;
			const move = mover.moveTo(px, py);
			return withClamps(calls.concat(move), calls.clampAfter.concat(move.clampAfter.map((i) => i + calls.length)));
		};

		mover.resize = function (w, h) {
			mover.w = w;
			mover.h = h;
			mover.x = clamp(mover.x, 0, w - 1);
			mover.y = clamp(mover.y, 0, h - 1);
		};

		return mover;
	}

	// Over the first 128 non-zero samples of a capture: every one a whole number at a devicePixelRatio that is not, and the
	// browser is probably cutting the pointer's movement to device pixels. A single fraction clears it for good.
	function createWholeDetector(options) {
		const needed = (options && options.needed) || WHOLE_NUMBER_SAMPLES;
		const fractionalScale = !!options && options.dpr !== undefined && Math.round(options.dpr) !== options.dpr;
		const detector = { seen: 0, fractional: false, wholeOnly: false };
		detector.add = function (mx, my) {
			if (detector.fractional || (!mx && !my)) return detector.wholeOnly;
			if (!Number.isInteger(mx) || !Number.isInteger(my)) {
				detector.fractional = true;
				detector.wholeOnly = false;
				return false;
			}
			detector.seen++;
			if (detector.seen >= needed && fractionalScale) detector.wholeOnly = true;
			return detector.wholeOnly;
		};
		return detector;
	}

	// Turns wheel deltas into notches: lines count 16 pixels and pages the picture's height, a notch is `pixelsPerNotch`, the remainder
	// is kept between events and dropped after `resetMs` without one.
	function createWheel(options) {
		const pixelsPerNotch = (options && options.pixelsPerNotch) || WHEEL_PIXELS_PER_NOTCH;
		const resetMs = (options && options.resetMs) || 300;
		const wheel = { remainder: 0, last: -Infinity };
		wheel.add = function (delta, deltaMode, pictureHeight, now) {
			if (now - wheel.last > resetMs) wheel.remainder = 0;
			wheel.last = now;
			const pixels = delta * (deltaMode === 1 ? 16 : deltaMode === 2 ? pictureHeight : 1);
			wheel.remainder += pixels;
			const notches = Math.trunc(wheel.remainder / pixelsPerNotch);
			wheel.remainder -= notches * pixelsPerNotch;
			return notches;
		};
		return wheel;
	}

	// A finger is a trackpad. A drag moves the pointer by the finger's distance times FINGER_GAIN and the speed; a short, short-travelled
	// touch is a left click; a finger held still for 350 ms and then dragged holds the left button until it lifts; two fingers
	// tapped together are a right click. Each call takes a time in ms and returns actions: { kind: "move", dx, dy } in CSS pixels,
	// { kind: "press" | "release" | "click", button }.
	function createTouch(options) {
		const tapMs = (options && options.tapMs) || 300;
		const tapDistance = (options && options.tapDistance) || 10;
		const holdMs = (options && options.holdMs) || 350;
		const touch = { speed: (options && options.speed) || 1, gain: (options && options.gain) || FINGER_GAIN };
		const fingers = new Map();
		let primary = null;
		let multi = null;
		let held = false;

		touch.down = function (id, x, y, now) {
			fingers.set(id, { x, y, start: now, travel: 0, decided: false });
			if (fingers.size === 1) {
				primary = id;
				multi = null;
			} else if (!multi) multi = { start: Math.min(...[...fingers.values()].map((f) => f.start)), travel: 0 };
			return [];
		};
		touch.move = function (id, x, y, now) {
			const finger = fingers.get(id);
			if (!finger) return [];
			const dx = x - finger.x;
			const dy = y - finger.y;
			finger.x = x;
			finger.y = y;
			finger.travel += Math.hypot(dx, dy);
			if (multi) {
				multi.travel += Math.hypot(dx, dy);
				return [];
			}
			if (id !== primary) return [];
			const actions = [];
			if (!finger.decided && finger.travel >= tapDistance) {
				finger.decided = true;
				if (now - finger.start >= holdMs) {
					held = true;
					actions.push({ kind: "press", button: "left" });
				}
			}
			actions.push({ kind: "move", dx: dx * touch.gain * touch.speed, dy: dy * touch.gain * touch.speed });
			return actions;
		};
		touch.up = function (id, now) {
			const finger = fingers.get(id);
			fingers.delete(id);
			if (!finger) return [];
			if (multi) {
				if (fingers.size) return [];
				const tapped = now - multi.start < tapMs && multi.travel < tapDistance * 2;
				multi = null;
				return tapped ? [{ kind: "click", button: "right" }] : [];
			}
			if (held) {
				held = false;
				return [{ kind: "release", button: "left" }];
			}
			if (!finger.decided && now - finger.start < tapMs) return [{ kind: "click", button: "left" }];
			return [];
		};
		touch.cancel = function () {
			const actions = held ? [{ kind: "release", button: "left" }] : [];
			fingers.clear();
			held = false;
			multi = null;
			return actions;
		};
		return touch;
	}

	// Asks for a raw (unadjusted) pointer lock and says what came of it. Resolves to { raw: "yes", locked: true } when the browser
	// grants it, { raw: "no" } when it refuses, and { raw: "no", released: true } when requestPointerLock returns no promise (a lock it
	// may have taken is released at once, so the visitor is never captured by a probe).
	function probeRaw(element) {
		let result;
		try {
			result = element.requestPointerLock({ unadjustedMovement: true });
		} catch (error) {
			return Promise.resolve({ raw: "no" });
		}
		if (!result || typeof result.then !== "function") {
			try {
				element.ownerDocument.exitPointerLock();
			} catch (error) {
				// Nothing was locked.
			}
			return Promise.resolve({ raw: "no", released: true });
		}
		return result.then(
			() => ({ raw: "yes", locked: true }),
			() => ({ raw: "no" }),
		);
	}

	// The remembered answers (mode, raw, speed), under one key prefix. Every access is guarded: a browser with storage blocked works
	// without it. The raw answer carries the user agent and is not believed when that changes.
	function createStore(storage, prefix, userAgent) {
		const store = {};
		function read(name) {
			try {
				const text = storage && storage.getItem(prefix + "." + name);
				return text === null || text === undefined ? null : JSON.parse(text);
			} catch (error) {
				return null;
			}
		}
		function write(name, value) {
			try {
				if (storage) storage.setItem(prefix + "." + name, JSON.stringify(value));
			} catch (error) {
				// Storage is full or blocked.
			}
		}
		store.getRaw = function () {
			const saved = read("raw");
			return saved && saved.ua === userAgent && (saved.raw === "yes" || saved.raw === "no") ? saved.raw : null;
		};
		store.setRaw = (raw) => write("raw", { raw, ua: userAgent });
		store.getMode = function () {
			const mode = read("mode");
			return mode === "follow" || mode === "capture" ? mode : null;
		};
		store.setMode = (mode) => write("mode", mode);
		store.getSpeed = function () {
			const speed = read("speed");
			return typeof speed === "number" && speed >= 0.25 && speed <= 4 ? speed : null;
		};
		store.setSpeed = (speed) => write("speed", speed);
		return store;
	}

	// Follow unless the visitor chose Capture, or raw capture is known to work here.
	function chooseMode(setting, raw) {
		if (setting === "follow" || setting === "capture") return setting;
		return raw === "yes" ? "capture" : "follow";
	}

	// ---------------------------------------------------------------------------------------------------------------------------

	function safeStorage(win) {
		try {
			return win.localStorage;
		} catch (error) {
			return null;
		}
	}

	// Binds the core to a stage and an adapter. See docs/plans/pointer.md for the options and the handle. Beyond the plan: `lockless`
	// makes Capture work without a pointer lock (the lab and the tests dispatch movement), `baseGain` multiplies Capture's counts (the
	// ReactOS page's 1.5), `stopEvents` stops the stage's pointer and mouse events from reaching its children (for an emulator with its
	// own handlers), `userAgent` and `storage` are for tests, and `defaults.force` makes the page's mode and speed win over the stored ones.
	function attach(options) {
		const stage = options.stage;
		const doc = stage.ownerDocument;
		const win = doc.defaultView;
		const send = options.send;
		const units = Object.assign({ x: 1, y: 1 }, options.units);
		const defaults = Object.assign({ mode: "auto", speed: 1, force: false }, options.defaults);
		const userAgent = options.userAgent || (win.navigator && win.navigator.userAgent) || "";
		const store = createStore(options.storage !== undefined ? options.storage : safeStorage(win), options.storageKey || "quill-pointer", userAgent);
		const baseGain = options.baseGain || 1;
		const lockless = !!options.lockless;
		const now = () => (win.performance && win.performance.now ? win.performance.now() : Date.now());

		let setting = defaults.force ? defaults.mode : store.getMode() || defaults.mode;
		let speed = defaults.force ? defaults.speed : store.getSpeed() || defaults.speed;
		let raw = store.getRaw();
		let mover = null;
		let needsPark = true;
		let frameRequest = 0;
		let followTarget = null;
		let captureX = 0;
		let captureY = 0;
		let fingerX = 0;
		let fingerY = 0;
		let captured = false;
		let probing = false;
		let selected = false;
		let sentButtons = [false, false, false];
		let heldButtons = 0;
		let lastPointerType = "mouse";
		let lastMoveEvent = null;
		let positionSamples = 0;
		let fractionalSamples = 0;
		let coalescedAvailable = false;
		let callsLastFrame = 0;
		let wholeDetector = createWholeDetector({ dpr: win.devicePixelRatio });
		let lastSignature = "";
		let touchActions = createTouch({ speed });
		let tracing = false;
		let traceInput = [];
		let traceOutput = [];
		let traceEvents = [];

		stage.style.touchAction = "none";

		function guestSize() {
			const picture = options.picture();
			return { w: Math.max(1, picture.w), h: Math.max(1, picture.h), el: picture.el };
		}

		// Where the picture is drawn, in CSS pixels in the viewport: a canvas can be letterboxed inside its element.
		function shownRect() {
			const guest = guestSize();
			const box = guest.el.getBoundingClientRect();
			const w = Math.min(box.width, (box.height * guest.w) / guest.h);
			const h = (w * guest.h) / guest.w;
			return { left: box.left + (box.width - w) / 2, top: box.top + (box.height - h) / 2, w, h, guest };
		}

		function sync() {
			const guest = guestSize();
			if (!mover) {
				mover = createMover({ w: guest.w, h: guest.h, unitsX: units.x, unitsY: units.y, maxCount: options.maxCount });
			} else if (mover.w !== guest.w || mover.h !== guest.h) {
				mover.resize(guest.w, guest.h);
				needsPark = true;
				logEvent("park-pending");
			}
			mover.unitsX = units.x;
			mover.unitsY = units.y;
			return mover;
		}

		function effectiveMode(pointerType) {
			if (pointerType === "pen" || pointerType === "touch") return "follow";
			return chooseMode(setting, raw);
		}

		function trimTrace(list, time) {
			while (list.length && time - list[0].t > TRACE_MILLISECONDS) list.shift();
		}
		function logEvent(kind) {
			if (!tracing) return;
			const t = now();
			traceEvents.push({ t, kind });
			trimTrace(traceEvents, t);
		}
		function logInput(e, extra) {
			if (!tracing) return;
			const entry = Object.assign({ t: now(), type: e.type, pointerType: e.pointerType, x: e.clientX, y: e.clientY, mx: e.movementX, my: e.movementY, buttons: e.buttons, coalesced: 0 }, extra);
			traceInput.push(entry);
			trimTrace(traceInput, entry.t);
		}

		function state() {
			const rect = shownRect();
			return {
				mode: chooseMode(setting, raw),
				setting,
				raw: raw || "unknown",
				wholeOnly: wholeDetector.wholeOnly,
				following: chooseMode(setting, raw) === "follow" && lastPointerType === "mouse" && !captured,
				captured,
				secure: !!win.isSecureContext,
				coalesced: coalescedAvailable,
				fractionalShare: positionSamples ? fractionalSamples / positionSamples : 0,
				packetsPerFrame: callsLastFrame,
				pointerType: lastPointerType,
				speed,
				dpr: win.devicePixelRatio,
				model: mover ? [mover.x, mover.y] : null,
				remainder: mover ? [mover.remX, mover.remY] : [0, 0],
				shown: { left: rect.left, top: rect.top, w: rect.w, h: rect.h },
			};
		}
		function notify() {
			if (!options.onState) return;
			const s = state();
			const signature = [s.mode, s.setting, s.raw, s.wholeOnly, s.following, s.captured, s.coalesced, s.pointerType, s.speed].join("|");
			if (signature === lastSignature) return;
			lastSignature = signature;
			options.onState(s);
		}

		// Two lists of calls as one, keeping where the clamps are.
		function joined(first, second) {
			return withClamps(first.concat(second), (first.clampAfter || []).concat((second.clampAfter || []).map((i) => i + first.length)));
		}

		// `send.move(dx, dy, clamps)`: the third argument is true after a call that leaves the guest clamped at an edge (a park, or the
		// overshoot on arriving at one). An emulator that joins the calls of one tick into one report is given a pause there (js-dos).
		function emit(calls) {
			const t = now();
			const clampAfter = calls.clampAfter || [];
			for (let i = 0; i < calls.length; i++) {
				const call = calls[i];
				send.move(call[0], call[1], clampAfter.includes(i));
				if (tracing) {
					traceOutput.push({ t, dx: call[0], dy: call[1], model: [mover.x, mover.y] });
					trimTrace(traceOutput, t);
				}
			}
			callsLastFrame = calls.length;
		}

		// One animation frame's worth: the follow target, or the capture counts, or the finger's moves, as calls to the adapter.
		function flush() {
			frameRequest = 0;
			const model = sync();
			const rect = shownRect();
			let calls = [];
			if (followTarget) {
				const gx = ((followTarget.x - rect.left) * rect.guest.w) / rect.w;
				const gy = ((followTarget.y - rect.top) * rect.guest.h) / rect.h;
				followTarget = null;
				if (needsPark && !heldButtons) {
					calls = model.park(gx, gy);
					needsPark = false;
					logEvent("park");
				} else {
					const edgeBefore = model.edgeX + "," + model.edgeY;
					calls = model.moveTo(gx, gy);
					if (edgeBefore !== model.edgeX + "," + model.edgeY && (model.edgeX || model.edgeY)) logEvent("edge");
				}
			}
			if (captureX || captureY) {
				const k = baseGain * speed;
				calls = joined(calls, model.moveBy(captureX * (rect.guest.w / rect.w) * k, captureY * (rect.guest.h / rect.h) * k));
				captureX = 0;
				captureY = 0;
			}
			if (fingerX || fingerY) {
				calls = joined(calls, model.moveBy(fingerX, fingerY));
				fingerX = 0;
				fingerY = 0;
			}
			emit(calls);
			notify();
		}
		function schedule() {
			if (!frameRequest) frameRequest = win.requestAnimationFrame(flush);
		}
		// What the visitor is pointing at must reach the guest before a press does.
		function flushNow() {
			if (frameRequest) win.cancelAnimationFrame(frameRequest);
			flush();
		}

		function sendButtons(bits) {
			const wanted = [!!(bits & 1), !!(bits & 4), !!(bits & 2)];
			if (wanted.every((value, i) => value === sentButtons[i])) return;
			sentButtons = wanted;
			send.buttons(wanted[0], wanted[1], wanted[2]);
		}
		function releaseAll() {
			heldButtons = 0;
			touchActions.cancel();
			sendButtons(0);
		}

		function inside(rect, x, y) {
			return x >= rect.left && x <= rect.left + rect.w && y >= rect.top && y <= rect.top + rect.h;
		}
		function updateCursor(e) {
			const hide = effectiveMode(e.pointerType) === "follow" && e.pointerType !== "touch" && inside(shownRect(), e.clientX, e.clientY);
			stage.style.cursor = hide ? "none" : "";
		}

		function requestCapture() {
			if (lockless) {
				captured = true;
				logEvent("lock");
				notify();
				return;
			}
			try {
				const ask = raw === "yes" ? stage.requestPointerLock({ unadjustedMovement: true }) : stage.requestPointerLock();
				if (ask && ask.catch) ask.catch(() => {});
			} catch (error) {
				// The browser refused; the next press asks again.
			}
		}

		function onDown(e) {
			if (options.stopEvents) e.stopPropagation();
			lastPointerType = e.pointerType || "mouse";
			logInput(e);
			if (e.pointerType === "touch") return touchDown(e);
			if (e.pointerType === "pen" && e.buttons & 32) return;
			const mode = effectiveMode(e.pointerType);
			if (mode === "capture") {
				if (!captured && !probing) requestCapture();
				else if (captured) {
					heldButtons = e.buttons;
					flushNow();
					sendButtons(e.buttons);
				}
				notify();
				return;
			}
			if (e.pointerType === "mouse" && setting === "auto" && raw === null && !lockless && !probing && typeof stage.requestPointerLock === "function") {
				probing = true;
				heldButtons = e.buttons;
				followTarget = { x: e.clientX, y: e.clientY };
				probeRaw(stage).then((result) => {
					probing = false;
					raw = result.raw;
					store.setRaw(raw);
					if (result.raw === "yes") {
						followTarget = null;
						releaseAll();
					} else {
						press(e, heldButtons);
					}
					notify();
				});
				try {
					stage.setPointerCapture(e.pointerId);
				} catch (error) {
					// The pointer is already gone.
				}
				return;
			}
			try {
				stage.setPointerCapture(e.pointerId);
			} catch (error) {
				// A synthetic pointer has nothing to capture.
			}
			heldButtons = e.buttons;
			press(e, e.buttons);
		}

		// A press in Follow: the guest pointer is brought under the visitor's, then the button goes down.
		function press(e, bits) {
			followTarget = { x: e.clientX, y: e.clientY };
			flushNow();
			if (!selected) {
				selected = true;
				if (options.onSelect) options.onSelect();
			}
			sendButtons(bits);
			if (!bits) {
				// Released while the probe was out: the click still happens.
				sendButtons(1);
				win.setTimeout(() => sendButtons(0), 60);
			}
		}

		function onMove(e) {
			if (options.stopEvents) e.stopPropagation();
			lastPointerType = e.pointerType || "mouse";
			if (e.pointerType === "touch") return touchMove(e);
			if (e.pointerType === "pen" && e.buttons & 32) return;
			const mode = effectiveMode(e.pointerType);
			if (mode === "capture" && (captured || lockless)) {
				const list = typeof e.getCoalescedEvents === "function" ? e.getCoalescedEvents() : [];
				coalescedAvailable = typeof e.getCoalescedEvents === "function";
				const useList = list.length > 0 && list.some((sample) => sample.movementX || sample.movementY);
				const samples = useList ? list : [e];
				for (const sample of samples) {
					captureX += sample.movementX || 0;
					captureY += sample.movementY || 0;
					wholeDetector.add(sample.movementX || 0, sample.movementY || 0);
				}
				logInput(e, { coalesced: useList ? list.length : 0 });
				schedule();
				return;
			}
			if (mode === "capture") return;
			lastMoveEvent = e;
			coalescedAvailable = typeof e.getCoalescedEvents === "function";
			positionSamples++;
			if (!Number.isInteger(e.clientX) || !Number.isInteger(e.clientY)) fractionalSamples++;
			followTarget = { x: e.clientX, y: e.clientY };
			updateCursor(e);
			logInput(e, { coalesced: coalescedAvailable && e.getCoalescedEvents ? e.getCoalescedEvents().length : 0 });
			schedule();
		}

		function onUp(e) {
			if (options.stopEvents) e.stopPropagation();
			logInput(e);
			if (e.pointerType === "touch") return touchUp(e);
			if (effectiveMode(e.pointerType) === "capture" && !captured) return;
			flushNow();
			heldButtons = e.type === "pointercancel" ? 0 : e.buttons;
			sendButtons(heldButtons);
		}

		function onEnter(e) {
			if (e.pointerType === "touch") return;
			logEvent("enter");
			if (options.parkOnEnter && !heldButtons && effectiveMode(e.pointerType) === "follow") needsPark = true;
		}
		function onLeave(e) {
			if (e.pointerType === "touch") return;
			logEvent("leave");
			if (effectiveMode(e.pointerType) === "follow" && !heldButtons && lastMoveEvent) {
				followTarget = { x: e.clientX, y: e.clientY };
				schedule();
			}
			stage.style.cursor = "";
		}

		// Ctrl with the wheel is the browser's zoom (and a pinch on a touchpad): never taken. With `wheelOverPicture` the wheel acts as soon
		// as the pointer is over the stage; otherwise only once the screen has been pressed, so a visitor scrolling past still scrolls the page.
		function onWheel(e) {
			if (e.ctrlKey) return;
			if (options.stopEvents) e.stopPropagation();
			if (!send.wheel || !(selected || options.wheelOverPicture)) return;
			const rect = shownRect();
			const notches = wheelAccumulator.add(e.deltaY, e.deltaMode, rect.h, now());
			e.preventDefault();
			if (notches) send.wheel(notches);
		}

		function touchDown(e) {
			sync();
			logEvent("touch");
			touchApply(touchActions.down(e.pointerId, e.clientX, e.clientY, e.timeStamp || now()));
			if (!selected) {
				selected = true;
				if (options.onSelect) options.onSelect();
			}
			notify();
		}
		function touchMove(e) {
			logInput(e);
			touchApply(touchActions.move(e.pointerId, e.clientX, e.clientY, e.timeStamp || now()));
			if (e.cancelable) e.preventDefault();
		}
		function touchUp(e) {
			touchApply(touchActions.up(e.pointerId, e.timeStamp || now()));
		}
		function touchApply(actions) {
			for (const action of actions) {
				if (action.kind === "move") {
					// A finger's drag is in CSS pixels of the shown picture; the machine has applied the finger's gain and the speed.
					const rect = shownRect();
					fingerX += (action.dx * rect.guest.w) / rect.w;
					fingerY += (action.dy * rect.guest.h) / rect.h;
					schedule();
				} else if (action.kind === "press") {
					flushNow();
					sendButtons(action.button === "left" ? 1 : 2);
				} else if (action.kind === "release") {
					flushNow();
					sendButtons(0);
				} else if (action.kind === "click") {
					flushNow();
					const bit = action.button === "left" ? 1 : 2;
					sendButtons(bit);
					win.setTimeout(() => sendButtons(0), 60);
				}
			}
		}
		function onLockChange() {
			const locked = doc.pointerLockElement === stage;
			if (locked === captured) return;
			if (lockless && !locked) return;
			captured = locked;
			logEvent(locked ? "lock" : "unlock");
			if (locked) {
				wholeDetector = createWholeDetector({ dpr: win.devicePixelRatio });
				if (setting === "auto" && raw === "yes") stage.style.cursor = "";
			} else releaseAll();
			notify();
		}
		function onLockError() {
			probing = false;
		}
		function onLostCapture() {
			if (heldButtons || sentButtons.some(Boolean)) {
				logEvent("lost-capture");
				releaseAll();
			}
		}
		function onBlur() {
			releaseAll();
		}
		function onVisibility() {
			if (doc.visibilityState === "hidden") releaseAll();
		}
		function onOutside(e) {
			if (!stage.contains(e.target)) selected = false;
		}
		function onContextMenu(e) {
			e.preventDefault();
		}

		const wheelAccumulator = createWheel({ pixelsPerNotch: options.wheelPixelsPerNotch });
		const listeners = [
			[stage, "pointerdown", onDown, { capture: true }],
			[stage, "pointermove", onMove, { capture: true }],
			[stage, "pointerup", onUp, { capture: true }],
			[stage, "pointercancel", onUp, { capture: true }],
			[stage, "pointerenter", onEnter, { capture: true }],
			[stage, "pointerleave", onLeave, { capture: true }],
			[stage, "lostpointercapture", onLostCapture, { capture: true }],
			[stage, "wheel", onWheel, { capture: true, passive: false }],
			[stage, "contextmenu", onContextMenu, { capture: true }],
			[doc, "pointerlockchange", onLockChange, false],
			[doc, "pointerlockerror", onLockError, false],
			[doc, "pointerdown", onOutside, { capture: true }],
			[doc, "visibilitychange", onVisibility, false],
			[win, "blur", onBlur, false],
		];
		for (const [target, name, fn, opts] of listeners) target.addEventListener(name, fn, opts);
		if (options.stopEvents) {
			for (const name of ["mousedown", "mousemove", "mouseup", "mouseleave", "touchstart", "touchmove", "touchend", "touchcancel"]) {
				const swallow = (e) => e.stopPropagation();
				listeners.push([stage, name, swallow, { capture: true, passive: false }]);
				stage.addEventListener(name, swallow, { capture: true, passive: false });
			}
		}

		if (setting === "capture" && lockless) captured = true;
		sync();
		notify();

		const handle = {
			setMode(mode) {
				if (mode !== "follow" && mode !== "capture" && mode !== "auto") return;
				setting = mode;
				if (mode === "auto") store.setMode(null);
				else store.setMode(mode);
				if (mode === "follow") {
					if (doc.pointerLockElement === stage && doc.exitPointerLock) doc.exitPointerLock();
					captured = false;
					releaseAll();
				} else if (mode === "capture" && lockless) captured = true;
				else if (mode === "auto" && captured && chooseMode(setting, raw) === "follow") {
					if (doc.exitPointerLock) doc.exitPointerLock();
					captured = false;
				}
				logEvent("mode");
				notify();
			},
			setSpeed(value) {
				const next = Number(value);
				if (!(next > 0)) return;
				speed = next;
				touchActions.speed = next;
				store.setSpeed(next);
				notify();
			},
			setUnits(next) {
				units.x = next.x;
				units.y = next.y;
				sync();
			},
			park() {
				needsPark = true;
			},
			// The guest's pointer is known to be at (x, y), in guest pixels (a program that centres it on starting): no park is needed.
			assume(x, y) {
				const model = sync();
				model.x = clamp(x, 0, model.w - 1);
				model.y = clamp(y, 0, model.h - 1);
				model.remX = 0;
				model.remY = 0;
				model.edgeX = model.x <= 0 ? -1 : model.x >= model.w - 1 ? 1 : 0;
				model.edgeY = model.y <= 0 ? -1 : model.y >= model.h - 1 ? 1 : 0;
				needsPark = false;
				logEvent("assume");
			},
			state,
			trace: {
				start() {
					tracing = true;
					traceInput = [];
					traceOutput = [];
					traceEvents = [];
				},
				stop() {
					tracing = false;
					const rect = shownRect();
					return {
						version: 1,
						page: win.location ? win.location.pathname : "",
						userAgent,
						dpr: win.devicePixelRatio,
						secure: !!win.isSecureContext,
						mode: chooseMode(setting, raw),
						units: { x: units.x, y: units.y },
						picture: { shown: { left: rect.left, top: rect.top, w: rect.w, h: rect.h }, guest: { w: rect.guest.w, h: rect.guest.h } },
						input: traceInput,
						output: traceOutput,
						events: traceEvents,
					};
				},
			},
			detach() {
				for (const [target, name, fn, opts] of listeners) target.removeEventListener(name, fn, opts);
				if (frameRequest) win.cancelAnimationFrame(frameRequest);
				releaseAll();
				stage.style.cursor = "";
			},
		};
		return handle;
	}

	root.QuillPointer = {
		core: { splitCounts, createMover, createWholeDetector, createWheel, createTouch, probeRaw, createStore, chooseMode, FINGER_GAIN },
		attach,
	};
})(globalThis);
