// The pointer lab: Quill's pointer module against a stand-in guest, with the direction error of each stroke.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

(function () {
	"use strict";

	var GUEST = { w: 640, h: 480 };
	var STROKE_PAUSE_MS = 250;
	var JSDOS_UNITS = { x: 1 / 1.37, y: 1 / 1.1 };

	var query = new URLSearchParams(location.search);
	var stage = document.querySelector("[data-stage]");
	var canvas = document.querySelector("[data-picture]");
	var context = canvas.getContext("2d");
	var $ = function (name) {
		return document.querySelector("[data-" + name + "]");
	};

	// The stand-in guest: a relative pointer that moves by whole counts and clamps at its edges.
	var units = { x: 1, y: 1 };
	var callsThisFrame = 0;
	var guest = { countsX: 0, countsY: 0, buttons: [false, false, false], wheel: 0, calls: [] };
	function guestX() {
		return guest.countsX / units.x;
	}
	function guestY() {
		return guest.countsY / units.y;
	}

	var send = {
		move: function (dx, dy) {
			callsThisFrame++;
			guest.countsX = Math.max(0, Math.min((GUEST.w - 1) * units.x, guest.countsX + dx));
			guest.countsY = Math.max(0, Math.min((GUEST.h - 1) * units.y, guest.countsY + dy));
			guest.calls.push({ t: performance.now(), dx: dx, dy: dy, x: guestX(), y: guestY() });
			if (guest.calls.length > 5000) guest.calls.shift();
		},
		buttons: function (left, middle, right) {
			guest.buttons = [left, middle, right];
		},
		wheel: function (notches) {
			guest.wheel += notches;
		},
	};

	var forced = ["follow", "capture"].indexOf(query.get("mouse")) >= 0 ? query.get("mouse") : query.get("mouse") === "lock" ? "capture" : null;
	var handle = QuillPointer.attach({
		stage: stage,
		picture: function () {
			return { el: canvas, w: GUEST.w, h: GUEST.h };
		},
		units: units,
		send: send,
		maxCount: 120,
		parkOnEnter: false,
		defaults: { mode: forced || "auto", speed: Number(query.get("speed")) || 1, force: !!forced || !!query.get("speed") },
		storageKey: "quill-pointer-lab",
		lockless: query.get("lockless") === "1",
		onState: showState,
		onSelect: function () {},
	});
	handle.trace.start();

	// Where the picture is drawn on the page, as the module works it out.
	function shown() {
		var s = handle.state().shown;
		return s;
	}

	// The visitor's path in guest pixels, for scoring: the position under the pointer in Follow, the accumulated movement in
	// Capture. A test that supplies its own truth sets external.
	var input = { x: 0, y: 0, active: false, last: 0, external: false };
	var frames = [];
	var stroke = null;
	var lastStroke = null;

	function noteInput(x, y) {
		var now = performance.now();
		if (!input.active) {
			input.active = true;
			frames = [];
		}
		input.x = x;
		input.y = y;
		input.last = now;
	}

	stage.addEventListener(
		"pointermove",
		function (e) {
			if (input.external || e.pointerType === "touch") return;
			var rect = shown();
			var state = handle.state();
			var kx = GUEST.w / rect.w;
			var ky = GUEST.h / rect.h;
			if (state.mode === "capture") {
				if (!state.captured) return;
				if (!input.active) {
					input.x = guestX();
					input.y = guestY();
				}
				noteInput(input.x + (e.movementX || 0) * kx, input.y + (e.movementY || 0) * ky);
			} else noteInput((e.clientX - rect.left) * kx, (e.clientY - rect.top) * ky);
		},
		{ capture: true },
	);

	function strokeScore() {
		if (frames.length < 4) return null;
		var inputs = frames.map(function (f) {
			return { t: f.t, x: f.ix, y: f.iy };
		});
		var outputs = frames.map(function (f) {
			return { t: f.t, x: f.gx, y: f.gy };
		});
		// Where one count is more than a pixel the output moves in coarser steps, so one axis only has to be missed by more.
		var axisMin = 2 * Math.max(1, 1 / Math.min(units.x, units.y));
		return { frames: frames, metric: QuillPointerMetric.directionError(inputs, outputs, { window: 8, axisMin: axisMin }) };
	}

	function loop(time) {
		var now = performance.now();
		if (input.active) {
			frames.push({ t: time || now, ix: input.x, iy: input.y, gx: guestX(), gy: guestY(), calls: callsThisFrame });
			callsThisFrame = 0;
			if (now - input.last > STROKE_PAUSE_MS) {
				input.active = false;
				var scored = strokeScore();
				if (scored && scored.metric.windows > 0) {
					lastStroke = scored;
					showStroke(scored.metric);
				}
			}
		}
		draw();
		showLive();
		requestAnimationFrame(loop);
	}

	function drawPath(points, colour) {
		if (points.length < 2) return;
		context.beginPath();
		context.moveTo(points[0][0] + 0.5, points[0][1] + 0.5);
		for (var i = 1; i < points.length; i++) context.lineTo(points[i][0] + 0.5, points[i][1] + 0.5);
		context.strokeStyle = colour;
		context.lineWidth = 1;
		context.stroke();
	}

	function draw() {
		var dark = matchMedia("(prefers-color-scheme: dark)").matches;
		context.fillStyle = dark ? "#0b1230" : "#fffdf6";
		context.fillRect(0, 0, GUEST.w, GUEST.h);
		context.strokeStyle = dark ? "#1d2756" : "#ece3cb";
		context.lineWidth = 1;
		for (var x = 0; x <= GUEST.w; x += 80) {
			context.beginPath();
			context.moveTo(x + 0.5, 0);
			context.lineTo(x + 0.5, GUEST.h);
			context.stroke();
		}
		for (var y = 0; y <= GUEST.h; y += 80) {
			context.beginPath();
			context.moveTo(0, y + 0.5);
			context.lineTo(GUEST.w, y + 0.5);
			context.stroke();
		}
		var current = input.active ? frames : lastStroke ? lastStroke.frames : [];
		drawPath(
			current.map(function (f) {
				return [f.ix, f.iy];
			}),
			"#e07a00",
		);
		drawPath(
			current.map(function (f) {
				return [f.gx, f.gy];
			}),
			"#2a6fdb",
		);
		var px = Math.round(guestX());
		var py = Math.round(guestY());
		context.fillStyle = guest.buttons[0] || guest.buttons[2] ? "#d6336c" : dark ? "#ffffff" : "#000000";
		context.strokeStyle = dark ? "#000000" : "#ffffff";
		context.beginPath();
		context.moveTo(px, py);
		context.lineTo(px, py + 17);
		context.lineTo(px + 4, py + 13);
		context.lineTo(px + 7, py + 19);
		context.lineTo(px + 10, py + 18);
		context.lineTo(px + 7, py + 12);
		context.lineTo(px + 12, py + 12);
		context.closePath();
		context.fill();
		context.stroke();
	}

	function text(name, value) {
		var el = $(name);
		if (el && el.textContent !== value) el.textContent = value;
	}

	function showStroke(m) {
		text("stroke-windows", String(m.windows));
		text("stroke-median", m.median.toFixed(1) + "°");
		text("stroke-p95", m.p95.toFixed(1) + "°");
		text("stroke-axis", (m.axisOnly * 100).toFixed(1) + "%");
		text("stroke-end", "(" + m.endpoint[0].toFixed(1) + ", " + m.endpoint[1].toFixed(1) + ") pixels");
	}

	function yesNo(value) {
		return value ? "yes" : "no";
	}

	function showLive() {
		var s = handle.state();
		text("state-mode", s.mode === "follow" ? "Follow" : "Capture");
		text("state-raw", s.raw);
		text("state-secure", yesNo(s.secure));
		text("state-coalesced", yesNo(s.coalesced));
		text("state-fraction", (s.fractionalShare * 100).toFixed(0) + "%");
		text("state-whole", yesNo(s.wholeOnly));
		text("state-dpr", String(s.dpr));
		text("state-calls", String(s.packetsPerFrame));
		var b = guest.buttons;
		text("state-buttons", (b[0] ? "left " : "") + (b[1] ? "middle " : "") + (b[2] ? "right " : "") + "wheel " + guest.wheel);
	}

	function showState(s) {
		var capture = s.mode === "capture";
		$("speed-row").hidden = !(capture || s.pointerType === "touch");
		$("whole").hidden = !s.wholeOnly;
		text("hint", capture ? (s.captured ? "Capture: the pointer is captured. Press Esc to let go." : "Capture: press the screen to capture the pointer.") : "In Follow, the guest pointer stays under yours.");
		Array.prototype.forEach.call(document.querySelectorAll('input[name="mouse"]'), function (radio) {
			radio.checked = radio.value === s.setting;
		});
	}

	Array.prototype.forEach.call(document.querySelectorAll('input[name="mouse"]'), function (radio) {
		radio.addEventListener("change", function () {
			if (radio.checked) handle.setMode(radio.value);
		});
	});
	Array.prototype.forEach.call(document.querySelectorAll('input[name="units"]'), function (radio) {
		radio.addEventListener("change", function () {
			if (!radio.checked) return;
			var next = radio.value === "jsdos" ? JSDOS_UNITS : { x: 1, y: 1 };
			// The guest keeps its place in pixels; its counts change with the units.
			var px = guestX();
			var py = guestY();
			units.x = next.x;
			units.y = next.y;
			guest.countsX = px * next.x;
			guest.countsY = py * next.y;
			handle.setUnits(next);
		});
	});
	$("speed-row").querySelector("input").addEventListener("input", function (e) {
		handle.setSpeed(e.target.value);
		text("speed-value", String(e.target.value));
	});
	$("use-follow").addEventListener("click", function () {
		handle.setMode("follow");
	});
	$("clear").addEventListener("click", function () {
		frames = [];
		lastStroke = null;
		input.active = false;
	});
	$("save").addEventListener("click", function () {
		var trace = handle.trace.stop();
		handle.trace.start();
		var link = document.createElement("a");
		link.href = URL.createObjectURL(new Blob([JSON.stringify(trace)], { type: "application/json" }));
		link.download = "pointer-trace-" + new Date().toISOString().replace(/[:.]/g, "-") + ".json";
		document.body.appendChild(link);
		link.click();
		link.remove();
		setTimeout(function () {
			URL.revokeObjectURL(link.href);
		}, 1000);
	});

	// Handles for the tests (harness/pointer-lab.ts): the guest's own record and the last stroke's frames.
	window.__lab = {
		handle: handle,
		guest: guest,
		units: units,
		input: input,
		guestPosition: function () {
			return [guestX(), guestY()];
		},
		lastStroke: function () {
			return lastStroke ? { frames: lastStroke.frames, metric: lastStroke.metric } : null;
		},
		reset: function () {
			frames = [];
			lastStroke = null;
			input.active = false;
			input.external = false;
			guest.wheel = 0;
			guest.calls.length = 0;
		},
		// A test that knows the true hand path gives it here, one position per call, in guest pixels.
		setInput: function (x, y) {
			input.external = true;
			noteInput(x, y);
		},
	};

	text("speed-value", String(handle.state().speed));
	document.querySelector("#speed").value = String(handle.state().speed);
	showState(handle.state());
	requestAnimationFrame(loop);
})();
