# Provenance of the vendored Host Link

Everything in this folder was built from one commit of the `v86-host-link` repository, which is local (no remote) at `~/source/liminal-hq-labs/v86-host-link`, and copied here by `tools/fetch-v86-host-link.sh`. Quill never edits these files: a fix goes to `v86-host-link` first and comes back by running the script again. `tools/check-v86-host-link.sh` holds every file to `SHA256SUMS` and fails on a test-only file.

- **Commit:** `776f072d9f5f1a9d72267524c2cf9dc207b7d436` (Merge branch 'feat/hostlink-sys')
- **Licence:** Apache-2.0 OR MIT, the repository's own
- **Built with:** `make win32` (the guest programs: Rust, no C compiler, no C library, a Pentium target; the budgets and the import and instruction scans are that command's checks), `make kit` (the Host Link Kit CD, `kit/dist/hostlink-kit.iso`) and `bun run check:built` in `page/` (the module is the one the repository commits, shown equal to what its package builds)

| File | Bytes | SHA-256 | What it is |
|---|---:|---|---|
| `guest/HLAUDIO.DLL` | 8704 | `f1a538bd43694c08dc092c6be5d1882bd0069dc870ce15cb8f54e43072916a98` | The audio module |
| `guest/HLLOG.DLL` | 5632 | `f6d2527fd92a8b0760acb772da35291ae35b49757a58852a04b80f5e5afc98d7` | The log module |
| `guest/HLPOWER.DLL` | 6656 | `0e145a5b84656d2cb25f70f6aa5e3481005f3f74ffd1a8ff3ee35c0a0f42c0b8` | The power module |
| `guest/HLSYS.DLL` | 7680 | `6fb1a47728099001c5cb380a9de9e38179aa4f31cdec219346f987ddfeb9d14c` | The system information module |
| `guest/HLTRAY.EXE` | 15872 | `d6dfafdeb902724733c72a2dd5a84bb090b79a0e586ba01be81f26024b9751ea` | The tray icon and the Sound card window |
| `guest/HOSTLINK.EXE` | 34304 | `db53db4ec0fff40c7be67a79ce7c6954cc8527e4e7e4885f44aa76857cb8b945` | The guest helper: owns COM1, the clipboard, files, time and the screen's size, and loads the modules |
| `host-link.js` | 39687 | `0d03ac0f652ce8f73955bafc7472b7c8a0577b8983557ac57d2ba8e84b0b98d7` | The page's end of the link, one minified ES module |
| `kit/hostlink-kit.iso` | 258048 | `458e4ff3a51c4c2e6ebf49dbc2f937dc98c69fc3e3f8ba5f9c9275345c9590af` | The Host Link Kit CD (setup, the helpers, AUTORUN.INF) |

The helper looks for `HL*.DLL` beside itself, so the guest programs go together into one folder (`C:\Quill\` in the QAGRR image). The test-only programs (`HLTEST*.DLL`, `HLBRK.EXE`, `HLCLIP.EXE`, `HOSTLINKW.EXE`, `HLTRAYT.EXE`, the audio probe) are never vendored.
