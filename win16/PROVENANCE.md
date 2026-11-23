# Provenance

The Win16 runtime in this folder is copied from the `rust-win16` template repository (extracted from the Liminal Installer's `feat/win16-spike` work), commit `69d34b593aa171cba49bbe795816aaa80b161819`. House rules allow copies with provenance, so the files are kept close to the originals and every change is listed here.

- **Licence:** Apache-2.0 OR MIT, the same as this repository; every copied file keeps its header.

## What was copied

| Here | From | Changes |
|---|---|---|
| `win16.json`, `link.ld`, `rustfmt.toml` | `rust-win16` | None |
| `.cargo/config.toml` | `rust-win16` | Adds `-C llvm-args=-min-jump-table-entries=1000`: a four-way `match` in the T1 probe compiled to a jump table whose entry sent the program into the middle of an instruction. Cause not found yet; see `docs/progress.md` |
| `src/lib.rs` | `rust-win16` | The wizard module is dropped; the result log grows from 3 KB to 8 KB |
| `src/imports.rs`, `tools/gen_imports.py`, `tools/imports.tbl` | `rust-win16` | Rows added for `WinExec`, `ExitWindows`, `SetTimer`, `KillTimer`, `GetTickCount` and `FindWindow` |
| `tools/ne_pack.py`, `tools/ne_dump.py` | `rust-win16` | None |
| `src/probe.rs` | `rust-win16` | Rewritten for T1: probes for calls that yield, FS and GS, a busy loop, calls through the import stubs, and a second task with other patterns |

## What is Quill's own

`examples/probe/` (the T1 program), `build.sh`, `tools/run-w31.sh` (the Windows 3.11 harness), `tools/dosbox-x/*.conf` and `tools/dosbox-x.Dockerfile`.

## Refreshing

When the `rust-win16` template moves, diff each file in the table against the template ignoring whitespace, copy the changed ones, re-apply the changes above, rebuild with `win16/build.sh` and run the probes again with `win16/tools/run-w31.sh`.
