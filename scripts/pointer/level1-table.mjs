// Writes the level 1 matrix as JSON and a Markdown summary: every cell of the plan's matrix through the core and the metric.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Usage: node scripts/pointer/level1-table.mjs OUT_DIR
// Follow, and Capture under the three input models (exact, Chromium, Firefox on Wayland), for 7 angles, a circle and a figure eight,
// 4 speeds, 125 and 1000 Hz and 4 scales. The Firefox model is expected to fail the pass lines in most cells; the table says where.

import fs from "node:fs";
import path from "node:path";
import { centredOrigin, handPath, passes, runCapture, runFollow, score } from "./simulate.mjs";

const outDir = process.argv[2];
if (!outDir) {
	console.error("usage: level1-table.mjs OUT_DIR");
	process.exit(2);
}
fs.mkdirSync(outDir, { recursive: true });

const kinds = [0, 7, 27, 45, 63, 83, 90, "circle", "eight"];
const rows = [];
for (const kind of kinds)
	for (const speed of [0.03, 0.1, 0.4, 1.5])
		for (const hz of [125, 1000])
			for (const scale of [1, 1.25, 1.5, 2]) {
				const path = handPath(kind, speed, hz, centredOrigin(kind, speed));
				const runs = { follow: runFollow(path), "capture exact": runCapture(path, "exact", { dpr: scale }), "capture Chromium": runCapture(path, "chromium", { dpr: scale }), "capture Firefox on Wayland": runCapture(path, "firefox", { dpr: scale }) };
				for (const [model, run] of Object.entries(runs)) {
					const metric = score(run);
					rows.push({ kind, speed, hz, scale, model, windows: metric.windows, median: metric.median, p95: metric.p95, axisOnly: metric.axisOnly, endpoint: metric.endpoint, pathRatio: metric.pathRatio, maxCalls: run.maxCalls, pass: passes(metric) && run.maxCalls <= (model === "follow" ? 1 : 2) });
				}
			}

const summary = [];
for (const model of ["follow", "capture exact", "capture Chromium", "capture Firefox on Wayland"]) {
	const mine = rows.filter((row) => row.model === model && row.windows > 0);
	const passed = mine.filter((row) => row.pass).length;
	const worst = Math.max(...mine.map((row) => row.p95));
	summary.push(`| ${model} | ${mine.length} | ${passed} | ${worst.toFixed(1)}° | ${Math.max(...mine.map((row) => row.axisOnly * 100)).toFixed(1)}% |`);
}
// The plan's key cell, which is not on the matrix's speeds: 27 degrees at 0.8 px/ms, 1000 Hz, scale 1.5.
const keyPath = handPath(27, 0.8, 1000, centredOrigin(27, 0.8));
const keyRuns = { follow: runFollow(keyPath), "capture exact": runCapture(keyPath, "exact", { dpr: 1.5 }), "capture Chromium": runCapture(keyPath, "chromium", { dpr: 1.5 }), "capture Firefox on Wayland": runCapture(keyPath, "firefox", { dpr: 1.5 }) };
const keyLines = Object.entries(keyRuns).map(([model, run]) => {
	const metric = score(run);
	rows.push({ kind: 27, speed: 0.8, hz: 1000, scale: 1.5, model, windows: metric.windows, median: metric.median, p95: metric.p95, axisOnly: metric.axisOnly, endpoint: metric.endpoint, pathRatio: metric.pathRatio, maxCalls: run.maxCalls, pass: passes(metric) && run.maxCalls <= (model === "follow" ? 1 : 2) });
	return `| ${model} | ${metric.windows} | ${metric.median.toFixed(1)}° | ${metric.p95.toFixed(1)}° | ${(metric.axisOnly * 100).toFixed(1)}% | ${passes(metric) ? "pass" : "FAIL"} |`;
});
fs.writeFileSync(path.join(outDir, "level1.json"), JSON.stringify(rows, null, 1));
const text = `# Pointer level 1: the model\n\nThe core's counts for a generated hand path, applied to a stand-in guest, scored by the direction error over windows of 8 guest pixels. ${rows.length} runs: 9 paths, 4 speeds, 2 report rates and 4 scales for each of 4 input models, and the key cell.\n\n| Model | Cells | Pass the lines | Worst p95 | Worst one-axis share |\n|---|---|---|---|---|\n${summary.join("\n")}\n\nThe plan's key cell, 27 degrees at 0.8 px/ms, 1000 Hz, scale 1.5:\n\n| Model | Windows | Median | p95 | One axis | Lines |\n|---|---|---|---|---|---|\n${keyLines.join("\n")}\n`;
fs.writeFileSync(path.join(outDir, "level1.md"), text);
console.log(text);
