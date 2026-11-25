# v86, as fetched here

The x86 emulator for the ReactOS demo page, taken unchanged from the `v86` package on npm, with the BIOS files taken from the matching commit of its repository. The binaries are not committed: `tools/fetch-v86.sh` downloads them from the addresses below, checks the SHA-256 of the archive and of each file against the sums in the script and in the table here, and writes only the five files marked *fetched*. This folder keeps the licence texts, the type definitions and this note.

- **Package:** [`v86`](https://www.npmjs.com/package/v86) 0.5.473 (`package.json` reports `0.5.473+g315b2b0`), published 2026-10-30, more than two weeks before it was vendored. Downloaded from <https://registry.npmjs.org/v86/-/v86-0.5.473.tgz> (the same file `npm pack v86@0.5.473` makes); the tarball's SHA-256 is `364ab67a68c1d83646e2ddf7caf73c24574694a132b2b43689ad0f72e7242636`. Source: <https://github.com/copy/v86>, commit `315b2b0b758703ef0e6cc7b4088d6068f83f3c20`.
- **Licence of the emulator:** BSD-2-Clause (`LICENSE`, from the package). v86's WebAssembly build also contains zstd (BSD-3-Clause or GPL-2.0, at its users' option) and SoftFloat (BSD-3-Clause); their notices are in the v86 repository's `LICENSE` and `src/` headers.
- **BIOS files:** `bios/seabios.bin` and `bios/vgabios.bin`, fetched from `https://raw.githubusercontent.com/copy/v86/315b2b0/bios/`, the same commit as the package. They are builds of SeaBIOS and SeaVGABIOS, **licensed LGPL-3.0**; the licence text is `bios/COPYING.LESSER` (also from that commit). The source is SeaBIOS `rel-1.16.2` (<https://git.seabios.org/seabios.git>), built by `bios/fetch-and-build-seabios.sh` in the v86 repository with its `seabios.config`. Serving these binaries publicly means offering that source, which the licences page of the site does for the other GPL and LGPL components.
- **Where it is used:** `try-reactos.js` imports `v86/libv86.mjs` when the visitor selects Start and passes `wasm_path: "v86/v86.wasm"` and the two BIOS URLs. The page makes no request outside its own origin.

## Files

| Here | SHA-256 | What it is |
|---|---|---|
| `libv86.mjs` (fetched) | `f9f3b4cae7dafb882e10e51aae9137ec52d5aed5ffbc9026f9081c73bbaae4c8` | The emulator's JavaScript half (ES module), from `build/libv86.mjs` |
| `v86.wasm` (fetched) | `2f0c4150f266415b754a84a32e6f8e4c975a1d3abb94c78b047fa30027db1ef1` | The CPU core and JIT, from `build/v86.wasm` |
| `v86-fallback.wasm` (fetched) | `4eb64c9b7908d5931c1868eea4824b3a0a628eb86e6eac09d133776b50922d81` | The core without newer WebAssembly features, from `build/v86-fallback.wasm` |
| `v86.d.ts` | | The type definitions, committed as the API reference |
| `LICENSE` (committed) | `3f61ba65e3215b09ef264ba8d777d73c19644309fbb705967634eb5147e500a0` | BSD-2-Clause |
| `bios/seabios.bin` (fetched) | `73e3f359102e3a9982c35fce98eb7cd08f18303ac7f1ba6ebfbe6cdc1c244d98` | SeaBIOS 1.16.2 (LGPL-3.0) |
| `bios/vgabios.bin` (fetched) | `a4bc0d80cc3ca028c73dafa8fee396b8d054ce87ebd8abfbd31b06b437607880` | SeaVGABIOS 1.16.2 (LGPL-3.0) |
| `bios/COPYING.LESSER` (committed) | `a853c2ffec17057872340eee242ae4d96cbf2b520ae27d903e1b2fef1a5f9d1c` | LGPL-3.0 |

Left out: the debug builds (`libv86-debug.*`, 12.5 MB `v86-debug.wasm`) and the CommonJS build `libv86.js`.

## Updating

Pick a version at least two weeks old, download its package, take the BIOS files from the commit in the new `package.json`'s version string, and update the version, commit, URLs and sums in `tools/fetch-v86.sh` and the hashes above. A saved machine state is tied to one v86 build, so rebuild any state with the new version.
