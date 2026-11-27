// Builds the package into one minified ES module, site/try/lib/dos-bridge.js, or with --check fails when the committed copy differs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const OUT = join(import.meta.dir, "../../lib/dos-bridge.js");
const BUDGET = 16 * 1024;
const BANNER = [
	"// The DOS Try Quill page's bridge to the guest, built from site/try/packages/dos-bridge by `bun run build`. Do not edit.",
	"//",
	"// (c) Copyright 2026 Liminal HQ, Scott Morris",
	"// SPDX-License-Identifier: Apache-2.0 OR MIT",
	"",
].join("\n");

const result = await Bun.build({
	entrypoints: [join(import.meta.dir, "src/index.ts")],
	target: "browser",
	format: "esm",
	minify: true,
	banner: BANNER,
});
if (!result.success || result.outputs.length !== 1) {
	console.error(result.logs.join("\n"));
	process.exit(1);
}
const built = (await result.outputs[0]!.text()).replace(/\s*$/, "\n");
const size = new TextEncoder().encode(built).length;
if (size > BUDGET) {
	console.error(`dos-bridge.js is ${size} bytes, over the ${BUDGET} byte budget`);
	process.exit(1);
}

if (process.argv.includes("--check")) {
	const current = existsSync(OUT) ? readFileSync(OUT, "utf8") : "";
	if (current !== built) {
		console.error("site/try/lib/dos-bridge.js differs from what the package builds: run `bun run build` in site/try/packages/dos-bridge and commit it");
		process.exit(1);
	}
	console.log(`dos-bridge.js is current (${size} bytes)`);
} else {
	mkdirSync(dirname(OUT), { recursive: true });
	writeFileSync(OUT, built);
	console.log(`wrote ${OUT} (${size} bytes)`);
}
