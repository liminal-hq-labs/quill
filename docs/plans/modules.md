# Optional modules

Quill keeps running into sizing notes: `QUILLDOS.EXE` has about 4.6 KB of its budget left, the Windows 3.x data segment about 1.9 KB, and the planned features (Preferences, Reveal Codes, Unicode, spelling, pictures, PDF) each carry a size row. The owner's idea (2026-11-27): use DLLs for the platforms to split out the work. This page plans that as one architecture with a binding per platform: an **optional module** is a cold feature built from shared `no_std` crates behind one C-style, versioned interface, packaged as an NE DLL on Windows 3.x, as a loadable `.DMD` file on DOS, and linked in statically everywhere else.

Written against `origin/main` at `b060d36`. It supersedes the DOS and Win16 verdicts of the earlier, unmerged `~/Documents/Liminal HQ/Quill/Quill modules and DLLs plan.md` (2026-11-25), keeps the decision ids that plan and the Liminal Installer's [shared libraries plan](https://github.com/liminal-hq-labs/liminal-installer/blob/main/docs/plans/shared-libraries.md) already use for Win16, and adopts Desk's `.DMD` container for DOS. Nothing here changes behaviour until a package lands, so `SPEC.md` and `docs/decisions.md` change with the packages.

**Status.** WP-MOD-1 is built: `crates/quill-modabi` (CORE-90) and Desk's `.DMD` crates in `dos/module/` (DOS-50), host only, with no executable changed. WP-MOD-2 (the loader) is next; nothing else in the work packages has started.

**Revised 2026-11-27 (Windows 95).** The owner reversed W9X-40: "let's do the DLLs for Windows 95 so they keep in line with the app split in its siblings". Windows 95, 98 and Me now split the same way as DOS and Windows 3.x, with Win32 DLLs beside `QUILL32.EXE` over the same `quill-modabi` contract and module crates; only the loader differs. [Section 19](#19-windows-95-and-later-dll-modules) is the design, measured on `origin/main` at `08bc6dc`, with a throwaway no-CRT DLL built and run under Wine to prove the toolchain (WP-MOD-9 to WP-MOD-13).

**Markers.** **[M]** measured for this plan (method, scripts and raw output in `~/Documents/Liminal HQ/Quill/Modules/`); **[V]** read in the code or documents named; **[W]** from Microsoft, DJGPP, Wine or otvdm sources, not run here; **[E]** estimate for the package to measure; **[U]** not checked, the package must check it. No emulator was started and nothing was built for this plan: the DOS figures come from an existing build of `refactor/quill-picture` (on `39b5683`, the day's `main` plus WP-ARCH-2 in progress), read with `nm`.

## Table of contents

- [1. Bottom line](#1-bottom-line)
- [2. What the sizing notes say](#2-what-the-sizing-notes-say)
- [3. Measurements](#3-measurements)
- [4. One module, two packagings](#4-one-module-two-packagings)
- [5. DOS: `.DMD` modules](#5-dos-dmd-modules)
- [6. Windows 3.x: NE DLLs](#6-windows-3x-ne-dlls)
- [7. Windows 95 and later, Linux, WinUI and the web](#7-windows-95-and-later-linux-winui-and-the-web)
- [8. Which features go where](#8-which-features-go-where)
- [9. The installer](#9-the-installer)
- [10. The DOS budget policy and the ledger](#10-the-dos-budget-policy-and-the-ledger)
- [11. The ARCH and ED lanes, Reveal Codes and Preferences](#11-the-arch-and-ed-lanes-reveal-codes-and-preferences)
- [12. Risks](#12-risks)
- [13. Tests](#13-tests)
- [14. Decisions](#14-decisions)
- [15. Work packages](#15-work-packages)
- [16. Briefs](#16-briefs)
- [17. Owner decisions](#17-owner-decisions)
- [18. Found while planning](#18-found-while-planning)
- [19. Windows 95 and later DLL modules](#19-windows-95-and-later-dll-modules)

## 1. Bottom line

A module relieves a different limit on each platform, and only where the feature is **cold** (used now and then, not per key or per paint) and **narrow** (plain data in and out, no `App` across the boundary).

| Platform | What a module relieves | Verdict | First step |
|---|---|---|---|
| DOS | The `QUILLDOS.EXE` budget (DOS-70's stop at 480 KB) and the memory the program holds at start. **Not** the floppy: a module sits on the same disk | **GO**, with Desk's `.DMD` format (copied with provenance) and a loader in `dos/rt` | WP-MOD-1 to WP-MOD-3: `QUILLPIC.DMD` (PNG decoding) out of the EXE, end to end, with the installer |
| Windows 3.x | A second 64 KB data segment for a module's own working memory. **Not** the 1.9 KB left in `QUILL.EXE`'s segment: what fills it (the `App`, the stack, the UI text, the `Store` table) cannot move | **GO** for picture decoding: rust-win16's DLL runtime passed its go/no-go (rust-win16 pull request 6, D28 to D30) | WP-MOD-6: `QUILLPIC.DLL` from the same module crate as DOS, on the refreshed vendored packer |
| Windows 95 and later | No hard limit: a PE image is paged on demand and the address space is flat. What a module gives here is the **same split as the siblings** (one module crate and one contract on three builds), cold code out of `QUILL32.EXE` (about 28 to 42 KB [E from M]) and Print to PDF as a component of its own | **GO** (W9X-40 revised by the owner, 2026-11-27; it was NO-GO) | WP-MOD-9: the Win32 module toolchain and loader now, then `QUILLPIC.DLL` from the same crate as `QUILLPIC.DMD` (section 19) |
| Linux (GTK and terminal), WinUI, web | Nothing: no segment, file or memory limit applies | **NO** | — |

**The proof** is one module on one platform: `QUILLPIC.DMD` on DOS (WP-MOD-3), built on the format and loader packages WP-MOD-1 and WP-MOD-2. It is chosen because it is the narrowest real interface (bytes in, rows out), it already exists with goldens, it is the same module Windows 3.x needs, and JPEG (10 to 16 KB on DOS, pictures plan J2) then lands in it without touching the EXE. Its own saving is small (the EXE moves by −1 to +1 KB [E], because the loader costs about what PNG frees); the **first real saving** is WP-MOD-4, the DOS file dialog as `QUILLDLG.DMD`, about **−26 to −29 KB** of `QUILLDOS.EXE` [E from M].

**What it buys the track.** Without modules the editor ledger and Reveal Codes take `QUILLDOS.EXE` to about 480 to 502 KB, past DOS-70's stop. With the file dialog and the modal dialogs (Options, Page Layout, Numbering, Header and Footer) in `QUILLDLG.DMD`, the same features land at about **438 to 458 KB** (section 10), and no package needs to ask about the stop. On Windows 3.x the sizing notes are about the data segment, and the cure there stays the W16-60 reliefs (WP-ED-5, WP-ARCH-5, WP-ARCH-9, about 4.4 KB); a DLL keeps new working memory out of it, which is what pictures need.

## 2. What the sizing notes say

| Limit | Now | What is coming | Source |
|---|---|---|---|
| `QUILLDOS.EXE` | 437,760 of 442,368 bytes: **4,608 left** [M] (DOS-48) | Editor ledger +31 to +48 KB; Reveal Codes +11 to +16.5 KB; Preferences +10.8 to +19.1 KB (inside the ledger as WP-ED-13 and WP-ED-15); Unicode U2 +5 to +8 KB (inside the ledger); JPEG +10 to 16 KB | [work-packages.md](../work-packages.md#budgets), [reveal-codes.md](reveal-codes.md) (pull request 161), [preferences.md](preferences.md), pictures plan |
| DOS-70's stop | 480 KB (491,520 bytes), or the DOS floppy keeping less than 128 KB free | Likely reached "near the dialogs" (work-packages.md) | DOS-70 |
| DOS floppy | About 427,000 bytes free (DOS-48, estimated); the EXE packs to about 58 % (255,525 of 437,760 bytes with `gzip -9` [M]) | +16 KB of code costs about 9.5 KB of disk | DOS-48, [M] |
| Windows 3.x data segment | About **1.9 KB** free; nothing merges below 1,024 (W16-60) | Preferences +100 to +340 bytes, Reveal Codes up to +72, Unicode +20 to +150; reliefs −4.4 KB planned | W16-60, preferences.md, reveal-codes.md |
| Windows 3.x code | 8 segments of 253 possible, each under its budget | Not short | `segments.json` [V] |
| `QUILL32.EXE` | 441,856 bytes on `08bc6dc` [M]: Print to PDF is 32,256 of it and picture decoding 18,432 (section 19.2); 452,608 with the rich clipboard (pull request 196 [V]) | No limit beyond the Windows 95 disk; section 19 adds a budget row | section 19.2 [M], shared libraries plan |

**Two different problems.** DOS has one flat address space and plenty of extended memory, and its limit is a number the project chose to keep the program lean and the floppy comfortable; a module moves bytes out of that number. Windows 3.x has a hard 64 KB near segment, and its limit is physical; a DLL cannot take anything out of it, but gives new features a segment of their own.

**Is the DOS EXE budget the real constraint, or the floppy?** Neither binds physically at 480 KB. The floppy is the product's physical limit, and splitting a program into files on the same disk does not shrink it (it adds 3 to 6 KB packed per module, section 5.9). Memory does not bind either: on the 4 MB floor, CWSDPMI has about 3 MB of extended memory [E], and the image, its 256 KB stack and the heap fit with room. The EXE budget is the **design limit** that keeps the program honest, and it is what the sizing notes keep meeting. So on DOS a module is worth it for three reasons, in order: it ends the package-by-package budget negotiation for cold features; it keeps cold code out of memory until it is used (about 30 KB less held at start once the dialogs move); and it lets an optional feature be an unticked installer component (help, PDF, converters). It is not worth it for the floppy.

## 3. Measurements

### 3.1 DOS, by area [M]

From the `quill-dos` ELF (`.text` 414,208, `.data` 512, `.bss` 271,360), each text and data symbol charged to the path of its function with generic arguments removed (`groups.py`). LTO inlines across crates, so a large dispatcher holds pieces of many areas; read the figures as lower bounds for each area.

| Area | Bytes | Share |
|---|---:|---:|
| App (editor, commands, dispatch) | 97,486 | 25.0 % |
| Graphics screen (`quill-paint`, strikes, VGA) | 69,564 | 17.8 % |
| **File dialog** | **43,572** | 11.2 % |
| Text screens | 32,958 | 8.5 % |
| Core and layout | 24,211 | 6.2 % |
| Pointer | 23,620 | 6.1 % |
| RTF | 19,995 | 5.1 % |
| Runtime (`core`, `alloc`, `rt`) | 17,159 | 4.4 % |
| Spelling | 12,240 | 3.1 % |
| QLD and text formats | 9,925 | 2.5 % |
| Autosave and recovery | 9,116 | 2.3 % |
| Pictures | 8,231 | 2.1 % |
| Other | 20,381 | 5.2 % |

### 3.2 What could move [M]

| Area | Movable into a module | Stays in the program (the seam, the editor side) |
|---|---:|---:|
| File dialog | 31,274: `quill_dlg`, `quill_paint::filedlg`, the text painter `filepaint` | 12,498: `App::try_file_dialog`, `file_outcome`, `file_overlay`, `quill_dos::file_dialog`, `DosFiles` |
| Pictures | 4,009: `quill_image` (PNG, dither, scale), `quill_inflate` | 4,222: `App::insert_picture`, geometry, layout, `quill_picture` |
| Spelling | 2,801: `quill_spell` | 9,439: the scanner, the dialog flow and the marks in `App` |

The history agrees: the text file dialog took the EXE from 382,464 to 415,232 bytes once its painter was shared (DOS-37), and the graphics painter about 10 KB more (DOS-38).

**What this shows.** On DOS the big cold piece is the dialog machinery, not the decoders: the file dialog is about 31 KB of movable code, and every modal dialog the editor track adds is more of the same kind. Spelling and pictures are small today; pictures grow with JPEG. Reveal Codes, Unicode U2 and the editor commands are in the 97 KB of `App` and the core, which a module cannot hold.

### 3.3 Windows 3.x [V]

`QUILL.EXE` is 406,832 bytes in the build above [M]. The data segment's tenants were measured by the 2026-11-25 plan (`quill::APP` 7,840, `quill::STORE` 1,280, `w16::fault::STACK` 1,024, `w16::LOG` 644, `gdi::FONTS` 480, 15,160 bytes of anonymous read-only data owned mostly by the paint path, the tooltips, the menus and the RTF tables) and are near because every build reaches them through DS. A PNG at the shared 4,096-pixel limit needs about 126 KB of working memory and a JPEG a 32 KB work block, which is why Windows 3.x still draws PNG as a grey box.

## 4. One module, two packagings

### 4.1 The parts

| Part | Where | What it is |
|---|---|---|
| **`quill-modabi`** | `crates/quill-modabi`, base layer, `no_std`, no `alloc`, no floats | The interface and nothing else: `#[repr(C)]` structs of `u16` and `u32`, status codes, kind ids, the version constants, the start block, the chunk list, and for each module kind its operation list (ordinals for Win16, a table of `extern "C"` entries for DOS). No Rust types, slices, traits or `Option` cross (CORE-90) |
| **The module's logic** | An ordinary shared crate below `quill-app`, such as `crates/quill-modpic` (base: over `quill-image` and `quill-inflate`) | Platform-free code written against caller-given buffers (CORE-91): a session state machine, bytes in from a chunk list, rows out to a caller's block. Tested on the host like any crate |
| **The seam** | The crate that owns the feature: `quill-picture` for decoding, `quill-dlg` and `quill-flow` for dialogs | A trait the shells implement, with a `Static` implementation that calls the logic directly and a module implementation per platform (ARCH-40). `App` never knows which it has |
| **The packaging** | `dos/mods/<name>` (a `.DMD`), `win16/examples/<name>` (a DLL) | A few dozen lines of glue: the entry, the table, address translation (near offsets on DOS, far pointers on Win16), the panic handler |
| **The loader** | `dos/rt/src/module.rs`; `w16::dll` from rust-win16 | Finds the file beside the program, checks it, binds it, and reports a status the seam turns into a greyed feature |

A Linux, WinUI or web build links the logic crate statically through `Static` and never sees `quill-modabi` at run time; the host tests use `Static` too, plus a fake that answers `Missing` to test the absent path (CORE-96). The Win32 build was in that list until 2026-11-27; it is now a third packaging (`win9x/mods/<name>`, a Win32 DLL) with its loader in `win9x/quill-win9x/src/modules.rs` (section 19).

### 4.2 The interface (CORE-90)

```rust
pub const ABI_MAJOR: u16 = 1;
pub const ABI_MINOR: u16 = 0;

#[repr(C)]
pub struct ModInfo {    // filled by the module's info entry
	pub size: u16,      // of this struct, so a newer module can add fields
	pub abi_major: u16, // the caller refuses another major
	pub abi_minor: u16, // a higher minor is accepted
	pub kind: u16,      // KIND_PIC 1, KIND_DLG 2, KIND_HLP 3 (reserved), KIND_PDF 4 (reserved)
	pub build_id: u32,  // the commit both files were built from; release builds require equality (CORE-94)
	pub arena: u32,     // bytes of working memory the module holds (Win16), 0 on DOS
	pub limit: u32,     // kind-specific, for pictures the largest side
}

#[repr(C)]
pub struct Chunk { pub addr: u32, pub len: u32 } // addr: a near offset on DOS, sel:off on Win16

pub type Status = u16; // OK 0, MORE 1, BUSY 2, UNSUPPORTED 3, CORRUPT 4, TOO_LARGE 5, NOALIAS 6, INTERNAL 7, MISSING 8, VERSION 9
```

The picture kind (`KIND_PIC`, version 1) has the four operations of the 2026-11-25 plan: `info(*ModInfo)`, `open(owner, kind, *ChunkList, *PicOpen)`, `rows(*PicRows)` (decode up to N rows at the shown size into the caller's DIB rows, `MORE` until the last) and `close()`. The rows-per-call shape fits the idle-slice scheduler of the threading plan. On Win16 they are ordinals 2 to 5 with `WEP` at 1; on DOS they are a `PicTableV1` the module fills at start.

**Addresses.** One `u32` "module address" type: a near offset on DOS (the flat 4 GiB data segment of `rt::dpmi`), a packed selector and offset on Win16. Only the packaging glue turns it into something it reads or writes; the logic crate sees `&[u8]` and `&mut [u8]` given to it by the glue, one chunk or one row at a time.

### 4.3 The module-ready rule (CORE-92, ARCH-41)

A cold feature that might become a module is written so that it can, at no cost now:

1. Its logic lives in a crate below `quill-app` (base or kit), with no `App`, no `Store` generic in its public interface, and no statics.
2. It takes and returns plain data (`#[repr(C)]`-able structs, byte slices, ids), and draws through a surface trait or into a buffer it is given.
3. It runs as a state machine the shell drives (a key, a point, a paint, an answer), never as a loop that waits for input.
4. Its texts are its own table (or resources on Win16), not `&str`s scattered through `App`.

`quill-dlg`'s file dialog already nearly follows it (DOS-32 to DOS-39). The Preferences dialog (WP-PREF-5, WP-PREF-6) and the editor dialogs (WP-ED-17 to WP-ED-19) are asked to follow it from their first commit (section 11).

### 4.4 When a module is absent (CORE-93)

- **Never a system box, never a crash.** Windows 3.x loads with `SetErrorMode(SEM_NOOPENFILEERRORBOX)`; DOS opens the file itself.
- **The feature greys out where it is offered**, through the menu state (`quill-menu`'s enable rule gains "needs module"), and says why where it would have acted: a picture of a kind that needs `QUILLPIC` shows the grey box with `This picture needs QUILLPIC.DMD, which is missing or from another build of Quill.` (`.DLL` on Windows 3.x); a dialog in `QUILLDLG` that is missing gives `Quill cannot show this dialog: QUILLDLG.DMD is missing or from another build. Run Setup again to repair Quill.` on the status line.
- **What never needs a module.** Text editing, opening and saving through a typed name (the file dialog's fallback is the name field alone, as the Windows builds' common dialog falls back), plain BMP (drawn by GDI on Windows 3.x, decoded by a 300-byte reader in the EXE on DOS), printing.
- **Why.** The first modules are in required components, so an absent one means a damaged install or a hand-copied EXE; the message points at Setup. Optional modules (help, PDF) grey their menu item with the same message naming the component.
- **The reason is logged** (`QUILL.LOG` with `[Debug] Trace=1` on DOS, `PICS.LOG` on Windows 3.x): the file, the status, the build ids.

### 4.5 Versions (CORE-94)

- **The set is the version.** The EXE and its modules are built by one `build.sh` run, and both embed the same 32-bit build id (the commit's first eight hex digits, with the top bit set for a dirty tree). A release build accepts only its own build id and the same `abi_major`; a development build accepts another build id with a log line, so a builder can rebuild one side.
- **No digit in the file name.** The shared libraries plan's rule 3 (`AGCORE1.DLL`) is for files that outlive their programs; Quill's modules are private and always shipped with their EXE, so `QUILLPIC.DLL` and `QUILLPIC.DMD`. On Windows 3.x the module name is global, so another copy of Quill's `QUILLPIC` already loaded is caught by the build id, and the message names it.
- **The DOS file carries more.** The `.DMD` header has a format version (the magic), an SDK version (the host-service table, section 5.6) and a CRC-32 of the whole file, so a damaged floppy copy is refused before any of it runs.
- **A `VERSIONINFO`** in each DLL matching the EXE's, for File Manager; it is not a check.

### 4.6 Where modules are found (CORE-95)

Beside the program only, by full path: `GetModuleFileName` on Windows 3.x, the program's path from the environment block on DOS (`rt::env` [U: confirm it reads the path after the environment's double NUL; it is what DOS 3 and later put there]). Never the current folder, `PATH`, `C:\WINDOWS` or `C:\WINDOWS\SYSTEM`, so a stale copy elsewhere cannot win (Windows 3.1's `LoadLibrary` with a bare name looks in the current folder and the Windows folders first [W]).

## 5. DOS: `.DMD` modules

### 5.1 What DOS has, and what it does not

- **No DLLs, no loader.** `QUILLDOS.EXE` is a pure-Rust `no_std` program on Desk's `d0-rust-dos` foundation, packed behind CWSDPMI's `CWSDSTUB.EXE` by `elf2go32.py`; the stub loads one image at a fixed offset with no relocations [V: `dos/build.sh`, `dos/link.ld`].
- **DXE3 is out.** It needs DJGPP's C library and `dlopen` at run time and `dxe3gen` at build time; Desk proved it (`S1-DXE3`) and rejected it for its product [V: Desk `docs/LOADER.md`].
- **Classic overlays are out.** Under a DPMI host the program is flat in extended memory; an overlay manager that swaps code into a fixed window saves no memory that matters and needs a linker that understands it.
- **What the extender allows** (CWSDPMI r7 inside the stub, DPMI 0.9): descriptors (0000h to 000Ch, including 0008h to set a limit), memory blocks (0501h to 0503h, page-aligned, committed), DOS memory and interrupts; `rt` already lifts DS to 4 GiB with 0008h [V: `rt/src/dpmi.rs`]. A 386 has no no-execute bit, so heap memory can hold code once CS reaches it. CS keeps the stub's limit, the image block, so code in a heap block is outside it until the loader widens CS (section 5.4) [V: Desk `LOADER.md`, inferred there, not observed].
- **Desk's answer exists.** Desk designed and built `.DMD`, "an own container: one image linked at `0x0040_0000` with static code, a table of absolute words to relocate, named import slots, three entry offsets, a kind, a format version, an SDK version and a CRC-32", with a `no_std`, `forbid(unsafe_code)` format crate (`desk-module`: parse, validate, write, `load_into`), a Rust packer (`dmdpack`, with a two-base cross check that proves every absolute word is listed) and a guest crate (`desk-guest`: the `import!` macro and the panic handler), all host-tested [V: `liminal-hq/desk` `module/` at `862ec02`]. The native loader runs (M4); the **hosted** DOS loader (Desk's M5) is not built yet [V].

**Decision DOS-50:** Quill's DOS modules are Desk `.DMD` files (kind 1, module), with `desk-module`, `dmdpack` and `desk-guest` copied into `dos/module/` with provenance as the rest of `dos/` is, and the hosted loader written in Quill's `rt` (and offered back to Desk as its M5).

### 5.2 Building a module

```text
dos/mods/quillpic (no_std, no_main bin; target ../dos-elf.json; -Tmodule.ld --emit-relocs; link base 0x400000)
  --cargo -Zbuild-std=core,alloc,compiler_builtins--> ELF (kept in dos/target/mods/ for debugging)
  --dmdpack pack--> dos/dist/QUILLPIC.DMD
  --dmdpack cross (the same sources linked at 0x800000)--> fails the build if any absolute word is unlisted
```

`dos/tools/build-mods.sh` runs this for every module, and `dos/build.sh` calls it after the EXE. The module's profile matches the EXE's (`opt-level = "z"`, LTO inside the module, `panic = "abort"`, `-Zlocation-detail=none`, `-Zfmt-debug=none`), and the paths are remapped as Desk's are so no build-host path is in the file.

### 5.3 The loader (`rt::module`)

1. **Find and read.** Open `<program folder>\QUILLPIC.DMD`, read it whole into a heap buffer (a module is at most 256 KB in Quill, under the format's 16 MiB).
2. **Check before anything runs.** `desk_module::load_into` checks the header, the layout, every relocation and import and the CRC, then copies the image into a destination of `memory size` bytes at the header's alignment, zeroes `.bss`, adds the delta to each relocated word and writes each import's address into its slot. It runs nothing and leaves a refused destination untouched [V].
3. **Bind.** Imports resolve by name against the host's table (section 5.6). A missing name refuses the module with that name.
4. **Reach it.** Widen CS once (section 5.4).
5. **Start.** Call the module's `app_start(1, &ModStart, size)`: the host's ABI version and build id in, the kind's table out. A module that refuses the host's major returns non-zero, and the loader frees it.
6. **Free the file buffer.** The image stays; the table entries are kept in a fixed array of four slots in `quill-dos`.

The loader in the EXE costs about **2.5 to 4 KB** of `QUILLDOS.EXE` (`desk-module`'s checks and CRC at `opt-level = "z"`, the error enum as codes, the file read) and **0.5 to 1 KB** of glue [E]. That is paid once, by the first module.

### 5.4 The code segment (DOS-53)

The loader sets the limit of CS to 4 GiB with DPMI 0008h and reloads CS with a far return, once, before the first call into a module; DJGPP's near-pointer mode widens CS the same way [W, unverified]. It first checks that CS and DS have the same base (DPMI 0006h on both), because the module's addresses are offsets from that base, and refuses modules (`NOALIAS`, logged) if they do not or if 0008h fails.

**Fallback.** If CWSDPMI or another host refuses, a cargo feature `module-arena` reserves the module space inside the EXE's own `.bss` (64 KB for `QUILLPIC` and `QUILLDLG` together), which CS already covers. It costs memory, not file bytes. WP-MOD-2 tests the widening before anything depends on it.

**Negative test.** DOSBox may not enforce the CS limit on instruction fetch [U] (Desk's M5 names the same doubt), so the "call without widening faults" check can only pass where the limit is enforced: DOSBox-X with `cputype=386_prefetch` or 86Box if at hand; otherwise it is recorded as not verified, and the positive path is what the gate needs.

### 5.5 Memory, stack and lifetime (DOS-54)

- **Memory.** The image is placed in a heap block; `.bss` is zeroed there. Decoders run on the caller's 256 KB stack as they do today, so `QUILLPIC.DMD` needs no arena on DOS.
- **Load on first use, keep until exit.** The first PNG on the graphics screen loads `QUILLPIC`; the first Open loads `QUILLDLG`. Extended memory is plentiful, so a module stays loaded; when memory runs out, the existing "drop pictures out of view" step also unloads `QUILLPIC` if no session is open.
- **No interrupt context.** Modules never run in the IRQ 12 handler or the INT 24h stub.
- **Panics (version 1).** The module's panic handler imports `desk_panic`, which the host binds to its own panic path: a panic in a module is a panic of the program, exactly as today when the same code is linked in. Containment (Desk's guard, `desk_guard_call`, its M7) can come later without a format change.

### 5.6 Host services (the SDK version)

| SDK | Services | Used by |
|---|---|---|
| 1 | `desk_log(ptr, len)`, `desk_panic(ptr, len)`, `desk_alloc(size, align)`, `desk_free(ptr, size, align)` (Desk's four names, bound to `rt`'s log and heap) | `QUILLPIC.DMD` (only the first two) |
| 2 | The dialog host (`quill_dlg_*`): present the text grid's rows; graphics primitives over the shadow (fill, bevel, text in a strike, icon, cursor rectangle); the file system calls `quill-dlg::fs` needs (`find_first`, `find_next`, drives, the critical-error status); settings get and set by id; the clock | `QUILLDLG.DMD` (WP-MOD-4) |

Services never call back into a module. A module may assume the direction flag is clear and the x87 at the host's control word.

### 5.7 Debugging

The ELF is kept beside each `.DMD` in the target folder. With `[Debug] Trace=1` the loader writes the module's load address to `QUILL.LOG`, so `gdb`'s `add-symbol-file quillpic.elf -o <address − 0x400000>` gives symbols; the DOSBox debugger and `addr2line` take the same offset. A module panic prints its message through the host's panic path, with the module's name.

### 5.8 Start-up time on a 386

The EXE gets smaller, so start-up gets slightly faster (about 30 KB less to read once `QUILLDLG` moves: tens of milliseconds from a hard disk [E]). The first use of a module costs an open, a read of 8 to 40 KB, the CRC (table-driven, about 10 cycles a byte: 25 ms for 40 KB on a 16 MHz 386SX [E]) and a relocation pass over a few hundred words: about **50 to 150 ms on a 386SX from a hard disk** [E], once a session. The bench gate (WP-MOD-3) measures it under DOSBox with the 386 cycles profile and records it.

### 5.9 The floppy and the budget accounting (DOS-51, DOS-52)

- **The disk.** A module adds its 96-byte header, its relocation table (about 2 to 3 % of the image [E], against Desk's measured 54 words for 4.6 KB), and its own copy of the bits of `core` and `compiler_builtins` it uses (1.5 to 3 KB [E]). Packed, about **3 to 6 KB per module** more than the same code inside the EXE. Against about 427 KB free on the DOS floppy, that is nothing; it is reported, not gated.
- **The accounting.** Three figures, each in `dos/tools/size-report.sh` and each package's PR:
  1. `QUILLDOS.EXE` against its budget (DOS-70's rule, section 10).
  2. Each module against its own budget: `QUILLPIC.DMD` 16 KB until JPEG, then 40 KB; `QUILLDLG.DMD` 64 KB.
  3. **Program bytes**: the EXE and its required modules, packed, on the DOS disk image (DOS-51), with the floppy's free bytes. This is the real limit, and the 128 KB floor of DOS-70 stays on it.

## 6. Windows 3.x: NE DLLs

### 6.1 What a DLL is for here

A Win16 DLL with the single-data flag has one data segment of its own, shared by every task that loads it, up to 64 KB, apart from `QUILL.EXE`'s [W]. Exports are called far on the caller's stack (SS is the caller's, DS the DLL's), and LLVM's `code16` output treats every pointer as DS-relative, so Rust never runs on the caller's stack: each export switches to a private stack inside the DLL's own segment through a 32-bit alias (W16-42). That design, the loader rules (W16-43) and the packer's library mode (W16-41) are the 2026-11-25 plan's and the shared libraries plan's (WP-SL-2), and are not repeated here.

**The gate passed** [V: rust-win16 D28 to D30]. The packer's library mode merged (rust-win16 pull request 5), and the runtime (`w16dll`: `LibEntry`, `WEP`, `export_dll!` as a 53-byte thunk onto a 6,144-byte private stack on a 32-bit alias, the panic path, `DYN` rows for `GetProcAddress` slots) went **go** in pull request 6: 30 of 30 runs of `DLLHOST.EXE` on Windows for Workgroups 3.11 in DOSBox-X (full path and bare name, the data segment moved between calls, a panic and the poisoned state, 200 load and unload cycles with the same alias selector and the free selectors as in a fresh Windows) and 3 of 3 under Wine. Not run there: otvdm on ReactOS, Windows 3.1 and 95 to Me, `WEP` at system exit. Pull request 6 was closed when its base branch merged, so the runtime is on rust-win16's `feat/win16-dll` [V]; WP-MOD-6 copies it once it is on rust-win16's `main` (or from that branch's commit, recorded in `PROVENANCE.md`). Four findings change WP-MOD-6:

- **The vendored packer must be refreshed** in `win16/tools/` from rust-win16 (library mode, `DYN` rows, the new KERNEL rows) per D89, before Quill builds a DLL.
- **Quill calls `FreeLibrary` itself**, at exit as well as after idle: Windows 3.11 does not unload a library loaded with `LoadLibrary` when the task that loaded it ends, so a Quill that exits without it leaves `QUILLPIC` and its 64 KB loaded until Windows restarts.
- **The arena is about 57 KB**: 64 KB less the 6,144-byte private stack, the shadow stack, the read-only data and the header, so PNG's two raw rows stay near up to wider pictures than the 48 KB this plan first assumed.
- **Every `INT 31h` keeps DS** in the runtime (Wine assumes DS = SS across a DPMI call, which is false in a library); Quill's glue keeps the rule.

**What a DLL does not do:** free any of the 1.9 KB in `QUILL.EXE`'s segment. A `QUILLPIC.DLL` costs the EXE about 60 to 120 bytes (the module handle, four entry slots of 4 bytes, the name, two messages) [E].

### 6.2 Which features (W16-49)

| Feature | Why a DLL | Verdict |
|---|---|---|
| **Picture decoding** (PNG now, then JPEG, RLE4 and the other BMP variants) | About 57 KB of near working memory the EXE can never have; compute only, no Windows calls | **GO** (W16-44; the gate passed, D30 in rust-win16): `QUILLPIC.DLL`, from `quill-modpic`, the same crate as `QUILLPIC.DMD` |
| PDF writing (if Print to PDF comes to Windows 3.x) | Font metric tables and buffers would land in the EXE's segment as read-only data; in a DLL they have a segment of their own | **Later**, when a plan asks for PDF on Windows 3.x: `QUILLPDF.DLL`, an optional component |
| Spelling engine and dictionary | One dictionary loaded once for every instance; room for D88's paging; 10 `Store` entries back | **Later** (W16-47), when as-you-type or paging needs it |
| Preferences (Options) and the editor dialogs | A dialog procedure calls Windows, which a module may not do without W16-48's import stubs; their data cost (+100 to +340 bytes) fits after the W16-60 reliefs | **NO**: in the EXE, through `DialogBox` and resources |
| Reveal Codes pane | Per paint, GDI drawing, caret semantics in `App`; ≤ +72 bytes of data | **NO** |
| Unicode tables | Resources (`RCDATA`), Unicode plan U0 | **NO** |
| Help | `WinHelp` and `QUILL.HLP` | **NO** |
| Layout, paint, RTF, print, autosave, styles | Hot, or tied to the `Doc` and `App` | **NO** |

### 6.3 Loader rules, lifetimes and versions (W16-43, unchanged)

Loaded on the first picture that needs it, by full path with `SEM_NOOPENFILEERRORBOX`, entry points by ordinal through 4-byte slots `GetProcAddress` fills; the info call's `abi_major` and build id checked; freed 30 seconds after the decode queue empties, on `WM_COMPACTING` and always at exit by Quill itself (Windows 3.11 does not free it when the task ends); one session across instances (`BUSY` and a retry on the next paint); every failure the grey box and a logged reason. The DLL imports nothing (the packer refuses imports in library mode), so it never yields on its private stack.

## 7. Windows 95 and later, Linux, WinUI and the web

- **Windows 95 and later (W9X-40, revised).** *Superseded on 2026-11-27 by section 19.* The verdict here was one `QUILL32.EXE`, since a PE image is mapped and paged on first touch, so unused code costs nothing on an 8 MB machine, and a DLL adds a second no-CRT link, relocations, a version check and an import scan, and loses LTO across the boundary. Those costs are real and section 19 prices them (about 1.5 to 3 KB a module, section 19.2); the owner chose the split for consistency with DOS and Windows 3.x. The recipe is no longer waiting for the savers' `AGCORE1.DLL` (WP-SL-1 has not been built): section 19.2's spike proved it with Quill's own toolchain.
- **Linux, WinUI and the web.** No size limit applies; the logic crates link statically through each seam's `Static` implementation. The web demo's DOS page (`site/try/`, built by `scripts/build-try.sh`) ships the DOS build, so its bundle gains the `.DMD` files (WP-MOD-3), and `scripts/check-try.sh` checks they are there.

## 8. Which features go where

Savings are for `QUILLDOS.EXE`; Win16 figures are the data segment; the Windows 95 column's savings are `QUILL32.EXE`'s (section 19.2).

| Feature | DOS | Windows 3.x | Windows 95 and later | Saving (DOS EXE) | Order |
|---|---|---|---|---|---|
| **Picture decoders** (PNG; JPEG, RLE4 later) | `QUILLPIC.DMD`, required (`appdos`); plain BMP stays in the EXE | `QUILLPIC.DLL`, required (`app16`) | `QUILLPIC.DLL`, required (`app32`), BMP included: about −15 to −16 KB of `QUILL32.EXE` [E from M] | PNG −4.0 KB [M] against the loader's +3 to +5 KB once: about −1 to +1 KB now; JPEG's +10 to 16 KB never enters the EXE | **1st** (the proof) |
| **File dialog** (Open, Save As, Insert Picture, both DOS screens) | `QUILLDLG.DMD`, required | n/a (common dialogs) | n/a (common dialogs) | **−26 to −29 KB** [E from 31,274 movable, M], after +2 to +4 KB of dialog host in the EXE | **2nd** (the first real saving) |
| **Options** (Preferences WP-PREF-5, WP-PREF-6) | Joins `QUILLDLG.DMD` | In the EXE | Joins `QUILLDLG.DLL` (WP-MOD-12), its template a DLL resource | −7 to −12 KB of the ledger's +8 to +14 | 3rd, as the dialog lands |
| **Page Layout, Numbering, Header and Footer** (WP-ED-17 to WP-ED-19) | Join `QUILLDLG.DMD` | In the EXE | Join `QUILLDLG.DLL` | −2 to −4 KB each of their +3 to +5 | As they land |
| **Modal dialogs of today** (About, Fonts, Tabs) | Already `quill-dlg` widgets in the EXE | In the EXE | `QUILLDLG.DLL`, about −4 to −6 KB [E from M]; Find, Spelling and the save question stay | — | WP-MOD-12 |
| **Help viewer** (`QUILL.HLX`, roadmap H2) | `QUILLHLP.DMD` in a new DOS `help` component (`default = true`, optional) | n/a (WinHelp) | n/a (WinHelp reads `QUILL.HLP`; `QUILLHLP` stays a reserved kind) | Its whole size, about 10 to 20 KB [G], never in the EXE | When H2 makes `.HLX` |
| PDF writing | `QUILLPDF.DMD`, optional unticked component, if a plan asks for PDF on DOS | `QUILLPDF.DLL`, likewise | `QUILLPDF.DLL` in a ticked `pdf32` component (WP-MOD-10): the writer moves, the document walk stays; about −14 to −20 KB [E from 32,256 M] | All of it (34 KB on Win32 [V]) | Win32 2nd; DOS and Windows 3.x later |
| Spelling engine | Stays in the EXE: 2.8 KB movable against 9.4 KB that is `App`'s | `QUILLSP.DLL` later (W16-47) | Stays (3.4 KB, the dictionary is a file already) | About −2 KB net: not worth a module | Later or never |
| **Reveal Codes** | Stays in the EXE: per key and per paint, caret and undo in `App` | In the EXE | In the EXE | — (it spends the room the dialogs free) | — |
| **Unicode U1 and U2** | Stays in the EXE: the text buffers and every caller | In the EXE; tables as resources | In the EXE | — | — |
| Preferences model (WP-PREF-2, `quill-settings`) | Stays in the EXE: every build reads it at start | In the EXE | In the EXE | — | — |
| RTF, QLD, layout, paint, print, autosave | Stay | Stay | Stay | — | — |

## 9. The installer

- **Private files in `{install}` (INS-20).** Each module is an ordinary `{install}/NAME` file in the component that owns its feature: `QUILLPIC.DMD` and `QUILLDLG.DMD` in `appdos`, `QUILLPIC.DLL` in `app16`. The Liminal Installer records each as `F name size crc32` in `INSTALL.LIR` and removes it on uninstall like the program (installer D111: a private DLL needs nothing new [V]).
- **Never `{windows}` or `{system}`.** The new placeholders (installer D95, merged for Windows 3.1 in installer pull request 70 [V]) are for files the system must find, such as the Ghostwriter screen saver. A module there would be found by any copy of Quill loading by bare name, its global Win16 module name would clash across copies, and D95's back-up-and-restore rule is wrong for a shared library (shared libraries plan section 11.2, whose `system-folder-library` check warns about exactly this). Quill loads by full path from its own folder (CORE-95), so it never needs them.
- **Unticked components (INS-21).** Only a feature that is optional by nature becomes a component of its own, with the installer's `default = false` (supported since installer D90 [V]) when it is a rarely wanted extra: a DOS `help` component (`QUILLHLP.DMD`, `QUILL.HLX`) ticked by default; `pdf` unticked. Pictures and dialogs are core features and stay in the required components; the absent-module message (CORE-93) covers a hand-made install.
- **In use.** Windows 3.x cannot replace a loaded module; the retro setup already asks the person to close Quill, and `GetModuleHandle("QUILLPIC")` is the check to add when the first DLL ships (shared libraries plan section 6.5 [U: not in the runtime yet]). DOS has nothing loaded during install.
- **Staging and checks.** `installer/tools/stage.sh` copies the modules into `dist/dos-x86/` and `dist/win16-x86/`; `verify-disks.sh` compares them with the builds; `e2e-dos.sh` and `e2e-w31.sh` check each is installed, listed in `INSTALL.LIR` and removed by uninstall, and that Quill draws a PNG afterwards.
- **Disks.** DOS +3 to 6 KB packed per module; Windows 3.x, the DLL's headers and its runtime copy, 2 to 4 KB packed [E]; Windows 95 about 1 to 2 KB packed per module (headers, relocations, the runtime copy) [E from M].
- **Windows 95 and later (section 19.9).** `QUILLPIC.DLL` and, later, `QUILLDLG.DLL` are files of the required `app32`; `QUILLPDF.DLL` is the one file of a new `pdf32` component, ticked by default (INS-26). Never `{system}`: Windows 95's `LoadLibraryA` with a bare name looks in the program's folder, the current folder, the system folder and the Windows folder, so a copy there would be found by a hand-copied `QUILL32.EXE` anywhere; Quill loads by full path. No reference counts: the files are private. Where both Windows builds share a folder (the ReactOS demo's `C:\Quill`), each build gets its own folder, since both name their picture module `QUILLPIC.DLL` (INS-27).

## 10. The DOS budget policy and the ledger

### 10.1 The projection [E]

| | Without modules | With `QUILLDLG` and `QUILLPIC` |
|---|---:|---:|
| `QUILLDOS.EXE` today | 437.8 KB | 437.8 KB |
| Editor ledger (WP-ED-1 to WP-ED-19) | +31 to +48 | +19 to +29 (the Options and three editor dialogs in the module, their glue in the EXE) |
| Reveal Codes (WP-CODES-2 to 8) | +11 to +16.5 | +11 to +16.5 |
| WP-MOD-3 (`QUILLPIC`, the loader) | — | −1 to +1 |
| WP-MOD-4 (`QUILLDLG`, the file dialog) | — | −29 to −26 |
| **`QUILLDOS.EXE` at the end** | **about 480 to 502 KB** | **about 438 to 458 KB** |
| Modules (`QUILLPIC` about 8, `QUILLDLG` with the moved dialogs about 46 to 56) | — | about 54 to 64 KB |
| Program bytes, packed at about 58 % (EXE and modules) | about 281 to 294 KB | about 288 to 305 KB |

So the modules take the EXE back below where it was before the stop came into view. The price is the loader, the dialog host and each module's own runtime copy, about 12 to 20 KB raw and **7 to 11 KB on the floppy**, against about 427 KB free.

### 10.2 The revised rule (DOS-52, revising DOS-70)

1. **The 480 KB stop stays**, and is no longer expected to be reached.
2. **Module first.** A feature package whose code passes the EXE's budget checks whether its feature is module-ready and cold (section 4.3). If it is and its module exists, the code goes into the module and the EXE's budget does not move. If the module does not exist yet, the package raises the budget as DOS-70 says (savings first, the measured cost rounded up to 4 KB, a DOS row of its own), and names the module package that will take it out again; **no package waits for a module**.
3. **A move-out lowers nothing automatically.** The budget stays at its figure after a module takes code out, so the freed room is what the features that must stay in the EXE (Reveal Codes, Unicode U2, editor commands) spend without asking.
4. **Every module has its own budget** (section 5.9), and every package reports the three figures: EXE, modules, program bytes on the disk with the floppy's free bytes.
5. **The floppy floor stays** at 128 KB free, measured on program bytes (DOS-51).

## 11. The ARCH and ED lanes, Reveal Codes and Preferences

### 11.1 The lanes (ARCH-41)

- **Side lane.** WP-MOD-1, WP-MOD-2, WP-MOD-3, WP-MOD-6, WP-MOD-7 and WP-MOD-8 touch no file in `crates/quill-app/src`; they are side packages under ARCH-6, and WP-MOD-1, WP-MOD-2 and WP-MOD-8 may run during the Unicode freeze.
- **ARCH lane.** WP-MOD-4 moves the file dialog's painters and state behind a seam: it is a move ("a move changes no behaviour": every golden byte-equal), so it takes the ARCH lane's slot after the freeze, after WP-ARCH-7 `quill-flow` (the answering layer the dialog's outcome goes through).
- **ED lane.** Unchanged in order. Each new cold feature is written module-ready (CORE-92); WP-MOD-5 moves each finished dialog into `QUILLDLG.DMD` after it lands, in the side lane.

### 11.2 The splits

| Split | What modules need from it | Coordination |
|---|---|---|
| WP-ARCH-2 `quill-picture` (in flight) | The decoder seam lives here: `quill_picture::Decoders` with `Static` and the shells' module implementations | WP-MOD-3 branches after it merges and owns a new file in it (`modular.rs`) and named lines in `lib.rs` |
| WP-ARCH-3 `quill-session` | The idle cadence, which drives Win16's unload after 30 seconds and a module's `rows` slices | WP-MOD-6 adds one idle hook through the session's existing callback, no new API |
| WP-ARCH-5 `quill-menu` | The enable rule gains "needs module `kind`" | WP-MOD-3 adds the predicate after WP-ARCH-5 merges; before it, the seam answers at the command, with the same message |
| WP-ARCH-6 `quill-settings` | `[Debug] Modules=0` forces every module absent, for tests and diagnosis | One schema row, added by WP-MOD-3 through the schema file's usual path |
| WP-ARCH-7 `quill-flow` | The dialogs' answers come back as plain data and are applied by the flow | WP-MOD-4 waits for it |
| `scripts/check-layers.py` | `quill-modabi` and `quill-modpic` are base crates; crates not listed in `ORDER` are not ordered, so no edit is needed [V] | — |

### 11.3 Reveal Codes (pull request 161)

Reveal Codes stays in the EXE on every build: its view model runs per paint, the caret steps over codes and a code's deletion is an undo step in `App`. No package of it changes. What changes is its DOS budget rows: if `QUILLDLG` has landed before a WP-CODES package, the package spends the room the dialogs freed and needs no raise; if not, it raises as DOS-70 says. Its owner question 5 ("go past 480 KB?") becomes moot under DOS-52, and the default it took (yes, by the measured cost, as long as the floppy keeps 128 KB) still holds as the fallback. WP-CODES-4's graphics pane is not worth a module (+2.5 to +3.5 KB, drawn every paint).

### 11.4 Preferences (`preferences.md`)

- **WP-PREF-2** (settings by id) stays in the EXE: every build reads its schema at start.
- **WP-PREF-5 and WP-PREF-6** (the Options dialog on the DOS text and graphics screens) are asked to follow the module-ready rule: the dialog's state machine and painters in `quill-dlg` (or a new `quill-optdlg` kit crate), drawing through a surface trait, answering through the flow, its labels in the schema's table. They land **in the EXE first**, under DOS-70 as planned, and WP-MOD-5 moves them into `QUILLDLG.DMD`, which gives back 7 to 12 KB [E].
- **WP-PREF-3, WP-PREF-7 and WP-PREF-8** (Windows) are unchanged: the Windows 3.x Options dialog is a `DialogBox` in the EXE, and its +100 to +340 bytes fit after the W16-60 reliefs.
- Its question "if the DOS budget reaches the 480 KB stop?" is answered by DOS-52: module first, then a raise.

## 12. Risks

| Risk | Where | Containment |
|---|---|---|
| CWSDPMI refuses to widen CS, or the reload misbehaves | DOS | WP-MOD-2 tests it before anything depends on it; the `module-arena` fallback inside `.bss` (DOS-53) |
| DOSBox does not enforce the CS limit, so a missing widen passes in tests and faults on hardware | DOS | The widen is unconditional, not a reaction to a fault; DOSBox-X with a prefetch core or 86Box for the negative run where possible; a hardware run best effort |
| `--emit-relocs` misses a word after a toolchain bump | DOS | `dmdpack cross` (two bases, equal images) in every build; the packer refuses unknown relocation types [V] |
| The module and the EXE drift apart (ABI, table order, build id) | All | `quill-modabi` is the only definition; layout tests fix every size and offset; the build id must match in release builds |
| Rust runs on the caller's stack in a Win16 DLL (SS ≠ DS) | Win16 | Only the export thunk runs there; `hellodll` checks a stack buffer against a near copy (the gate) |
| A Win16 module calls Windows and yields on its private stack | Win16 | The packer refuses imports in library mode (W16-41) |
| DLL hell: a stale `QUILLPIC.DLL` elsewhere, or another copy's module already loaded | Win16 | Full-path loading (CORE-95), never `{system}` (INS-20), the build id, a message naming the other copy |
| Two copies of `core` and `compiler_builtins` (EXE and module) | DOS, Win16 | 1.5 to 3 KB a module, reported in program bytes; `-Zfmt-debug=none` and `-Zlocation-detail=none` in the modules too |
| Debugging across the boundary | All | The ELF kept beside each `.DMD`, the load address in `QUILL.LOG`; a `.MAP` per DLL and `ne_addr2sym.py`; `/FAULT` names the module |
| The test matrix grows with each module and platform | All | One "absent" and one "wrong build" scenario per module per platform, not per feature; the goldens run with modules present, so the screens prove "unchanged" |
| Start-up or first-use pause on a 386 | DOS, Win16 | Load on first use; the bench gate records the first-use cost (section 5.8) |
| A module panic takes the program down | DOS | The same as today, when the code is linked in; Desk's guard later (DOS-54) |
| The hosted loader diverges from Desk's own M5 when Desk builds it | DOS | The loader is offered upstream; `PROVENANCE.md` lists what Quill changed |
| Builders start moving features into modules that are hot | All | Section 8's table; CORE-92's "cold and narrow" test in each brief |
| The Win32 risks: a DLL linked with the program's `/FIXED`, a second allocator, the loader lock, Windows 9x reusing a loaded module by its base name, the two `QUILLPIC.DLL` files in one folder | Win32 | Section 19.10 |

## 13. Tests

| Level | What |
|---|---|
| Host | `quill-modabi` layout tests (`size_of`, `align_of`, `offset_of!` for every struct); the copied `desk-module` and `dmdpack` suites; `quill-modpic`'s session against the decoders' existing goldens and property tests (a PNG decoded through the session in slices of 1, 7 and all rows is byte-equal to `quill-image`'s own); the seam's `Static` and a `Missing` fake (the grey box and the message) |
| DOS, `rt-test` | `HELLO.DMD`: loaded at two addresses with equal output, a missing import refused by name, a bad CRC refused, 200 loads and frees with the heap's total unchanged, a panic through the host's path, the CS base check |
| DOS, Quill | Every `make dos` scenario byte-equal with `QUILLPIC.DMD` present; `m15-pictures` (or the picture golden) with the module absent shows the grey box and the message; a wrong-build module refused with the log line; the bench profile records the first-use load |
| Win16 | The rust-win16 gate (passed, D30), then the 2026-11-25 plan's gate 2: every PNG in `tests/docs` draws on Windows 3.11 at 256 and 16 colours and under otvdm; `m1-edit` byte-equal; the DLL missing, wrong build, two instances, 200 cycles |
| Installer | `e2e-dos.sh` and `e2e-w31.sh`: the module installed, listed in `INSTALL.LIR`, removed on uninstall; Quill draws a PNG after the install |
| Web demo | `scripts/check-try.sh` finds the `.DMD` files in the bundle |
| Win32 | Section 19.11: `MODHOST.EXE` under Wine (200 cycles, relocated, missing, wrong build, refused pointers), every Wine script with the modules present, the absent and wrong-build scenarios, `check-dll.sh`, the Pentium scan and the per-module import lists; `e2e-w95.sh` on Windows 95 (plain and Internet Explorer 4 bases); the ReactOS 0.4.16 guest; Windows 98 SE and Me by hand |

## 14. Decisions

**Ranges.** Above every id on `main` and on the open branches (pull requests 160, 161, 168, 169, 170), checked on 2026-11-27. W16-40 to W16-49, W9X-40, W9X-41, DOS-50 to DOS-52 and CORE-90 to CORE-91 are the ids the 2026-11-25 plan and the Liminal Installer's shared libraries plan already name for this work (none is used in the repository), kept so those documents stay right; the rest extend them. Reserved: **CORE-90 to CORE-99**, **DOS-50 to DOS-59**, **W16-40 to W16-49**, **W9X-40 to W9X-41**, **INS-20 to INS-22**, **ARCH-40 to ARCH-42**; for Windows 95 (section 19, `scripts/next-id.sh` on 2026-11-27) **W9X-130 to W9X-139** and **INS-26 to INS-28**, and CORE-97, CORE-98 and ARCH-42 from the spares above. INS-22 was taken by the packed-size gate (pull request 191, WP-MOD-8). Each package copies its rows into `docs/decisions.md` (Status **Assumed** unless the owner has answered) and the SPEC in the same change.

| Id | Decision | Rejected | Why |
|---|---|---|---|
| CORE-90 | `quill-modabi`: the one definition of every module interface: `#[repr(C)]` structs of `u16` and `u32`, status codes, kinds, `ABI_MAJOR` and `ABI_MINOR`, the start block, chunk lists, per kind the ordinals (Win16) and the entry table (DOS) | Rust types or traits across; one interface per platform | Two loaders, one contract; layout tests catch drift |
| CORE-91 | Module logic works in caller-given buffers (a work block, row buffers), not large locals | The decoders' `MAX_SIDE` locals | Win16 needs an arena; DOS and the static builds give stack or heap; the pictures plan's P11 fallback needs it too |
| CORE-92 | A cold feature is written module-ready: a crate below `quill-app`, plain data in and out, a state machine the shell drives, its own texts | Retrofitting each feature when it overflows | Costs nothing at the start; makes a later move a small package |
| CORE-93 | An absent or refused module greys its feature with a message naming the file and Setup, never a system box or a crash; the reason is logged | Refusing to start; a loader box | A damaged install keeps a working editor |
| CORE-94 | Versions: the EXE and modules share a build id, required equal in release builds; `abi_major` refused if different, `abi_minor` accepted if higher; no version digit in the file names | `NAME1.DLL` naming (shared libraries rule 3) | Private modules always ship as a set; the build id is stricter |
| CORE-95 | Modules are found beside the program only, by full path | The current folder, `PATH`, the Windows folders | A stale copy elsewhere can never win |
| CORE-96 | One module crate, three packagings (`.DMD`, NE DLL, Win32 DLL), linked statically through the seam's `Static` on Linux, WinUI, the web and in the host tests. *Revised 2026-11-27*: it said two packagings and named Win32 among the static builds | A module build on Linux, WinUI or the web | Nothing to relieve there; one code path for the tests; Win32 follows the owner's split (W9X-40) |
| CORE-97 | `quill-modabi` gains, with the Win32 packages: `WIN32_EXTENSION`, the two export names, `QS_NOMEMORY` (10, a loader's answer only, so an addition within major 1), and the PDF and Win32 dialog kinds' tables (`PdfTableV1`, `DlgTableV1`) when WP-MOD-10 and WP-MOD-12 define them, each with layout tests | A Win32 header of its own | One contract (CORE-90) |
| CORE-98 | DOS and Win32 share the flat-address glue: a module address is a flat 32-bit pointer on both, so the entries that turn a `PicTableV1` call into slices (`quill-modpic::flat`) and the host side that calls a `PicTableV1` (`quill_picture::modular::TableDecoders`) are written once; only finding the table differs (`rt::module` on DOS, `LoadLibraryA` and `QuillModuleStart` on Win32). Win16 keeps its far-pointer glue | Glue per platform | "Only the loader differs" |
| CORE-99 | Spare | | |
| DOS-50 | DOS modules are Desk `.DMD` files, with `desk-module`, `dmdpack` and `desk-guest` copied into `dos/module/` with provenance, and the hosted loader in `rt::module` | DXE3; classic overlays; a Quill-only format; ELF PIE | Desk's format is built, host-tested, `no_std` and pure Rust; one container across Liminal's DOS programs |
| DOS-51 | The DOS size gate also measures program bytes (the EXE and its required modules, packed) on the disk image, with the floppy's free bytes | Raw EXE bytes alone | The floppy is the physical limit |
| DOS-52 | DOS-70 revised: the 480 KB stop stays; a module-ready cold feature goes into its module instead of raising the budget once the module exists; a move-out does not lower the budget; each module has its own budget | One raise to 480 KB now; savings only | Ends the package-by-package negotiation without hiding bytes |
| DOS-53 | The loader widens CS to 4 GiB with DPMI 0008h after checking CS and DS share a base; the fallback is a module arena in `.bss` (`module-arena`) | Always the arena | No memory held when no module is used |
| DOS-54 | Modules load on first use and stay loaded until exit (`QUILLPIC` also unloads when pictures are dropped for memory); a module panic is the program's panic in version 1 | Unload when idle; Desk's guard now | Extended memory is plentiful; the guard is Desk's M7 and changes no format |
| DOS-55 | DOS modules: `QUILLPIC.DMD` (decoders other than plain BMP), `QUILLDLG.DMD` (the file dialog, Options and the editor dialogs); later `QUILLHLP.DMD` (help viewer) and `QUILLPDF.DMD` (PDF) | Spelling, Reveal Codes, Unicode, the graphics screen | Section 8: cold and narrow only |
| DOS-56 to DOS-59 | Spare | | |
| W16-40 | Helper DLLs on Windows 3.x for cold, compute-only modules whose bulk is working memory; code stays multi-segment | D14's rejection of DLLs, revised | The only way to more near memory |
| W16-41 | `ne_pack.py --library` and the `w16dll` runtime from rust-win16 (pull requests 5 and 6, D27 to D30 there), copied in per D89 by refreshing the vendored packer | Packing DLLs in Quill alone | One owner for the packer |
| W16-42 | The DLL runtime: a private stack and 32-bit alias in the DLL's moveable segment, switched in each export thunk; no `SwitchStackTo`; no Windows calls from a module; a panic poisons the module and returns `INTERNAL` | Running Rust on the caller's stack | SS ≠ DS breaks `code16` pointers |
| W16-43 | Loading on first use by full path with `SEM_NOOPENFILEERRORBOX`, ordinals through slots, the info check, freed 30 s after idle and on `WM_COMPACTING` | Implicit linking | A missing DLL would stop the EXE starting |
| W16-44 | `QUILLPIC.DLL` decodes PNG, then JPEG and RLE4, into the EXE's DIB blocks, from `quill-modpic`; one session across instances | The pictures plan's P11 (W16-19 there) and J3's code segment | PNG and JPEG at near speed |
| W16-45 | The Windows 3.x install set is `QUILL.EXE` and `QUILLPIC.DLL` in the required `app16` component | INS-2's "one program", amended | Pictures are core |
| W16-46 | Cold EXE segments (FILE, EDIT) load on call and are discardable, after a 30-run check | — | Memory on 4 MB machines, without a DLL |
| W16-47 | (Later) `QUILLSP.DLL` in `dicts` | — | When paging or as-you-type needs it |
| W16-48 | (Later) DLL import stubs that switch back to the caller's stack, for modules that must call Windows | — | Only if a dialog module is ever wanted on Windows 3.x |
| W16-49 | Windows 3.x modules: pictures now; PDF and spelling later; dialogs, Reveal Codes, Unicode, help and the editor stay in the EXE | DLLs to relieve `QUILL.EXE`'s own segment | Section 6.2 |
| W9X-40 | *Revised by the owner, 2026-11-27.* Windows 95 and later split like DOS and Windows 3.x: `QUILLPIC.DLL` (decoders), `QUILLPDF.DLL` (the PDF writer) and later `QUILLDLG.DLL` (modal dialogs) beside `QUILL32.EXE`, from the shared module crates over `quill-modabi`; help stays WinHelp. It said: one `QUILL32.EXE`, no DLL until an optional feature passes 100 KB | One `QUILL32.EXE` (the earlier verdict); codec or engine DLLs outside the module contract | The owner wants the app split in line with its siblings; the costs are measured (section 19.2) |
| W9X-41 | *Revised 2026-11-27.* The Win32 module recipe is Quill's own, proved by this plan's spike (section 19.2) and recorded in section 19.3, and offered to the shared libraries plan's section 9 for the savers. It said: taken from the savers' `AGCORE1.DLL` | Waiting for WP-SL-1 (not built) | Proved with the toolchain Quill already has |
| W9X-130 to W9X-138 | The Windows 95 module design | | Section 19.12 |
| INS-20 | Modules are private `{install}` files in the component that owns the feature; never `{windows}` or `{system}` | The system folder | Global module names, stale copies, D95's rule is for owned files |
| INS-21 | A module gets a component of its own only for an optional-by-nature feature (help ticked, PDF unticked with `default = false`) | Every module optional | Core features must not be missing |
| INS-22 | Taken by the packed-size gate (pull request 191) | | |
| INS-26 to INS-28 | The Windows 95 installer rows | | Section 19.12 |
| ARCH-40 | The module seam is a trait in the crate that owns the feature, with `Static` and module implementations; shells choose | A module registry in `App` | `App` stays unaware of packaging |
| ARCH-41 | Module packages are side-lane unless they move code out of `crates/quill-app/src`, which makes them ARCH-lane moves | A third lane | ARCH-6's rules hold unchanged |
| ARCH-42 | The PDF seam: `quill_pdfdoc::Writer`, a trait the document walk writes pages through, with `Static` over `quill-pdf` and a module implementation in the Win32 shell (WP-MOD-10) | The walk in the module (it needs the `Doc`) | ARCH-40's rule for the PDF kind |

## 15. Work packages

### 15.1 Order

| Order | Package | Lane | Needs | Size | Parallel with |
|---:|---|---|---|---|---|
| 1 | **WP-MOD-1** `quill-modabi` and the `.DMD` toolchain, host only | Side | — | M | Anything, the freeze included |
| 1 | **WP-MOD-8** The DOS packed-size gate and the module rows in `size-report.sh` | Side | — | S | Anything |
| 2 | **WP-MOD-2** The DOS loader in `rt` and `HELLO.DMD` in DOSBox | Side | WP-MOD-1 | M | Anything |
| 3 | **WP-MOD-3** `QUILLPIC.DMD`: PNG out of `QUILLDOS.EXE`, with the installer (**the proof**) | Side | WP-MOD-2, WP-ARCH-2 | M to L | ED and ARCH lanes (no `quill-app/src` files) |
| 4 | **WP-MOD-6** `QUILLPIC.DLL` on Windows 3.x | Side | WP-MOD-3; the rust-win16 runtime on its `main` (the gate passed, D30) | L | WP-MOD-4 |
| 5 | **WP-MOD-4** `QUILLDLG.DMD`: the dialog host and the file dialog (**first real saving**) | ARCH | WP-MOD-3, WP-ARCH-7, the freeze ended | L | WP-MOD-6 |
| 6 | **WP-MOD-5** Options and the editor dialogs into `QUILLDLG.DMD` | Side | WP-MOD-4, and WP-PREF-5, WP-PREF-6, WP-ED-17 to WP-ED-19 as each lands | M per dialog | ED lane |
| 7 | **WP-MOD-7** JPEG and RLE4 into `QUILLPIC` on DOS and Windows 3.x | Side | WP-MOD-3, the pictures plan's J1 and J2; WP-MOD-6 for the Win16 half | M | Anything |
| — | Later, each with a plan of its own: `QUILLHLP.DMD` (after H2), `QUILLSP.DLL` (W16-47), `QUILLPDF` (if PDF on the retro builds), W16-46 cold segments | — | — | — | — |
| 1 | **WP-MOD-11** The Windows 95 rows in the packed-size gate: `QUILL32.EXE`'s budget, DLL rows by target (section 19.13) | Side | Pull request 191 merged | S | Anything |
| 2 | **WP-MOD-9**, pull request 1: the Win32 module toolchain, runtime crate, loader and `MODHOST.EXE` under Wine | Side | — | M | Anything, the freeze included |
| 4 | **WP-MOD-9**, pull request 2: `QUILLPIC.DLL` on Windows 95 | Side | WP-MOD-3 (`quill-modpic`, the seam), WP-MOD-9 PR 1, WP-MOD-11 | M | WP-MOD-6, WP-MOD-4 |
| 5 | **WP-MOD-10** `QUILLPDF.DLL` and the `pdf32` component | Side | WP-MOD-9 | M to L | ED lane |
| 6 | **WP-MOD-13** The Windows 95 module set in the ReactOS 0.4.16 guest and on Windows 98 SE and Me | Side | WP-MOD-10 | S | Anything |
| 8 | **WP-MOD-12** `QUILLDLG.DLL` on Windows 95 | Side | WP-MOD-10, WP-PREF-8 (Options on Windows 95) | M | ED lane |

At most three builders at once: WP-MOD-1 and WP-MOD-8 now beside the two lanes' builders leaves room for one; WP-MOD-2 follows WP-MOD-1.

### 15.2 The packages

#### WP-MOD-1 — `quill-modabi` and the `.DMD` toolchain

- **Finish line.** `crates/quill-modabi` exists with the interface of section 4.2 and its layout tests; Desk's `desk-module`, `dmdpack` and `desk-guest` are in `dos/module/` with provenance, built and tested on the host by `dos/check.sh`; `dos/mods/module.ld` and `link.rs` are in place; no executable changes.
- **Owns.** `crates/quill-modabi/**` (new), `dos/module/**` (new: `Cargo.toml`, `.cargo/config.toml`, `format/`, `pack/`, `guest/`, `rustfmt.toml`, `clippy.toml`), `dos/mods/module.ld`, `dos/mods/link.rs` (new), `dos/PROVENANCE.md` (new rows), `dos/check.sh` (one line), the root `Cargo.toml` (one `members` entry), `Cargo.lock` (regenerate).
- **Must not touch.** `crates/quill-app/**`, `crates/quill-paint/**`, `crates/quill-picture/**`, `dos/quill-dos/**`, `dos/rt/**`, `win16/**`, `win9x/**`, `linux/**`, `winui/**`, `installer/**`.
- **Tests.** The copied suites pass unchanged (`cd dos/module && cargo test`); `quill-modabi`'s layout tests; G-HOST; `dos/check.sh` format and clippy.
- **Budgets.** `QUILLDOS.EXE`, `QUILL.EXE` and `QUILL32.EXE` byte-identical (SHA-256 before and after).
- **Decisions.** CORE-90, DOS-50.

#### WP-MOD-8 — the DOS packed-size gate

- **Finish line.** `installer/tools/build-disks.sh` (or `verify-disks.sh`) prints the packed bytes of `QUILLDOS.EXE` and of every `*.DMD` staged for `dos-x86`, their sum (program bytes) and the DOS floppy's free bytes, and fails below 128 KB free; `dos/tools/size-report.sh` lists each `dos/dist/*.DMD` against its budget (none yet: the section prints "no modules").
- **Owns.** `installer/tools/verify-disks.sh`, `dos/tools/size-report.sh`. **Must not touch** everything else.
- **Tests.** G-DISK; `size-report.sh` on a build. **Budgets.** No executable changes. **Decisions.** DOS-51.

#### WP-MOD-2 — the DOS loader and `HELLO.DMD`

- **Finish line.** `rt::module` loads a `.DMD` from a path into the heap, binds Desk's four services, widens CS after the base check, starts it and returns the table; `dos/rt-test` gains a module scenario that passes under headless DOSBox Staging; `dos/mods/hello` builds and packs, with `dmdpack cross`; `QUILLDOS.EXE` unchanged.
- **Owns.** `dos/rt/src/module.rs` (new), `dos/rt/src/lib.rs` (one line), `dos/rt/Cargo.toml` (the `desk-module` path), `dos/rt-test/**` (the module scenario), `dos/tools/rt-test.sh`, `dos/mods/Cargo.toml`, `dos/mods/.cargo/config.toml`, `dos/mods/hello/**` (new), `dos/tools/build-mods.sh` (new), `dos/build.sh` (one line calling it).
- **Must not touch.** `dos/quill-dos/**`, `crates/**`, `win16/**`, `win9x/**`, `installer/**`.
- **Tests.** The `rt-test` module scenario (two addresses, a missing import by name, a bad CRC, 200 loads with the heap unchanged, a panic through the host's path, the CS base check); a run with `module-arena` too.
- **Budgets.** `QUILLDOS.EXE` byte-identical (the loader is not referenced by `quill-dos` yet).
- **Decisions.** DOS-53, DOS-54.

#### WP-MOD-3 — `QUILLPIC.DMD` (the proof)

- **Finish line.** On the DOS graphics screen a PNG is decoded by `QUILLPIC.DMD` through `quill_picture::Decoders`; every DOS golden is byte-equal with the module present; with it absent a PNG shows the grey box and the CORE-93 message, and plain BMP still draws; a wrong-build module is refused with a log line; the installer stages, installs and removes the module; the web demo's bundle carries it.
- **Owns.** `crates/quill-modpic/**` (new), `crates/quill-picture/src/modular.rs` (new) and its `lib.rs` lines, `dos/mods/quillpic/**` (new), `dos/quill-dos/src/modules.rs` (new), `dos/quill-dos/src/main.rs` (named lines: building the decoder set), `dos/quill-dos/Cargo.toml`, `installer/components.toml` (the `appdos` files line), `installer/tools/stage.sh`, `installer/tools/e2e-dos.sh` (the module checks), `scripts/build-try.sh`, `scripts/check-try.sh`, `dos/tools/e2e.sh` (the absent and wrong-build scenarios), `SPEC.md` (DOS pictures, the file list), decision rows.
- **Must not touch.** `crates/quill-app/src/**` (the seam is in `quill-picture`), `crates/quill-image/src/png.rs` (apart from CORE-91's buffer change if it is not in yet), `win16/**`, `win9x/**`, `linux/**`, `winui/**`.
- **Tests.** G-HOST (the session against `quill-image`'s goldens; the `Missing` fake), G-DOS, G-DISK, `installer/tools/e2e-dos.sh`, `scripts/check-try.sh`; the first-use load timed under DOSBox's 386 profile.
- **Budgets.** `QUILLDOS.EXE` within −2 and +2 KB of the branch point (stop and report past +2); `QUILLPIC.DMD` ≤ 16 KB; program bytes and the floppy reported.
- **Decisions.** CORE-91 to CORE-96, DOS-55, INS-20, ARCH-40.

#### WP-MOD-6 — `QUILLPIC.DLL` on Windows 3.x

- **Finish line.** The 2026-11-25 plan's PRs 4 to 6 in one or two pull requests: the vendored packer refreshed and rust-win16's `w16dll` runtime copied in per `PROVENANCE.md`; `win16/build.sh` builds `QUILLPIC.DLL` from `quill-modpic` (`win16/examples/quillpic/` with `segments.json` and `exports.json`); the loader in `picture.rs` behind `quill_picture::Decoders`, with `FreeLibrary` at exit; PNG drawn on Windows 3.11; `app16` lists the DLL.
- **Owns.** `win16/examples/quillpic/**`, `win16/examples/quill/picture.rs`, `win16/tools/` (the copied packer and `check_dll.py`), `win16/build.sh`, `win16/PROVENANCE.md`, `installer/components.toml` (the `app16` line), `installer/tools/stage.sh`, `installer/tools/e2e-w31.sh`.
- **Must not touch.** `crates/quill-app/src/**`, `dos/**`, `win9x/**`.
- **Tests.** The 2026-11-25 plan's gate 2; G-W16; `e2e-w31.sh`.
- **Budgets.** `QUILL.EXE` data segment +60 to +120 bytes at most, with 1,024 free (W16-60); the DLL's data, private stack and arena reported; stack margin ≥ 2,048.
- **Decisions.** W16-40 to W16-45, W16-49.

#### WP-MOD-4 — `QUILLDLG.DMD` and the file dialog

- **Finish line.** The DOS file dialog's state and painters run from `QUILLDLG.DMD` on both DOS screens through the SDK 2 dialog host; every golden byte-equal (a move); with the module absent, Open and Save As fall back to the name field with the CORE-93 message.
- **Owns.** `crates/quill-moddlg/**` (new: the module side), `crates/quill-dlg/src/host.rs` (new: the surface and file-system traits the module uses), the moved `quill_paint::filedlg` and `quill_app::paint::filepaint` (to `quill-dlg` or `quill-moddlg`), `crates/quill-app/src` lines for `Mode::File` (named), `dos/mods/quilldlg/**`, `dos/quill-dos/src/modules.rs`, `dos/quill-dos/src/dialog_host.rs` (new), `installer/components.toml`, `stage.sh`.
- **Must not touch.** Windows shells; the page painter; `quill-core`.
- **Tests.** G-HOST (the dialog's host goldens through `Static`), G-DOS (every file dialog scenario, `m10-*`), the absent scenario, G-DISK, `e2e-dos.sh`.
- **Budgets.** `QUILLDOS.EXE` −26 to −29 KB [E]; `QUILLDLG.DMD` ≤ 64 KB.
- **Decisions.** The SDK 2 table recorded in DOS-50's sub-decision DOS-50.1; DOS-55.

#### WP-MOD-5 — the other dialogs into `QUILLDLG.DMD`

- **Finish line.** One pull request per dialog as each lands in the EXE (Options from WP-PREF-5 and WP-PREF-6, then Page Layout, Numbering, Header and Footer): the dialog moves into the module, goldens byte-equal, its EXE bytes reported going down.
- **Owns.** The dialog's crate files, `crates/quill-moddlg/**`, `dos/mods/quilldlg/**`. **Must not touch** the Windows builds.
- **Budgets.** `QUILLDOS.EXE` −2 to −12 KB per dialog [E]; `QUILLDLG.DMD` ≤ 64 KB.

#### WP-MOD-7 — JPEG and RLE4 in `QUILLPIC`

- **Finish line.** The pictures plan's J2 decoder is called through `quill-modpic` on DOS and Windows 3.x (replacing J3's code segment), RLE4 and the OS/2 BMP header on Windows 3.x.
- **Budgets.** `QUILLDOS.EXE` ≤ +1 KB; `QUILLPIC.DMD` ≤ 40 KB; the DLL's arena unchanged.

## 16. Briefs

Every brief starts with [the common brief](../work-packages.md#the-common-brief) of work-packages.md, with two changes for this track: decision ids come from this page's reserved ranges (section 14), and evidence goes to `~/Documents/Liminal HQ/Quill/Modules/<WP-ID>/`. Paste the common brief, then the package's own.

### WP-MOD-1 (full)

```text
Package: WP-MOD-1 — quill-modabi and the .DMD toolchain (host only). Branch feat/module-abi, worktree
~/source/worktrees/quill/mod-1. Read docs/plans/modules.md sections 4, 5.1, 5.2, 14 and 15.2 first.

Goal: the shared module interface and Desk's .DMD format, packer and guest crate in Quill, built and tested on
the host, with no executable changing. Nothing loads a module yet.

1. Measure first: sha256sum dos/dist/QUILLDOS.EXE, win16/out/QUILL.EXE and win9x/dist/QUILL32.EXE from
   `bash dos/build.sh`, `bash win16/build.sh` and `bash win9x/build.sh` at the branch point (if a build tool is
   missing, say so and compare the ELF and the cargo metadata instead). Write them to the evidence folder.

2. Copy from liminal-hq/desk (git -C ~/source/liminal-hq/desk fetch origin; use origin/main, record the commit):
     module/format  -> dos/module/format   (crate desk-module)
     module/pack    -> dos/module/pack     (crate dmdpack, binary dmdpack)
     module/guest   -> dos/module/guest    (crate desk-guest)
     module/mods/module.ld, module/mods/link.rs -> dos/mods/module.ld, dos/mods/link.rs
   Make dos/module its own host workspace, as dos/hal is: Cargo.toml with the three members and Quill's
   [workspace.package]; .cargo/config.toml setting the host target and an empty build-std (copy dos/hal's and
   its comment); rustfmt.toml with hard tabs at 130 columns; clippy.toml keeping Desk's compare-and-swap ban.
   Reformat with cargo fmt (hard tabs) and change nothing else in the sources. Every copied file keeps its
   header. Add [package.metadata.quill] to each manifest the way dos/hal's crates have it (desk-module and
   desk-guest: layer "base", no_std true; dmdpack: layer "tool").
   Add a section to dos/PROVENANCE.md: source repository and commit, the licence, a table of what was copied
   and what changed (the formatting, the workspace, the metadata tables), and "Refreshing from Desk keeps them".

3. Wire the tests: in dos/check.sh, after the hal line, add
     (cd module && cargo fmt --all -- --check && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked)
   All of Desk's tests must pass unchanged. If one depends on a Desk path (module/pack/tests/real.rs builds a
   real module with Desk's kernel target), point it at ../../dos-elf.json and dos/mods/module.ld; if that is
   more than a path change, mark that one test #[ignore] with a reason and list it in the PR as not verified.

4. Create crates/quill-modabi (base layer; #![no_std]; no alloc; no floats; [package.metadata.quill] layer
   "base", no_std true, alloc false, float false; SPDX headers; add it to the root Cargo.toml members). It holds
   exactly the interface of modules.md section 4.2:
   - ABI_MAJOR = 1, ABI_MINOR = 0; KIND_PIC = 1, KIND_DLG = 2, KIND_HLP = 3, KIND_PDF = 4 (3 and 4 documented
     as reserved);
   - Status (u16) constants OK 0, MORE 1, BUSY 2, UNSUPPORTED 3, CORRUPT 4, TOO_LARGE 5, NOALIAS 6, INTERNAL 7,
     MISSING 8, VERSION 9;
   - #[repr(C)] ModInfo, ModStart (size u16, abi_major u16, abi_minor u16, kind u16, build_id u32, table u32,
     table_len u32), Chunk, ChunkList (count u16, pad u16, chunks u32), PicOpen (kind u16, flags u16, width u32,
     height u32, orientation u16, depth u16), PicRows (dst u32, stride u32, shown_width u16, rows u16, done u16,
     pad u16, palette u32);
   - the Win16 ordinals PIC_WEP 1, PIC_INFO 2, PIC_OPEN 3, PIC_ROWS 4, PIC_CLOSE 5;
   - the DOS table PicTableV1 { size: u32, info, open, rows, close } with each entry an
     Option<unsafe extern "C" fn(...) -> u16> of the signatures in section 4.2 (u32 addresses, not pointers);
   - a doc comment on every item saying what crosses and who owns the memory (the caller, always).
   Tests: size_of, align_of and core::mem::offset_of! of every struct and field, so a change to the layout fails
   the build; Status values distinct; ModInfo::size equals size_of::<ModInfo>().

5. Docs: docs/decisions.md gains CORE-90 and DOS-50 (Assumed; the text from modules.md section 14); SPEC.md's
   component list names quill-modabi and dos/module in one line each; docs/progress.md records WP-MOD-1.

Gates: G-HOST (make validate), `bash dos/check.sh` (its format, clippy and module tests at least; the DOSBox
parts if the tools are present), bash scripts/check-headers.sh, G-MD. Budgets: the three executables
byte-identical (sha256 before and after, in the PR). Decision ids: CORE-90, DOS-50 only.

Must not touch: crates/quill-app/**, crates/quill-paint/**, crates/quill-picture/**, crates/quill-image/**,
dos/quill-dos/**, dos/rt/**, win16/**, win9x/**, linux/**, winui/**, installer/**, site/**.

If Desk's crates do not build under Quill's toolchain (rust-toolchain.toml) without source changes, stop and
report the error rather than editing them; the fallback (OWNER DECISION) is to pin Desk's toolchain for
dos/module only.

Report: the PR URL, the Desk commit copied, the test counts, the three sha256 pairs, anything ignored and why.
```

### WP-MOD-8

```text
Package: WP-MOD-8 — the DOS packed-size gate (DOS-51). Branch feat/dos-program-bytes, worktree
~/source/worktrees/quill/mod-8. Read modules.md sections 5.9 and 10.
Change installer/tools/verify-disks.sh to print, for the DOS disk: the packed bytes of QUILLDOS.EXE and of each
staged *.DMD (none yet), their sum as "program bytes", and the floppy's free bytes, failing when the floppy
keeps less than 131,072 bytes free. Change dos/tools/size-report.sh to list each dos/dist/*.DMD against a budget
table at the top of the script (QUILLPIC.DMD 16,384; QUILLDLG.DMD 65,536), printing "no modules" when there are
none. Gates: G-DISK, size-report.sh on a build, check-headers, G-MD. No executable changes. Decision: DOS-51.
Must not touch anything but those two scripts, docs/decisions.md and docs/progress.md.
```

### WP-MOD-2

```text
Package: WP-MOD-2 — the DOS loader in rt and HELLO.DMD. Branch feat/dos-module-loader, worktree
~/source/worktrees/quill/mod-2. Needs WP-MOD-1 merged. Read modules.md section 5 and Desk's docs/LOADER.md
("Address-space assumptions", "Containing a crash") and LOADER-PLAN.md M5.
1. dos/mods: its own workspace (target ../dos-elf.json via .cargo/config.toml, build-std core, alloc,
   compiler_builtins with compiler-builtins-mem, rustflags -nostdlib --emit-relocs -zmax-page-size=0x1000,
   -Zlocation-detail=none -Zfmt-debug=none, opt-level "z", panic abort, --remap-path-prefix as Desk's script
   does). dos/mods/hello: a module whose app_start logs a line through desk_log, fills a 16-byte table, and has
   a second entry that panics. dos/tools/build-mods.sh builds every module, packs it with dmdpack into
   dos/dist/<NAME>.DMD, keeps the ELF in the target folder, and runs `dmdpack cross` against a second link at
   0x800000; dos/build.sh calls it after the EXE.
2. dos/rt/src/module.rs: load(path, services) reads the file, calls desk_module::load_into into a heap block of
   memory size at the header's alignment, binds desk_log, desk_panic, desk_alloc and desk_free to rt's log,
   panic and heap, checks DPMI 0006h gives CS and DS the same base, widens CS with 0008h and reloads it with a
   far return (once per run), calls app_start(1, &start, size) and returns the entry addresses; free() frees
   the block. Every refusal is a status with the desk-module error code. A cargo feature module-arena places
   modules in a 64 KiB .bss arena instead of the heap and skips the widening.
3. dos/rt-test: a module scenario run by dos/tools/rt-test.sh under headless DOSBox Staging: HELLO.DMD loaded,
   its line printed; loaded twice at different addresses with equal output; a copy with one flipped byte
   refused (CRC); a module importing an unknown name refused with that name; 200 loads and frees with
   rt::heap's total unchanged after the first; the panic entry printing through rt's panic path (last, since
   it ends the program); the same run built with module-arena.
4. If DOSBox shows no fault when the widening is skipped, record "CS limit not enforced by DOSBox" in the PR
   as not verified; do not add a test that depends on it.
Gates: G-DOS (make dos), check-headers, G-MD. Budgets: QUILLDOS.EXE byte-identical (sha256). Decisions:
DOS-53, DOS-54. Must not touch: dos/quill-dos/**, crates/**, win16/**, win9x/**, installer/**.
Report as the common brief says, plus the load time per module in the rt-test log and HELLO.DMD's size.
```

### WP-MOD-3

```text
Package: WP-MOD-3 — QUILLPIC.DMD, the proof. Branch feat/dos-quillpic, worktree ~/source/worktrees/quill/mod-3.
Needs WP-MOD-2 and WP-ARCH-2 (quill-picture) merged. Read modules.md sections 4, 5, 8, 9 and 13.
1. crates/quill-modpic (base; no_std; alloc false unless the session needs it; float as quill-image): a
   Session over quill-image's PNG decoder, the dither and the scaler, working in caller-given buffers (CORE-91;
   if quill-image still decodes into large locals, change PNG to take a work block first, in its own commit,
   goldens unchanged): open(chunks, kind) -> PicOpen, rows(dst, stride, shown_width, n) -> MORE or OK,
   close(). Host tests: the session's rows in slices of 1, 7 and all are byte-equal to quill-image's own
   output on every PNG in tests/docs.
2. crates/quill-picture/src/modular.rs: the Decoders trait (decode a picture of a kind into rows; available(kind)
   -> Status), a Static implementation over quill-modpic, and a Missing fake for tests. The picture path uses
   Decoders for every kind but plain BMP, which stays direct. With Missing, a PNG shows the grey box and the
   message of modules.md section 4.4, and the status line says it once a session.
3. dos/mods/quillpic: app_start checks ModStart's abi_major and fills a PicTableV1; the entries wrap
   quill-modpic, turning u32 addresses into slices with the flat-segment rule. dos/quill-dos/src/modules.rs: a
   Decoders implementation that loads QUILLPIC.DMD from the program's folder on the first PNG, checks ModInfo's
   abi_major and build id (equal in release builds; QUILL_BUILD_ID from dos/build.sh, which passes the same
   value to build-mods.sh), and answers MISSING or VERSION with a QUILL.LOG line when [Debug] Trace=1.
   [Debug] Modules=0 forces every module absent.
4. dos/tools/e2e.sh: every scenario with the module present; the picture scenario with it absent (its own
   golden) and with a module from another build (refused, the same golden as absent).
5. Installer: QUILLPIC.DMD in appdos's files (installer/components.toml), stage.sh copies it, verify-disks.sh
   compares it, e2e-dos.sh checks it installed, listed in INSTALL.LIR and removed by SETUP /U, and that Quill
   draws the sample PNG afterwards. Web demo: scripts/build-try.sh puts it in the bundle beside QUILLDOS.EXE
   and scripts/check-try.sh checks it.
6. Time the first load under DOSBox with the 386 cycles profile and write the figure in the PR.
Gates: G-HOST, G-DOS, G-DISK, installer e2e-dos, scripts/check-try.sh, G-MD. Budgets: QUILLDOS.EXE within -2 and
+2 KB of the branch point (stop and report past +2 KB), QUILLPIC.DMD <= 16 KB, program bytes and floppy free
bytes reported (WP-MOD-8's figures if merged). Decisions: CORE-91 to CORE-96, DOS-55, INS-20, ARCH-40.
Must not touch: crates/quill-app/src/** (if the seam needs an App line, stop and report), win16/**, win9x/**,
linux/**, winui/**.
```

### WP-MOD-6

```text
Package: WP-MOD-6 — QUILLPIC.DLL on Windows 3.x. Branch feat/w16-quillpic, worktree
~/source/worktrees/quill/mod-6. Needs WP-MOD-3 merged. The rust-win16 DLL runtime passed its go/no-go (D28 to D30: 30 of 30 on
Windows 3.11, 3 of 3 under Wine; otvdm not yet run, so run 10 under otvdm here if the image is at hand). Read
modules.md section 6 and ~/Documents/Liminal HQ/Quill/Quill modules and DLLs plan.md sections 3.3 to 3.6.
Refresh the vendored packer in win16/tools from rust-win16 (library mode, DYN rows, the new KERNEL rows)
and copy the w16dll runtime per win16/PROVENANCE.md (D89), from rust-win16 main or, if it has not landed
there, from feat/win16-dll at a recorded commit; build win16/examples/quillpic
(segments.json, exports.json with WEP 1 and the PIC ordinals of quill-modabi) from quill-modpic with the arena the
runtime leaves (about 57 KB); add the dynamic stub kind rows (LoadLibrary 95, FreeLibrary 96, GetProcAddress 50, checked with
--check-spec); win16/examples/quill/picture.rs implements quill_picture::Decoders over the DLL with the loader
rules of W16-43 and the one-session rule, and calls FreeLibrary at exit itself (Windows 3.11 does not
unload it when the task ends; check GetModuleUsage after Quill exits). PNG draws on Windows 3.11; the DLL missing, from another build, two
instances and 200 load cycles behave as modules.md says. Installer: app16 lists QUILLPIC.DLL; e2e-w31.sh checks
it. Gates: G-W16 plus the 2026-11-25 plan's gate 2, installer e2e-w31, G-MD. Budgets: QUILL.EXE data segment at
most +120 bytes with 1,024 free (W16-60), stack margin >= 2,048, the DLL's data, private stack and arena
reported. Decisions: W16-40 to W16-45, W16-49. Must not touch: crates/quill-app/src/**, dos/**, win9x/**.
```

### WP-MOD-4

```text
Package: WP-MOD-4 — QUILLDLG.DMD and the file dialog (an ARCH-lane move). Branch refactor/dos-quilldlg, worktree
~/source/worktrees/quill/mod-4. Needs WP-MOD-3, WP-ARCH-7 (quill-flow) merged and the Unicode freeze ended.
Read modules.md sections 4.3, 5.6, 8 and 10, docs/decisions.md DOS-32 to DOS-39.
Define the SDK 2 dialog host (DOS-50.1): text rows presented from a grid the module writes; graphics fill,
bevel, strike text, icon and cursor rectangle over the shadow; quill-dlg::fs's calls; settings by id; the
clock. Move the file dialog's state (quill-dlg) and painters (quill_paint::filedlg, quill_app::paint::filepaint)
behind traits in crates/quill-dlg/src/host.rs so they compile into dos/mods/quilldlg and, through Static, into
the host tests; dos/quill-dos/src/dialog_host.rs serves the table. With QUILLDLG.DMD absent, Open and Save As
fall back to the name field with the CORE-93 message. A move: every golden byte-equal (git diff tests/golden
empty), m10-* and the graphics dialog scenarios included. Gates: G-HOST, G-DOS, G-DISK, e2e-dos. Budgets:
QUILLDOS.EXE -26 to -29 KB expected (report the figure), QUILLDLG.DMD <= 64 KB. Decisions: DOS-50.1, DOS-55.
Must not touch: the Windows shells, the page painter, quill-core.
```

### WP-MOD-5 and WP-MOD-7

```text
WP-MOD-5 — one dialog into QUILLDLG.DMD. Branch refactor/dos-dlg-<name>. Needs WP-MOD-4 and the dialog merged
in the EXE (WP-PREF-5/6, WP-ED-17, 18 or 19). Move the dialog's state machine and painters into the module
through the SDK 2 host, answering through quill-flow; goldens byte-equal; the absent path greys the menu item
with the CORE-93 message. Budgets: QUILLDOS.EXE down by the dialog's movable bytes (report), QUILLDLG.DMD
<= 64 KB. Gates: G-HOST, G-DOS, G-DISK.

WP-MOD-7 — JPEG and RLE4 in QUILLPIC. Branch feat/quillpic-jpeg. Needs WP-MOD-3 and the pictures plan's J2
(and WP-MOD-6 for the Windows 3.x half). The J2 decoder is called through quill-modpic's session on DOS and
Windows 3.x instead of J3's code segment; RLE4 and the OS/2 header on Windows 3.x. Budgets: QUILLDOS.EXE
<= +1 KB, QUILLPIC.DMD <= 40 KB, the DLL's arena unchanged. Gates: G-HOST, G-DOS, G-W16 with the JPEG corpus at
16 and 256 colours.
```

## 17. Owner decisions

| # | Question | Default taken | Where to swap |
|---:|---|---|---|
| 1 | DOS modules at all, reversing the 2026-11-25 plan's "no dynamic modules on DOS"? | **Yes**, as `.DMD` (DOS-50) | DOS-50; drop WP-MOD-2 to WP-MOD-5 and keep DOS-70's raises |
| 2 | Desk's `.DMD`, copied with provenance, or a Quill format of its own? | **Desk's** | DOS-50; `dos/module/` |
| 3 | The proof: `QUILLPIC.DMD` (narrow, small saving) or the file dialog first (big saving, wide interface)? | **`QUILLPIC` first** | Swap WP-MOD-3 and WP-MOD-4 in section 15.1 |
| 4 | DOS-70: keep the 480 KB stop, module first, no automatic lowering after a move-out? | **Yes** (DOS-52) | DOS-52 and `dos/tools/size-report.sh` |
| 5 | Modules in the required components (`appdos`, `app16`), not optional? | **Required** for pictures and dialogs; optional only for help and PDF | INS-21; `installer/components.toml` |
| 6 | The Options dialog on DOS lands in the EXE first and moves later, or waits for `QUILLDLG`? | **Lands first** (no package waits) | Section 11.4; WP-PREF-5's brief |
| 7 | Reveal Codes stays in the EXE on every build? | **Yes** | Section 11.3; a codes module is not recommended |
| 8 | A module panic on DOS ends the program (version 1), or wait for Desk's guard? | **Ends the program**, as today | DOS-54 |
| 9 | Release builds require the same build id in the EXE and the modules? | **Yes**; development builds log and accept | CORE-94 |
| 10 | No version digit in module file names (`QUILLPIC.DMD`, not `QUILPIC1.DMD`)? | **No digit** | CORE-94 |
| 11 | `QUILL32.EXE` stays one file? | **Answered by the owner, 2026-11-27: no.** Windows 95 splits like its siblings (W9X-40 revised); its own questions are in section 19.15 | W9X-40 |
| 12 | Offer the hosted loader back to Desk as its M5? | **Yes**, after WP-MOD-2 merges | A Desk issue; not a Quill package |

## 18. Found while planning

- **The installer's shared libraries plan says DOS modules are a no-go** ("as Quill's modules plan found"); this plan reverses that for Quill. Its table row (section 1, the DOS and Linux line) should point here once this merges; the Win16 and Win32 verdicts agree.
- **Desk's hosted loader (M5) is not built** [V]; WP-MOD-2 is that loader for Quill, and the first real check of the CS-widening assumption Desk's design records as inferred.
- **The SPEC disagrees with itself on the DOS dictionary**: the build note under D88 says it is "read whole into memory when the first check begins", the dictionary section says it is "read in 32 KB pages, so only the pages a check touches are in memory" [V: `SPEC.md` lines 262 and 493]. Worth a one-line fix by whoever owns spelling next.
- **`[Quill] PictureMemory` defaults to 4,096 KB on DOS** [V: `SPEC.md` line 536], the whole of the 4 MB memory floor the SPEC promises; a 4 MB machine cannot give pictures that much. Worth checking when WP-MOD-3 measures memory.
- **Disk space.** `/home` had 19 GB free when this plan started, under the 20 GB floor builders keep, so no build was made; the figures come from an existing build. Builders of WP-MOD-1 and WP-MOD-2 should check before building.

## 19. Windows 95 and later DLL modules

**What changed (2026-11-27).** Section 7 gave Windows 95 a NO-GO (W9X-40) because nothing there binds: a PE image is paged on demand and the address space is flat. That is still true. The owner reversed the verdict for another reason: "let's do the DLLs for Windows 95 so they keep in line with the app split in its siblings". So the reason for modules on Windows 95, 98 and Me is the shape of the product, not a limit: one module crate per feature, one `quill-modabi` contract and three loaders, so a feature that is a file of its own on DOS and Windows 3.x is a file of its own on Windows 95 where Windows 95 has the feature at all. Two facts changed since section 7 was written: `QUILL32.EXE` grew to 452,608 bytes with the rich clipboard (pull request 196), and the no-CRT Win32 DLL recipe, which W9X-41 meant to borrow from the savers' `AGCORE1.DLL`, was never built there (WP-SL-1 has not started), so this plan proved it with Quill's own toolchain (section 19.2).

Measured for this section on `origin/main` at `08bc6dc`; the raw figures, the scripts and the spike's sources are in `~/Documents/Liminal HQ/Quill/Modules/Win95 spike/`. The spike's build and the Wine prefix were deleted afterwards. Markers as at the top of this page.

### 19.1 Verdict

| Module | Windows 95, 98 and Me | Why | What it takes out of `QUILL32.EXE` |
|---|---|---|---|
| `QUILLPIC.DLL` (PNG, BMP, then JPEG and RLE4) | **GO, first** (WP-MOD-9) | The same crate and kind as `QUILLPIC.DMD` and the Windows 3.x DLL; the narrowest interface; already measured | About **−15 to −16 KB** [E from 18,432 M, less 2 to 3 KB of loader and seam] |
| `QUILLPDF.DLL` (the PDF writer) | **GO, second** (WP-MOD-10), in a component of its own | Print to PDF exists only on Windows 95 today; as a file it becomes an optional component, as `QUILLPDF` is planned for the siblings. The walk over the document stays in the EXE (it needs the `Doc`); the writer moves | About **−14 to −20 KB** [E from 32,256 M] |
| `QUILLDLG.DLL` (modal dialogs) | **GO, later** (WP-MOD-12), after Options on Windows 95 | DOS puts its dialogs in `QUILLDLG`; on Win32 a DLL may call `USER32` freely (Windows 3.x's may not, W16-48), so the dialog procedures and their templates can move. Small today; it grows with Options, Page Layout, Numbering and Header and Footer | About −4 to −6 KB now, −15 to −25 KB once those dialogs land [E from M] |
| `QUILLHLP` | **None** | Help on Windows is WinHelp reading `QUILL.HLP`, as on Windows 3.x; the program holds one `WinHelpA` call. The kind stays reserved | — |

**The target.** From 452,608 bytes with pull request 196: about **418 to 424 KB** once `QUILLPIC` and `QUILLPDF` are out, and about **412 to 420 KB** with today's dialogs in `QUILLDLG` [E]. As on DOS (DOS-52 rule 3), a move-out lowers no budget: the 28 to 42 KB freed is what Unicode on Windows (WP-UNI-11), Reveal Codes (WP-CODES-5) and Options (WP-PREF-8) spend without a raise (W9X-138).

**The disk.** Each DLL carries its headers, relocations and runtime copy, about 1.5 to 3 KB raw and 1 to 2 KB packed [E from M]; the three together cost the Windows 95 floppy about 3 to 6 KB against its 64 KB floor. Splitting saves nothing on the disk; it was never the reason.

### 19.2 What was measured [M]

**`QUILL32.EXE` today.** 441,856 bytes: `.text` 342,016 on disk (341,786 in memory), `.rdata` 27,648, `.data` 512, `.rsrc` 70,656 (icons 63,118, the five dialog templates 2,850, the menu 2,238, the version 640), headers 1,024. Image base `0x400000`, `/FIXED` (no `.reloc`), section alignment 4,096, file alignment 512, subsystem 4.0 [M: `llvm-readobj`].

**What each feature costs it**, by building variants with `win9x/build.sh` and reverting them:

| Variant | Bytes | Δ | `.text` Δ | `.rdata` Δ |
|---|---:|---:|---:|---:|
| `main` at `08bc6dc` | 441,856 | — | — | — |
| Print to PDF cut (`pdf.rs`'s `render` returns an empty `Vec`) | 409,600 | **−32,256** | −27,860 | −4,364 |
| and picture decoding cut (both `quill_picture::render` calls in `gdi.rs` stubbed) | 391,168 | **−18,432** | −18,068 | −160 |

The order matters: `quill-pdfdoc` draws pictures through `quill_picture::render` too, so with Print to PDF in, stubbing the screen's calls frees almost nothing. **So `QUILLPIC.DLL` saves bytes only if every caller decodes through the seam**, the PDF walk and the clipboard's picture included (WP-MOD-9's finish line says so).

**By area**, from a build with DWARF (`CARGO_PROFILE_RELEASE_DEBUG=limited`, `.text` unchanged at 341,786; `win32-groups.py` charges each out-of-line function to its path; LTO inlines across crates, so these are lower bounds): `quill-app` 111,067; the Win32 shell 106,823 (its painter's closures hold the inlined picture and PDF code: `gdi::draw_line::closure$4` alone is 14,996); `quill-core` 33,010; `quill-formats` 25,633; `core` 13,615; `quill32::pdf` 12,097; `quill-pdfdoc` 11,551; the Win32 dialog procedures 7,445 (`tabs_proc` 1,462, `find_proc` 1,394, `about_proc` 1,241, `fonts_proc` 1,152, `spelling_proc` and its fill 1,035, `confirm_save` 280); `quill-inflate` 5,385; `quill-spell` 3,371; `quill-pdf` 3,109; `alloc` 3,101; `quill-image` 2,693.

**The spike: a no-CRT `QUILLPIC.DLL` with the existing toolchain.** A `cdylib` in a scratch workspace with `win9x/win9x.json`, `-Zbuild-std=core,compiler_builtins`, `opt-level = "z"`, LTO, `panic = "abort"`, linked by `lld-link` with the program's flags less `/ENTRY:start` and `/FIXED`, plus `/ENTRY:QuillDllEntry /BASE:0x10000000 /FILEALIGN:512` from its `build.rs`. It wraps `quill-image`'s PNG decoder and scaler over a chunk list behind `PicTableV1`, with the inflate window as a 32 KB static, and exports `QuillModuleInfo` and `QuillModuleStart`. A test program (`HOST.EXE`, 3,584 bytes, the same toolchain) builds the full path from `GetModuleFileNameA`, loads it with `LoadLibraryA` under `SetErrorMode`, checks `ModInfo` with `check_info` (release policy), starts it with a `ModStart`, decodes `tests/docs/pic.png` into 64 rows and frees it, 200 times.

| Finding | Figure |
|---|---|
| `QUILLPIC.DLL` (PNG and the scaler; no BMP, no dither) | **11,776 bytes**: `.text` 8,742, `.rdata` 403, `.data` 32,804 in memory and 0 on disk (the window is `.bss`), `.reloc` 68 (27 fixups), headers and tables the rest |
| Characteristics | `DLL`, no `RELOCS_STRIPPED`, subsystem 4.0, section alignment 4,096, file alignment 512, entry at the assembly stub; lld sets `DYNAMIC_BASE` and `NX_COMPAT` on a DLL unless told not to |
| Imports | `KERNEL32!ExitProcess` only (the panic handler); `check-imports.sh` and `scan-pentium.sh` pass on the DLL unchanged |
| Exports | Exactly `QuillModuleInfo` and `QuillModuleStart`, undecorated, **once `DllMain` and `_fltused` are assembly labels**: as `#[no_mangle]` Rust items a `cdylib` exports them too |
| Wine 11.21 | Exit 0: 200 load, check, start, decode and free cycles with one checksum; 10 with the picture missing, 11 with the DLL missing (a null handle, no box). `WINEDEBUG=+loaddll` shows the DLL loaded at `0x7B540000`, not its preferred `0x10000000` (Wine moves a `DYNAMIC_BASE` DLL), so the relocations were applied and work |

So the answers to the toolchain questions are yes: the existing Win32 toolchain emits a no-CRT, no-C-runtime DLL with Pentium-only code, relocations, subsystem 4.0, 4 KiB sections and 512-byte file alignment (both what Windows 95 accepts, and what `QUILL32.EXE` already uses), importing only `KERNEL32` ANSI functions on the allowlist. What the spike did not show: Windows 95, 98 and Me themselves, ReactOS, BMP and dither in the module, the shell's own loader, and the size with every decoder in (WP-MOD-9 measures it; 14 to 18 KB [E]).

### 19.3 The image (W9X-132)

- **Where.** `win9x/mods/<name>`: one `cdylib` package per module in the `win9x` workspace, so it shares `win9x.json`, `-Zbuild-std` and the release profile (`opt-level = "z"`, LTO inside the module, `panic = "abort"`, `strip`, `overflow-checks`).
- **Link flags.** `/ENTRY:start` and `/FIXED` move from `win9x/.cargo/config.toml`'s shared `rustflags` into `quill-win9x/build.rs` as `cargo:rustc-link-arg-bins` (the program's bytes stay identical, checked by SHA-256). Each module's `build.rs` adds `/ENTRY:QuillDllEntry`, `/BASE:<its base>`, `/DYNAMICBASE:NO`, `/FILEALIGN:512` and `/MAP` (the map is kept in the target folder for fault addresses, not shipped). Kept from the program: `/NODEFAULTLIB`, `/SUBSYSTEM:WINDOWS,4.0`, `/SAFESEH`, `/LARGEADDRESSAWARE`, no jump tables.
- **Bases.** `QUILLPIC` `0x61000000`, `QUILLDLG` `0x61100000`, `QUILLPDF` `0x61200000`: inside the private arena every Windows 9x process has (below `0x80000000`), clear of the program at `0x400000`, of `0x10000000` (where most third-party DLLs, shell extensions in the Open dialog among them, ask to be), of the savers' `0x63000000` and of the system DLLs. Relocations stay in every module, so a taken base costs a relocation pass, never a failure. `/DYNAMICBASE:NO` makes the load address the same on every run (Windows 9x ignores the flag; NT, ReactOS and Wine honour it).
- **Entry.** `QuillDllEntry@12` in assembly: `mov eax, 1; ret 12`. It stores nothing and runs nothing under the loader lock. Assembly, so the `cdylib` does not export it; `_fltused` is an assembly label for the same reason.
- **No TLS, no constructors.** The target says `has-thread-local: false`; Rust has no static constructors, so no C runtime start-up is missing; the loader zeroes `.bss` [W].
- **Checks per file** (`win9x/tools/check-dll.sh`, new): the `DLL` characteristic, no `RELOCS_STRIPPED`, a `.reloc` section, subsystem 4.0, section alignment 4,096 and file alignment 512, no TLS directory, the base the table above gives, and exactly the two exports. `scan-pentium.sh` and `check-imports.sh` run on every DLL, the latter against the module's own list (W9X-134).

### 19.4 The runtime and the boundary (W9X-133)

- **One runtime crate.** `quill-win9x/src/rt.rs` (the allocator over the process heap, `__chkstk`, the `_alldiv` family, `_fltused`, the panic handler) becomes `win9x/quill-w32rt`, used by the program and every module, with a feature `heap` for the allocator and `dll` for the entry macro and the label form of `_fltused`. It is a move: `QUILL32.EXE` byte-identical.
- **No shared heap.** `QUILLPIC.DLL` has no allocator at all (`quill-image` and `quill-inflate` allocate nothing): its working memory is its own `.bss` (the 32 KB window), per process on every Windows [W], so `ModInfo::arena` is 0 as on DOS. `QUILLPDF.DLL` needs a heap for the PDF it builds; it gets its own `#[global_allocator]` over `GetProcessHeap`, and nothing it allocates crosses: it hands the finished bytes out by copying into the caller's buffers (19.5). Only `#[repr(C)]` structs, flat addresses of caller memory and statuses cross (CORE-90).
- **Statics.** Each module has its own; nothing in the program is reachable from a module except what a call names. One session at a time, called only from the window thread (`QS_BUSY` otherwise); a module starts no thread.
- **Panics.** `panic = "abort"` on both sides, and no unwinding crosses a call (Rust aborts at an `extern "C"` boundary in any case). A module's panic handler ends the process with code 3, exactly as the program's does today, so a module panic is the program's panic, as DOS-54 says for DOS version 1. Containing it (a poisoned module answering `QS_INTERNAL`, as Windows 3.x's runtime does) would need a stack switch in every export; not now.
- **Never panics on input.** The glue checks every pointer and `size` before it reads (null, below the struct's own size, a chunk list longer than its count), and answers `QS_CORRUPT`; the logic crates keep their property tests (a damaged picture never panics). `overflow-checks` stays on, as in the program.

### 19.5 Exports and the contract (W9X-130, CORE-97, CORE-98)

- **Two exports by name, no ordinals.** `QuillModuleInfo(*ModInfo) -> Status` and `QuillModuleStart(*ModStart) -> Status`, `extern "C"` (cdecl, so the i586 names carry no `@n`). Names, because `GetProcAddress` by name is how every Windows, ReactOS and Wine find a function the same way; ordinals matter only on Windows 3.x, where the packer fixes them. No `.def` file and no import library: nothing links against a module.
- **The tables are the DOS tables.** On Win32 a module address is a flat 32-bit pointer, which is what it is on DOS (an offset in the flat 4 GiB segment), so `QuillModuleStart` fills the kind's table exactly as a `.DMD`'s `app_start` does: `PicTableV1` for pictures, unchanged. `quill-modabi` gains `WIN32_EXTENSION`, the two export names as constants and `QS_NOMEMORY` (10, a loader's answer only, so an addition within major 1); `PdfTableV1` and the Win32 `DlgTableV1` arrive with WP-MOD-10 and WP-MOD-12, with layout tests.
- **One glue, two packagings (CORE-98).** The entries that turn a `PicTableV1` call into slices of caller memory move from `dos/mods/quillpic` (WP-MOD-3) into `quill-modpic::flat`, which both `dos/mods/quillpic` and `win9x/mods/quillpic` call; the host side that drives a `PicTableV1` (WP-MOD-3's `Decoders` over a table in `dos/quill-dos/src/modules.rs`) moves into `quill_picture::modular::TableDecoders`. Only finding the table differs: `rt::module` on DOS, `LoadLibraryA` and `QuillModuleStart` on Win32. Windows 3.x keeps its far-pointer glue.
- **PDF (WP-MOD-10, ARCH-42).** `quill_pdfdoc` writes pages through a `Writer` trait (begin with the document's `Info`, a page of a size and resolution, text runs, rectangles, a picture as 24-bit rows, end of page, finish), with `Static` over `quill-pdf` and the Win32 shell's implementation over `PdfTableV1`. Pictures reach the module as rows the program decoded through `QUILLPIC`, so the PDF module holds no decoder; the finished file comes back through `read(*Chunk, *written)`, `QS_MORE` until the last byte, into the program's buffer.
- **Dialogs (WP-MOD-12).** `DlgTableV1 { size, info, run }` with `run(dialog, owner window, module handle, model, model length) -> Status`: the dialog's model (`#[repr(C)]`, generated beside the control ids in `quill-dialogs`) goes in, the person's answers and an `answer` field (OK or Cancel) come out, and the template is a resource of the DLL (`DialogBoxParamA` with the module's handle). Find (it searches as the person types), Spelling (it calls the checker) and the save question (it must work with every module missing) stay in the program.

### 19.6 Loading, versions and lifetime (W9X-131, W9X-135)

1. **The path.** `GetModuleFileNameA(NULL)`, cut after the last `\`, plus `QUILLPIC.DLL`. Never a bare name: Windows 95's `LoadLibraryA` with a bare name searches the program's folder, the current folder, the system folder, the Windows folder and `PATH` [W], so a stale copy in any of them could win.
2. **Load.** `SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX)` around `LoadLibraryA`, the old mode put back after, so a missing, damaged or 16-bit file never raises a system box [W]. On a null handle `GetLastError` sorts the reason: file or path not found is `QS_MISSING`; not enough memory (8, 14) is `QS_NOMEMORY`; a bad image (193, such as Windows 3.x's `QUILLPIC.DLL` in the same folder) is `QS_VERSION`, logged as "not a Win32 module".
3. **Check.** `GetProcAddress` for both names (`QS_VERSION` if either is missing); `QuillModuleInfo` then `quill_modabi::check_info` against the program's kind, `ABI_MAJOR` and build id; then `QuillModuleStart` with a `ModStart`, on which the module runs `check_start` from its side. A refused module is freed at once.
4. **Lifetime.** Loaded on first use (the first PNG drawn, the first Print to PDF, the first dialog), kept until exit, freed with `FreeLibrary` before `ExitProcess` (Windows frees it at exit anyway, unlike Windows 3.11; freeing it ourselves keeps `DLL_PROCESS_DETACH` in a sane process and makes the 200-cycle test the shell's own path). No idle unload: address space and memory are plentiful, as DOS-54 says for DOS. A "swap" is only ever a refused module freed; a module is never replaced while loaded.
5. **Retries.** `QS_VERSION` and `QS_CORRUPT` are remembered for the session (the file will not change under a running program); `QS_MISSING` and `QS_NOMEMORY` are retried at the next command that needs the module.
6. **Versions (CORE-94, W9X-135).** `win9x/build.sh` passes one `QUILL_BUILD_ID` to the program and every module. **Release policy when the build id is clean, development when its dirty bit is set**: anything staged for a disk comes from a clean commit and accepts only its own modules; a builder's dirty tree accepts another build with a log line. A `VERSIONINFO` resource in each DLL matching the program's (for Explorer's Properties), not a check.
7. **The log.** With `[Debug] Trace=1` in `QUILL.INI`, one line per load in `QUILL.LOG` beside `QUILL.INI`: the file, the load address, the status and both build ids, so a fault address can be read against the module's `.MAP`.

### 19.7 Failure modes

| What | What the person sees | Status, log |
|---|---|---|
| `QUILLPIC.DLL` missing | Each PNG or BMP is the grey box with `This picture needs QUILLPIC.DLL, which is missing or from another build of Quill. Run Setup again to repair Quill.` once a session on the status line; text, saving and printing work | `QS_MISSING` |
| `QUILLPDF.DLL` missing (the `pdf32` component unticked) | File > Print to PDF is greyed, its hint names the component (`Print to PDF is not installed. Run Setup to add it.`); `/PDF OUT` exits with code 2 | `QS_MISSING` |
| `QUILLDLG.DLL` missing | Format > Fonts, Tabs and the later dialogs are greyed with the CORE-93 message; Help > About falls back to a `MessageBoxA` with the name and version | `QS_MISSING` |
| A module from another build (release) or another `ABI_MAJOR` | As missing, with `from another build` in the log | `QS_VERSION`, both ids |
| Windows 3.x's `QUILLPIC.DLL` in the folder | As missing | `QS_VERSION`, "not a Win32 module" (error 193) |
| Not enough memory to load it | The same message with `not enough memory` in place of `missing`; retried next time | `QS_NOMEMORY` |
| Windows 95 without Internet Explorer, or without the OLE updates | Nothing: the modules import only what Windows 95's own `KERNEL32`, `USER32`, `GDI32` and `COMCTL32` 4.00 export, never `OLE32`, `OLEAUT32`, `SHLWAPI`, `MSVCRT` or a later `COMCTL32` (W9X-134) | — |
| A panic in a module | The program ends with code 3, as a panic does today | — |

### 19.8 Windows 98, Me, 2000 and later, ReactOS and Wine

- **Windows 95, 98 and Me** share the Windows 9x loader. It honours a full path, but a module whose base name matches one already loaded in the process is reused without looking at the path [W]: a foreign `QUILLPIC.DLL` already in Quill's process (pulled in by a shell extension in the Open dialog, say) would be the one Quill gets. The build id refuses it and the log names it. A DLL whose file alignment is below 4 KB is read into memory rather than mapped [W]; at 12 to 30 KB that is nothing. Windows 9x ignores `DllCharacteristics` and has no ASLR or NX [W].
- **Windows Me**'s System Restore keeps copies of changed `.exe` and `.dll` files outside its excluded folders [W], so a restore could put back an older module beside a newer program; the build id refuses the mix and the message sends the person to Setup.
- **Windows 2000, XP and later** run the same `QUILL32.EXE`. The NT loader takes the full path, the program's folder comes first for a module's own imports (all system DLLs, all `KnownDLLs`), `DllMain` never touches the loader lock, and `/DYNAMICBASE:NO` keeps the base fixed.
- **ReactOS 0.4.16**'s loader (`ntdll`) applies relocations and calls the entry with `DLL_PROCESS_ATTACH` as Windows does [U: the guest run, WP-MOD-13]. The demo images put `QUILL32.EXE` and `QUILL.EXE` together in `C:\Quill`, and both builds name their picture module `QUILLPIC.DLL`; the Windows 3.x build moves to `C:\Quill\WIN31\` (INS-27).
- **Wine** (11.21 here [M]) loads and relocates the spike as above; it is the everyday gate (`win9x/tools/wine-*.sh`), with `WINEDEBUG=+loaddll,+module` for the load address.

### 19.9 The installer and the gate (INS-26 to INS-28, W9X-138)

- **Files.** `QUILLPIC.DLL` (and later `QUILLDLG.DLL`) join `app32`'s `files`, required; `QUILLPDF.DLL` is the one file of a new component `pdf32`, "Print to PDF", for `windows-x86`, **ticked by default** (`default = true`), because Print to PDF ships today and unticking it would take a working feature away (INS-26; DOS's `pdf` stays unticked, INS-21). All are `{install}/NAME`, recorded `F name size crc32` in `INSTALL.LIR` and removed on uninstall (INS-20, installer D111); none goes to `{system}` or `{windows}`; no reference counting (they are private).
- **In use.** Windows 95 cannot replace a loaded DLL; only Quill loads these, so the installer's step that asks the person to close Quill covers them [U: whether `liminst` checks the program for Windows 95 today; WP-MOD-9 records what it finds].
- **Staging.** `win9x/build.sh` writes the modules to `win9x/dist/`; `installer/tools/stage.sh` copies them (by name) to `dist/windows-x86/`; `verify-disks.sh` compares them; `e2e-w95.sh` checks each is installed, listed and removed, and that Quill draws the sample's picture afterwards; a second run with `pdf32` unticked checks Print to PDF is greyed.
- **The gate (WP-MOD-11, INS-28).** `installer/budgets.toml` (pull request 191) gains `[executable.QUILL32]` (the budget: the measured size at that package's branch point rounded up to 4 KB; the stop 524,288) and module rows **keyed by target** for DLLs, since Windows 3.x and Windows 95 both ship a `QUILLPIC.DLL`: `[module."windows-x86/QUILLPIC.DLL"]` 24,576, `[module."windows-x86/QUILLPDF.DLL"]` 32,768, `[module."windows-x86/QUILLDLG.DLL"]` 49,152 (and WP-MOD-6's `win16-x86/QUILLPIC.DLL`). `size-gate.py` checks each staged `*.DLL` of `win16-x86` and `windows-x86` against its row and refuses one without; the `.DMD` rows keep their bare names. Program bytes (DOS-51) for Windows 95 are `QUILL32.EXE` and its required modules, packed; the 64 KB floor of the Windows 95 disk is unchanged.
- **Everywhere else the programs are copied.** `.github/workflows/reactos-demo.yml` and `reactos-qagrr.yml` copy `QUILL32.EXE` and `QUILL.EXE` only, `site/try-reactos/guest/build-all.sh` checks for those two, and `site/try-reactos/qagrr/qagrr.toml` lists the files for `C:\Quill`: each must carry the modules, or the demo loses its pictures the day `QUILLPIC.DLL` lands. The media's disk listing (`assets/media/gen_media.py`, "SETUP.EXE, QUILL32.EXE, QUILL.HLP and three dictionaries") changes with it. WP-MOD-9 owns these lines.

### 19.10 Risks

| Risk | Containment |
|---|---|
| A module linked with the program's `/ENTRY:start` or `/FIXED` (the flags are workspace-wide today) | They move to `quill-win9x/build.rs` (bins only); `check-dll.sh` fails on `RELOCS_STRIPPED`, a missing `.reloc` or the wrong entry |
| Relocations wrong, so a module works only at its base | `MODHOST.EXE` loads a second copy linked at `0x400000`, a base that is always taken, and compares its output (the spike's Wine run already relocated) |
| DLL hell: a stale or foreign `QUILLPIC.DLL` found first | Full path only (CORE-95), never `{system}` (INS-20), the build id, the log naming the file; Windows 9x's reuse by base name (19.8) is caught by the same check |
| Windows 3.x's `QUILLPIC.DLL` beside `QUILL32.EXE` | Separate folders where both builds sit (INS-27); error 193 is logged as "not a Win32 module" |
| A second allocator, or memory freed on the wrong side | `QUILLPIC` has none; `QUILLPDF`'s is its own and nothing it allocates crosses; the PDF leaves by copy into caller buffers |
| Work under the loader lock, TLS callbacks, missing CRT start-up | The entry is three instructions; no TLS (`check-dll.sh`); nothing needs a constructor |
| A panic in a module | The program's own panic path (exit code 3), as today when the code is linked in |
| Exports drift (an extra `#[no_mangle]` exported, a decorated name) | `check-dll.sh` lists exactly the two names |
| An import Windows 95 lacks, or one from Internet Explorer's updates | A per-module allowlist (W9X-134), `check-imports.sh` on each DLL, `e2e-w95.sh` on the plain base as well as the IE4 one |
| Decoders still linked into the program through the PDF walk or the clipboard, so nothing is saved | WP-MOD-9's finish line: every caller of `quill_picture::render` in the Win32 build goes through `Decoders`; the size figure must fall by at least 10 KB or the package stops and reports |
| The demo and the media forget the new files | WP-MOD-9 owns the workflow, `build-all.sh`, `qagrr.toml` and media lines (19.9) |
| Debugging across the boundary | A `.MAP` per DLL in the target folder, the load address in `QUILL.LOG`, Wine's `+loaddll`, ReactOS's serial log |

### 19.11 Tests

| Level | What |
|---|---|
| Host | `quill-modabi`'s layout tests for the new items (`QS_NOMEMORY`, the export names, `PdfTableV1` and `DlgTableV1` as they land, each 32-bit size checked under `target_pointer_width = "32"`); `quill-modpic::flat` against `quill-image`'s goldens; `quill_picture::modular::TableDecoders` over a fake table; `quill_pdfdoc`'s golden PDF through `Static` and through a recording `Writer` (the same calls in the same order) |
| `MODHOST.EXE` under Wine (`win9x/tools/wine-modules.sh`) | `MODHELLO.DLL` and `QUILLPIC.DLL`: 200 load, start, call and free cycles with one checksum; the relocated copy equal; a missing file `QS_MISSING` with no box; a wrong-build copy `QS_VERSION` and freed; a 16-bit DLL `QS_VERSION`; null and short structs `QS_CORRUPT` |
| Quill under Wine | Every `win9x/tools/wine-*.sh` with the modules present, byte-equal to today (`wine-picture.sh`, `wine-pic8.sh`, `wine-pdf.sh` above all); `wine-e2e.sh`'s picture scenarios with `QUILLPIC.DLL` absent and from another build (their own goldens: the grey box and the message); `wine-pdf.sh` with `QUILLPDF.DLL` absent (the item greyed, `/PDF` exit code 2) |
| Each DLL | `check-dll.sh`, `scan-pentium.sh`, `check-imports.sh` against the module's list |
| Windows 95 | `make installer-w95` (`e2e-w95.sh` in DOSBox-X, Windows 95 OSR2.5) on the plain base and with `W95_BASE_KIND=ie4`: installed, listed, removed; the sample's picture drawn; Print to PDF writes a file; `pdf32` unticked greys it |
| ReactOS 0.4.16 | The demo guest (WP-MOD-13): the demo letter's picture, Print to PDF, and the module renamed |
| Windows 98 SE and Me | By hand on the VMs (WP-MOD-13): the same three checks, a screenshot each |
| Installer and gate | `make disks` with the new rows (G-DISK); `installer/tools/test_size_gate.py` gains the DLL rows and the unlisted-DLL refusal |

### 19.12 Decisions

Ids from `scripts/next-id.sh` (W9X-130 to W9X-139, INS-26 to INS-28) and the spares of section 14 (CORE-97, CORE-98, ARCH-42). W9X-40 and W9X-41 are in `docs/decisions.md` with this plan; the rest are copied there, **Assumed**, by the package that builds them, as section 14 says.

| Id | Decision | Rejected | Why |
|---|---|---|---|
| W9X-130 | A Win32 module exports two functions by name, `QuillModuleInfo` and `QuillModuleStart` (cdecl, undecorated), and `QuillModuleStart` fills the kind's DOS table (`PicTableV1`) with flat addresses; no ordinals, no `.def`, no import library | Ordinals; one export per operation; implicit linking | `GetProcAddress` by name behaves the same everywhere; the flat tables are already defined and tested |
| W9X-131 | Modules load by full path from `GetModuleFileNameA` under `SetErrorMode(SEM_FAILCRITICALERRORS \| SEM_NOOPENFILEERRORBOX)`, on first use, stay until exit and are freed with `FreeLibrary` before `ExitProcess` or at once when refused; `GetLastError` sorts missing, memory and bad image | Idle unload; a bare name; loading at start | No system box; nothing to save by unloading on Win32 |
| W9X-132 | The image: a `cdylib` in `win9x/mods/<name>`, entry `QuillDllEntry@12` in assembly, bases from `0x61000000` a megabyte apart, relocations kept, `/DYNAMICBASE:NO`, `/FILEALIGN:512`, 4 KiB sections, subsystem 4.0, Pentium code, no TLS; `check-dll.sh` checks it | `0x10000000`; stripped relocations; `/FILEALIGN:4096` | Clear of the program, the shell extensions' usual base and the savers; relocatable everywhere |
| W9X-133 | One runtime crate, `win9x/quill-w32rt`, for the program and its modules; a module allocates only if it must, on its own allocator, and nothing it allocates crosses; `panic = "abort"`, a module's panic ends the process as the program's does | A shared heap; containing panics now | No second owner of any memory; the containment needs a stack switch per export |
| W9X-134 | Each module has its own import list: `QUILLPIC` and `QUILLPDF` `KERNEL32` only, `QUILLDLG` the `USER32`, `GDI32` and `COMCTL32` 4.00 functions already on the program's list; never `OLE32`, `OLEAUT32`, `SHLWAPI`, `MSVCRT` or a later `COMCTL32`. The program adds `LoadLibraryA`, `FreeLibrary`, `SetErrorMode` and `GetLastError` | One list for every file | Windows 95 with no Internet Explorer must load every module |
| W9X-135 | On Win32 the policy follows the build id: release when it is clean, development when its dirty bit is set | A cargo feature; always release | Every staged build is from a clean commit; a builder can rebuild one side |
| W9X-136 | `QUILLPDF.DLL` holds the PDF writer (`quill-pdf`: pages, fonts, deflate, the file); the walk (`quill-pdfdoc`) stays in the program and writes through `quill_pdfdoc::Writer`; pictures reach it as rows decoded by `QUILLPIC` | The walk in the module; a decoder in the PDF module | The walk needs the `Doc`; one decoder in one module |
| W9X-137 | `QUILLDLG.DLL` on Win32 holds modal dialogs whose state is a model in and out (About, Fonts, Tabs, Options, Page Layout, Numbering, Header and Footer), with their templates as its resources; Find, Spelling and the save question stay in the program | Every dialog; none | Narrow dialogs only; the save question must work with every module missing |
| W9X-138 | `QUILL32.EXE` gets a budget row and each Win32 module its own (WP-MOD-11); a move-out lowers no budget (as DOS-52) | No budget on Windows 95 | The freed room is what the planned Windows features spend |
| W9X-139 | Spare | | |
| INS-26 | `QUILLPDF.DLL` is the component `pdf32`, "Print to PDF", ticked by default; `QUILLPIC.DLL` and `QUILLDLG.DLL` are in the required `app32` | `pdf32` unticked; PDF in `app32` | Print to PDF ships today; the component makes it optional without taking it away |
| INS-27 | Where both Windows builds share a folder (the ReactOS demo's `C:\Quill`, a future all-in-one disc), each build's set sits in its own folder (`C:\Quill` for Windows 95, `C:\Quill\WIN31` for Windows 3.x); module stems stay the same on every build | A Win32 stem of its own (`QPIC32.DLL`) | Both builds call their picture module `QUILLPIC.DLL` (CORE-94, `quill-modabi`) |
| INS-28 | Budget rows for DLL modules are keyed by target (`windows-x86/QUILLPIC.DLL`), and the gate refuses a staged DLL with no row; `.DMD` rows keep their bare names | Bare names for DLLs | Two `QUILLPIC.DLL` files, one per Windows build |
| CORE-96 (revised), CORE-97, CORE-98, ARCH-42 | Section 14 | | |

### 19.13 Work packages

**Order.** The side lane takes all of them (no file in `crates/quill-app/src`, ARCH-41), one builder at a time beside the two lanes' builders, inside the cap of three.

| Order | Package | Needs | Size | Saves (`QUILL32.EXE`) |
|---:|---|---|---|---|
| 1 | **WP-MOD-11** The Windows 95 rows in the packed-size gate | Pull request 191 merged | S | — (the budget row) |
| 2 | **WP-MOD-9**, pull request 1: the Win32 module toolchain, `quill-w32rt`, the loader, `MODHELLO.DLL` and `MODHOST.EXE` | — (can start now) | M | 0 (byte-identical) |
| 3 | **WP-MOD-9**, pull request 2: `QUILLPIC.DLL` | WP-MOD-3 merged (`quill-modpic`, `quill_picture::Decoders`), PR 1, WP-MOD-11 | M | −15 to −16 KB [E] |
| 4 | **WP-MOD-10** `QUILLPDF.DLL` and `pdf32` | WP-MOD-9 | M to L | −14 to −20 KB [E] |
| 5 | **WP-MOD-13** The set in the ReactOS 0.4.16 guest and on Windows 98 SE and Me | WP-MOD-10 | S | — (evidence) |
| 6 | **WP-MOD-12** `QUILLDLG.DLL` | WP-MOD-10; WP-PREF-8 (Options on Windows 95) merged | M | −4 to −6 KB now, −15 to −25 KB with the later dialogs [E] |

Against the siblings: WP-MOD-9's second pull request follows WP-MOD-3 as WP-MOD-6 does, and the two can run together (different shells; `quill-modpic::flat` and `TableDecoders` are WP-MOD-9's only shared-crate files, and WP-MOD-6 uses neither). WP-MOD-7 (JPEG) then lands in `QUILLPIC` on all three builds at once.

#### WP-MOD-9 — the Win32 module toolchain, the loader and `QUILLPIC.DLL`

- **Finish line, pull request 1.** `win9x/quill-w32rt` holds the runtime (moved from `rt.rs`) and the loader (`quill_w32rt::module`: path, load, check, start, free, the status and the log line); the link flags are per artefact; `win9x/mods/hello` builds `MODHELLO.DLL` (a `KIND_PIC` module that writes a gradient) and `win9x/mods/modhost` builds `MODHOST.EXE` (never staged); `win9x/build.sh` builds the modules after the program with one `QUILL_BUILD_ID`; `check-dll.sh` and `wine-modules.sh` pass; `QUILL32.EXE` is byte-identical (SHA-256).
- **Finish line, pull request 2.** `win9x/mods/quillpic` builds `QUILLPIC.DLL` from `quill-modpic` through `quill-modpic::flat` (PNG, every BMP the program reads, the scaler); `win9x/quill-win9x/src/modules.rs` gives the shell a `quill_picture::Decoders` over it through `quill_picture::modular::TableDecoders`; every caller of picture decoding in the Win32 build goes through it (the screen, Print Preview and printing, the PDF walk, the clipboard's DIB); every Wine script byte-equal with the module present; absent and wrong-build behave as 19.7; the installer, the gate row, the demo pipeline and the media list carry the file.
- **Owns.** PR 1: `win9x/quill-w32rt/**` (new), `win9x/quill-win9x/src/rt.rs` (removed), `win9x/quill-win9x/src/main.rs` (the `rt` lines), `win9x/quill-win9x/Cargo.toml`, `win9x/quill-win9x/build.rs` (new), `win9x/.cargo/config.toml`, `win9x/Cargo.toml`, `win9x/Cargo.lock`, `win9x/mods/hello/**`, `win9x/mods/modhost/**`, `win9x/build.sh`, `win9x/PROVENANCE.md`, `win9x/tools/check-dll.sh` (new), `win9x/tools/wine-modules.sh` (new), `win9x/tools/check-imports.sh` (a list argument), `win9x/tools/imports.allow`, `crates/quill-modabi/src/lib.rs` and `tests/abi.rs` (`WIN32_EXTENSION`, the export names, `QS_NOMEMORY`). PR 2: `win9x/mods/quillpic/**`, `win9x/quill-win9x/src/modules.rs` (new), `win9x/quill-win9x/src/gdi.rs` and `pdf.rs` (the decode calls, named lines), `crates/quill-modpic/src/flat.rs` (new, moved from `dos/mods/quillpic`), `dos/mods/quillpic/src/*.rs` (the call into `flat`), `crates/quill-picture/src/modular.rs` (`TableDecoders`, moved from `dos/quill-dos/src/modules.rs` if WP-MOD-3 put it there), `crates/quill-pdfdoc/src/lib.rs` (its picture call through `Decoders`), `installer/components.toml` (`app32`'s files), `installer/tools/stage.sh`, `installer/tools/e2e-w95.sh`, `installer/budgets.toml` (the row's figure), `.github/workflows/reactos-demo.yml` and `reactos-qagrr.yml` (the copy lines), `site/try-reactos/guest/build-all.sh` (the input check and copy), `site/try-reactos/qagrr/qagrr.toml` (the files; `QUILL.EXE` and its files to `Quill/WIN31/`, its shortcut's paths), `assets/media/gen_media.py` (the disk listing), `SPEC.md` (the Windows 95 file list and pictures), the decision rows.
- **Must not touch.** `crates/quill-app/src/**` (if the seam needs an `App` line, stop and report), `crates/quill-image/src/**`, `crates/quill-inflate/src/**`, `dos/rt/**`, `win16/**`, `linux/**`, `winui/**`.
- **Budgets.** PR 1: `QUILL32.EXE` byte-identical; `MODHELLO.DLL` ≤ 8 KB. PR 2: `QUILL32.EXE` −10 KB at least (−15 to −16 KB expected; stop and report under −10 KB); `QUILLPIC.DLL` ≤ 24,576; each DLL's figure and the Windows 95 disk's free bytes in the PR.
- **Tests.** G-HOST, G-W32 with `wine-modules.sh`, `wine-picture.sh`, `wine-pic8.sh`, `wine-pdf.sh`, `check-dll.sh`; G-DISK; `make installer-w95` on both bases (PR 2).
- **Evidence.** `~/Documents/Liminal HQ/Quill/Modules/WP-MOD-9/`: `win32-wine-picture.png` (present), `win32-wine-picture-absent.png`, `win95-picture.png` (from `e2e-w95.sh`'s shots), `modhost.txt` (the cycle log), `sizes.txt` (before and after, `llvm-readobj` of each file). Not verified by them: Windows 98 and Me, ReactOS (WP-MOD-13).
- **Decisions.** W9X-130 to W9X-135, W9X-138, CORE-96 (revised), CORE-97, CORE-98, INS-27, INS-28.

#### WP-MOD-10 — `QUILLPDF.DLL` and the `pdf32` component

- **Finish line.** `quill_pdfdoc` writes through a `Writer` trait (ARCH-42) with `Static` over `quill-pdf`; `quill-modabi` has `PdfTableV1` and its structs with layout tests; `win9x/mods/quillpdf` builds `QUILLPDF.DLL` over `quill-pdf` with its own allocator; the shell's `Writer` drives it, passing pictures as rows from `QUILLPIC`; `wine-pdf.sh` gives byte-equal PDFs with the module present (the same `qpdf`, `pdftotext` and `pdfimages` checks); with it absent, Print to PDF is greyed with its hint and `/PDF OUT` exits with code 2; `pdf32` installs and removes it.
- **Owns.** `crates/quill-pdfdoc/src/**`, `crates/quill-pdf/src/lib.rs` (the `Writer` implementation, named lines), `crates/quill-modabi/**` (the PDF kind), `win9x/mods/quillpdf/**`, `win9x/quill-win9x/src/pdf.rs`, `win9x/quill-win9x/src/modules.rs` (the PDF lines), the menu's greying (`win9x/quill-win9x/src/ui.rs`, named lines), `win9x/build.sh`, `installer/components.toml` (`pdf32`), `installer/tools/stage.sh`, `installer/tools/e2e-w95.sh`, `installer/budgets.toml` (the row), `SPEC.md`.
- **Must not touch.** `crates/quill-app/src/**`, `linux/**` (the GTK client uses `quill-pdfdoc` through `Static`: its PDFs must stay byte-equal; run `make gtk`'s PDF test), `dos/**`, `win16/**`.
- **Budgets.** `QUILL32.EXE` −10 KB at least (−14 to −20 KB expected); `QUILLPDF.DLL` ≤ 32,768.
- **Tests.** G-HOST (`quill-pdf`, `quill-pdfdoc` goldens unchanged through `Static`), G-W32 with `wine-pdf.sh` present and absent, G-GTK's PDF check, G-DISK, `make installer-w95` with `pdf32` ticked and unticked.
- **Decisions.** W9X-136, INS-26, ARCH-42, CORE-97 (the PDF table).

#### WP-MOD-11 — the Windows 95 rows in the packed-size gate

- **Finish line.** `installer/budgets.toml` has `[executable.QUILL32]` (`file = "windows-x86/QUILL32.EXE"`, the budget the measured size rounded up to 4 KB, `stop = 524288`) and the target-keyed DLL rows of 19.9; `size-gate.py` checks `QUILL32.EXE`, each staged `*.DLL` in `win16-x86` and `windows-x86` against its row, refuses one with no row, and counts required modules in program bytes for each disk; `test_size_gate.py` covers it. No DLL exists yet, so the rows are checked by the tests only.
- **Owns.** `installer/budgets.toml`, `installer/tools/size-gate.py`, `installer/tools/test_size_gate.py`, `docs/installer.md` (the gate's section), the decision rows. **Must not touch** everything else.
- **Tests.** `python3 installer/tools/test_size_gate.py`, G-DISK, G-MD. **Budgets.** No executable changes.
- **Decisions.** W9X-138, INS-28.

#### WP-MOD-12 — `QUILLDLG.DLL` on Windows 95

- **Finish line.** `quill-modabi`'s Win32 `DlgTableV1`; each moved dialog's model generated in `quill-dialogs` beside its control ids; `win9x/mods/quilldlg` holds About, Fonts, Tabs and the Options, Page Layout, Numbering and Header and Footer dialogs that have landed, their templates in its own `.res` (`res_pack.py` writes the program's and the module's); the program keeps Find, Spelling and the save question; every dialog scenario under Wine byte-equal; absent, the CORE-93 message and About's `MessageBoxA`.
- **Owns.** `win9x/mods/quilldlg/**`, `win9x/quill-win9x/src/dialogs.rs`, `win9x/tools/res_pack.py` and its test, `crates/quill-dialogs/**` (the models), `crates/quill-modabi/**` (the kind), `installer/components.toml` (`app32`), `stage.sh`, `e2e-w95.sh`, `budgets.toml` (the row).
- **Must not touch.** `crates/quill-app/src/**`, `dos/**`, `win16/**`.
- **Budgets.** `QUILL32.EXE` down by the moved procedures and templates (report); `QUILLDLG.DLL` ≤ 49,152.
- **Tests.** G-W32 with `wine-dialogs.sh` and `wine-e2e.sh`'s `m2-dialogs`; `check-dll.sh`; G-DISK.
- **Decisions.** W9X-137, CORE-97 (the dialog table).

#### WP-MOD-13 — the set on ReactOS 0.4.16, Windows 98 SE and Me

- **Finish line.** The ReactOS demo image built with the modules (`site/try-reactos`, the QAGRR recipe) boots in the guest; the demo letter's picture draws, Print to PDF writes a file, and with `QUILLPIC.DLL` renamed the grey box and the message show with no system box; the Windows 3.x build still runs from `C:\Quill\WIN31`. The same three checks by hand on the Windows 98 SE and Me VMs. `docs/testing.md` and `docs/progress.md` record it.
- **Owns.** `site/try-reactos/qagrr/check-state.sh` (the checks), `docs/testing.md`, `docs/progress.md`. **Must not touch** the programs.
- **Evidence.** `~/Documents/Liminal HQ/Quill/Modules/WP-MOD-13/`: `reactos-picture.png`, `reactos-pdf.png`, `reactos-absent.png`, `win98-picture.png`, `winme-picture.png`. Not verified: Windows 95 retail on real hardware.

### 19.14 Briefs

Each starts with [the common brief](../work-packages.md#the-common-brief); evidence goes to `~/Documents/Liminal HQ/Quill/Modules/<WP-ID>/`; decision ids are section 19.12's.

#### WP-MOD-9 (full)

```text
Package: WP-MOD-9 — the Win32 module toolchain, the loader and QUILLPIC.DLL (two pull requests).
Branches feat/w32-modules (PR 1) and feat/w32-quillpic (PR 2); worktrees ~/source/worktrees/quill/mod-9
and ~/source/worktrees/quill/mod-9b. Read docs/plans/modules.md sections 4, 14 and 19 (all of it) first,
then win9x/PROVENANCE.md, win9x/.cargo/config.toml, win9x/quill-win9x/src/rt.rs and sys.rs, and
crates/quill-modabi. The spike in ~/Documents/Liminal HQ/Quill/Modules/Win95 spike/spike/ is a working
reference for the DLL's link flags, entry stub and exports; copy ideas, not files.

PR 1 can start now. PR 2 waits for WP-MOD-3 (quill-modpic, quill_picture::Decoders) and WP-MOD-11 to
merge; branch it from origin/main after they do.

PR 1 — the toolchain and the loader (no behaviour change):
1. Measure first: sha256sum and size of win9x/dist/QUILL32.EXE from `bash win9x/build.sh` at the branch
   point, into the evidence folder's sizes.txt.
2. Move win9x/quill-win9x/src/rt.rs into a new crate win9x/quill-w32rt (no_std; [package.metadata.quill]
   layer "shell"; SPDX headers), with features `heap` (the GlobalAlloc over the process heap) and `dll`
   (a `dll_entry!()` macro emitting `.globl _QuillDllEntry@12` / `mov eax, 1` / `ret 12` in global_asm,
   and `_fltused` as an assembly label instead of the #[no_mangle] static). Without `dll` it must produce
   exactly today's symbols. The program uses it with `heap`. QUILL32.EXE must stay byte-identical.
3. Link flags: take /ENTRY:start and /FIXED out of win9x/.cargo/config.toml's rustflags and emit them from
   a new win9x/quill-win9x/build.rs with cargo:rustc-link-arg-bins. Each module's build.rs emits, with
   cargo:rustc-link-arg-cdylib: /ENTRY:QuillDllEntry, /BASE:<base>, /DYNAMICBASE:NO, /FILEALIGN:512 and
   /MAP:<target dir>/<NAME>.MAP. Bases: MODHELLO 0x61F00000, QUILLPIC 0x61000000 (PR 2). Check
   QUILL32.EXE is still byte-identical.
4. The loader, quill_w32rt::module (feature `loader`): load(kind) builds <program folder>\<NAME>.DLL from
   GetModuleFileNameA(NULL) (never a bare name), sets SetErrorMode(SEM_FAILCRITICALERRORS |
   SEM_NOOPENFILEERRORBOX) around LoadLibraryA and restores it, maps a null handle through GetLastError
   (2 and 3 -> QS_MISSING; 8 and 14 -> QS_NOMEMORY; 193 and any other -> QS_VERSION, with the code kept for
   the log), gets QuillModuleInfo and QuillModuleStart with GetProcAddress (either missing -> QS_VERSION),
   calls info and quill_modabi::check_info (kind, ABI_MAJOR, build id; Policy::Release unless
   quill_modabi::is_dirty(own build id)), then start with a ModStart, and frees the module on any refusal.
   It returns { module handle, table address, table length, ModInfo, Accepted }. free() calls FreeLibrary.
   Every field it reads from a module is checked (size first). Add LoadLibraryA, FreeLibrary, SetErrorMode
   and GetLastError to win9x/tools/imports.allow (all Windows 95 KERNEL32) and to the sys declarations.
5. crates/quill-modabi: add WIN32_EXTENSION ".DLL" (beside WIN16_EXTENSION), WIN32_INFO_EXPORT
   "QuillModuleInfo", WIN32_START_EXPORT "QuillModuleStart", QS_NOMEMORY = 10 (a loader's answer; doc
   comment says so) and its status_name; tests for each (distinct statuses, names). ABI_MINOR stays 0
   unless a struct changes (none does).
6. win9x/mods/hello: a cdylib MODHELLO.DLL of kind KIND_PIC (quill-w32rt with `dll`, no `heap`) whose
   QuillModuleStart runs check_start and fills a PicTableV1 whose rows entry writes a gradient of the
   asked width; its own imports.allow (KERNEL32!ExitProcess only). A second build of it linked at
   /BASE:0x00400000 (always taken by the program) as MODHELLOR.DLL, for the relocation test. Use the
   build id QUILL_BUILD_ID from the environment (build.sh sets it: the first eight hex digits of HEAD,
   the top bit set when `git status --porcelain` is not empty).
7. win9x/mods/modhost: MODHOST.EXE (a test program, never staged) that uses quill_w32rt::module to:
   load, start, call (open, rows, close) and free MODHELLO.DLL 200 times with one checksum; load the
   relocated copy and compare its rows; ask for a missing name (QS_MISSING, no box); load a copy built
   with another build id (QS_VERSION, and the handle freed); load win16's QUILL.EXE renamed to a .DLL if
   present, or any NE file, for error 193 (QS_VERSION); call the table with a null and a short struct
   (QS_CORRUPT). It writes one line per check to MODHOST.LOG and exits 0 only if all pass.
8. win9x/build.sh: after the program, build every module and copy it as an upper-case NAME.DLL into
   dist/ (MODHELLO.DLL and MODHOST.EXE into dist/test/, which stage.sh never reads), with one
   QUILL_BUILD_ID for all. win9x/tools/check-dll.sh FILE BASE: llvm-readobj must show the DLL
   characteristic, no RELOCS_STRIPPED, a .reloc section, subsystem 4.0, SectionAlignment 4096,
   FileAlignment 512, no TLS directory, ImageBase = BASE, and exactly the exports QuillModuleInfo and
   QuillModuleStart. check-imports.sh takes an optional second argument, the list file. Run
   scan-pentium.sh on every DLL. win9x/tools/wine-modules.sh runs MODHOST.EXE under Wine (own prefix under
   ~/.cache, WINEDEBUG=+loaddll kept in the log) and fails unless it exits 0 and MODHELLOR.DLL loaded
   away from 0x400000.
9. Docs: win9x/PROVENANCE.md gains the DLL recipe (what came from the spike, what is Quill's);
   docs/toolchain-needs.md if it lists Win32 needs; decisions W9X-130 to W9X-135, CORE-97, CORE-98
   (Assumed; text from modules.md 19.12); docs/progress.md.
Gates PR 1: G-HOST, bash win9x/build.sh, check-imports.sh (the program and each DLL with its list),
scan-pentium.sh (each file), check-dll.sh (each DLL), wine-modules.sh, wine-e2e.sh (unchanged),
check-headers, G-MD. Budget: QUILL32.EXE byte-identical (sha256 in the PR); MODHELLO.DLL <= 8 KB.

PR 2 — QUILLPIC.DLL:
1. Measure QUILL32.EXE at the branch point. Move WP-MOD-3's flat-address glue (the entries turning u32
   addresses into slices, in dos/mods/quillpic) into crates/quill-modpic/src/flat.rs and call it from
   dos/mods/quillpic (G-DOS must stay byte-equal: QUILLPIC.DMD's behaviour and QUILLDOS.EXE unchanged),
   and the host side that calls a PicTableV1 into quill_picture::modular::TableDecoders if WP-MOD-3 left
   it in dos/quill-dos. If either move is more than a move, stop and report.
2. win9x/mods/quillpic: QUILLPIC.DLL (base 0x61000000) over quill-modpic::flat: PNG, every BMP the
   program decodes today, the scaler; no allocator; its imports.allow KERNEL32!ExitProcess only.
3. win9x/quill-win9x/src/modules.rs: a quill_picture::Decoders over quill_w32rt::module and
   TableDecoders: loads on the first picture, remembers QS_VERSION and QS_CORRUPT for the session,
   retries QS_MISSING and QS_NOMEMORY at the next picture, frees before ExitProcess; the grey box and
   the message of modules.md 19.7 once a session; [Debug] Trace=1 writes the load line to QUILL.LOG beside
   QUILL.INI (file, load address, status, both build ids). Route every decode of the Win32 build through
   it: gdi.rs's screen and printing paths, Print Preview, and quill-pdfdoc's picture call (give it the
   Decoders through its Options or a generic parameter, with Static for GTK; GTK's PDFs must stay
   byte-equal: run its PDF test). If the clipboard writes a DIB of a picture by decoding it, that path
   too. Nothing in crates/quill-app/src changes; if it must, stop and report.
4. Installer and copies: QUILLPIC.DLL in app32's files (installer/components.toml), stage.sh copies it
   by name, e2e-w95.sh lists it among the installed files, checks it is removed, and checks the sample's
   picture draws; the budget row windows-x86/QUILLPIC.DLL = 24576 (WP-MOD-11's file). The ReactOS demo:
   .github/workflows/reactos-demo.yml and reactos-qagrr.yml copy QUILLPIC.DLL with QUILL32.EXE;
   site/try-reactos/guest/build-all.sh checks for it; qagrr.toml writes it to Quill/ and moves QUILL.EXE,
   DEMO16.* and the Windows 3.x shortcut's paths to Quill/WIN31/ (INS-27), so WP-MOD-6's QUILLPIC.DLL can
   never sit beside this one. assets/media/gen_media.py's Windows 95 disk listing names the DLL.
5. Tests: every win9x/tools/wine-*.sh byte-equal with the module present (wine-picture, wine-pic8,
   wine-pdf, wine-print, wine-e2e above all; copy the module into each script's work folder through one
   helper, win9x/tools/lib-dist.sh, with names spelled out); wine-e2e.sh's m7-picture and
   m15-picture-select with QUILLPIC.DLL absent and with a wrong-build copy (new goldens of their own:
   the grey box and the message, the same golden for both); MODHOST.EXE also cycles QUILLPIC.DLL 200
   times over tests/docs/pic.png.
6. Docs: SPEC.md (the Windows 95 files, pictures needing the module), decisions W9X-138 (if WP-MOD-11 has
   not), INS-27, CORE-96 revised; docs/progress.md.
Gates PR 2: G-HOST, G-DOS (the glue move), G-W32 with the scripts above, check-dll.sh, check-imports.sh
and scan-pentium.sh on QUILLPIC.DLL, G-GTK's PDF test, G-DISK, make installer-w95 on the plain base and
with W95_BASE_KIND=ie4, check-headers, G-MD. Budgets: QUILL32.EXE at least -10 KB from the branch point
(-15 to -16 KB expected; under -10 KB stop and report what is still linked, by building once with
CARGO_PROFILE_RELEASE_DEBUG=limited and reading the DWARF as modules.md 19.2 did); QUILLPIC.DLL <= 24,576;
each DLL's size and the Windows 95 disk's free bytes in the PR.

Must not touch: crates/quill-app/src/**, crates/quill-image/src/**, crates/quill-inflate/src/**,
dos/rt/**, win16/**, linux/** (except running its tests), winui/**.
Evidence (~/Documents/Liminal HQ/Quill/Modules/WP-MOD-9/): sizes.txt (before and after, llvm-readobj of
each file), modhost.txt, win32-wine-picture.png, win32-wine-picture-absent.png, win95-picture.png (from
e2e-w95.sh's SHOTS). Say what they do not show: Windows 98 and Me, ReactOS (WP-MOD-13).
OWNER DECISION defaults to take and record if unanswered: modules.md 19.15 rows 1 to 6.
Report: both PR URLs, the gates and results, the sizes before and after, the evidence folder, and what
was not verified.
```

#### WP-MOD-10

```text
Package: WP-MOD-10 — QUILLPDF.DLL and the pdf32 component. Branch feat/w32-quillpdf, worktree
~/source/worktrees/quill/mod-10. Needs WP-MOD-9 merged. Read modules.md 19 (19.5's PDF bullet, 19.7,
19.9) and docs/decisions.md W9X-90 to W9X-94.
1. quill-pdfdoc writes through a Writer trait (ARCH-42): begin(Info), page(width, height, dpi),
   text(Run), rect(Rgb, x, y, w, h), picture(Picture as 24-bit rows), end_page(), finish(); Static over
   quill-pdf is today's behaviour (every quill-pdf and quill-pdfdoc golden byte-equal, GTK's PDFs too).
2. quill-modabi: KIND_PDF's PdfTableV1 { size, info, begin, page, text, rect, picture, end_page, finish,
   read, close } and its #[repr(C)] structs (text as Windows-1252 bytes in a caller chunk with a font id
   and size; a picture as a chunk list of rows), with size, align and offset tests.
3. win9x/mods/quillpdf: QUILLPDF.DLL (base 0x61200000) over quill-pdf with quill-w32rt's `heap` (its own
   allocator; nothing it allocates crosses); read() copies the finished file into the caller's chunk,
   QS_MORE until the last byte; imports KERNEL32!ExitProcess, GetProcessHeap, HeapAlloc and HeapFree.
4. The shell: pdf.rs implements Writer over the table through quill_w32rt::module; pictures go through
   the QUILLPIC Decoders first. Absent: File > Print to PDF greyed with "Print to PDF is not installed.
   Run Setup to add it." as its hint; /PDF OUT exits with code 2 and a QUILL.LOG line under Trace.
5. Installer: a component pdf32 ("Print to PDF", targets windows-x86, default = true) with QUILLPDF.DLL;
   stage.sh, e2e-w95.sh (installed; and a run with pdf32 unticked: the item greyed), the budget row
   windows-x86/QUILLPDF.DLL = 32768.
Gates: G-HOST, G-W32 with wine-pdf.sh (byte-equal PDFs present; absent behaviour), check-dll.sh,
check-imports.sh and scan-pentium.sh on the DLL, G-GTK's PDF test, G-DISK, make installer-w95, G-MD.
Budgets: QUILL32.EXE at least -10 KB (-14 to -20 KB expected; under -10 stop and report); QUILLPDF.DLL
<= 32,768. Decisions: W9X-136, INS-26, ARCH-42, CORE-97. Must not touch: crates/quill-app/src/**,
dos/**, win16/**, linux/** except its tests.
```

#### WP-MOD-11

```text
Package: WP-MOD-11 — the Windows 95 rows in the packed-size gate. Branch feat/w95-size-rows, worktree
~/source/worktrees/quill/mod-11. Needs pull request 191 (installer/budgets.toml, size-gate.py) merged.
Read modules.md 19.9 and INS-22. Add [executable.QUILL32] (file windows-x86/QUILL32.EXE; budget the size
at your branch point rounded up to 4,096; stop 524288) and the DLL rows keyed by target
(windows-x86/QUILLPIC.DLL 24576, windows-x86/QUILLPDF.DLL 32768, windows-x86/QUILLDLG.DLL 49152,
win16-x86/QUILLPIC.DLL with WP-MOD-6's figure if it has one, else 65536 with a comment). size-gate.py:
check each staged *.DLL in win16-x86 and windows-x86 against its row, refuse an unlisted one, add each
disk's required modules to its program bytes; keep the .DMD rows' bare names working. Tests in
test_size_gate.py for every new path (over, under, unlisted, a target with none). Update
docs/installer.md's gate section and record W9X-138 and INS-28 (Assumed). Gates: the test file, G-DISK,
G-MD, check-headers. No executable changes. Must not touch anything else.
```

#### WP-MOD-12 and WP-MOD-13

```text
WP-MOD-12 — QUILLDLG.DLL on Windows 95. Branch feat/w32-quilldlg. Needs WP-MOD-10 and WP-PREF-8 merged.
Define the Win32 DlgTableV1 { size, info, run(dialog, owner, module, model, model_len) } in quill-modabi
with layout tests and each moved dialog's #[repr(C)] model in quill-dialogs; move About, Fonts, Tabs and
the landed Options, Page Layout, Numbering, Header and Footer procedures into win9x/mods/quilldlg (base
0x61100000) with their templates in its own .res (res_pack.py writes both files); Find, Spelling and the
save question stay. Absent: the items greyed with the CORE-93 message, About as a MessageBoxA. Gates:
G-W32 with wine-dialogs.sh and m2-dialogs byte-equal, check-dll.sh, the per-module import list (USER32,
GDI32, COMCTL32 functions already on the program's list), G-DISK, make installer-w95. Budgets: report the
program's fall; QUILLDLG.DLL <= 49,152. Decisions: W9X-137, CORE-97.

WP-MOD-13 — the module set on ReactOS 0.4.16, Windows 98 SE and Me. Branch test/w32-modules-guests. Needs
WP-MOD-10 merged. Build the QAGRR image with the modules (site/try-reactos/qagrr, build-with-rl.sh) under
~/.cache (check df -h /home and free -g first), boot it headless, and check: the demo letter's picture,
Print to PDF to C:\Quill\OUT.PDF, QUILLPIC.DLL renamed (the grey box, the message, no system box), and
the Windows 3.x build from C:\Quill\WIN31. Then the same three by hand on the Windows 98 SE and Me VMs.
Add the guest checks to qagrr/check-state.sh; record the results in docs/testing.md and progress.md.
Evidence as modules.md 19.13 lists. Your own QEMU and Xvfb only; kill only your PIDs.
```

### 19.15 Owner decisions

| # | Question | Default taken | Where to swap |
|---:|---|---|---|
| 1 | Print to PDF on Windows 95 as a ticked component (`pdf32`), or unticked as DOS's `pdf` would be? | **Ticked** (it ships today) | INS-26; `installer/components.toml`'s `default` |
| 2 | `QUILLDLG.DLL` on Windows 95 at all, though it saves only 4 to 6 KB until Options and the editor dialogs land? | **Yes, last** (WP-MOD-12 after WP-PREF-8) | W9X-137; drop WP-MOD-12 |
| 3 | Where both Windows builds share a folder, separate folders (`C:\Quill\WIN31`) or a Win32 stem of its own? | **Separate folders**, one stem | INS-27; `qagrr.toml` and `quill_modabi::NAME_PIC` |
| 4 | The policy: release when the build id is clean, development when dirty (Win32; offered to DOS and Windows 3.x)? | **Yes** | W9X-135; `quill_w32rt::module` |
| 5 | A module panic ends the program (as today), or contain it first? | **Ends the program** | W9X-133; the export thunk |
| 6 | Win32 first as the proof (no CS-widening risk) instead of waiting for `QUILLPIC.DMD`? | **No**: WP-MOD-3 stays the proof; WP-MOD-9's first pull request starts now, its second follows WP-MOD-3 | Section 19.13's order; PR 2 would then own `quill-modpic` and the seam |
| 7 | `QUILL32.EXE`'s budget: the measured size rounded up, not lowered as modules take code out? | **Yes** (as DOS-52) | W9X-138; `installer/budgets.toml` |

### 19.16 Found while planning

- **The decoders are linked twice over.** `quill-pdfdoc` decodes pictures through `quill_picture::render` on its own, so taking the screen's decoding out of `QUILL32.EXE` frees nothing until the PDF walk goes through the same seam (19.2). The same will hold for any other caller (the clipboard's picture, Print Preview).
- **The Win32 link flags are workspace-wide.** `/ENTRY:start` and `/FIXED` sit in `win9x/.cargo/config.toml`'s `rustflags`, so the first `cdylib` in the workspace would be linked as a fixed program; they must move to the program's `build.rs` first (WP-MOD-9).
- **`#[no_mangle]` exports leak from a `cdylib`.** `DllMain` and `_fltused` were exported by the spike until they became assembly labels.
- **The ReactOS demo and its CI copy two files by name** (`QUILL32.EXE`, `QUILL.EXE`) into one folder, so the first module on either Windows build would vanish from the demo, and the two `QUILLPIC.DLL` files would collide there (19.9, INS-27). WP-MOD-6 should take the folder split if it lands before WP-MOD-9's second pull request.
- **The shared libraries plan** (liminal-installer) still says W9X-41 waits for `AGCORE1.DLL` and gives Quill a NO-GO on Win32; its section 9 can cite this plan's spike instead, and its verdict table should point here once this merges.
- **`QUILL32.EXE` carries 63,118 bytes of icons** in `.rsrc` (13 images), 14 % of the file: more than any module saves. Not a module question; worth a look by whoever owns the icon set.
