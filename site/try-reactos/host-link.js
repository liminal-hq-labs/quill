// The page's end of the host link: attaches the `host-link` package's link to the machine's COM1 when it starts, drops it when it stops, and
// shows the link in the diagnostics panel. No clipboard, file or time behaviour yet; those arrive as the link's channels are used.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// The ?v= is the first 8 hex of the module's SHA-256; tools/stamp-web.py keeps it current (SITE-14).
import { attach as attachLink } from "./lib/host-link.js?v=7ce01e18";

const readout = document.querySelector("[data-link-readout]");
let host = null;
let timer = null;

function codes(counts) {
	const entries = Object.entries(counts);
	return entries.length ? entries.map(([code, n]) => code + " " + n).join(", ") : "none";
}

function describe(stats) {
	if (stats.state === "closed") return "off";
	if (!stats.peer) return "no helper";
	const peer = (stats.peer.app || "helper") + ", protocol " + stats.peer.proto;
	if (stats.state === "version") return peer + ": a different version, so the link is off";
	const credit = stats.credit.window - stats.credit.outstanding + " of " + stats.credit.window + " bytes of credit";
	const state = stats.state === "stalled" ? " (waiting for credit)" : "";
	return peer + state + ". In " + stats.bytesIn + " bytes, " + stats.messagesIn + " messages. Out " + stats.bytesOut + " bytes, " + stats.messagesOut + " messages. " + credit + ". Errors: " + codes(stats.errors) + ". Reported by the helper: " + codes(stats.peerErrors) + ".";
}

function render() {
	if (!readout) return;
	const dt = document.createElement("dt");
	dt.textContent = "Link";
	const dd = document.createElement("dd");
	dd.textContent = host ? describe(host.stats()) : "off";
	readout.replaceChildren(dt, dd);
}

export function attach(emulator) {
	detach();
	host = attachLink(emulator);
	if (window.tryReactos) window.tryReactos.hostLink = host;
	render();
	timer = setInterval(render, 1000);
}

export function detach() {
	if (timer !== null) clearInterval(timer);
	timer = null;
	if (host) host.detach();
	if (window.tryReactos) window.tryReactos.hostLink = null;
	render();
	host = null;
}
