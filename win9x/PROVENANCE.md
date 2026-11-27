# Provenance

The Win32 build follows `win9x-rust` (`~/source/win9x-rust`), which proved a Rust program with no C runtime running on Windows ME: `lld-link` with `/NODEFAULTLIB`, `/ENTRY:start`, `/SUBSYSTEM:WINDOWS,4.0`, `/FIXED`, and `raw-dylib` imports of ANSI entry points.

| Here | From | Changes |
|---|---|---|
| `.cargo/config.toml` | `win9x-rust/.cargo/config.toml` | The target is `win9x.json` and `core` and `alloc` are built from source (`-Zbuild-std`), so the code is Pentium class and the prebuilt SSE2 `core` (D33) is not used |
| `win9x.json` | `rustc --print target-spec-json --target i686-pc-windows-msvc` | CPU `pentium`, MMX, SSE and x87 off, the soft-float ABI, `i586` target triple, panic abort |
| `quill-win9x/src/sys.rs` | The import and structure declarations in `win9x-rust/src/main.rs` | More calls: files, timers, fonts, the heap |

Quill's own: `quill-w32rt` (the allocator over the process heap, the 64-bit division helpers the MSVC ABI expects, `__chkstk`, `_fltused` and the panic handler, as the macro `runtime!` each artefact expands into itself; the DLL entry macro; the module loader), `quill-win9x/src/main.rs` and `quill-win9x/build.rs`.

## The DLL recipe

Modules are `cdylib` crates in `mods/` of this workspace (W9X-41, W9X-132). The recipe was proved by a throwaway spike (`~/Documents/Liminal HQ/Quill/Modules/Win95 spike/`, `docs/plans/modules.md` section 19.2); the spike's ideas are here, none of its files.

| Piece | From the spike | Quill's |
|---|---|---|
| Toolchain | The program's `win9x.json`, `-Zbuild-std`, the release profile and `lld-link` | Unchanged; `/ENTRY:start`, `/FIXED` and the resources moved from the shared `.cargo/config.toml` into `quill-win9x/build.rs`, so a DLL is not linked as a fixed program |
| Module link flags | `/ENTRY:QuillDllEntry`, `/BASE`, `/FILEALIGN:512` | Also `/DYNAMICBASE:NO` and a `/MAP` beside the DLL (`mods/tst/build.rs` is the pattern) |
| Entry and `_fltused` | Assembly labels, because a `cdylib` exports every `#[no_mangle]` item | `quill_w32rt::dll_entry!()` and `runtime!(dll)` |
| Exports | `QuillModuleInfo` and `QuillModuleStart` as `#[no_mangle] extern "C"` | The same, and the glue reads a caller's struct only after its `size` (`quill_w32rt::dll`) |
| Loader | A test program's `LoadLibraryA` loop | `quill_w32rt::module`, the loader the shell will use, and `mods/modhost`, which tests it under `tools/wine-modules.sh` |
| Checks | `check-imports.sh` and `scan-pentium.sh` unchanged | Also `tools/check-dll.sh` (the image) and a list argument for `check-imports.sh` (a module's own imports) |

`tools/pe-digest.py` prints a program's SHA-256 with the link's time stamps, the PDB name and its GUID zeroed: two links of one source in one folder give one digest, which is how a change to the program that must not change its code is shown.

Not done yet: the instruction scan (D33) and the import allowlist for Windows 95 (see `docs/toolchain-needs.md`), and `.res` resources (D35).
