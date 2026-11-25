// The Try Quill page: starts the js-dos player on the FreeDOS disk when the visitor asks, and not before. The page works without it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

(function () {
	"use strict";

	var player = document.querySelector("[data-player]");
	if (!player) return;
	var startButton = player.querySelector("[data-start]");
	var stopButton = player.querySelector("[data-stop]");
	var fullButton = player.querySelector("[data-fullscreen]");
	var status = player.querySelector("#player-status");
	var stage = player.querySelector("[data-stage]");
	var host = player.querySelector("[data-dos]");

	var BUNDLE = "quill.jsdos";
	var PREFIX = "js-dos/emulators/";
	var running = null;
	var loading = null;

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

	if (!canRun()) {
		document.getElementById("nowasm").hidden = false;
		startButton.disabled = true;
		say("This browser can't run the player.");
		return;
	}

	// The player's style sheet and script are fetched on the first start. Both come from this site.
	function load() {
		if (loading) return loading;
		loading = new Promise(function (resolve, reject) {
			var css = document.createElement("link");
			css.rel = "stylesheet";
			css.href = "js-dos/js-dos.css";
			document.head.appendChild(css);
			var script = document.createElement("script");
			script.src = "js-dos/js-dos.js";
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

	function show(state) {
		player.setAttribute("data-state", state);
		startButton.hidden = state !== "idle";
		stopButton.hidden = state === "idle";
		fullButton.hidden = state !== "running";
	}

	// The disk bundle holds the FreeDOS floppy and the DOSBox settings that boot it.
	function start() {
		startButton.disabled = true;
		say("Loading the player…");
		show("loading");
		load()
			.then(function (Dos) {
				say("Starting FreeDOS…");
				running = Dos(host, {
					url: BUNDLE,
					pathPrefix: PREFIX,
					backend: "dosbox",
					autoStart: true,
					kiosk: true,
					noCloud: true,
					noNetworking: true,
					mouseCapture: false,
					renderAspect: "4/3",
					imageRendering: "pixelated",
					theme: "dark",
					onEvent: function (event) {
						if (event === "emu-ready") say("FreeDOS is starting. It asks which display you want, and chooses the text mode after five seconds.");
						if (event === "ci-ready") {
							show("running");
							say("Running. Select the screen, then use the keys below.");
							stage.focus();
						}
					},
				});
			})
			.catch(function (error) {
				show("idle");
				startButton.disabled = false;
				say("Quill could not start: " + error.message + " Reload the page to try again.");
			});
	}

	function stop() {
		if (running && typeof running.stop === "function") {
			try {
				running.stop();
			} catch (e) {
				// The player is already gone.
			}
		}
		running = null;
		host.textContent = "";
		show("idle");
		startButton.disabled = false;
		say("Stopped. Select Start Quill to boot it again; saved files are gone.");
	}

	startButton.addEventListener("click", start);
	stopButton.addEventListener("click", stop);
	fullButton.addEventListener("click", function () {
		if (stage.requestFullscreen) stage.requestFullscreen();
	});
	show("idle");
})();
