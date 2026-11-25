// Turn out/results/*.json into a Markdown table, one row per run.
// Usage: node summarise.ts [DIR]
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const dir = process.argv[2] || path.join(here, "..", "out", "results");
const mb = (n: number | undefined | null) => (n === undefined || n === null ? "-" : (n / 1e6).toFixed(1));
const f = (n: number | undefined | null) => (n === undefined || n === null ? "-" : String(n));

const rows = fs
	.readdirSync(dir)
	.filter((x) => x.endsWith(".json"))
	.sort()
	.map((x) => JSON.parse(fs.readFileSync(path.join(dir, x), "utf8")));

console.log("| Run | Profile | Seconds to live desktop | MB served | Disk MB read | Typing median / p95 ms | Scroll fps | Renderer RSS MB | Note |");
console.log("|---|---|---|---|---|---|---|---|---|");
for (const r of rows) {
	const live = r.restore ? r.restore.seconds : r.desktop?.seconds;
	const at = r.restore ?? r.desktop;
	const quill = r.appStart?.seconds !== undefined ? `; Quill +${f(r.appStart.seconds)} s` : "";
	console.log(
		`| ${r.name} | ${r.profile?.name} | ${f(live)}${quill} | ${mb(at?.served)} | ${mb(at?.disk)} | ${f(r.typing?.medianMs)} / ${f(r.typing?.p95Ms)} | ${f(r.scroll?.fps)} | ${f(r.memory?.rendererRssMB)} | ${r.error ? "ERROR: " + r.error : ""} |`,
	);
}
