// The page's address book: reads web-manifest.json (uncached) and turns it into the disk, the saved state and the emulator files the
// page starts with. Pure helpers plus two fetches; nothing here touches the machine itself. SITE-14 has the scheme.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export const MANIFEST_FILE = "web-manifest.json";

// What the manifest must name for the page to use it.
function problem(m) {
	if (!m || typeof m !== "object") return "it is not a JSON object";
	if (m.version !== 1) return "its version is " + String(m.version) + " and this page reads version 1";
	if (typeof m.parts !== "string" || !m.parts) return "it names no disk parts";
	if (typeof m.state !== "string" || !m.state) return "it names no saved state";
	if (!m.v86 || typeof m.v86.libv86 !== "string" || typeof m.v86.wasm !== "string") return "it does not say which emulator build the state was taken with";
	return "";
}

// Load the manifest. `name` is the address bar's ?manifest= (empty for the default, "none" to ignore the manifest). The result is
// { status: "none" | "missing" | "bad" | "ready", manifest, url, message }. A missing manifest is a normal state (a local folder
// with no data), not an error.
export async function loadManifest(name, fresh) {
	if (name === "none") return { status: "none", manifest: null, url: "", message: "The manifest is switched off (?manifest=none)." };
	const url = new URL(name || MANIFEST_FILE, location.href);
	let response;
	try {
		const probe = new URL(url);
		probe.searchParams.set("t", String(Date.now()));
		response = await fetch(probe, { cache: fresh ? "reload" : "no-store" });
	} catch (error) {
		return { status: "missing", manifest: null, url: url.href, message: "Could not read " + (name || MANIFEST_FILE) + ": " + error.message + "." };
	}
	if (!response.ok) {
		return { status: "missing", manifest: null, url: url.href, message: "There is no " + (name || MANIFEST_FILE) + " here (" + response.status + "), so there is no saved desktop to start from." };
	}
	let manifest;
	try {
		manifest = await response.json();
	} catch (error) {
		return { status: "bad", manifest: null, url: url.href, message: (name || MANIFEST_FILE) + " is not valid JSON." };
	}
	const why = problem(manifest);
	if (why) return { status: "bad", manifest: null, url: url.href, message: (name || MANIFEST_FILE) + " cannot be used: " + why + "." };
	return { status: "ready", manifest, url: url.href, message: "" };
}

// The page's own address for something the manifest names relative to itself.
function pathOf(relative, base) {
	const u = new URL(relative, base);
	return u.pathname + u.search;
}

// The values a manifest gives the panel: the disk (the parts' address, which asyncImage reads the part list from), the state.
export function manifestValues(loaded) {
	const m = loaded.manifest;
	return { disk: pathOf(m.parts.replace(/\/?$/, "/") + ".img.zst", loaded.url), state: pathOf(m.state, loaded.url) };
}

// Fill the panel's disk and state from the manifest, except the fields the address bar names: those win, even when empty.
export function applyManifest(form, query, loaded) {
	const values = manifestValues(loaded);
	for (const field of ["disk", "state"]) {
		if (!query.has(field)) form.elements[field].value = values[field];
	}
	return values;
}

async function sha256Hex(bytes) {
	const digest = await crypto.subtle.digest("SHA-256", bytes);
	return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("");
}

// `extra` is a bare query pair such as run=ID (the measuring server reads it), or empty.
// The emulator files the page loads, named by the manifest's hashes (?v=), and checked against them. A saved state belongs to
// one v86 build; a different one would start from a broken machine, so a mismatch throws a message for the visitor. The page is
// served over http://127.0.0.1 or https, where crypto.subtle exists; elsewhere the check is skipped and the files still load.
export async function emulatorFiles(manifest, extra, fresh) {
	const files = { libv86: "v86/libv86.mjs", wasm: "v86/v86.wasm", bios: "v86/bios/seabios.bin", vgabios: "v86/bios/vgabios.bin" };
	if (!manifest) return { libv86: "./" + files.libv86 + (extra ? "?" + extra : ""), wasm: files.wasm, bios: files.bios, vgabios: files.vgabios };
	const version = manifest.v86.libv86;
	const out = { libv86: "./" + files.libv86 + "?v=" + version + (extra ? "&" + extra : ""), wasm: files.wasm + "?v=" + manifest.v86.wasm, bios: files.bios + "?v=" + version, vgabios: files.vgabios + "?v=" + version };
	if (!globalThis.crypto || !crypto.subtle) return out;
	for (const [key, want] of [["libv86", manifest.v86.libv86], ["wasm", manifest.v86.wasm]]) {
		const response = await fetch(files[key] + "?v=" + want, { cache: fresh ? "reload" : "default" });
		if (!response.ok) throw new Error("the emulator file " + files[key] + " is missing (" + response.status + ")");
		const got = (await sha256Hex(await response.arrayBuffer())).slice(0, want.length);
		if (got !== want) {
			throw new Error(
				"the saved desktop was made with a different build of the emulator than this page has (" + files[key] + " is " + got + ", the manifest names " + want + "). Choose Force fresh copy in the diagnostics; if that does not help, the site is being updated, so try again in a few minutes",
			);
		}
	}
	return out;
}

// The build line for the diagnostics panel.
export function buildLine(loaded) {
	if (loaded.status !== "ready") return loaded.message;
	const m = loaded.manifest;
	const short = (text, n) => (typeof text === "string" ? text.slice(0, n) : "?");
	const parts = ["Build " + (m.built || "(undated)"), "disk " + short(m.partsSha256, 12), "state " + short(m.stateSha256, 12), "v86 " + (m.v86.version || "?") + " (" + m.v86.libv86 + ", " + m.v86.wasm + ")"];
	if (m.quill) parts.push("Quill " + short(m.quill, 7));
	if (m.reactosLiminal) parts.push("reactos-liminal " + short(m.reactosLiminal, 7));
	return parts.join(" · ");
}

// Replace the browser's own copy of the files that decide what the page starts: the manifest and the emulator. Everything else is
// named by its content, so it cannot be stale.
export async function refetchFresh(loaded) {
	const urls = [loaded.url];
	if (loaded.status === "ready") {
		urls.push("v86/libv86.mjs?v=" + loaded.manifest.v86.libv86, "v86/v86.wasm?v=" + loaded.manifest.v86.wasm);
	}
	await Promise.all(urls.map((u) => fetch(u, { cache: "reload" }).catch(() => {})));
}
