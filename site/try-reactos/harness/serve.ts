// Local static server for the ReactOS-in-the-browser experiment: HTTP range requests, per-client byte counters and a JSON request log.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Runs on Bun or Node 22+: `bun harness/serve.ts` (from site/try-reactos). Binds to 127.0.0.1 unless HOST is set (HOST=0.0.0.0 serves the LAN, read-only: the write endpoints answer this machine only).
//
// Environment: PORT (default 8086), IMAGES_DIR (default ~/.cache/reactos/web, mapped to /images/),
// COI=1 adds Cross-Origin-Opener-Policy and Cross-Origin-Embedder-Policy, LOG_FILE (default out/requests.json).
//
// Endpoints: GET /__stats (totals per client and per kind), POST /__stats/reset (optionally ?client=ID),
// POST /__stats/flush (write the log now), POST /__put/NAME (save the request body into the images folder). A "client" is the `run` query parameter of the page that made
// the request (read from the Referer, or from the request's own query string), or the remote address when there is none.

import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const tryRoot = path.resolve(here, "..");
// The whole product site is served, so the page's ../css and ../img links resolve; the page is at /try-reactos/.
const siteRoot = path.resolve(tryRoot, "..");
const port = Number(process.env.PORT || 8086);
const imagesDir = path.resolve(process.env.IMAGES_DIR || path.join(os.homedir(), ".cache/reactos/web"));
const coi = process.env.COI === "1";
const logFile = path.resolve(process.env.LOG_FILE || path.join(tryRoot, "out/requests.json"));

// The emulator and its BIOS files are not committed; tools/fetch-v86.sh downloads them, pinned and checked.
const v86Files = ["libv86.mjs", "v86.wasm", "v86-fallback.wasm", "bios/seabios.bin", "bios/vgabios.bin"];
const v86Missing = v86Files.filter((f) => !fs.existsSync(path.join(tryRoot, "v86", f)));
if (v86Missing.length > 0) {
	console.error(`The v86 emulator files are missing from site/try-reactos/v86/: ${v86Missing.join(", ")}.`);
	console.error("Run `bash site/try-reactos/tools/fetch-v86.sh` first.");
	process.exit(1);
}

const types: Record<string, string> = {
	".html": "text/html; charset=utf-8",
	".js": "text/javascript; charset=utf-8",
	".mjs": "text/javascript; charset=utf-8",
	".css": "text/css; charset=utf-8",
	".json": "application/json; charset=utf-8",
	".wasm": "application/wasm",
	".svg": "image/svg+xml",
	".png": "image/png",
	".ico": "image/x-icon",
	".txt": "text/plain; charset=utf-8",
	".md": "text/plain; charset=utf-8",
	".iso": "application/octet-stream",
	".img": "application/octet-stream",
	".bin": "application/octet-stream",
	".zst": "application/zstd",
};

interface Entry {
	t: number; // ms since server start
	client: string;
	method: string;
	path: string;
	kind: string;
	range: string | null;
	status: number;
	bytes: number;
	ms: number;
}

const started = Date.now();
const log: Entry[] = [];
let dirty = false;

interface Totals {
	requests: number;
	bytes: number;
	firstMs: number | null; // first request of this kind, ms since the client's first request
	firstByteAt: number | null; // ms since server start
}
interface ClientStats {
	firstSeen: number; // ms since server start
	requests: number;
	bytes: number;
	kinds: Record<string, Totals>;
	// Distinct disk bytes read, as merged intervals of [start, end).
	diskRanges: [number, number][];
}
const clients = new Map<string, ClientStats>();

function kindOf(p: string): string {
	if (p.startsWith("/images/")) {
		if (/\.(iso)$/i.test(p)) return "cdrom";
		if (/\.zst$/.test(p) && /state/i.test(p)) return "state";
		if (/state/i.test(p) && /\.bin$/.test(p)) return "state";
		return "disk";
	}
	if (p.startsWith("/try-reactos/v86/bios/")) return "bios";
	if (p.startsWith("/try-reactos/v86/")) return "engine";
	if (p.startsWith("/__")) return "stats";
	return "page";
}

function addRange(list: [number, number][], start: number, end: number) {
	list.push([start, end]);
	list.sort((a, b) => a[0] - b[0]);
	const out: [number, number][] = [];
	for (const r of list) {
		const last = out[out.length - 1];
		if (last && r[0] <= last[1]) last[1] = Math.max(last[1], r[1]);
		else out.push([r[0], r[1]]);
	}
	list.length = 0;
	list.push(...out);
}

function clientOf(req: http.IncomingMessage): string {
	const url = new URL(req.url || "/", "http://x");
	const own = url.searchParams.get("run");
	if (own) return own;
	const ref = req.headers.referer;
	if (ref) {
		try {
			const r = new URL(ref).searchParams.get("run");
			if (r) return r;
		} catch {
			// A malformed Referer is ignored.
		}
	}
	return req.socket.remoteAddress || "unknown";
}

function record(e: Entry) {
	log.push(e);
	dirty = true;
	let c = clients.get(e.client);
	if (!c) {
		c = { firstSeen: e.t, requests: 0, bytes: 0, kinds: {}, diskRanges: [] };
		clients.set(e.client, c);
	}
	if (e.kind === "stats") return;
	c.requests++;
	c.bytes += e.bytes;
	let k = c.kinds[e.kind];
	if (!k) {
		k = { requests: 0, bytes: 0, firstMs: e.t - c.firstSeen, firstByteAt: e.t };
		c.kinds[e.kind] = k;
	}
	k.requests++;
	k.bytes += e.bytes;
	if (e.kind === "disk" && e.range) {
		const m = /bytes (\d+)-(\d+)\//.exec(e.range);
		if (m) addRange(c.diskRanges, Number(m[1]), Number(m[2]) + 1);
	}
}

function flush() {
	if (!dirty) return;
	try {
		fs.mkdirSync(path.dirname(logFile), { recursive: true });
		fs.writeFileSync(logFile, JSON.stringify(log) + "\n");
		dirty = false;
	} catch (e) {
		console.error("log write failed:", e);
	}
}
setInterval(flush, 2000).unref();

function statsJson(only: string | null) {
	const out: Record<string, unknown> = {};
	for (const [id, c] of clients) {
		if (only && id !== only) continue;
		const distinct = c.diskRanges.reduce((n, r) => n + (r[1] - r[0]), 0);
		out[id] = {
			firstSeenMs: c.firstSeen,
			requests: c.requests,
			bytes: c.bytes,
			kinds: c.kinds,
			diskDistinctBytes: distinct,
			diskRangeCount: c.diskRanges.length,
		};
	}
	return { now: Date.now() - started, clients: out };
}

function resolveFile(p: string): string | null {
	let rel: string;
	let base: string;
	if (p.startsWith("/images/")) {
		base = imagesDir;
		rel = p.slice("/images/".length);
	} else {
		base = siteRoot;
		rel = p.slice(1);
	}
	if (rel === "" || rel.endsWith("/")) rel += "index.html";
	const full = path.resolve(base, decodeURIComponent(rel));
	if (full !== base && !full.startsWith(base + path.sep)) return null;
	// The harness folder's own tooling and results are not served.
	if (base === siteRoot && /^try-reactos\/(harness|tools|out)(\/|$)/.test(path.relative(siteRoot, full))) return null;
	return full;
}

function parseRange(header: string, size: number): [number, number] | "bad" | null {
	const m = /^bytes=(\d*)-(\d*)$/.exec(header.trim());
	if (!m) return null; // Not a single byte range: serve the whole file, as the standard allows.
	let start: number;
	let end: number;
	if (m[1] === "" && m[2] === "") return "bad";
	if (m[1] === "") {
		const n = Number(m[2]);
		if (n === 0) return "bad";
		start = Math.max(0, size - n);
		end = size - 1;
	} else {
		start = Number(m[1]);
		end = m[2] === "" ? size - 1 : Math.min(Number(m[2]), size - 1);
	}
	if (start >= size || start > end) return "bad";
	return [start, end];
}

const server = http.createServer((req, res) => {
	const t0 = performance.now();
	const url = new URL(req.url || "/", "http://x");
	const p = url.pathname;
	const client = clientOf(req);
	const method = req.method || "GET";
	const kind = kindOf(p);
	let status = 200;
	let sent = 0;
	let range: string | null = null;

	res.setHeader("Cache-Control", "no-store");
	res.setHeader("Accept-Ranges", "bytes");
	if (coi) {
		res.setHeader("Cross-Origin-Opener-Policy", "same-origin");
		res.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
		res.setHeader("Cross-Origin-Resource-Policy", "same-origin");
	}
	let done = false;
	const finish = () => {
		if (done) return;
		done = true;
		record({
			t: Date.now() - started,
			client,
			method,
			path: p,
			kind,
			range,
			status,
			bytes: sent,
			ms: Math.round((performance.now() - t0) * 10) / 10,
		});
	};
	res.on("close", finish);

	const json = (code: number, body: unknown) => {
		status = code;
		const b = Buffer.from(JSON.stringify(body, null, 1));
		res.writeHead(code, { "Content-Type": "application/json", "Content-Length": b.length });
		res.end(b);
		sent = b.length;
	};

	// The write endpoints answer this machine only, so serving on the LAN (HOST=0.0.0.0) exposes the page and the images read-only.
	const ra = req.socket.remoteAddress || "";
	const loopback = ra === "127.0.0.1" || ra === "::1" || ra === "::ffff:127.0.0.1";
	if (p.startsWith("/__") && method === "POST" && !loopback) return json(403, { error: "local only" });

	if (p === "/__stats" && method === "GET") return json(200, statsJson(url.searchParams.get("client")));
	if (p === "/__stats/reset" && method === "POST") {
		const only = url.searchParams.get("client");
		if (only) clients.delete(only);
		else clients.clear();
		return json(200, { ok: true });
	}
	if (p === "/__stats/flush" && method === "POST") {
		flush();
		return json(200, { ok: true, file: logFile, entries: log.length });
	}
	// POST /__put/NAME stores the body in the images folder (saved machine states come from the page this way). Local only.
	if (p.startsWith("/__put/") && method === "POST") {
		const nm = path.basename(decodeURIComponent(p.slice("/__put/".length)));
		if (!nm || nm.startsWith(".")) return json(400, { error: "bad name" });
		const dest = path.join(imagesDir, nm);
		// ?offset=N writes at that position (0 starts the file afresh), so a large body can arrive in pieces.
		const off = Number(url.searchParams.get("offset") || 0);
		const ws = fs.createWriteStream(dest, off > 0 ? { flags: "r+", start: off } : { flags: "w" });
		req.pipe(ws);
		ws.on("finish", () => json(200, { ok: true, file: dest, bytes: ws.bytesWritten }));
		ws.on("error", () => json(500, { error: "write failed" }));
		return;
	}
	if (p === "/__log" && method === "GET") return json(200, log);

	if (method !== "GET" && method !== "HEAD") return json(405, { error: "method not allowed" });
	const file = resolveFile(p);
	let st: fs.Stats | null = null;
	try {
		if (file) st = fs.statSync(file);
	} catch {
		st = null;
	}
	if (!file || !st || !st.isFile()) return json(404, { error: "not found" });

	const size = st.size;
	const type = types[path.extname(file).toLowerCase()] || "application/octet-stream";
	const headers: Record<string, string | number> = { "Content-Type": type, "Last-Modified": st.mtime.toUTCString() };
	let start = 0;
	let end = size - 1;
	const rh = req.headers.range;
	if (rh) {
		const r = parseRange(rh, size);
		if (r === "bad") {
			status = 416;
			res.writeHead(416, { ...headers, "Content-Range": `bytes */${size}`, "Content-Length": 0 });
			res.end();
			return;
		}
		if (r) {
			[start, end] = r;
			status = 206;
			range = `bytes ${start}-${end}/${size}`;
			headers["Content-Range"] = range;
		}
	}
	const len = size === 0 ? 0 : end - start + 1;
	headers["Content-Length"] = len;
	res.writeHead(status, headers);
	if (method === "HEAD" || len === 0) {
		res.end();
		return;
	}
	const stream = fs.createReadStream(file, { start, end });
	stream.on("data", (chunk) => {
		sent += chunk.length;
	});
	stream.on("error", () => res.destroy());
	res.on("close", () => stream.destroy());
	stream.pipe(res);
});

// HOST=0.0.0.0 serves the LAN; the default stays on this machine.
const host = process.env.HOST || "127.0.0.1";
server.listen(port, host, () => {
	console.log(`serving ${siteRoot} (page at /try-reactos/) and /images/ from ${imagesDir} on http://${host}:${port}/ (pid ${process.pid})`);
});
process.on("SIGTERM", () => {
	flush();
	process.exit(0);
});
process.on("SIGINT", () => {
	flush();
	process.exit(0);
});
