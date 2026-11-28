// The Try Quill page: starts the js-dos player on the FreeDOS disk when the visitor asks, and not before. The page works without it.
//
// Cache busting (SITE-4): the page asks try-manifest.json which bundle is current (never from a cache), loads that content-hashed
// bundle, gives js-dos a storage folder named after the hash, and deletes the folders of every other build. See docs/decisions.md.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

(function () {
	"use strict";

	var player = document.querySelector("[data-player]");
	if (!player) return;
	var startButton = player.querySelector("[data-start]");
	var restartButton = player.querySelector("[data-restart]");
	var fullButton = player.querySelector("[data-fullscreen]");
	var status = player.querySelector("#player-status");
	var stage = player.querySelector("[data-stage]");
	var host = player.querySelector("[data-dos]");
	var keysBar = player.querySelector("[data-keybar]");
	var modes = player.querySelectorAll("input[name=mode]");
	var speedRow = player.querySelector("[data-speed]");
	var speedInput = player.querySelector("[data-speed-input]");
	var mouseHint = player.querySelector("[data-mouse-hint]");
	var mouseControls = player.querySelector("[data-mouse-controls]");
	var mouseMode = player.querySelector("[data-mouse-mode]");
	var mouseRadios = player.querySelectorAll("input[name=mouse]");
	var pointerDebug = player.querySelector("[data-pointer-debug]");
	var traceButton = player.querySelector("[data-trace]");
	var buildLine = document.querySelector("[data-build]");
	var freshLink = document.querySelector("[data-fresh]");

	var PREFIX = "js-dos/emulators/";
	// FreeDOS's start-up menu (scripts/try-disk/FDCONFIG.SYS) has one entry per mode, chosen by its digit key (js-dos key codes).
	var MODE_KEYS = { gfx: 49, g16: 50, text: 51 };
	var MODE_NAMES = { gfx: "graphics, 256 colours", g16: "graphics, 16 colours", text: "text mode" };
	// js-dos key codes for the on-screen keys.
	var KEYS = { esc: 256, enter: 257, tab: 258, up: 265, down: 264, left: 263, right: 262, f10: 299, f7: 296 };
	var STORE = "quill-try-";
	// A key pressed before FreeDOS shows its menu waits in the keyboard buffer, so the delay only keeps the press from landing on the boot messages.
	var MENU_DELAY_MS = 1000;

	var manifest = null;
	var manifestReady = null;
	var running = null;
	var loading = null;
	var ci = null;
	var menuTimer = 0;

	function say(text) {
		status.textContent = text;
	}

	function canRun() {
		try {
			return typeof WebAssembly === "object" && typeof WebAssembly.instantiate === "function";
		} catch (e) {
			return false;
		}
	}

	function currentMode() {
		for (var i = 0; i < modes.length; i++) if (modes[i].checked) return modes[i].value;
		return "gfx";
	}

	try {
		var remembered = window.localStorage.getItem("quill-try-mode");
		if (remembered && MODE_KEYS[remembered]) {
			for (var i = 0; i < modes.length; i++) modes[i].checked = modes[i].value === remembered;
		}
	} catch (e) {
		// Storage is blocked (a private window): the default mode stays.
	}

	// The manifest says which bundle is current. It is asked for with no-store and a unique query, so neither the browser's cache nor a
	// shared one can answer with an older build.
	function loadManifest() {
		return fetch("try-manifest.json?t=" + Date.now(), { cache: "no-store" })
			.then(function (r) {
				if (!r.ok) throw new Error("The build manifest is missing (" + r.status + ").");
				return r.json();
			})
			.then(function (m) {
				if (!m || !/^quill\.[0-9a-f]{12}\.jsdos$/.test(m.bundle || "")) throw new Error("The build manifest is not valid.");
				manifest = m;
				return m;
			});
	}

	function hashOf(m) {
		return m.bundle.slice(6, 18);
	}

	function showBuild(m) {
		if (!buildLine) return;
		var day = (m.built || "").slice(0, 10);
		buildLine.textContent = "Build: " + hashOf(m) + (day ? " " + day : "");
		buildLine.title = "sha-256 " + m.sha256 + ", " + m.size + " bytes, built " + m.built;
	}

	// js-dos 8 keeps the bundles it has fetched in the browser's origin-private file system, in a folder it is told to use (the
	// opfsRoot option), under a name made from the bundle's address. The folder here is named after the bundle's hash, so a new build
	// starts empty and cannot find an old copy; folders of other builds, and js-dos's own default one (an earlier version of this
	// page used it), are deleted. Without the file system (an insecure address, a private window) there is nothing to clear.
	function clearCaches(keep) {
		var storage = navigator.storage;
		if (!storage || typeof storage.getDirectory !== "function") return Promise.resolve(0);
		return storage
			.getDirectory()
			.then(function (root) {
				var names = [];
				var walk = typeof root.keys === "function" ? root.keys() : null;
				if (!walk) return 0;
				var next = function () {
					return walk.next().then(function (step) {
						if (step.done) return null;
						names.push(step.value);
						return next();
					});
				};
				return next().then(function () {
					var stale = names.filter(function (n) {
						return n === "jsdos" || (n.indexOf(STORE) === 0 && n !== keep);
					});
					return Promise.all(
						stale.map(function (n) {
							return root.removeEntry(n, { recursive: true }).catch(function () {});
						}),
					).then(function () {
						return stale.length;
					});
				});
			})
			.catch(function () {
				return 0;
			});
	}

	function forceFresh(event) {
		event.preventDefault();
		say("Clearing the saved copy…");
		var urls = ["try-manifest.json", "try.js", "js-dos/js-dos.js", "js-dos/js-dos.css"];
		if (manifest) urls.push(manifest.bundle);
		// Replace whatever the browser's own cache holds for the files, then clear js-dos's, then load the page again.
		var refetch = urls.map(function (u) {
			return fetch(u + (u === "try-manifest.json" ? "?t=" + Date.now() : ""), { cache: "reload" }).catch(function () {});
		});
		Promise.all(refetch)
			.then(function () {
				return clearCaches("");
			})
			.then(function () {
				window.location.reload();
			});
	}

	if (freshLink) freshLink.addEventListener("click", forceFresh);

	manifestReady = loadManifest().then(
		function (m) {
			showBuild(m);
			clearCaches(STORE + hashOf(m));
			return m;
		},
		function (error) {
			if (buildLine) buildLine.textContent = "Build: not available";
			throw error;
		},
	);
	manifestReady.catch(function () {});

	if (!canRun()) {
		document.getElementById("nowasm").hidden = false;
		startButton.disabled = true;
		say("This browser can't run the player.");
		return;
	}

	// The player's style sheet and script are fetched on the first start, with the version the manifest names. Both come from this site.
	function load(m) {
		if (loading) return loading;
		var v = "?v=" + m.assets["js-dos.js"];
		loading = new Promise(function (resolve, reject) {
			var css = document.createElement("link");
			css.rel = "stylesheet";
			css.href = "js-dos/js-dos.css?v=" + m.assets["js-dos.css"];
			document.head.appendChild(css);
			var script = document.createElement("script");
			script.src = "js-dos/js-dos.js" + v;
			script.onload = function () {
				if (typeof window.Dos === "function") resolve(window.Dos);
				else reject(new Error("The player did not load."));
			};
			script.onerror = function () {
				reject(new Error("The player could not be fetched."));
			};
			document.head.appendChild(script);
		});
		loading.catch(function () {
			loading = null;
		});
		return loading;
	}

	// The pointer is the page's own (SITE-22): js-dos's mouse capture is off and the stage stops its mouse, pointer and touch events, so
	// js-dos sees none of them. site/js/pointer.js follows the visitor's pointer (or captures it, or acts as a trackpad for a finger) and
	// gives this adapter whole counts of relative motion, once an animation frame.
	//
	// The units are CuteMouse's, measured (docs/plans/pointer.md, "js-dos units"). The disk's driver is started with /R13 and its speed-up
	// is switched off by scripts/patch-ctmouse.py, so it is linear: one count moves the pointer 111/256 of a pixel across and 193/256 of a pixel
	// down (0.4336 and 0.7539; the driver keeps the fraction), whatever the speed. The guest's pointer range is 640 by 480 in both graphics modes; in text mode it is a virtual 640 by 200,
	// shown over the whole 4 by 3 screen, so the module is given a 640 by 480 picture and the vertical units are scaled by 200/480.
	var COUNTS_PER_PIXEL = { x: 256 / 111, y: 256 / 193 };
	var TEXT_RANGE_HEIGHT = 200;
	var PICTURE = { w: 640, h: 480 };

	var pointer = null;
	var query = new URLSearchParams(window.location.search);
	var debugPointer = query.get("pointer") === "debug";
	var debugTimer = 0;
	var frameSizeAt = "";

	function pointerUnits(mode) {
		return { x: COUNTS_PER_PIXEL.x, y: (COUNTS_PER_PIXEL.y * (mode === "text" ? TEXT_RANGE_HEIGHT : PICTURE.h)) / PICTURE.h };
	}

	// ?mouse=follow or capture (or the old lock) and ?speed=N set the pointer for this visit; without them the stored choice, or Follow,
	// is used.
	function pointerDefaults() {
		var asked = query.get("mouse");
		var mode = asked === "lock" ? "capture" : asked === "follow" || asked === "capture" ? asked : "auto";
		var speed = Number(query.get("speed"));
		return { mode: mode, speed: speed > 0 ? speed : 1, force: mode !== "auto" || speed > 0 };
	}

	// Says whether the browser has the pointer, on a screen that can lock it (a touch screen cannot, and has its own gestures).
	var touchOnly = typeof window.matchMedia === "function" && window.matchMedia("(pointer: coarse)").matches;

	function hint(state) {
		mouseHint.textContent = "";
		if (!ci || !state) return;
		if (touchOnly || state.pointerType === "touch") {
			mouseHint.textContent = "Drag a finger to move Quill's pointer, tap to click, tap with two fingers to right-click.";
		} else if (state.mode === "capture") {
			mouseHint.textContent = state.captured
				? "The mouse is captured. Press Esc to get your pointer back."
				: "Click the screen to capture the mouse, so Quill's pointer follows yours.";
			if (state.wholeOnly) {
				mouseHint.appendChild(document.createTextNode(" This browser reports captured moves in whole pixels, which can lose diagonals. Follow is smoother here. "));
				var useFollow = document.createElement("button");
				useFollow.type = "button";
				useFollow.className = "button button--alt";
				useFollow.textContent = "Use Follow";
				useFollow.addEventListener("click", function () {
					if (pointer) pointer.setMode("follow");
				});
				mouseHint.appendChild(useFollow);
			}
		} else {
			mouseHint.textContent = "In Follow, Quill's pointer stays under yours.";
		}
	}

	function renderPointer(state) {
		for (var i = 0; i < mouseRadios.length; i++) mouseRadios[i].checked = mouseRadios[i].value === state.mode;
		mouseMode.hidden = touchOnly;
		// The slider acts only where counts are scaled by it: Capture, and a finger. In Follow the guest's pointer is under yours.
		speedRow.hidden = !(state.mode === "capture" || touchOnly || state.pointerType === "touch");
		if (Number(speedInput.value) !== state.speed) speedInput.value = String(state.speed);
		hint(state);
	}

	function pointerLine() {
		if (!pointer) return;
		var s = pointer.state();
		pointerDebug.textContent =
			"Pointer: " + s.mode + (s.captured ? " (captured)" : "") + ", raw " + s.raw + ", secure " + (s.secure ? "yes" : "no") + ", coalesced " + (s.coalesced ? "yes" : "no") +
			", fractional " + Math.round(s.fractionalShare * 100) + " %, whole-only " + (s.wholeOnly ? "yes" : "no") + ", dpr " + s.dpr + ", calls a frame " + s.packetsPerFrame +
			", units " + pointerUnitsText + ", model " + (s.model ? Math.round(s.model[0]) + "," + Math.round(s.model[1]) : "none");
	}
	var pointerUnitsText = "";

	function saveTrace() {
		if (!pointer) return;
		var trace = pointer.trace.stop();
		trace.page = "try";
		trace.display = currentRunMode;
		var link = document.createElement("a");
		link.href = URL.createObjectURL(new Blob([JSON.stringify(trace)], { type: "application/json" }));
		link.download = "quill-try-pointer-trace.json";
		document.body.appendChild(link);
		link.click();
		link.remove();
		window.setTimeout(function () {
			URL.revokeObjectURL(link.href);
		}, 1000);
		pointer.trace.start();
	}

	var currentRunMode = "gfx";

	// DOSBox joins the calls that arrive in one tick of its clock (about 25 ms; measured) into one report for the mouse, and a report holds
	// 255 counts an axis, so more wraps round (one call of 300 counts moves the pointer as 44 do). A call that clamps the guest pointer at an
	// edge (a park, or the overshoot on reaching an edge) also needs its own report: joined with the call after it, the two would cancel before
	// the guest clamped. So calls go out in windows of 30 ms that hold at most 80 counts an axis between them (three windows can share a tick
	// when the emulator stalls: 240 is under 255), and a window that has had a clamping call is closed to the calls after it.
	var PACE_WINDOW_MS = 30;
	var PACE_COUNTS = 80;
	var paceQueue = [];
	var paceTimer = 0;
	var paceFrom = -1e9;
	var paceX = 0;
	var paceY = 0;
	var paceClosed = false;

	function paceRun() {
		paceTimer = 0;
		while (paceQueue.length && ci) {
			var job = paceQueue[0];
			var now = window.performance.now();
			var fresh = now - paceFrom >= PACE_WINDOW_MS;
			var room = job.run || (Math.abs(paceX + job.dx) <= PACE_COUNTS && Math.abs(paceY + job.dy) <= PACE_COUNTS);
			if (!fresh && (paceClosed || !room)) {
				paceTimer = window.setTimeout(paceRun, paceFrom + PACE_WINDOW_MS - now + 1);
				return;
			}
			if (fresh) {
				paceFrom = now;
				paceX = 0;
				paceY = 0;
				paceClosed = false;
			}
			paceQueue.shift();
			if (job.run) job.run();
			else {
				ci.sendMouseRelativeMotion(job.dx, job.dy);
				paceX += job.dx;
				paceY += job.dy;
				if (job.clamps) paceClosed = true;
			}
		}
	}

	function pace(job) {
		paceQueue.push(job);
		if (!paceTimer) paceRun();
	}

	// What is left (a button release from detaching) goes at once, and the rest is dropped: the player is being replaced.
	function paceFlush() {
		window.clearTimeout(paceTimer);
		paceTimer = 0;
		if (ci)
			paceQueue.forEach(function (job) {
				if (job.run) job.run();
			});
		paceQueue = [];
		paceFrom = -1e9;
		paceClosed = false;
	}

	function attachPointer(mode) {
		detachPointer();
		if (!window.QuillPointer) return;
		var units = pointerUnits(mode);
		pointerUnitsText = units.x.toFixed(3) + " by " + units.y.toFixed(3);
		var down = { left: false, right: false };
		pointer = window.QuillPointer.attach({
			stage: stage,
			picture: function () {
				return { el: host.querySelector("canvas") || host, w: PICTURE.w, h: PICTURE.h };
			},
			units: units,
			send: {
				move: function (dx, dy, clamps) {
					pace({ dx: dx, dy: dy, clamps: clamps });
				},
				// js-dos has a left and a right button; the middle one is dropped.
				buttons: function (left, middle, right) {
					pace({
						run: function () {
							if (left !== down.left) ci.sendMouseButton(0, (down.left = left));
							if (right !== down.right) ci.sendMouseButton(1, (down.right = right));
						},
					});
				},
				wheel: null,
			},
			maxCount: PACE_COUNTS,
			// The units are exact (the driver is linear) and every count is sent, so the model stays true between visits to the screen. Where the
			// guest's pointer is comes from Quill's start (the display changes and Quill centres it: watchFrameSize), and from the edges.
			parkOnEnter: false,
			defaults: pointerDefaults(),
			storageKey: "quill-try-pointer",
			stopEvents: true,
			onState: renderPointer,
			// The press that selects the screen gives the emulated PC the keyboard.
			onSelect: function () {
				stage.focus({ preventScroll: true });
			},
		});
		if (debugPointer) {
			pointerDebug.hidden = false;
			traceButton.hidden = false;
			pointer.trace.start();
			window.clearInterval(debugTimer);
			debugTimer = window.setInterval(pointerLine, 500);
		}
	}

	function detachPointer() {
		window.clearInterval(debugTimer);
		pointerDebug.hidden = true;
		traceButton.hidden = true;
		if (pointer) pointer.detach();
		pointer = null;
		paceFlush();
		mouseHint.textContent = "";
	}

	// A change of the guest's display (Quill starting, after FreeDOS's menu or the prompt) is Quill putting its pointer in the middle of its
	// screen: the middle of 640 by 480 in graphics, and in text mode the middle cell of the 80 by 25 (column 40, row 12) of eight virtual
	// pixels each, in the 640 by 480 the module is given.
	function watchFrameSize() {
		if (!ci || typeof ci.events !== "function") return;
		ci.events().onFrameSize(function (width, height) {
			var key = width + "x" + height;
			if (key === frameSizeAt) return;
			frameSizeAt = key;
			if (pointer) pointer.assume(PICTURE.w / 2, currentRunMode === "text" ? (12 * 8 * PICTURE.h) / TEXT_RANGE_HEIGHT : PICTURE.h / 2);
		});
	}

	function show(state) {
		player.setAttribute("data-state", state);
		startButton.hidden = state !== "idle";
		restartButton.hidden = state === "idle";
		fullButton.hidden = state !== "running";
		keysBar.hidden = state !== "running";
		mouseControls.hidden = state !== "running";
		for (var i = 0; i < modes.length; i++) modes[i].disabled = state === "loading";
	}

	// Picks the FreeDOS menu entry for the chosen mode. The menu is on screen when the emulator is ready and waits there for a key.
	function chooseMode(mode) {
		window.clearTimeout(menuTimer);
		menuTimer = window.setTimeout(function () {
			if (ci) ci.simulateKeyPress(MODE_KEYS[mode]);
		}, MENU_DELAY_MS);
	}

	function start() {
		var mode = currentMode();
		try {
			window.localStorage.setItem("quill-try-mode", mode);
		} catch (e) {
			// Not remembered.
		}
		startButton.disabled = true;
		restartButton.disabled = true;
		say("Loading the player…");
		show("loading");
		manifestReady
			.then(function (m) {
				return load(m).then(function (Dos) {
					say("Starting FreeDOS…");
					ci = null;
					running = Dos(host, {
						// The bundle's name holds its hash, and so does the storage folder: one build never sees another's copy.
						url: m.bundle,
						opfsRoot: STORE + hashOf(m),
						fsChanges: { local: false },
						pathPrefix: PREFIX,
						pathSuffix: "?v=" + m.assets.emulators,
						backend: "dosbox",
						mouseCapture: false,
						autoStart: true,
						kiosk: true,
						noCloud: true,
						noNetworking: true,
						renderAspect: "4/3",
						imageRendering: "pixelated",
						theme: "dark",
						onEvent: function (event, arg) {
							if (event === "emu-ready") say("Starting FreeDOS…");
							if (event === "ci-ready") {
								ci = arg;
								if (window.quillBridge) window.quillBridge.attach(arg);
								currentRunMode = mode;
								// js-dos never captures the mouse or reads the stage's pointer: the page's pointer module does both.
								if (running && typeof running.setMouseCapture === "function") running.setMouseCapture(false);
								attachPointer(mode);
								watchFrameSize();
								chooseMode(mode);
								show("running");
								restartButton.disabled = false;
								say("Running in " + MODE_NAMES[mode] + ". Move the mouse over the screen, or on a touch screen drag to move the pointer and tap to click.");
							}
						},
					});
				});
			})
			.catch(function (error) {
				show("idle");
				startButton.disabled = false;
				restartButton.disabled = false;
				say("Quill could not start: " + error.message + " Reload the page to try again.");
			});
	}

	function stop() {
		window.clearTimeout(menuTimer);
		if (running && typeof running.stop === "function") {
			try {
				running.stop();
			} catch (e) {
				// The player is already gone.
			}
		}
		detachPointer();
		running = null;
		ci = null;
		frameSizeAt = "";
		host.textContent = "";
	}

	// Restart boots the emulated PC again, in the mode chosen now; saved files are gone.
	function restart() {
		stop();
		start();
	}

	speedInput.addEventListener("input", function () {
		if (pointer) pointer.setSpeed(speedInput.value);
	});
	for (var m = 0; m < mouseRadios.length; m++) {
		mouseRadios[m].addEventListener("change", function (event) {
			if (pointer && event.target.checked) pointer.setMode(event.target.value);
		});
	}
	traceButton.addEventListener("click", saveTrace);
	startButton.addEventListener("click", start);
	restartButton.addEventListener("click", restart);
	fullButton.addEventListener("click", function () {
		var el = stage;
		var go = el.requestFullscreen || el.webkitRequestFullscreen;
		if (go) go.call(el);
		else say("This browser can't show the screen full size; the page's screen is as large as it will go.");
	});
	keysBar.addEventListener("click", function (event) {
		var b = event.target.closest("[data-key]");
		if (!b || !ci) return;
		var names = b.getAttribute("data-key").split("+");
		ci.simulateKeyPress.apply(
			ci,
			names.map(function (n) {
				return KEYS[n];
			}),
		);
	});
	show("idle");
})();
