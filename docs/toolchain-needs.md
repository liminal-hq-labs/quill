# Toolchain needs

What Quill needs from its three toolchains, in the order the [roadmap](roadmap.md) needs it. Each Win16 item is a request for the `rust-win16` template repository (extracted from the Liminal Installer's `feat/win16-spike` work in `retro/win16/`); each Win32 item for its sibling, `rust-win9x` (extracted from `~/source/win9x-rust`); DOS items build on Desk's `dos/` foundation as the Liminal Installer's copy did. Until the templates exist, Quill keeps copies with `PROVENANCE.md` files (D27).

Priorities: **P0** blocks the first milestone that uses the platform, **P1** blocks a later milestone, **P2** makes life better.

## What exists today

| Piece | Where | State |
|---|---|---|
| Win16 target `win16.json` (`i386-unknown-none-code16`, 386 code with prefixes, softfloat ABI, `panic = abort`) | Spike, `retro/win16/` | Works for one code and one data segment |
| `link.ld` (code segment 1 and data segment 2, both at offset 0, 16 reserved bytes first in data) | Spike | One code segment only |
| Runtime `src/lib.rs`: task entry through `InitTask`, `export_proc!` wrapper, `cs!`, selector helpers, one-byte far read, `Log` | Spike | Enough for dialogs and a window |
| `gen_imports.py` and `imports.tbl` (64 KERNEL, USER and GDI imports by ordinal, Pascal convention, stubs that save EBX, ESI, EDI and reload ES) | Spike | Grows by table row |
| `ne_pack.py` (NE header, segment table, imports, entry table, selector fixups, dialog, string and raw resources from JSON) and `ne_dump.py` | Spike | One code segment; no menus, icons or bitmaps |
| Register probes (`probe.rs`): EBX, ESI, EDI, EBP and ESP across `GetVersion`, `GlobalAlloc`, `PeekMessage` and `Yield` | Spike | Showed Windows 3.11 clobbers high halves, hence the saving stubs. Yielding and interrupt cases not yet probed |
| `win9x-rust`: `i686-pc-windows-msvc`, `target-cpu=pentium`, `lld-link` with `/NODEFAULTLIB`, `/ENTRY:start`, `/SUBSYSTEM:WINDOWS,4.0`, `/FIXED`; `raw-dylib` ANSI imports; `memcpy` shims | `~/source/win9x-rust` | Runs on the Windows ME VM; no resources |
| Desk `dos/`: `dos-elf.json` (i486, x87), `link.ld`, `rt` (DPMI, heap, INT 21h, keyboard, timer, VGA, panic), `elf2go32.py`, `dosrun.sh` | Desk; copied into the Liminal Installer's `dos/` | Proven by Desk and the installer's `SETUP.EXE` |

## Win16 (`rust-win16`)

| Pri | Need | For | Notes |
|---|---|---|---|
| P0 | Runtime as a library crate with a stable API, and the build script as a reusable tool | M5, T0 | Today it is a spike crate with bins inside |
| P0 | Resource types in the packer: `RT_MENU`, `RT_ACCELERATOR`, `RT_ICON` with `RT_GROUP_ICON`, `RT_CURSOR` with `RT_GROUP_CURSOR`, `RT_BITMAP`, `RT_VERSION`; reading `.ico`, `.cur` and `.bmp` files | T0 | Menus and accelerators from the `quill-app` tree via JSON |
| P0 | Generated Rust constants for resource and command IDs | T0 | So code and resources cannot drift |
| P0 | Size report per build: each segment against its budget, DGROUP statics, stack and heap, resources, file size; non-zero exit past a budget | T0 | CI gate |
| P0 | Register probes for yielding calls: `GetMessage` while a second task runs, `DialogBox`, `SendMessage` to another task's window, a long loop under timer interrupts; FS and GS before and after | T1 | The top risk. Result decides whether stubs need to save more |
| P0 | Far-memory helpers: copy far to near, near to far, far to far, fill, compare; FS (or GS) loaded per call; ES restored to DS | T1 | Inline assembly, tested on the host by a pure-Rust model of the same API |
| P0 | Much larger import table: window and class APIs, carets, scroll bars, timers, menus, accelerators, GDI fonts, text, DIBs, palettes, regions, printing (`StartDoc` family, `SetAbortProc`, `Escape`), clipboard, profile APIs, `DOS3Call`, `GetFreeSystemResources`, `WinHelp`, hooks | T0–T2 | Rows in `imports.tbl`, checked against Wine's spec files |
| P0 | `COMMDLG.DLL` imports (open, save, font, colour, find, replace, print, extended error) and `SHELL.DLL` (registration database for file associations) | M5 | Ordinals from Wine's `commdlg.dll16.spec` and `shell.dll16.spec` |
| P0 | General callback wrappers beyond `export_proc!`'s four arguments: font enumeration (`lpLogFont`, `lpTextMetric`, type, data), abort procedure, hook procedures, common dialog hooks | M5 | A generator like the import one, by Pascal signature |
| P0 | A DOSBox-X harness for Windows 3.11: unpack the tree to a scratch directory, install the program and a `WIN.INI` `run=` line, boot, type keys, take screenshots, read `QUILL.LOG`, exit when the program calls `ExitWindows`; under Xvfb | T0 | Local only (D21). Screenshots from DOSBox-X's capture or the X window |
| P1 | **Multiple code segments**: `segments.toml` to linker script; ld.lld `--emit-relocs`; the packer finds near calls and jumps whose target is in another segment and rewrites each to a veneer in the caller's segment; veneers keep a shadow return stack in DGROUP; return trampolines per segment; per-segment relocation tables (internal far and selector fixups); segment flags (fixed, moveable, preload, load on call, discardable); entry-table bundles per segment | T2, M6 | Design in [architecture.md](architecture.md#code-segments). Includes stack-depth limits for the shadow stack and a clear failure when it overflows |
| P1 | Indirect-call table: every address-taken function gets a slot at the same offset in every code segment; `R_386_32` relocations to function starts are redirected to the slot; jump tables inside a function are left alone | T2 | The check below proves nothing was missed |
| P1 | Branch check: after packing, decode every near `call`, `jmp` and conditional branch and fail if its target lies outside its segment | T2 | The safety net for the veneer pass |
| P1 | Optional cloning of leaf helpers (`memcpy`, `memset`, `memmove`, `memcmp`, import stubs) into each segment that uses them | T2 | Saves a veneer on hot calls; measure first |
| P1 | `ne_addr2sym.py`: a `segment:offset` from a General Protection Fault box to a symbol, from a map the packer writes | T2 | The only debugger most of the time |
| P1 | Huge-block writer over `__AHINCR` for clipboard blocks over 64 KB | M6 | The only place Quill meets huge memory |
| P1 | `WH_MSGFILTER` hook support, for F1 in menus and dialogs | M9 | A callback shape the generator must cover |
| P1 | Stack high-water mark in test builds (fill pattern, report in the trace) | M6 | The 16 KB stack is a guess |
| P2 | Trace to a file (`QUILL.LOG`) and `OutputDebugString`, small enough for release builds behind a setting | M5 | The spike's `Log` grown up |
| P2 | Discardable segment testing: force discards (`GlobalCompact`) in a test build and check the program still runs | M11 | Moveable and discardable code is a memory saving on 4 MB machines |
| P2 | Sharing with the Liminal Installer: the installer's Win16 runtime (C and Open Watcom today, Rust under the spike) uses the same template, target, packer and harness, so a fix lands once | M11 | The installer project decides; Quill's needs are a superset of a setup program's |

## Win32 (`rust-win9x`)

| Pri | Need | For | Notes |
|---|---|---|---|
| P0 | Target and build: `build-std` of `core` for a Pentium (or the `i586-pc-windows-msvc` target), `lld-link` flags from `win9x-rust`, `panic = abort`, `opt-level = "z"`, LTO | T3, M3 | The prebuilt `core` assumes SSE2 (D33) |
| P0 | Instruction scan: disassemble the executable and fail on MMX, SSE, CMOV or anything else a Pentium lacks | T3 | Catches a stray prebuilt object |
| P0 | Import generator: a table like `imports.tbl` that writes `raw-dylib` blocks (`import_name_type = "undecorated"`), with an allowlist of functions Windows 95 exports, failing on anything else (no `...W`, no `...Ex` that 95 lacks) | T3 | The allowlist from Windows 95's `KERNEL32`, `USER32`, `GDI32`, `COMDLG32`, `SHELL32` export tables |
| P0 | Runtime: entry point with `GetCommandLineA` parsing, `memcpy`, `memmove`, `memset`, `memcmp` and `bcmp` that LLVM cannot recurse into, `__chkstk` for frames over 4 KB, `_fltused` guard, panic handler | T3 | Most of it exists in `win9x-rust` |
| P0 | Resource compiler: a `.res` writer from the same `quill.json` (menus, accelerators, dialogs in the Win32 template form, string tables, icons, cursors, bitmaps, `VS_VERSIONINFO`), linked by `lld-link` | T3 | D35. No manifest: 9x ignores one and Quill draws its own chrome. `llvm-rc` as an optional cross-check |
| P0 | Windows ME VM harness: start the VM headless, share a folder as a drive, run the program from `WIN.INI` `run=` or the StartUp group, type keys with `VBoxManage controlvm ... keyboardputscancode`, take screenshots with `screenshotpng`, read `QUILL.LOG` from the share | T3 | Local only |
| P0 | Wine harness: a `WINEPREFIX` set to Windows 98, Xvfb, `xdotool` for keys, `import` for screenshots | T3 | Runs in CI |
| P1 | Structure definitions at Windows 95 sizes (`OPENFILENAMEA` version 4, `DOCINFOA`, `CHOOSEFONTA`, `PRINTDLGA`) | M4 | A wrong size makes the dialog fail quietly on 95 |
| P1 | Size report like the Win16 one | M4 | One section, so simpler |

## DOS (Desk foundation)

| Pri | Need | For | Notes |
|---|---|---|---|
| P0 | Copy of Desk's `dos/` with `PROVENANCE.md`: target, linker script, `rt` with keyboard, timer and VGA kept (the installer's copy dropped them), `elf2go32.py`, `CWSDSTUB.EXE` packing, `fetch-tools.sh` | T4 | D36 |
| P0 | Headless DOSBox harness with `DOS_AUTOTYPE`, screen dumps (`=== screen N ===` frames, as the installer's `LI_DUMP` and `frames.py`) and golden comparison | T4 | From the Liminal Installer's `dos/tools/e2e.sh` |
| P0 | Text video: write the grid to `B800:0000` through a DPMI selector; 80 × 50 with the 8 × 8 font; hardware cursor shape for insert and overtype | M1 | Small additions to `rt::vga` |
| P1 | INT 33h mouse through DPMI real-mode calls | M2 | Optional at run time |
| P1 | VBE: mode list, linear frame buffer mapping (DPMI 0800h), 256-colour palette | M10 | Mode 12h planar comes from Desk's `vga` |
| P1 | WinOldAp clipboard (INT 2Fh, AX = 1700h family) when running in a Windows DOS box | M2 | Text only |
| P2 | A 386 build: `cpu` = `i386` and no x87 set-up, since Quill's core uses no floats | Later | Desk's floor is a 486; Quill's could be lower |

## Shared and host

| Pri | Need | For | Notes |
|---|---|---|---|
| P0 | Pinned toolchains: a `rust-toolchain.toml` per workspace (stable for the host, a dated nightly for `build-std`), `RUSTC_BOOTSTRAP` only where the spike already needs it | M0 | Desk pins `1.98.0`; the platform workspaces pin a nightly by date |
| P0 | Float ban: a link-map check that fails on softfloat symbols (`__addsf3`, `__divdf3` and the like) and on `_fltused` references in every build | M0 | D9. Fixed-point only |
| P0 | Panic-site count per build, reported and only allowed to go down | M0 | D19 |
| P0 | One command that builds every platform and runs every check that does not need a Windows image (`make validate` or `scripts/validate.sh`) | M0 | The AGENTS.md local gate |
| P1 | `quill-inflate` with a 32 KB ring window, resumable, from Lt. Fizz 2's `fizz-pack` decoder | M7 | D44. Shared by PNG and `.QPK` |
| P1 | Far-memory paging for the DAWG: a `PageSource` over file reads into `Store` chunks, with the four-page cache | M8 | Flat on Win32 and DOS, far chunks on Win16 |
| P1 | Metric capture tool: a Win16 test program that writes `GetCharWidth` and `GetTextMetrics` for the TrueType core fonts at common sizes, on the screen and on a printer information context, into `tests/metrics/` | M3 | Feeds `CapturedMeasure` |
| P1 | `quill-dict` with SCOWL and VarCon inputs, licence text carried into each `.DIC` | M8 | D49 |
| P1 | Desk face strike generator (`QUILL.FNT` and the host width tables) | M10 | From Desk's `fonts/` outline faces |
| P1 | `quill-pack` writer and reader; a FAT12 disk image writer (the Liminal Installer's `li-package` has one) | T5 | D57 |
| P2 | Liminal Installer hooks: the installer's DOS and Win16 runtimes link `quill-pack`'s reader | T5 | A proposal to the installer project, not a demand |

## Rules for anyone implementing these

- Name every path when removing files. Never remove with wildcard patterns (no `rm` with globs, no `find -delete` with patterns): they trip the owner's approval dialog, and a named list is easier to review anyway.
- Every tool gets a test that runs on the host, and every format a reader that is independent of its writer (as `ne_dump.py` is of `ne_pack.py`).
