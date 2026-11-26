// Split a raw disk image into zstd-compressed 8 MiB parts that v86's chunked async disk reads: DIR/START-END.img.zst.
// Usage: node tools/make-parts.mjs IMAGE OUTDIR [chunkBytes=8388608] [level=19]
// Every chunk gets a file, including all-zero ones (a few bytes each), because v86 asks for any chunk the guest reads.
// Writes OUTDIR/manifest.json with the sizes and the SHA-256 of the raw image.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";
import { createHash } from "node:crypto";

const [image, outDir, chunkArg, levelArg] = process.argv.slice(2);
if (!image || !outDir) {
	console.error("usage: node make-parts.mjs IMAGE OUTDIR [chunkBytes] [level]");
	process.exit(2);
}
const chunk = Number(chunkArg || 8388608);
const level = Number(levelArg || 19);
const size = fs.statSync(image).size;
fs.mkdirSync(outDir, { recursive: true });
const fd = fs.openSync(image, "r");
const sha = createHash("sha256");
const buf = Buffer.alloc(chunk);
let total = 0;
let zeroParts = 0;
for (let off = 0; off < size; off += chunk) {
	const n = fs.readSync(fd, buf, 0, chunk, off);
	const data = n === chunk ? buf : buf.subarray(0, n);
	sha.update(data);
	if (data.every((b) => b === 0)) zeroParts++;
	const z = zlib.zstdCompressSync(data, { params: { [zlib.constants.ZSTD_c_compressionLevel]: level } });
	fs.writeFileSync(path.join(outDir, `${off}-${off + chunk}.img.zst`), z);
	total += z.length;
}
fs.writeFileSync(path.join(outDir, "manifest.json"), JSON.stringify({ image: path.basename(image), size, chunk, level, parts: Math.ceil(size / chunk), zeroParts, compressedBytes: total, sha256: sha.digest("hex") }, null, 1));
console.log(JSON.stringify({ size, parts: Math.ceil(size / chunk), zeroParts, compressedBytes: total }));
