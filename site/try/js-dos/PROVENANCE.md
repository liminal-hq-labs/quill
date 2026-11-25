# js-dos, as vendored here

The browser player for the Try Quill page, copied unchanged from the `js-dos` package on npm.

- **Package:** [`js-dos`](https://www.npmjs.com/package/js-dos) 8.5.3, published 2026-10-07 (more than two weeks before it was vendored). Its source is at <https://github.com/caiiiycuk/js-dos>; the files under `emulators/` are the project's DOSBox build for WebAssembly, and DOSBox's source is at <https://www.dosbox.com/>.
- **Licence:** GPL-2.0, as the package's `package.json` states. The licence text is `LICENSE-GPL-2.0.txt` (from <https://www.gnu.org/licenses/old-licenses/gpl-2.0.txt>). The package ships no licence file of its own, and its repository has none; the licence field is the only statement. The player's sources are the ones named above, and they are served from this site, so a visitor can read and save them.
- **Fetched with:** `npm pack js-dos@8.5.3`, then the files below were copied out of `package/dist/`. The tarball's integrity is whatever npm verified at the time; the files here can be checked against a fresh `npm pack js-dos@8.5.3`.
- **Where it is used:** `try/try.js` loads `js-dos.css` and `js-dos.js` when the visitor selects Start, and the player loads the rest from `emulators/` (set by its `pathPrefix` option). Nothing is fetched from js-dos.com or any other host.

## Files

| Here | From `package/dist/` | What it is |
|---|---|---|
| `js-dos.js`, `js-dos.css` | `js-dos.js`, `js-dos.css` | The player and its style sheet |
| `emulators/emulators.js` | `emulators/emulators.js` | The emulator loader the player drives |
| `emulators/wdosbox.js`, `emulators/wdosbox.wasm` | `emulators/wdosbox.js`, `emulators/wdosbox.wasm` | DOSBox, compiled to WebAssembly |
| `emulators/wlibzip.js`, `emulators/wlibzip.wasm` | `emulators/wlibzip.js`, `emulators/wlibzip.wasm` | The zip reader that opens the `.jsdos` bundle |

Left out: the source maps and `.symbols` files, the DOSBox-X builds (about 8 MB each; the demo needs only the DOSBox build), the file explorer, and the WebRTC network helper.

## Network behaviour

The player's own code names a few outside addresses: the js-dos CDN (the default `pathPrefix`, which `try.js` overrides), `net.dos.zone` (the IPX network, which `noNetworking` turns off), the cloud save services (`noCloud`) and a Dhrystone benchmark that posts its result, which is a window only the player's sidebar opens and `kiosk` hides. With those options set the page makes no request outside this site. Check it again after any update of the player: load the page with the browser's network panel open and select Start.

## Caching

js-dos 8.5.3 keeps each bundle it fetches in the browser's origin-private file system (not IndexedDB), in the folder named by its `opfsRoot` option (default `jsdos`), under `caches/bundles/` and a name made from the bundle's URL; saved changes go under `saves/`. It fetches the bundle itself with `cache: 'no-store'`, and appends the `pathSuffix` option to the emulator files. `try.js` sets `opfsRoot` to `quill-try-<hash of the bundle>`, turns `fsChanges.local` off, passes the manifest's versions as `?v=` on the player files and as `pathSuffix`, and deletes other builds' folders (SITE-4). After an update of the player, check that the option names above still exist in `js-dos.js` (search for `opfsRoot`, `fsChanges` and `pathSuffix`) and that `scripts/check-try.sh` passes; the update changes the manifest's `assets` versions, which is what makes visitors fetch the new player.

## Updating

Pick a version at least two weeks old, run `npm pack js-dos@VERSION`, copy the same files, update the version in this file, `try/index.html` and `try/licences.html`, and run the page in a browser, with the network panel open, before publishing.
