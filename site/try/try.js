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
	var buildLine = document.querySelector("[data-build]");
	var freshLink = document.querySelector("[data-fresh]");

	var PREFIX = "js-dos/emulators/";
	// FreeDOS's start-up menu (scripts/try-disk/FDCONFIG.SYS) has one entry per mode, chosen by its digit key (js-dos key codes).
	var MODE_KEYS = { gfx: 49, g16: 50, text: 51 };
	var MODE_NAMES = { gfx: "graphics, 256 colours", g16: "graphics, 16 colours", text: "text mode" };
	// js-dos key codes for the on-screen keys.
	var KEYS = { esc: 256, enter: 257, tab: 258, up: 265, down: 264, left: 263, right: 262, f10: 299, f7: 296 };
	var STORE = "quill-try-";
	// Measured, in a browser with pointer lock: with CuteMouse's /R13 and sensitivity 0.5, the pointer moves about 1.05 screen pixels for each
	// pixel the mouse moves when the screen is drawn at its own size.
	var POINTER_SPEED = 1.05;
	// A key pressed before FreeDOS shows its menu waits in the keyboard buffer, so the delay only keeps the press from landing on the boot messages.
	var MENU_DELAY_MS = 1000;

	var manifest = null;
	var manifestReady = null;
	var running = null;
	var loading = null;
	var ci = null;
	var menuTimer = 0;
	var stageWatch = null;

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

	// The pointer is relative: js-dos sends each movement of the mouse (or of a finger, on a touch screen) times 8^(2t-1), for the
	// sensitivity t it is given (0.5 is 1:1). CuteMouse, started with /R13 in the disk's batch files, moves the pointer about the same
	// distance on both axes, so t is set to make the pointer move as far on the page as the mouse does: a screen drawn at 1.5 times its
	// size needs a pointer that moves two thirds as many pixels. It is set again when the stage changes size.
	function fitPointer() {
		var canvas = host.querySelector("canvas");
		if (!running || !canvas || !canvas.clientWidth || typeof running.setMouseSensitivity !== "function") return;
		var scale = (canvas.clientWidth / 640) * POINTER_SPEED / (Number(speedInput.value) || 1);
		var t = 0.5 + Math.log(1 / scale) / Math.log(8) / 2;
		running.setMouseSensitivity(Math.max(0, Math.min(1, t)));
	}

	// A touch screen has no pointer lock, and js-dos's own touch pad holds the button down for every touch. This one works like a laptop's:
	// a finger drags the pointer, a short tap clicks where the pointer is, and holding a finger still for a moment (then dragging) holds
	// the button, to select text or drag a scroll bar. It sends js-dos's raw pointer movements, which CuteMouse turns into about
	// UNIT_X by UNIT_Y screen pixels each, scaled so the pointer follows the finger across the page (with a little gain, as a pad has).
	var UNIT_X = 1.37;
	var UNIT_Y = 1.1;
	var FINGER_GAIN = 1.4;
	var HOLD_MS = 350;
	var TAP_MS = 300;
	var finger = null;

	function pointerUnits() {
		var canvas = host.querySelector("canvas");
		var scale = canvas && canvas.clientWidth ? canvas.clientWidth / 640 : 1;
		var gain = (FINGER_GAIN * (Number(speedInput.value) || 1)) / scale;
		return { x: gain / UNIT_X, y: gain / UNIT_Y };
	}

	function onTouch(event) {
		if (!ci) return;
		event.preventDefault();
		event.stopPropagation();
		var t = event.changedTouches[0];
		if (event.type === "touchstart") {
			if (finger) {
				window.clearTimeout(finger.timer);
				if (finger.held) ci.sendMouseButton(0, false);
			}
			finger = { id: t.identifier, x: t.clientX, y: t.clientY, at: Date.now(), moved: 0, held: false, timer: 0 };
			finger.timer = window.setTimeout(function () {
				if (finger && finger.moved < 8 && ci) {
					finger.held = true;
					ci.sendMouseButton(0, true);
				}
			}, HOLD_MS);
			return;
		}
		if (!finger || t.identifier !== finger.id) return;
		if (event.type === "touchmove") {
			var u = pointerUnits();
			var dx = t.clientX - finger.x;
			var dy = t.clientY - finger.y;
			finger.x = t.clientX;
			finger.y = t.clientY;
			finger.moved += Math.abs(dx) + Math.abs(dy);
			if (finger.moved >= 8 && !finger.held) window.clearTimeout(finger.timer);
			ci.sendMouseRelativeMotion(dx * u.x, dy * u.y);
			return;
		}
		window.clearTimeout(finger.timer);
		var tap = !finger.held && event.type === "touchend" && finger.moved < 10 && Date.now() - finger.at < TAP_MS;
		if (finger.held) ci.sendMouseButton(0, false);
		else if (tap) {
			ci.sendMouseButton(0, true);
			window.setTimeout(function () {
				if (ci) ci.sendMouseButton(0, false);
			}, 50);
		}
		finger = null;
	}

	["touchstart", "touchmove", "touchend", "touchcancel"].forEach(function (name) {
		stage.addEventListener(name, onTouch, { capture: true, passive: false });
	});

	// js-dos listens for pointer events as well as touch events; keep the finger's from reaching it.
	["pointerdown", "pointermove", "pointerup", "pointercancel"].forEach(function (name) {
		stage.addEventListener(
			name,
			function (event) {
				if (event.pointerType === "touch") event.stopPropagation();
			},
			{ capture: true },
		);
	});

	// Says whether the browser has the pointer, on a screen that can lock it (a touch screen cannot, and has its own gestures).
	var touchOnly = typeof window.matchMedia === "function" && window.matchMedia("(pointer: coarse)").matches;
	function hint() {
		if (touchOnly || !ci) {
			mouseHint.textContent = "";
			return;
		}
		mouseHint.textContent = document.pointerLockElement
			? "The mouse is captured. Press Esc to get your pointer back."
			: "Click the screen to capture the mouse, so Quill's pointer follows yours.";
	}
	document.addEventListener("pointerlockchange", hint);

	function watchStage() {
		if (stageWatch) stageWatch.disconnect();
		stageWatch = null;
		if (typeof ResizeObserver === "function") {
			stageWatch = new ResizeObserver(fitPointer);
			stageWatch.observe(stage);
		}
		fitPointer();
	}

	function show(state) {
		player.setAttribute("data-state", state);
		startButton.hidden = state !== "idle";
		restartButton.hidden = state === "idle";
		fullButton.hidden = state !== "running";
		keysBar.hidden = state !== "running";
		speedRow.hidden = state !== "running";
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
								watchStage();
								chooseMode(mode);
								show("running");
								hint();
								restartButton.disabled = false;
								say("Running in " + MODE_NAMES[mode] + ". Click the screen to capture the mouse, or on a touch screen drag to move the pointer and tap to click.");
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
		running = null;
		ci = null;
		hint();
		if (stageWatch) stageWatch.disconnect();
		stageWatch = null;
		host.textContent = "";
	}

	// Restart boots the emulated PC again, in the mode chosen now; saved files are gone.
	function restart() {
		stop();
		start();
	}

	speedInput.addEventListener("input", fitPointer);
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
