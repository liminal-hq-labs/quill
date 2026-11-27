// Scores a saved pointer trace: the direction error of each stroke, so a moment that felt wrong becomes numbers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Usage: node scripts/pointer/score-trace.mjs [--window N] [--json] FILE...
// A trace is what the lab page, or a page with ?pointer=debug, saves with "Save pointer trace". Strokes are split at pauses of
// 250 ms or more; each is scored over windows of 8 guest pixels (or --window) by QuillPointerMetric.scoreTrace.

import fs from "node:fs";

await import("../../site/js/pointer-metric.js");
const { scoreTrace } = globalThis.QuillPointerMetric;

const args = process.argv.slice(2);
const files = [];
let windowPixels = 8;
let asJson = false;
for (let i = 0; i < args.length; i++) {
	if (args[i] === "--window") windowPixels = Number(args[++i]);
	else if (args[i] === "--json") asJson = true;
	else files.push(args[i]);
}
if (!files.length) {
	console.error("usage: score-trace.mjs [--window N] [--json] FILE...");
	process.exit(2);
}

const report = [];
for (const file of files) {
	const trace = JSON.parse(fs.readFileSync(file, "utf8"));
	const strokes = scoreTrace(trace, { window: windowPixels });
	report.push({ file, mode: trace.mode, userAgent: trace.userAgent, dpr: trace.dpr, strokes });
}

if (asJson) console.log(JSON.stringify(report, null, 2));
else
	for (const entry of report) {
		console.log(`${entry.file}: mode ${entry.mode}, dpr ${entry.dpr}, ${entry.strokes.length} strokes`);
		entry.strokes.forEach((s, i) => {
			const m = s.metric;
			console.log(
				`  stroke ${i + 1}: ${s.samples} samples, ${m.windows} windows, median ${m.median.toFixed(1)}°, p95 ${m.p95.toFixed(1)}°, one axis only ${(m.axisOnly * 100).toFixed(1)}%, end (${m.endpoint[0].toFixed(1)}, ${m.endpoint[1].toFixed(1)}), path ratio ${m.pathRatio.toFixed(2)}`,
			);
		});
	}
