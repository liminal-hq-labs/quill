// Builds the package into one minified ES module, site/try-reactos/lib/host-link.js, or with --check fails when the committed copy differs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const OUT = join(import.meta.dir, "../../lib/host-link.js");
const BUDGET = 40 * 1024;
const BANNER = [
	"// The page's end of the host link (LHQ-PKT frames over COM1), built from site/try-reactos/packages/host-link by `bun run build`. Do not edit.",
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
	console.error(`host-link.js is ${size} bytes, over the ${BUDGET} byte budget`);
	process.exit(1);
}

if (process.argv.includes("--check")) {
	const current = existsSync(OUT) ? readFileSync(OUT, "utf8") : "";
	if (current !== built) {
		console.error("site/try-reactos/lib/host-link.js differs from what the package builds: run `bun run build` in site/try-reactos/packages/host-link and commit it");
		process.exit(1);
	}
	console.log(`host-link.js is current (${size} bytes)`);
} else {
	writeFileSync(OUT, built);
	console.log(`wrote ${OUT} (${size} bytes)`);
}
