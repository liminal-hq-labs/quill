# The Win16 data segment

What fills the 64 KB automatic data segment of `QUILL.EXE`, what happens when it is full, and what can live in far memory instead. Decision D200 in `docs/decisions.md`. Measured on `origin/main` at `b93df66` with the build in `win16/build.sh` (packer from `rust-win16` D18 to D27), and again after the changes this page describes.

## Table of contents

- [Why one segment](#why-one-segment)
- [What fills it](#what-fills-it)
- [What moved to far memory](#what-moved-to-far-memory)
- [What cannot move](#what-cannot-move)
- [Pushing it](#pushing-it)
- [Pictures](#pictures)
- [Recommendations](#recommendations)

## Why one segment

A Windows 3.x program has one automatic data segment, at most 64 KB, and it holds the program's statics (`.rodata`, `.data`, `.bss`), the packer's shadow return stack, the stack and the local heap, in that order. Rust addresses everything through DS (and SS), so a near pointer is an offset into this segment and nothing else: a reference can never point into a `GlobalAlloc` block. Far memory is reached by copying (`far_read` and `far_write` load FS or ES for the length of one `rep movsb`) or by handing the block's far pointer to Windows.

The packer adds the sizes and refuses a program whose data, stack and heap pass 64 KB (`data N + stack N + heap N is over one 64 KB segment`), so a full segment is a build error and not a run-time one. What no check catches is the stack running down into the statics: see [Pushing it](#pushing-it).

## What fills it

`origin/main` at `b93df66`, from the packer's report (`data 31448 + stack 29696 + heap 4096` = 65,240 of 65,536 bytes, 296 free) and the linker's map (`-Map`, summed by section; `llvm-nm -S` for the statics). Sorted by size.

| Bytes | What | Where it comes from |
|---:|---|---|
| 29,696 | Stack (`STACK_SIZE` 0x7400) | NE header; 23,078 of it used by the `m1-edit` play-back, 19,046 by a windowed session |
| 11,414 | Read-only data (`.rodata`, 541 pieces) | Strings, menu and command tables (every `&str` is 8 bytes here), message texts, a 512-byte CP437 table (`u32` for each of 128 entries), 256-byte class tables, LLVM constant pools. 8,235 bytes of it are 487 unnamed pieces, 3,912 of them in pieces over 64 bytes. No piece is over 512 bytes, and there are no font width tables: GDI measures text |
| 11,060 | `quill::APP` | The `App` object (a `QApp`). By host `size_of` (11,504 there, pointers are wider): the document 4,976 (five stories of 800 bytes each, the four header and footer stories included, and the tables of fonts, colours and formats of 96 each), spelling state 2,976 (ignore list and user words), the clipboard 2,048, the recent files 288, names 64 + 64, four messages of 104 |
| 4,096 | Local heap (`HEAP_SIZE`) | NE header. Windows allocates the text of edit and combo controls from it |
| 3,076 | `w16::LOG` | The runtime's text buffer for `STACK.LOG` and the fault report (3,072 bytes and a length) |
| 1,030 | Shadow return stack | 6-byte header and 256 four-byte entries after the data (D20) |
| 1,024 | `quill::STORE` | The `FarStore` table: 128 blocks of handle, far pointer and length (160 and 1,280 bytes after this change) |
| 1,024 | `w16::fault::STACK` | The `/FAULT` handler's own stack |
| 768 | `quill::chrome::FACES` | 24 face names of 32 bytes for the face box |
| 540 | `quill::PLAY` | The play-back state, 512 bytes of it the staging buffer for the screen dump |
| 480 | `quill::gdi::FONTS` | The cache of 10 GDI fonts |
| 328 | `quill::SH` | The window shell's state |
| 256 | `quill::CMDS` | The 64 commands a `/PLAY` run queues |
| 100 | `quill::HERE` | The program's own directory |
| 96, 68, 32, 28 | `fault::FRAME`, `CHROME`, `fault::REGS`, a dozen small statics | |
| 16 | `w16::RESERVED` | Windows writes here |

The same, grouped: **stack 29,696** (45%), **statics 30,420** (47%: `.bss` 18,511, `.rodata` 11,414, `.data` 432, reserved 16, padding 47), **heap 4,096** (6%), **shadow stack 1,030** (2%).

## What moved to far memory

`examples/quill/far.rs` is the machinery: a moveable global block that is locked once (its selector is stable, as for the `Store` chunks), read and written through `far_read` and `far_write`, and stepped by `AHINCR` (8, KERNEL's equate 114) every 64 KB for a block that size.

| Item | Before | After | How |
|---|---:|---:|---|
| `w16::LOG` | 3,076 | 1,028 | Not far: the buffer is cut from 3,072 to 1,024 bytes. The longest thing it holds is the fault report, about 600 bytes. Far memory would not do, because the fault handler writes it |
| `FACES` | 768 | 12 | A 768-byte block; `face(i)` reads one name |
| `PLAY` staging buffer | 540 | 32 | The 512 bytes sit after the script's text in the block that already holds it, and go to the file with `_lwrite` from the far pointer |
| `picture::SLOT` | 0 | 132 | The picture cache's table; the pictures are in far blocks |

Data goes from 31,448 to 29,084 bytes, so the segment has 2,660 bytes free instead of 296 (stack and heap unchanged). It was 28,532 and 3,212 before the store's table grew from 128 to 160 blocks (512 bytes) and the store learned to say what it refused (40 bytes); the cases under [Pushing it](#pushing-it) were measured on that earlier build, so add 552 bytes to their static sizes.

The main frame is the other half of the stack. `w16_main` holds the message loop, so its frame lasts for the whole run, and inlining the start-up code into it put the 2 KB buffer of `read_faces`, the editor's file and settings buffers and the command-line scratch there for good: 2,778 bytes were gone before the first `STACK.LOG` sample. With `read_faces`, `chrome::init`, `create_combos`, `build_menu`, `load_script` and the editor's `open`, `load_settings` and `save_settings` each in a function of their own (`#[inline(never)]`), the frame is 512 bytes.

| Windows 3.11, `m1-edit` play-back, stack 0x7400 | Used (from the sample at `w16_main`) | Margin above the shadow stack |
|---|---:|---:|
| `origin/main`, 3 runs | 23,078 | 3,838 |
| After, 20 runs of the final build (and 40 of earlier ones) | 23,078 | 6,042 |
| After, a windowed session that opens `pictures.rtf` | 19,046 | 7,326 (before the main-frame change) |

The margin is the same in every run, so it is the deepest call chain and not chance. `check_stack_log.py` wants 2,048.

## What cannot move

- **`App` (11,060).** Every field is reached through a DS-relative reference, so `App` cannot be anywhere but near. The parts of it that are byte buffers behind methods could be Store chunks: the clipboard (2,048), the spelling lists (2,976) and the four header and footer stories (3,200 of the document's 4,976) are 8.2 KB. That is a change to `quill-app` and `quill-core` for every build, so it is a recommendation here and not part of the change.
- **Read-only data (11,414).** Strings and tables are addressed by near pointers from code. No piece is large; the one table worth a look is `ansi_to_cp437`'s 512 bytes (128 `u32`), 256 of which `u16` would give back.
- **The stack (29,696), the heap (4,096) and the shadow stack (1,030).** The heap is Windows': the edit part of the face and size combo boxes takes its text from the program's local heap, and the build was not tried with less. The shadow stack is the packer's.
- **`STORE` (1,024).** The table of the store's blocks (128, now 160) could be a far array, but every store call reads it, and each read would be a far copy of its own for one kilobyte.
- **The `/FAULT` handler (1,152).** It needs a stack that Rust can use, which is in DS. A build feature could leave it out of a release.

## Pushing it

Each case is the `m1-edit` play-back on Windows for Workgroups 3.11 in DOSBox-X, on a build of the final code with one constant changed.

| Case | What happened |
|---|---|
| A static of 3,213 bytes | The packer refuses the build: `data 31748 + stack 29696 + heap 4096 is over one 64 KB segment` (the data is rounded up to 4) |
| A static of 3,212 bytes (the segment exactly full, 65,536) | The build is accepted and **Windows refuses to run it**: "Insufficient memory to run this application" from Program Manager. The packer's limit is not the loader's |
| 32 bytes free | Runs, 3 of 3 with the golden output (a smaller margin was not tried, so the loader's own need is somewhere between 1 and 32 bytes) |
| Stack 1,126 bytes short of its floor (`STACK_SIZE` 0x5800) | Runs, 3 of 3, **with the golden output and file**. The stack went into the shadow return stack, whose entries only a call through a function pointer into another segment uses. Only `check_stack_log.py` noticed (margin 0) |
| Stack 3,174 bytes short (0x5000) | Runs, and **stops after the first screen**: the play-back's own state was in the statics the stack reached, so the script looked finished (6,098 of 231,309 bytes of dump, `=== end ===`). No fault, no message |
| Local heap 1 KB or 256 bytes (`HEAP_SIZE` 0x0400, 0x0100) | The window, the bars, the combo boxes and the pictures are drawn, and the Find dialog opens and takes typing ("Quill cannot find that text"). The 4 KB heap may be mostly unused; the other dialogs, the common dialogs, printing and Help were not tried, so it is left alone |

A full segment is therefore a build error at the packer's limit and a load error 32 bytes or less short of it, but a stack that is too small is silent: the program runs and corrupts whatever the stack reaches. The 2 KB margin that `check_stack_log.py` asks for is the whole defence, and it is measured at the store and drawing calls, so it is a lower bound on the depth reached and not a proof.

## Pictures

Windows 3.x pictures were grey boxes because the portable decoders cannot run here. What they keep in locals, from the source, with `MAX_SIDE` at 4,096: a BMP takes a raw row (12,288), an RGB row (12,288), its palette (768) and the scaler's accumulators and row (15,360), about 41 KB; a PNG takes two raw rows (32,769 each, swapped by copy), an RGB row (12,288), the inflater's 32,768-byte window from the caller and the same scaler, about 126 KB. The stack is 29,696 bytes with 6,042 spare, and the segment had 2,660 free. Nothing is near enough to adapt: the buffers are sized for the worst picture and the code indexes them directly.

A BMP is already a device-independent bitmap, so `picture.rs` does not decode it. It checks the header (40-byte header, bottom-up, 1, 4, 8 or 24 bits, plain or RLE8, pixels inside the file), copies the bytes after the 14-byte file header from the picture pool into a block of global memory (1 KB at a time through the stack), and hands it to `StretchDIBits` with the block's far pointer for the bits and the header; GDI scales it to the size the page shows. Three pictures and 1 MB are cached, least recently used first, and one picture bigger than the budget is tried alone. A block over 64 KB is a chain of selectors, which GDI follows for the bits (`far.rs` steps the selector for the copy). Nothing is near except the 1 KB buffer and the cache table (132 bytes).

What is not the same as the other builds: GDI scales with `COLORONCOLOR`, which drops lines and columns where the Win32 build averages them with a box filter, and the colours come from the display driver, not Quill's dither (SPEC, Display). PNG is still a grey box.

Windows for Workgroups 3.11 under DOSBox-X (S3 640 by 480, 32 MB, `fast.conf`), windowed, one run each, `PICS.LOG` and screenshots (`docs/media/win16-picture-*`):

| Document | Result |
|---|---|
| `tests/docs/pictures.rtf` | PNG grey box, 64 by 48 BMP drawn (`win16-picture-pictures-rtf-w311.png`) |
| 24-bit BMP 320 by 240 (230 KB) | Drawn (`win16-picture-bmp320-w311.png`) |
| 24-bit BMP 640 by 480 (922 KB, over 64 KB) | Drawn, clipped by the window as it is wider than it (`win16-picture-bmp640-w311.png`) |
| 24-bit BMP 800 by 600 (1.44 MB, over the 1 MB budget) and 1024 by 768 (2.36 MB) | Drawn, alone in the cache |
| 8-bit 320 by 240, 1-bit 320 by 240 (dithered by the converter) | Drawn (`win16-picture-1bit-w311.png`) |
| Three 24-bit BMPs of about 170 KB | The two in view drawn, the third was not asked for (`win16-picture-three-w311.png`) |
| 24-bit BMP 1088 by 816 (2.66 MB) | Drawn (this was reported as dropped in an earlier run, from a file name that was not 8.3, so the document never opened) |
| 24-bit BMP 1280 by 1024 (3.93 MB, 120 blocks) | Drawn (`win16-picture-1280x1024-24bit-w311.png`), with the store's table at 160 blocks; with 128 the reader drops it |
| 8-bit BMP 1280 by 1024 (1.31 MB) and 1600 by 1200 (1.92 MB) | Drawn (`win16-picture-1280x1024-8bit-w311.png`, `win16-picture-1600x1200-8bit-w311.png`) |
| 24-bit BMP 1600 by 1200 (5.76 MB) | Not in the document: over `MAX_PICTURE_BYTES` (4 MB), so the reader drops it with no grey box and no message (`win16-picture-over-4mb-w311.png`) |
| 4-bit BMP 320 by 240 (written by hand, not by the converter) | Drawn (`win16-picture-4bit-w311.png`) |
| RLE8 BMP 320 by 240 | Drawn, correctly (`win16-picture-rle8-w311.png`) |
| Top-down BMP (negative height), 24 bits | Grey box by design. Handed to `StretchDIBits` as it is, GDI answered with a non-zero count and drew nothing, so the shell would have shown a hole: it is refused first (`win16-picture-top-down-w311.png`) |
| PNG 320 by 240 | Grey box (`PICS.LOG`: "PNG is not decoded here") |

Where it stopped, with the same build:

- **Picture size.** Two limits, neither the data segment's. (1) `FarStore` had 128 blocks of 32 KB, and the picture pool takes one block per 32 KB, so a document holding a picture of 3.9 MB needs 128 blocks and a few more for the rest. `ByteBuf::append` met the full table (`store.rs`, `alloc`), the reader's `pict_hex` marked the picture failed and `finish_pict` truncated it, so the picture was dropped and nothing said so. The table is 160 blocks now (1,280 bytes, 512 more), enough for the 4 MB a picture may take. (2) `MAX_PICTURE_BYTES`, 4 MB, in `quill-core/src/doc.rs`: a larger picture is dropped by the reader in every build, silently; that was not changed. Both are the same code path the DOS and Windows 95 builds run, but they keep the document in `HeapStore` (a `Vec` of chunks with no table), so only the 4 MB limit applies to them; they were not run for this.
- **Silent no more, for the store.** `FarStore` records the first block Windows (or the table) refused, `PICS.LOG` says `store: 1 refused; the first with 61 blocks held, asking 32768 bytes, 0 KB free` (or `the table full`), and the shell puts `Picture too large for the memory available` on the status line after the open. The status bar is below the 640 by 480 screen in these runs, so the message was not seen on a screenshot (`win16-picture-store-full-w311.png` shows the page without the picture); that is not proven. A file that does not open is logged as well (`the file did not open: ...`).
- **A wrong turn.** The first 1280 by 960, 1280 by 1024 and 1600 by 1200 test files had names over eight characters, which DOS cannot open, so Quill started with an empty "Untitled" document and `PICS.LOG` said "no pictures asked for". The conclusions in an earlier version of this page about a 2.5 MB pool limit and a doubled store table were wrong for that reason.
- **Memory.** On the 386SX profile (`386sx-4mb.conf`, 4 MB) with Windows' paging file, pictures to 1024 by 768 draw (slowly: 7 minutes of wall clock against 30 seconds). With paging turned off (`Paging=off` in `[386Enh]`) a 320 by 240 picture draws, a 640 by 480 picture is a grey box (`GlobalAlloc refused 921654 bytes`, `win16-picture-no-memory-w311.png`), and so is a 1280 by 1024 8-bit one (1,311,798 bytes refused): the document holds the picture, and the cache's copy is what Windows refuses. A 24-bit 1088 by 816 picture does not get into the document: the store was refused at 60 blocks held, 15 KB free. A picture exists twice while it is drawn, once in the pool and once in the cache.
- **A 386.** On that profile the first PNG header stopped the program with "QUILL caused an Illegal Instruction in module QUILL.EXE at 0003:2C65": `bswap`, a 486 instruction, which LLVM emits for `u32::from_be_bytes` even for an `i386` target. `SPEC.md` says 386 or later. Four of them were in `QUILL.EXE`; the readers now build the value a byte at a time, and `check_cpu.py` fails the build on `bswap`, `cmpxchg`, `xadd`, `cpuid`, `rdtsc`, `cmov` and the like.
- **The stack.** Drawing a picture is not what is deepest: a windowed session with every picture drawn left 9,566 bytes of margin, against 6,042 for the `m1-edit` play-back.

What is proven: BMP pictures of 1, 4, 8 (plain and RLE8) and 24 bits, from 64 by 48 to 1600 by 1200 (8-bit) and 1280 by 1024 (24-bit), draw on Windows for Workgroups 3.11 in DOSBox-X at 640 by 480, one run each, and the stack margin and play-back above. What is not: PNG; RLE4 and OS/2-header bitmaps (grey boxes by design, not tried); 16 and 32 bits; a damaged RLE8 bitmap (GDI decodes it, and a bad file might fault it); another display driver or colour depth (only the S3 driver's 256 colours); Windows 3.1 without the Workgroups update; real hardware; printing a picture; scrolling a page with several large pictures; the DOS and Windows 95 builds with these files; and the pictures staying correct after the document is edited while they are cached (the cache is checked by a fingerprint of the picture's size, place and first bytes, as in the Win32 build).

## Recommendations

**What to move.** Anything that is bulk and reached by index or by copy: logs, face lists, staging buffers, tables of names, a bitmap's bytes. The helper (`far.rs`) is 100 lines and the cost is a `far_read` per access. This took 3.3 KB of statics off the segment and, with the start-up work out of `w16_main`'s frame, gave the stack 2.2 KB of margin it already owned.

**What to leave.** `App`, the read-only data, the heap until the other dialogs have been tried with a small one, and the `FarStore` table (one far read more for every store call, for 1 KB).

**What the layers should do about far memory.**

1. *`quill-core` and `quill-app`: no buffer over a few hundred bytes in a type that every platform keeps in a static.* The clipboard (2,048), the spelling lists (2,976) and the header and footer stories (3,200) are 8.2 KB that belong behind the `Store`, which already is the platform's far memory. That is the one change that would shrink `App` for Win16 and cost the other builds nothing.
2. *`quill-image`: take the working memory from the caller.* The decoders and the scaler own `[u8; MAX_ROW]`-sized arrays (41 KB for a BMP, 126 KB for a PNG) because the Win32 and DOS builds have them to spare. A `Scratch` the caller lends, sized by the picture's real width, and an inflater whose window is a trait (so a far block can be the ring) would let Win16 decode PNG in a few kilobytes of near memory, one row at a time into a far DIB. Until then, PNG on Win16 cannot be done by tuning.
3. *The shell: let the platform say a picture is already drawable.* A BMP needs no decode on Windows, and the platform's own draw (GDI, here) is smaller and faster than the portable path. `quill_app::picture` could offer the picture's bytes as a DIB, and a platform that cannot use them would decode as it does now.
4. *Tools: scan, don't trust.* `check_cpu.py` found what a 386SX profile run found; the packer's own limit (65,536) is not the loader's, so `build.sh` should keep a few dozen bytes of the segment free.

## Tooltips back in the link (W16-3)

From `f29ecaf` until W16-3 the data figures above were for a program whose tooltips could not run: `EXIT_TIMER` equalled `tips::TIMER`, so the tooltip branch of `WM_TIMER` was unreachable, the linker dropped the tooltip code and its text, and the window closed a moment after it opened. With the ids apart the tooltips are linked in: data 29,236 to 31,696 bytes (+2,460, mostly the tip strings), five more imports, 7 KB more code. The segment is then 31,696 + 29,696 stack + 4,096 heap = 65,488 of 65,536 bytes, 48 free. A windowed session on Windows 3.11 (`e2e-launch-w31.sh`) used 19,822 bytes of the stack and left a margin of 9,298 bytes. Any new static now needs something moved to far memory first.
