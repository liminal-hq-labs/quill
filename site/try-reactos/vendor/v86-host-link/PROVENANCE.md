# Provenance of the vendored Host Link

Everything in this folder was built from one commit of the `v86-host-link` repository, which is local (no remote) at `~/source/liminal-hq-labs/v86-host-link`, and copied here by `tools/fetch-v86-host-link.sh`. Quill never edits these files: a fix goes to `v86-host-link` first and comes back by running the script again. `tools/check-v86-host-link.sh` holds every file to `SHA256SUMS` and fails on a test-only file.

- **Commit:** `d5586679e0f6fba256104fdb59aacc7c27831699` (Merge branch 'feat/hostlink-tray')
- **Licence:** Apache-2.0 OR MIT, the repository's own
- **Built with:** `make win32` (the guest programs: Rust, no C compiler, no C library, a Pentium target; the budgets and the import and instruction scans are that command's checks), `make kit` (the Host Link Kit CD, `kit/dist/hostlink-kit.iso`) and `bun run check:built` in `page/` (the module is the one the repository commits, shown equal to what its package builds)

| File | Bytes | SHA-256 | What it is |
|---|---:|---|---|
| `guest/HLAUDIO.DLL` | 8704 | `c99a27b38c82dcba82e7beab42bb9c58792355c323926dc2febeeb0ed48f429d` | The audio module |
| `guest/HLTRAY.EXE` | 15872 | `51b6c77899793c694ef83c12c673a8c196ec0a9bbf49f81a29a613c1547bb3e7` | The tray icon and the Sound card window |
| `guest/HOSTLINK.EXE` | 33792 | `fc8d0cd8d695a59158c36900fbe78d80cde499a5633469e36a34a8fdfaad2d52` | The guest helper: owns COM1, the clipboard, files, time and the screen's size, and loads the modules |
| `host-link.js` | 38189 | `9433171bb9199c5d80087dafb21cdd771b37a1657e424d8bab1a9c039103679f` | The page's end of the link, one minified ES module |
| `kit/hostlink-kit.iso` | 243712 | `ce2fa71e5d322f7a939c5468fd5c7efd4bd38dc02cf463153755a5598446920e` | The Host Link Kit CD (setup, the helpers, AUTORUN.INF) |

The helper looks for `HL*.DLL` beside itself, so the guest programs go together into one folder (`C:\Quill\` in the QAGRR image). The test-only programs (`HLTEST*.DLL`, `HLBRK.EXE`, `HLCLIP.EXE`, `HOSTLINKW.EXE`, `HLTRAYT.EXE`, the audio probe) are never vendored.
