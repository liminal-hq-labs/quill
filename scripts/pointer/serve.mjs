// A small static server for site/, bound to 127.0.0.1, for the pointer lab's browser checks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Usage: PORT=8231 PIDFILE=/tmp/pointer-serve.pid node scripts/pointer/serve.mjs
// With no PORT it picks a free one and prints "listening http://127.0.0.1:PORT/". PIDFILE, when set, holds this process's id, so it
// can be stopped by that id and never by name. Importable: startServer({ port }) resolves { server, base }.

import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "site");
const TYPES = { ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".css": "text/css; charset=utf-8", ".json": "application/json", ".svg": "image/svg+xml", ".png": "image/png", ".ico": "image/x-icon" };

export function startServer({ port = 0 } = {}) {
	const server = http.createServer((request, response) => {
		const url = new URL(request.url, "http://127.0.0.1");
		let file = path.join(siteRoot, decodeURIComponent(url.pathname));
		if (!file.startsWith(siteRoot)) {
			response.writeHead(403).end();
			return;
		}
		if (fs.existsSync(file) && fs.statSync(file).isDirectory()) file = path.join(file, "index.html");
		if (!fs.existsSync(file)) {
			response.writeHead(404).end("not found");
			return;
		}
		response.writeHead(200, { "content-type": TYPES[path.extname(file)] || "application/octet-stream", "cache-control": "no-store" });
		fs.createReadStream(file).pipe(response);
	});
	return new Promise((resolve) => {
		server.listen(port, "127.0.0.1", () => resolve({ server, base: `http://127.0.0.1:${server.address().port}` }));
	});
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	const { base } = await startServer({ port: Number(process.env.PORT || 0) });
	if (process.env.PIDFILE) fs.writeFileSync(process.env.PIDFILE, String(process.pid));
	console.log(`listening ${base}/`);
}
