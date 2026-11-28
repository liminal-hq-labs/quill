// The page's end of the link makes no network request: its sources name none of the browser's ways to send data off the machine (SITE-70).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const FORBIDDEN = /\b(fetch|XMLHttpRequest|WebSocket|EventSource|sendBeacon|importScripts|RTCPeerConnection|WebTransport)\b|new\s+Image\b|new\s+Worker\b|navigator\.share\b|\.src\s*=/;

function sources(dir: string): string[] {
	return readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? sources(join(dir, e.name)) : e.name.endsWith(".ts") ? [join(dir, e.name)] : []));
}

function code(text: string): string {
	return text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:])\/\/.*$/gm, "$1");
}

describe("no network", () => {
	test("no source of the package names a way to send data off the machine", () => {
		const files = sources(join(import.meta.dir, "../src"));
		expect(files.length).toBeGreaterThan(8);
		for (const file of files) expect({ file, hit: FORBIDDEN.exec(code(readFileSync(file, "utf8")))?.[0] ?? null }).toEqual({ file, hit: null });
	});

	test("the page's glue names none either", () => {
		const glue = code(readFileSync(join(import.meta.dir, "../../../host-link.js"), "utf8"));
		expect(FORBIDDEN.exec(glue)?.[0] ?? null).toBeNull();
	});
});
