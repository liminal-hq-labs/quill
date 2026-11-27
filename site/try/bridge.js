// The Try Quill page's bridge to the guest (SITE-60 to SITE-62): two hard disks the page and FreeDOS share, and through them the visitor's
// date. try.js hands over the player's command interface on `ci-ready` with one line; this file attaches the dos-bridge module to it,
// writes D: before the FreeDOS start-up menu is answered, shows the bridge's state, and lets go when the player is stopped.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `?bridge=off` makes this file do nothing at all: no module is fetched, no global is defined, and the page is the one it was before.
// Nothing here makes a network request beyond the module, which comes from this site. The module's address (and so its ?v= version) is
// the script tag's data-module, stamped by scripts/stamp-try.py like every other file of the page.

(function () {
	"use strict";

	var query = "";
	try {
		query = new URLSearchParams(window.location.search).get("bridge") || "";
	} catch (e) {
		// No URLSearchParams: the bridge is on.
	}
	if (query === "off") return;

	var script = document.currentScript;
	var moduleUrl = script && script.getAttribute("data-module");
	var line = document.querySelector("[data-bridge-state]");
	var host = document.querySelector("[data-dos]");
	var loaded = null;
	var current = null;
	var watcher = null;

	function say(text) {
		if (!line) return;
		line.textContent = text;
		line.hidden = !text;
	}

	// The module is fetched once, when the page loads, so it is there before the emulator is ready.
	function load() {
		if (loaded) return loaded;
		if (!moduleUrl) {
			loaded = Promise.reject(new Error("the module's address is missing"));
		} else {
			loaded = import(new URL(moduleUrl, document.baseURI).href);
		}
		loaded.catch(function () {});
		return loaded;
	}

	function detach() {
		if (watcher) watcher.disconnect();
		watcher = null;
		if (current) {
			current.detach();
			current = null;
		}
		say("");
	}

	// Called by try.js on `ci-ready` with js-dos's command interface. The write of D: is started at once and takes about a tenth of a second;
	// try.js presses the start-up menu's key a second later, and FreeDOS reads D: only after that.
	function attach(ci) {
		detach();
		say("bridge: starting");
		var mine = {};
		current = mine;
		load().then(
			function (module) {
				if (current !== mine) return;
				var bridge = module.attach(ci, {
					onChange: function (state) {
						if (current === bridge || current === mine) say(module.describe(state));
					},
				});
				current = bridge;
				// try.js empties the player's folder when it stops or restarts: that is the cue to let go.
				if (host && typeof MutationObserver === "function") {
					watcher = new MutationObserver(function () {
						if (!host.firstChild) detach();
					});
					watcher.observe(host, { childList: true });
				}
				window.quillBridge.ready = bridge.ready;
			},
			function (error) {
				if (current === mine) current = null;
				say("bridge: unavailable (" + (error && error.message ? error.message : "the module did not load") + ")");
			},
		);
	}

	// `active` is the attached bridge (what dos-bridge's attach returned), or null; the page's later features and the tests use it.
	window.quillBridge = {
		attach: attach,
		detach: detach,
		ready: null,
		active: function () {
			return current && typeof current.write === "function" ? current : null;
		},
	};
	load();
})();
