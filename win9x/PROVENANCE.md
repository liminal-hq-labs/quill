# Provenance

The Win32 build follows `win9x-rust` (`~/source/win9x-rust`), which proved a Rust program with no C runtime running on Windows ME: `lld-link` with `/NODEFAULTLIB`, `/ENTRY:start`, `/SUBSYSTEM:WINDOWS,4.0`, `/FIXED`, and `raw-dylib` imports of ANSI entry points.

| Here | From | Changes |
|---|---|---|
| `.cargo/config.toml` | `win9x-rust/.cargo/config.toml` | The target is `win9x.json` and `core` and `alloc` are built from source (`-Zbuild-std`), so the code is Pentium class and the prebuilt SSE2 `core` (D33) is not used |
| `win9x.json` | `rustc --print target-spec-json --target i686-pc-windows-msvc` | CPU `pentium`, MMX, SSE and x87 off, the soft-float ABI, `i586` target triple, panic abort |
| `quill-win9x/src/sys.rs` | The import and structure declarations in `win9x-rust/src/main.rs` | More calls: files, timers, fonts, the heap |

Quill's own: `quill-win9x/src/rt.rs` (the allocator over the process heap, the 64-bit division helpers the MSVC ABI expects, `__chkstk`, `_fltused`, the panic handler) and `quill-win9x/src/main.rs`.

Not done yet: the instruction scan (D33) and the import allowlist for Windows 95 (see `docs/toolchain-needs.md`), and `.res` resources (D35).
