# Provenance

The Win16 runtime and packer in this folder are copied from the `rust-win16` repository, branch `feat/multi-segment` (the shared multi-segment packer and 32-bit-stack runtime, PR #3), commit `4c0bac453eab98ae35f58fd60c950d534244a616`; its `docs/packer.md` is kept as `docs/win16-packer.md`. House rules allow copies with provenance, so the files are kept close to the originals and every change is listed here. Quill's own earlier multi-segment packer (D83) is replaced by it and stays in the history.

- **Licence:** Apache-2.0 OR MIT, the same as this repository; every copied file keeps its header.

## What was copied

| Here | From | Changes |
|---|---|---|
| `win16.json`, `.cargo/config.toml`, `rustfmt.toml` | `rust-win16` | None |
| `link.ld` | `rust-win16` | Defines `__w16_data_end` (the end of the data, 4-aligned) so Quill can tell how far its stack is from the data |
| `src/lib.rs` | `rust-win16` | Saves the command line `InitTask` returns (`W16_CMD`, `command_line`); adds `far_read` and `far_write` (copies to and from a far pointer); adds `W16_NOALIAS` and a test in `_start` (the `noalias` cargo feature keeps Rust on the real 16-bit stack, for experiments only) |
| `src/imports.rs`, `tools/gen_imports.py`, `tools/imports.tbl` | `rust-win16` | The `imports.tbl` gains rows for menus, GDI text and drawing, `GetCurrentTask` and the ToolHelp `InterruptRegister` pair, and the Windows calls Quill's earlier examples used (`WinExec`, `SetTimer`, `KillTimer`, `GetTickCount`, `FindWindow`, `DrawIcon`, `GetDeviceCaps`), then the calls of the native interface (off-screen drawing, scroll bars, dialogs and `COMMDLG.DLL`); `InvalidateRect` takes its rectangle as a far pointer (`l`), so 0 is a null pointer and not `DS:0`; `gen_imports.py --check-spec` also reads Wine's `commdlg.dll16.spec` and skips modules it has no spec for; `src/imports.rs` is regenerated |
| `tools/ne_pack.py` | `rust-win16` | Icons (`icons`: an `RT_GROUP_ICON` and its `RT_ICON` images from an `.ico`, as Quill's earlier packer wrote them), bitmaps (`bitmaps`: `RT_BITMAP` from a `.bmp`), `dialog_files` (dialogs read from another JSON file, `assets/dialogs/dialogs.json`), the control style names that file uses, and a static icon control's resource number (`icon`); without them `QUILL.EXE` had no resources and no icon |
| `tools/ne_check.py`, `ne_dump.py`, `ne_addr2sym.py`, `rust_demangle.py` | `rust-win16` | None |
| `src/probe.rs`, `src/wizard.rs` | `rust-win16` | `probe.rs` stays Quill's T1 version (it checks calls that yield, FS and GS, and a second task); `wizard.rs` is copied unchanged |

## What is Quill's own

`src/fault.rs` (the ToolHelp fault handler, switched on with `/FAULT`), `examples/{probe,segtest,icontest,quill,stackprobe}/` (each program's `segments.json` follows the packer's format), `build.sh`, `tools/run-w31.sh` (the Windows 3.11 harness), `tools/check_segments.py`, `tools/check_resources.py`, `tools/dosbox-x/*.conf` and `tools/dosbox-x.Dockerfile`.

## Refreshing

When `rust-win16` moves, diff each file in the table against it ignoring whitespace, copy the changed ones, re-apply the changes above, rebuild with `win16/build.sh` and run the probes and the 30-run Windows 3.11 loop again with `win16/tools/run-w31.sh`.
