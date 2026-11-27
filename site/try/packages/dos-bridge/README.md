# dos-bridge

The DOS Try Quill page's side of its bridge to the guest (SITE-60 to SITE-62). js-dos has no serial link or shared folder to reuse, so the page and FreeDOS share two hard disk images, each with one writer: `C:` (`c.img`) is written by the guest and read by the page, `D:` (`d.img`) is written by the page and read by the guest. This package reads and builds the FAT volumes on them. It has no runtime dependency and no DOM dependency outside `bridge.js`, the page's glue.

The plan is [docs/plans/dos-page-bridge.md](../../../../docs/plans/dos-page-bridge.md). This is WP-DOSWEB-1's part of it: the disks, the date and the core. Files, the clipboard, the phone keyboard and the PDF are later packages that add files here.

## API

```ts
import { attach, describe } from "./lib/dos-bridge.js";

// On js-dos's `ci-ready`, with its command interface (never keep that in a global called `ci`: js-dos defines a function of that name).
const bridge = attach(ci, {
	now: () => new Date(),       // the day to put on D:; the visitor's own by default
	onChange: (state) => {},     // the diagnostics line: describe(state)
});
await bridge.ready;              // D: holds SETDATE.BAT (or the write failed: bridge.state().status is "failed"); never rejects
await bridge.write([{ name: "HELLO.TXT", bytes, time: new Date() }]); // D: rebuilt whole from the files given, with the date file added
const c = await bridge.readC();  // C: as it is in the image now (an `openVolume` result)
bridge.detach();                 // later writes are refused; safe twice
```

`fat.ts` is also exported (`openVolume`, `listDir`, `readFile`, `statPath`, `buildVolume`), as are `shortNames`, `namesIndex` and `encodeWin1252` from `names.ts`.

## Layout

| File | What |
|---|---|
| `src/fat.ts` | An MBR and a FAT12 or FAT16 volume: list a folder, read a file by path, and build a new volume from the empty template and a list of files (FAT copies, directory entries, DOS times). Pure bytes in and out. FAT32, sector sizes other than 512, chains that loop and volumes larger than their image are refused with a `FatError`; nothing is written outside the image |
| `src/names.ts` | Long names to 8.3 (accents folded, collisions to `~N`, device names, `FILE` for an empty one), Windows-1252, and the `NAMES.TXT` index |
| `src/disks.ts` | The only file that calls `ci.fsReadFile` and `ci.fsWriteFile`: `readC`, `writeD` and the date file (`@DATE mm-dd-yyyy`) |
| `src/index.ts` | `attach` and the public types |
| `test/` | `bun test`: `fat.ts` against `mtools` images (`tests/dos-bridge/`), every volume it builds through `fsck.fat -n` and `mtools`, `names.ts` against a table, the bridge on a fake interface |
| `build.ts` | Builds `../../lib/dos-bridge.js` (`--check` fails on drift; 16 KB budget) |

The images are made by `scripts/make-bridge-disks.py` (geometry and the `SYNC.COM` that makes each side see the other's writes are described in `site/README.md`, *The DOS page's disks*). The tests that need `mtools` or `fsck.fat` skip themselves where they are not installed; CI installs both.

## Use

```sh
bun install
bun test
bun run typecheck
bun run build        # writes site/try/lib/dos-bridge.js; commit it, then python3 -I scripts/stamp-try.py
bun run check:built
python3 -I ../../../../tests/dos-bridge/make-goldens.py   # rewrites the golden images (needs mtools)
```
