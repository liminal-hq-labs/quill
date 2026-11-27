# Optional modules

Quill keeps running into sizing notes: `QUILLDOS.EXE` has about 4.6 KB of its budget left, the Windows 3.x data segment about 1.9 KB, and the planned features (Preferences, Reveal Codes, Unicode, spelling, pictures, PDF) each carry a size row. The owner's idea (2026-11-27): use DLLs for the platforms to split out the work. This page plans that as one architecture with a binding per platform: an **optional module** is a cold feature built from shared `no_std` crates behind one C-style, versioned interface, packaged as an NE DLL on Windows 3.x, as a loadable `.DMD` file on DOS, and linked in statically everywhere else.

Written against `origin/main` at `b060d36`. It supersedes the DOS and Win16 verdicts of the earlier, unmerged `~/Documents/Liminal HQ/Quill/Quill modules and DLLs plan.md` (2026-11-25), keeps the decision ids that plan and the Liminal Installer's [shared libraries plan](https://github.com/liminal-hq-labs/liminal-installer/blob/main/docs/plans/shared-libraries.md) already use for Win16, and adopts Desk's `.DMD` container for DOS. Nothing here changes behaviour until a package lands, so `SPEC.md` and `docs/decisions.md` change with the packages.

**Status.** WP-MOD-1 is built: `crates/quill-modabi` (CORE-90) and Desk's `.DMD` crates in `dos/module/` (DOS-50), host only, with no executable changed. WP-MOD-2 (the loader) is next; nothing else in the work packages has started.

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

## 1. Bottom line

A module relieves a different limit on each platform, and only where the feature is **cold** (used now and then, not per key or per paint) and **narrow** (plain data in and out, no `App` across the boundary).

| Platform | What a module relieves | Verdict | First step |
|---|---|---|---|
| DOS | The `QUILLDOS.EXE` budget (DOS-70's stop at 480 KB) and the memory the program holds at start. **Not** the floppy: a module sits on the same disk | **GO**, with Desk's `.DMD` format (copied with provenance) and a loader in `dos/rt` | WP-MOD-1 to WP-MOD-3: `QUILLPIC.DMD` (PNG decoding) out of the EXE, end to end, with the installer |
| Windows 3.x | A second 64 KB data segment for a module's own working memory. **Not** the 1.9 KB left in `QUILL.EXE`'s segment: what fills it (the `App`, the stack, the UI text, the `Store` table) cannot move | **GO** for picture decoding: rust-win16's DLL runtime passed its go/no-go (rust-win16 pull request 6, D28 to D30) | WP-MOD-6: `QUILLPIC.DLL` from the same module crate as DOS, on the refreshed vendored packer |
| Windows 95 and later | Nothing: a PE image is paged on demand and the address space is flat | **NO-GO** (W9X-40); later only for an optional feature over 100 KB | — |
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
| `QUILL32.EXE` | 441,344 bytes with Print to PDF (34,304 of it `quill-pdf` and `quill-pdfdoc`) | No limit beyond the Windows 95 disk | shared libraries plan [V] |

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

A Win32, Linux, WinUI or web build links the logic crate statically through `Static` and never sees `quill-modabi` at run time; the host tests use `Static` too, plus a fake that answers `Missing` to test the absent path (CORE-96).

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

- **Windows 95 and later (W9X-40).** One `QUILL32.EXE`. A PE image is mapped and paged on first touch, so unused code costs nothing on an 8 MB machine; a DLL would add a second no-CRT link, relocations, a version check and an import scan, and lose LTO across the boundary. The Windows 95 disk has room for `QUILL32.EXE` at 441 KB. A DLL only for an optional feature over 100 KB, as an unticked component (W9X-41); the no-CRT Win32 DLL recipe will by then be proved by the savers' `AGCORE1.DLL` (shared libraries plan, WP-SL-1).
- **Linux, WinUI and the web.** No size limit applies; the logic crates link statically through each seam's `Static` implementation. The web demo's DOS page (`site/try/`, built by `scripts/build-try.sh`) ships the DOS build, so its bundle gains the `.DMD` files (WP-MOD-3), and `scripts/check-try.sh` checks they are there.

## 8. Which features go where

Savings are for `QUILLDOS.EXE`; Win16 figures are the data segment.

| Feature | DOS | Windows 3.x | Saving (DOS EXE) | Order |
|---|---|---|---|---|
| **Picture decoders** (PNG; JPEG, RLE4 later) | `QUILLPIC.DMD`, required (`appdos`); plain BMP stays in the EXE | `QUILLPIC.DLL`, required (`app16`) | PNG −4.0 KB [M] against the loader's +3 to +5 KB once: about −1 to +1 KB now; JPEG's +10 to 16 KB never enters the EXE | **1st** (the proof) |
| **File dialog** (Open, Save As, Insert Picture, both DOS screens) | `QUILLDLG.DMD`, required | n/a (common dialogs) | **−26 to −29 KB** [E from 31,274 movable, M], after +2 to +4 KB of dialog host in the EXE | **2nd** (the first real saving) |
| **Options** (Preferences WP-PREF-5, WP-PREF-6) | Joins `QUILLDLG.DMD` | In the EXE | −7 to −12 KB of the ledger's +8 to +14 | 3rd, as the dialog lands |
| **Page Layout, Numbering, Header and Footer** (WP-ED-17 to WP-ED-19) | Join `QUILLDLG.DMD` | In the EXE | −2 to −4 KB each of their +3 to +5 | As they land |
| **Help viewer** (`QUILL.HLX`, roadmap H2) | `QUILLHLP.DMD` in a new DOS `help` component (`default = true`, optional) | n/a (WinHelp) | Its whole size, about 10 to 20 KB [G], never in the EXE | When H2 makes `.HLX` |
| PDF writing | `QUILLPDF.DMD`, optional unticked component, if a plan asks for PDF on DOS | `QUILLPDF.DLL`, likewise | All of it (34 KB on Win32 [V]) | Later |
| Spelling engine | Stays in the EXE: 2.8 KB movable against 9.4 KB that is `App`'s | `QUILLSP.DLL` later (W16-47) | About −2 KB net: not worth a module | Later or never |
| **Reveal Codes** | Stays in the EXE: per key and per paint, caret and undo in `App` | In the EXE | — (it spends the room the dialogs free) | — |
| **Unicode U1 and U2** | Stays in the EXE: the text buffers and every caller | In the EXE; tables as resources | — | — |
| Preferences model (WP-PREF-2, `quill-settings`) | Stays in the EXE: every build reads it at start | In the EXE | — | — |
| RTF, QLD, layout, paint, print, autosave | Stay | Stay | — | — |

## 9. The installer

- **Private files in `{install}` (INS-20).** Each module is an ordinary `{install}/NAME` file in the component that owns its feature: `QUILLPIC.DMD` and `QUILLDLG.DMD` in `appdos`, `QUILLPIC.DLL` in `app16`. The Liminal Installer records each as `F name size crc32` in `INSTALL.LIR` and removes it on uninstall like the program (installer D111: a private DLL needs nothing new [V]).
- **Never `{windows}` or `{system}`.** The new placeholders (installer D95, merged for Windows 3.1 in installer pull request 70 [V]) are for files the system must find, such as the Ghostwriter screen saver. A module there would be found by any copy of Quill loading by bare name, its global Win16 module name would clash across copies, and D95's back-up-and-restore rule is wrong for a shared library (shared libraries plan section 11.2, whose `system-folder-library` check warns about exactly this). Quill loads by full path from its own folder (CORE-95), so it never needs them.
- **Unticked components (INS-21).** Only a feature that is optional by nature becomes a component of its own, with the installer's `default = false` (supported since installer D90 [V]) when it is a rarely wanted extra: a DOS `help` component (`QUILLHLP.DMD`, `QUILL.HLX`) ticked by default; `pdf` unticked. Pictures and dialogs are core features and stay in the required components; the absent-module message (CORE-93) covers a hand-made install.
- **In use.** Windows 3.x cannot replace a loaded module; the retro setup already asks the person to close Quill, and `GetModuleHandle("QUILLPIC")` is the check to add when the first DLL ships (shared libraries plan section 6.5 [U: not in the runtime yet]). DOS has nothing loaded during install.
- **Staging and checks.** `installer/tools/stage.sh` copies the modules into `dist/dos-x86/` and `dist/win16-x86/`; `verify-disks.sh` compares them with the builds; `e2e-dos.sh` and `e2e-w31.sh` check each is installed, listed in `INSTALL.LIR` and removed by uninstall, and that Quill draws a PNG afterwards.
- **Disks.** DOS +3 to 6 KB packed per module; Windows 3.x, the DLL's headers and its runtime copy, 2 to 4 KB packed [E]; Windows 95 unchanged.

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

## 13. Tests

| Level | What |
|---|---|
| Host | `quill-modabi` layout tests (`size_of`, `align_of`, `offset_of!` for every struct); the copied `desk-module` and `dmdpack` suites; `quill-modpic`'s session against the decoders' existing goldens and property tests (a PNG decoded through the session in slices of 1, 7 and all rows is byte-equal to `quill-image`'s own); the seam's `Static` and a `Missing` fake (the grey box and the message) |
| DOS, `rt-test` | `HELLO.DMD`: loaded at two addresses with equal output, a missing import refused by name, a bad CRC refused, 200 loads and frees with the heap's total unchanged, a panic through the host's path, the CS base check |
| DOS, Quill | Every `make dos` scenario byte-equal with `QUILLPIC.DMD` present; `m15-pictures` (or the picture golden) with the module absent shows the grey box and the message; a wrong-build module refused with the log line; the bench profile records the first-use load |
| Win16 | The rust-win16 gate (passed, D30), then the 2026-11-25 plan's gate 2: every PNG in `tests/docs` draws on Windows 3.11 at 256 and 16 colours and under otvdm; `m1-edit` byte-equal; the DLL missing, wrong build, two instances, 200 cycles |
| Installer | `e2e-dos.sh` and `e2e-w31.sh`: the module installed, listed in `INSTALL.LIR`, removed on uninstall; Quill draws a PNG after the install |
| Web demo | `scripts/check-try.sh` finds the `.DMD` files in the bundle |

## 14. Decisions

**Ranges.** Above every id on `main` and on the open branches (pull requests 160, 161, 168, 169, 170), checked on 2026-11-27. W16-40 to W16-49, W9X-40, W9X-41, DOS-50 to DOS-52 and CORE-90 to CORE-91 are the ids the 2026-11-25 plan and the Liminal Installer's shared libraries plan already name for this work (none is used in the repository), kept so those documents stay right; the rest extend them. Reserved: **CORE-90 to CORE-99**, **DOS-50 to DOS-59**, **W16-40 to W16-49**, **W9X-40 to W9X-41**, **INS-20 to INS-22**, **ARCH-40 to ARCH-42**. Each package copies its rows into `docs/decisions.md` (Status **Assumed** unless the owner has answered) and the SPEC in the same change.

| Id | Decision | Rejected | Why |
|---|---|---|---|
| CORE-90 | `quill-modabi`: the one definition of every module interface: `#[repr(C)]` structs of `u16` and `u32`, status codes, kinds, `ABI_MAJOR` and `ABI_MINOR`, the start block, chunk lists, per kind the ordinals (Win16) and the entry table (DOS) | Rust types or traits across; one interface per platform | Two loaders, one contract; layout tests catch drift |
| CORE-91 | Module logic works in caller-given buffers (a work block, row buffers), not large locals | The decoders' `MAX_SIDE` locals | Win16 needs an arena; DOS and the static builds give stack or heap; the pictures plan's P11 fallback needs it too |
| CORE-92 | A cold feature is written module-ready: a crate below `quill-app`, plain data in and out, a state machine the shell drives, its own texts | Retrofitting each feature when it overflows | Costs nothing at the start; makes a later move a small package |
| CORE-93 | An absent or refused module greys its feature with a message naming the file and Setup, never a system box or a crash; the reason is logged | Refusing to start; a loader box | A damaged install keeps a working editor |
| CORE-94 | Versions: the EXE and modules share a build id, required equal in release builds; `abi_major` refused if different, `abi_minor` accepted if higher; no version digit in the file names | `NAME1.DLL` naming (shared libraries rule 3) | Private modules always ship as a set; the build id is stricter |
| CORE-95 | Modules are found beside the program only, by full path | The current folder, `PATH`, the Windows folders | A stale copy elsewhere can never win |
| CORE-96 | One module crate, two packagings (`.DMD`, NE DLL), linked statically through the seam's `Static` on every other build and in the host tests | A module build on Win32, Linux, WinUI or the web | Nothing to relieve there; one code path for the tests |
| CORE-97 to CORE-99 | Spare | | |
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
| W9X-40 | One `QUILL32.EXE`; no DLL until an optional feature passes 100 KB | Codec, speller, engine DLLs | Paged images; nothing to relieve |
| W9X-41 | (Later) The Win32 module recipe, taken from the savers' proven `AGCORE1.DLL` recipe | — | Not rediscovered |
| INS-20 | Modules are private `{install}` files in the component that owns the feature; never `{windows}` or `{system}` | The system folder | Global module names, stale copies, D95's rule is for owned files |
| INS-21 | A module gets a component of its own only for an optional-by-nature feature (help ticked, PDF unticked with `default = false`) | Every module optional | Core features must not be missing |
| INS-22 | Spare | | |
| ARCH-40 | The module seam is a trait in the crate that owns the feature, with `Static` and module implementations; shells choose | A module registry in `App` | `App` stays unaware of packaging |
| ARCH-41 | Module packages are side-lane unless they move code out of `crates/quill-app/src`, which makes them ARCH-lane moves | A third lane | ARCH-6's rules hold unchanged |
| ARCH-42 | Spare | | |

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
| 11 | `QUILL32.EXE` stays one file? | **Yes** (W9X-40) | W9X-40 |
| 12 | Offer the hosted loader back to Desk as its M5? | **Yes**, after WP-MOD-2 merges | A Desk issue; not a Quill package |

## 18. Found while planning

- **The installer's shared libraries plan says DOS modules are a no-go** ("as Quill's modules plan found"); this plan reverses that for Quill. Its table row (section 1, the DOS and Linux line) should point here once this merges; the Win16 and Win32 verdicts agree.
- **Desk's hosted loader (M5) is not built** [V]; WP-MOD-2 is that loader for Quill, and the first real check of the CS-widening assumption Desk's design records as inferred.
- **The SPEC disagrees with itself on the DOS dictionary**: the build note under D88 says it is "read whole into memory when the first check begins", the dictionary section says it is "read in 32 KB pages, so only the pages a check touches are in memory" [V: `SPEC.md` lines 262 and 493]. Worth a one-line fix by whoever owns spelling next.
- **`[Quill] PictureMemory` defaults to 4,096 KB on DOS** [V: `SPEC.md` line 536], the whole of the 4 MB memory floor the SPEC promises; a 4 MB machine cannot give pictures that much. Worth checking when WP-MOD-3 measures memory.
- **Disk space.** `/home` had 19 GB free when this plan started, under the 20 GB floor builders keep, so no build was made; the figures come from an existing build. Builders of WP-MOD-1 and WP-MOD-2 should check before building.
