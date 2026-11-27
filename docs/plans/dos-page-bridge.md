# The DOS page bridge: the host features on Try Quill

A plan for giving the DOS Try Quill page (`site/try/`, js-dos 8.5.3 running FreeDOS 1.3 and `QUILLDOS.EXE`) the same host features the ReactOS page gets through the host link: the shared clipboard with formatting, files both ways, a printout as a PDF, the clock, Fit to window and a phone keyboard. The owner, 2026-11-27: "Can we bridge the DOS Try Quill page too so there's a complete feature set?" Written on 2026-11-27 against `main` at `4ff6973`, with WP-ED-5 (pull request 196), the host link spike (WP-HOST-1, pull request 197), the guest helper (WP-HOST-3, pull request 200) open, and the page clipboard (WP-HOST-4) and the DOS page's pointer fix (WP-PTR-3, branch `feat/dos-page-pointer`) running. Where this plan and [decisions.md](../decisions.md) disagree later, the decisions win.

## Table of contents

- [Recommendation](#recommendation)
- [What was measured](#what-was-measured)
- [The design](#the-design)
- [The features](#the-features)
- [Privacy and gestures](#privacy-and-gestures)
- [What is in the first demo](#what-is-in-the-first-demo)
- [Testing](#testing)
- [Budgets](#budgets)
- [Decisions to record](#decisions-to-record)
- [Rules for every package](#rules-for-every-package)
- [Order and parallelism](#order-and-parallelism)
- [Work packages](#work-packages)
- [Builder briefs](#builder-briefs)
- [Owner decisions](#owner-decisions)
- [Owner's check list](#owners-check-list)
- [Risks](#risks)
- [What was not verified](#what-was-not-verified)

## Recommendation

The host link's serial port cannot be reused: the vendored DOSBox has no serial device that reaches the page. What it does have is a file system the page can read and write (`ci.fsReadFile`, `ci.fsWriteFile`), and that file system holds the disk images FreeDOS boots from. So the bridge is **two small hard disks that the page and the guest share, each with one writer**, plus two DOS calls that make each side see the other's writes.

| Feature | Verdict | Mechanism | Cost in `QUILLDOS.EXE` | First demo |
|---|---|---|---|---|
| Transport | **Disk images, not a serial link** (NO-GO for serial) | `wdosbox.wasm` has only the dummy serial device; `C:` (the guest writes, the page reads) and `D:` (the page writes, the guest reads) are hard disk images mounted before `boot` | — | — |
| Clipboard, copy out of Quill (rich text and text) | **GO** | Quill started with `/W` serves its clipboard through files: Copy writes `C:\HOSTCLIP.RTF` and `.TXT` and flushes; the page, which saw the copy gesture, reads `C:` and writes HTML and text to the host clipboard | In the next row | If ready; the ReactOS page first |
| Clipboard, paste into Quill (rich text and text) | **GO** | The page catches Ctrl+V, takes the browser's `paste` event, writes `D:\PASTE.RTF`, `.TXT` and `.SEQ`, then sends Ctrl+V; Quill resets the disk cache and reads the newest clipboard | **+0.8 to +1.5 KB [E]** for both directions, gated at 1,536 bytes | If ready |
| Clipboard at the DOS prompt and in fields Quill does not paste into | **GO, plain text** | **Type my clipboard**: the page types the text through `ci.sendKeyEvent`, ASCII and Alt with keypad codes, at most 2,000 characters at 30 a second | 0 | If ready |
| Files into the PC | **GO** | Drop or pick a file: the page rebuilds `D:` with it under an 8.3 name; Quill's file dialog resets the cache before it lists a drive | Included above (the reset) | **Yes** (page only) |
| Files out of the PC | **GO** | A *Files on the PC* panel lists `C:` (Quill's working folder from now on) with **Download** | Included above (the flush after Save) | **Yes** (page only; the flush comes with `/W`) |
| Print to PDF | **GO, as a text PDF**; a formatted PDF **later** | DOS Quill prints to a file (plain text); the panel offers **Download as PDF** for a `.PRN` or `.TXT`, made in the page (Courier, 80 columns, form feeds as page breaks) | 0 | Yes, with files out |
| Time | **GO, the date only** (the time is already right) | The guest's time is the host's, but its date is **25 July 1998**; the page writes `D:\SETDATE.BAT` before the shell reads `D:`, and the start-up batch files call it | 0 | **Yes** |
| Fit to window and full screen | **GO** | Page only: the stage fills the window at 4 by 3, optionally at whole multiples; Full screen stays | 0 | Yes |
| Sound | **Skip** | Quill for DOS makes no sound | 0 | — |
| Phones and tablets | **GO** | WP-PTR-3's touch mode for the pointer; a **Keyboard** button that opens the phone's own keyboard on a hidden text field and types what it sends; **Paste from your computer** and **Copy to your computer** in the chip's menu | 0 | Yes, the button; the chip with the clipboard |
| Saved state | **Later** (off by default) | No js-dos persistence (it would store the whole 13 MB of images); *Keep my documents in this browser* would store `C:` alone (8 MB) in the origin-private file system and write it back before the shell reads it | 0 | No |
| Both display modes | **GO** | Nothing depends on the mode: the three start-up batch files pass `/W` and call the date file | 0 | — |

Build order: WP-DOSWEB-1 (disks, date, the page package's core) now, beside WP-PTR-3, adding its one hook in `try.js` after WP-PTR-3 merges; WP-DOSWEB-2 (files both ways and the text PDF); WP-DOSWEB-3 (Quill's `/W`, once WP-ED-5 has merged and before the Unicode freeze starts in the shells); WP-DOSWEB-4 (the page's clipboard, once WP-HOST-4 has merged); WP-DOSWEB-5 (phone keyboard and Fit to window); WP-DOSWEB-6 (the owner's check). WP-DOSWEB-7 (keep my documents) and WP-DOSWEB-8 (a formatted PDF) are later.

## What was measured

All on this machine on 2026-11-27, with the vendored js-dos 8.5.3 (`site/try/js-dos/`) served from `127.0.0.1` by a throwaway server and driven through `playwright-core` 1.65 in headless Chromium 156 (`/usr/bin/chromium`) and Playwright's Firefox. The FreeDOS floppy was a copy of a local `fl.img` with its start-up menu replaced, so these runs used `COMMAND.COM`, not Quill. Scripts, logs and shots are in `~/Documents/Liminal HQ/Quill/Web demo/DOS page bridge/spike/`; the spike itself was deleted.

### What js-dos 8.5.3 offers the page

- **The command interface.** The object js-dos hands to `onEvent("ci-ready", ci)` has `sendKeyEvent`, `simulateKeyPress`, `sendMouseRelativeMotion`, `sendMouseButton`, `screenshot`, `pause`, `resume`, `mute`, `persist`, `fsTree`, `fsReadFile`, `fsWriteFile`, `fsDeleteFile`, `events().onStdout` and `config`, among others. (A page must not keep it in a global called `ci`: `js-dos.js` defines a top-level function of that name.)
- **No serial port to the page.** `wdosbox.wasm` carries the C++ classes `CSerial` and `CSerialDummy` only; `nullmodem`, `modem` and `directserial` appear in its help text but their classes are not compiled in, and the `Module` exports (`_addKey`, `_mouseMove`, `_fs`, `_packFsToBundle`, `_rescanFilesystem`, `_networkConnect` and the rest) include nothing for a serial line. The IPX network (`networkConnect`) needs a WebSocket server and a packet driver in the guest; it was not pursued.
- **No shared folder once FreeDOS boots.** The page boots `fl.img` with DOSBox's `boot`, after which DOSBox's own drives are gone and the guest sees only BIOS disks. `fsTree()` shows the emulator's file system: `.jsdos/dosbox.conf` and the disk images.

### Disks the page and the guest share

| Question | Answer |
|---|---|
| A second floppy, `imgmount 1 x.img -t floppy` before `boot` | **No.** FreeDOS takes B: for the phantom of A: ("Insert diskette in drive B:") and the file lands on A: |
| Hard disk images, `imgmount 2 c.img -size 512,63,16,16 -t hdd -fs none` and `imgmount 3 d.img -size 512,63,16,64 ...` before `boot fl.img` | **Yes**: FreeDOS has C: (7 MB, FAT12) and D: (31 MB, FAT16). The test images' partition tables had no end CHS, so the kernel printed a "suspect partition" warning at boot; the real images must carry correct CHS values |
| The guest writes a file, the page reads the image | The directory entry shows at once, but **the last sector written stays in DOSBox's stdio buffer**: the file read as zeros for over 10 s, and an INT 21h AH=0Dh did not change that. **An INT 13h read of the same disk flushes it**: the data was there on the next read, in both browsers |
| The page writes the image, the guest reads it | **Only after INT 21h AH=0Dh.** Without it FreeDOS answered from its buffers (the old listing, "File not found"), even before the guest had touched the disk itself (the kernel reads every disk at start-up). After the reset the guest saw the page's file at once |
| The page's file and the guest's later writes on one disk, with a reset between | Both files there, `fsck.fat -n` clean. A page write over a disk the guest holds dirty buffers for was not tried; the design never does it |
| `fsReadFile` | 8 MB image 7 to 42 ms; 32 MB 31 to 71 ms; the 2.88 MB floppy 3 to 52 ms (five reads each, both browsers) |
| `fsWriteFile` | 10 MB 113 to 149 ms; 32 MB 65 to 158 ms |
| The bundle | Two empty images (8 MB and 32 MB) add about 39 KB to the zipped bundle (133,169 to 172,251 bytes) |

The two DOS calls used are tiny programs: `RESET.COM` is `B4 0D CD 21 C3` (INT 21h AH=0Dh) and `READC.COM` is `B8 01 02 B9 01 00 BA 80 00 BB 00 02 CD 13 C3` (INT 13h AH=02h, one sector of drive 80h into the program's own segment).

### Keys, the clock and saving

- **Typing.** `ci.sendKeyEvent` typed a 90-character line of letters, digits and every US punctuation key into `COPY CON` with no loss at 36 and at 66 characters a second. Alt held while keypad keys 320 to 329 (js-dos's codes) are pressed gave `é` and `½` on the screen, so the BIOS's Alt-and-keypad entry works.
- **js-dos's key listener** is on `window`, in the bubble phase, skips events whose target's `type` is `text`, and calls `preventDefault` on every key it takes, as v86 does. A capture-phase listener that stops the V of Ctrl+V keeps it from js-dos, and a hidden `<input type="text">` (not a `textarea`, whose `type` is `textarea`) is ignored by it.
- **The clock.** FreeDOS's time was the host's (5:37:51 pm against 17:37:54) but its **date was Saturday 25 July 1998**, in both runs. A batch file `D:\SETDATE.BAT` holding `DATE 11-27-2026`, written by the page before the shell read D: and called after a reset, set the date. Today Quill's Insert Date and every saved file's time stamp on the page say 1998.
- **`ci.persist(true)`** returned a 13,271,238-byte zip in 66 ms: both changed images whole (2,949,120 and 10,321,920 bytes), uncompressed.
- **Firefox.** js-dos runs in headless Firefox; WebGL falls back to the canvas renderer in both browsers.

### Quill for DOS today

- **Keys** come from INT 16h AH=00h (`dos/quill-dos/src/keys.rs`): the BIOS's 15-key buffer, so typing from the page must be paced. A byte from 20h to FFh is `Key::Char` in Windows-1252 (the document's code page); the text screen converts to code page 437 only to draw.
- **The clipboard.** With WP-ED-5, a shell that calls `serve_clipboard(true)` gets a `ClipRequest` from `take_clip_request` and answers it with `copy_to` and `paste_from` over any `quill_clip::Clipboard`; a shell that serves nothing keeps RTF and text (1 MB each) in the document store, where the page cannot see them. The DOS shell has `DosSource` and `DosSink` over DOS files and `dpmi::simulate_int` for real-mode interrupts already.
- **Printing** is File > Print, "Print to file or device": `print_text` writes the document as plain text to the name typed. DOSBox has no printer here, so only a file is useful.
- **Size.** `QUILLDOS.EXE` is 438,784 bytes against a budget of 442,368 on `main`; WP-ED-5 makes it 450,048 against 454,656 (DOS-115), leaving 4,608 bytes, and the Unicode plan already needs WP-MOD-4 to make room for WP-UNI-5.

## The design

### Three disks, one writer each

| Drive | Image | Size | Who writes | Who reads | What is on it |
|---|---|---|---|---|---|
| A: | `fl.img` (2.88 MB floppy, unchanged) | 2.88 MB | Nobody after the build (Quill's settings file may be written) | The guest | FreeDOS, `QUILL\` with the program, fonts, dictionary, samples; `SYNC.COM` |
| C: | `c.img`, FAT12 | 8 MB | **The guest only** | The guest; the page reads the image | Quill's working folder: documents saved, printouts, `HOSTCLIP.RTF` and `HOSTCLIP.TXT` |
| D: | `d.img`, FAT16 | 32 MB | **The page only** (rebuilt whole from its own list) | The guest, after a reset; the page never reads it back | Files dropped on the page, `PASTE.RTF`, `PASTE.TXT`, `PASTE.SEQ`, `SETDATE.BAT`, `NAMES.TXT` |

One writer per disk is what makes this safe without a protocol: the page never writes a disk whose buffers the guest may hold dirty, and the guest never writes the disk the page rebuilds. `D:` is not write-protected in DOS (a visitor at the prompt could copy a file onto it); the page's next rebuild drops such a file, which the help says. Both images are in the bundle, empty, so their geometry is fixed in `dosbox.conf` at boot.

### The flush rules

- **Before the guest reads `D:`**, it calls INT 21h AH=0Dh (disk reset), which empties FreeDOS's buffers. Quill does it before a paste, before its file dialog lists a drive, and when the Edit menu opens (to grey Paste). The start-up batch files call `A:\SYNC.COM` before they look for `D:\SETDATE.BAT`.
- **After the guest writes `C:`**, it calls INT 21h AH=0Dh (so FreeDOS writes its buffers) and then reads one sector of `C:` with INT 13h AH=02h (so DOSBox writes its own). Quill does it after a Copy or Cut, after Save and Save As, after Print to a file and after an autosave.
- **`SYNC.COM`** on A: does both (reset, then a one-sector read of drive 80h); about 20 bytes, made by `build-try.sh` from its bytes, not committed as a binary. The help tells a visitor at the prompt to run `SYNC` after copying a file to `C:`, or before listing `D:`.
- **The page cannot flush for the guest**, and an unflushed file looks complete from outside (its directory entry and size are written; only its last sector is missing). So the page marks a `C:` file as *still being written* when the bytes of its last sector inside its length are all zero, which no text, RTF or printout ever has, and offers it anyway. A file Quill wrote with `/W` is always flushed.

### Quill's `/W` switch

`QUILLDOS.EXE /W` (only the page's batch files pass it; nothing changes on a real PC):

- calls `serve_clipboard(true)` and answers each `ClipRequest` with a small `FileClipboard` in `dos/quill-dos/src/bridge.rs`: `put(Rtf)` and `put(Text1252)` write `C:\HOSTCLIP.RTF` and `C:\HOSTCLIP.TXT` through `DosSink`; `get` reads through `DosSource` from whichever is newer, `D:\PASTE.*` when `D:\PASTE.SEQ` holds a number Quill has not seen yet, otherwise its own `C:\HOSTCLIP.*` (so Copy then Paste inside Quill keeps working with the bridge on); `offers` checks which of those files exist after a reset. No DIB: a picture travels inside the RTF, as the held clipboard does.
- runs the flush rules above at the points listed;
- starts the Save As and Open dialogs in `C:\` (the remembered folder in `QUILL.INI` is set to `C:\` on the page's disk, so the code may need nothing here);
- costs no memory beyond the open file's buffer: the RTF reader and writer, the DOS file types and the interrupt helper are already in the program.

### The page

A typed TypeScript package, `site/try/packages/dos-bridge/`, laid out as `host-link` and `ac97` are (SITE-31, SITE-51): private, no runtime dependency, strict `tsc` with `noUncheckedIndexedAccess`, `bun test`, one committed built module `site/try/lib/dos-bridge.js` with a drift check, its own CI job.

```text
src/index.ts      attach(ci, options) -> DosBridge; the public types
src/fat.ts        FAT12 and FAT16 behind an MBR: read the root and folders, read a file, build an image from a list (pure, no DOM)
src/names.ts      long names to 8.3, collisions, reserved device names, the NAMES.TXT index
src/disks.ts      the only file that calls ci.fsReadFile and ci.fsWriteFile: read C:, rebuild D:, the before-the-menu write
src/keys.ts       typing: characters to js-dos key codes, Shift, Alt with keypad codes (Windows-1252), pacing, cancel
src/clip.ts       the DOS clipboard flows over host-link's clipboard state machine and converters (WP-HOST-4)
src/pdf-text.ts   a printout (plain text, form feeds) to a PDF in Courier
```

The glue, `site/try/bridge.js`, builds the UI from the package's events (the chip, the files panel, the drop zone, the Keyboard button) and is attached from one line in `try.js` on `ci-ready`. Every string is plain Canadian English in the glue. The converters (RTF to HTML, HTML to RTF, Windows-1252) and the clipboard state machine (gestures, the 5 s window, dedupe, the privacy rules) are **reused from `host-link`**, imported from its built module, so both pages behave the same and are tested once; `dos-bridge` adds only what is different about DOS.

## The features

Each feature says what the visitor gets, how it works, what it costs, its limits, how it is tested, its risks and its evidence.

### 1. The clipboard

**For the visitor.** The same chip as the ReactOS page, beside Restart and Full screen: *Clipboard: Shared*. Copy in Quill (Ctrl+C, Ctrl+Insert, Edit > Copy, the toolbar) and paste on the computer, with bold, italic, fonts, sizes, colours and lists. Copy on the computer and press Ctrl+V in Quill: it arrives formatted. At the DOS prompt, or in a Quill field that does not paste, **Type my clipboard** in the chip's menu types the text.

**Mechanism, out.** Quill with `/W` writes `C:\HOSTCLIP.RTF` and `.TXT` on Copy and flushes. The page records the gesture (Ctrl+C, Ctrl+X, Ctrl+Insert or Shift+Delete on the stage, or a pointer-up on it, which covers the menu and the toolbar) and reads `C:` at 250 ms, 700 ms, 1.5 s and 3 s after it, stopping when `HOSTCLIP.RTF`'s size, time or content hash changes. On a change inside the 5 s gesture window it writes `text/html` (from the RTF) and `text/plain` to the host clipboard; later, the chip offers **Copy to your computer**. A pointer-up reads `C:` at most once a second, at 500 ms and 1.5 s.

**Mechanism, in.** Ctrl+V (Shift+Insert, Cmd+V) on the stage is caught in the capture phase before js-dos sees the V; focus moves to a hidden `<input type="text">` (js-dos ignores it); the browser's `paste` event gives `text/plain`, `text/html` and `text/rtf` where offered. Unless the text equals what the page last copied out of Quill (then Quill's own `C:\HOSTCLIP.RTF` is newer and richer), the page rebuilds `D:` with `PASTE.RTF` (the host's RTF as it is, or the HTML converted), `PASTE.TXT` (Windows-1252, `?` for what it lacks, CR LF) and `PASTE.SEQ` (the next number, ASCII), then sends Ctrl+V down and up with `sendKeyEvent`. Quill resets the cache and pastes. **Paste from your computer** (the chip's menu) does the same from `navigator.clipboard.read()` and says "Your clipboard is in the PC: paste there", which is how Edit > Paste with the mouse and phones get it.

**Mechanism, typed.** **Type my clipboard** reads the host clipboard on that click and types it with `keys.ts`: printable ASCII through the US layout, Enter for line ends, Tab, and every other Windows-1252 character as Alt held with its decimal code on the keypad (Quill takes the byte as Windows-1252); characters outside Windows-1252 become `?`. At most 2,000 characters, at 30 a second, waiting whenever the page has sent 8 keys the guest may not have read; Esc on the stage stops it. Used when Quill is not running, when the bridge is off, or in a dialog field.

**Cost.** In `QUILLDOS.EXE`: the `FileClipboard`, the `/W` switch and the flush points, **+0.8 to +1.5 KB [E]**, gated at 1,536 bytes. On the page: `dos-bridge` 12 KB minified at most; the converters are `host-link`'s.

**Limits.** RTF and text 1 MB each out of Quill (the held clipboard's own limit); into Quill, RTF 4 MB and text 1 MB (`D:` has the room). No pictures on their own (inside RTF only; Quill for DOS has no DIB clipboard). Text mode and both graphics modes alike. The 5 s rule as on the ReactOS page.

**Tests.** Unit: `fat.ts` against images made by `mtools` (goldens), `keys.ts` (every Windows-1252 character to its key sequence), the flows over a fake `ci` and a fake `ClipboardPort` (the privacy rules as tests). Page level (headless Chromium and Firefox, a locally built bundle): Ctrl+A, Ctrl+C in Quill with the sample document, the host clipboard's HTML has the bold title; HTML written to the host clipboard, Ctrl+V in Quill, then copied out again, has its bold, italic and size; Copy then Paste inside Quill with the bridge on gives Quill's own text; **Type my clipboard** at the prompt into `COPY CON` gives the bytes back exactly; in all three display modes. LibreOffice imports the HTML (as WP-HOST-4's level 5 does).

**Risks.** Quill's Paste with the bridge on reads a file, so a paste is a disk read (a few milliseconds in DOSBox); a `D:` rebuild racing a Quill read of `D:` (the page writes only on a paste gesture, before it sends the V). The polling is bounded per gesture and costs at most 60 ms of the emulator's time per read.

**Evidence.** The chip in each state on this page, the host clipboard's HTML after a copy, Quill after a paste in each mode, timings.

### 2. Files into the PC

**For the visitor.** Drag a file onto the screen, or **Send a file to the PC** (needed on phones). It appears in `D:\`, and Quill's Open dialog shows it on drive D:. A note under the screen lists each file with its DOS name: *My letter (draft).rtf is D:\MYLETT~1.RTF*.

**Mechanism.** The page keeps the dropped files in memory and rebuilds `D:` whole with `fat.ts` from the empty template (the bundle's `d.img`), adding the files, `PASTE.*` if any, `SETDATE.BAT` and `NAMES.TXT`; `fsWriteFile` replaces the image. Quill's file dialog resets the cache before listing; at the prompt, `SYNC` then `DIR D:`.

**8.3 names** (`names.ts`). The name is folded to ASCII (accents dropped, so `é` gives `E`), upper-cased; every character other than letters, digits, `!#$%&'()-@^_{}~` and the backtick becomes `_`; the extension is the part after the last dot, cut to 3; the base is cut to 8; a name that collides (or was cut) becomes the first 6 characters, `~` and a number (`MYLETT~1.RTF`, then `~2`, and 5 characters with `~10`); a base that is a device name (`CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9`, `CLOCK$`) gets `_` in front; an empty base is `FILE`. `NAMES.TXT` lists every short name with the long name. Dropping a file with the same long name again replaces it.

**Limits.** 32 MiB a file (the ReactOS page's limit); `D:` holds about 31 MB in all and 500 files in its root; over either, the page refuses with the size and the limit and writes nothing. Folders dropped are refused. The files live in the tab's memory and go on Restart.

**Cost.** 0 in the program (the reset is the clipboard's).

**Tests.** `names.ts` against a table of names (accents, spaces, many dots, device names, collisions to `~10`); a dropped 1 MB and a 30 MB file arrive in the guest with their SHA-256 equal (read back by copying them to `C:` at the prompt and reading `C:` from the page); Quill opens a dropped `.RTF` from drive D:; over 32 MiB is refused.

**Risks.** A visitor at the prompt who lists `D:` without `SYNC` sees the old listing (the help says so).

### 3. Files out of the PC, and Print to PDF

**For the visitor.** A *Files on the PC* panel under the screen lists what is on `C:` (and the samples on `A:\QUILL\SAMPLES`, marked read only), with name, size, time and **Download**; it refreshes when opened, after a Save in Quill and after a print. A `.PRN` or `.TXT` file also has **Download as PDF**.

**Mechanism.** The page reads `C:` with `fsReadFile` and `fat.ts` and offers the file through a `Blob` and an `<a download>`; nothing leaves the tab except by that click. Quill with `/W` flushes after Save, Save As, Print to a file and autosave, and the start-up batch files make `C:\` Quill's working folder, so its Save As lands there. A printout is plain text from `print_text`; `pdf-text.ts` lays it out in Courier 10 point, 80 columns by 66 lines on Letter (A4 by the browser's locale), a form feed starting a page, Windows-1252 through the PDF's WinAnsi encoding, with no font embedded (Courier is a standard PDF font).

**Download names.** The DOS name, lower-cased (`letter.rtf`), or the long name from `NAMES.TXT` when the file came from `D:` first.

**Limits.** 64 MiB a file (the ReactOS page's limit, more than `C:` holds). A formatted PDF (fonts, pictures, layout) is not in this package: DOS Quill has no PDF writer in the program, and adding one would cost far more than the budget. **Later** (WP-DOSWEB-8): run Quill's own `quill-pdfdoc` built for WebAssembly in the page over the saved `.QLD`; an owner decision.

**Cost.** 0 in the program beyond the flush; `pdf-text.ts` about 3 KB.

**Tests.** A saved document downloads with its SHA-256 equal to the file in the image; a printout's PDF passes `qpdf --check` and `pdftotext -layout` gives the printout back; a file written at the prompt without `SYNC` is shown as *still being written*, and is shown complete after `SYNC`.

### 4. Time

**For the visitor.** Quill's Insert Date and the files' times show today, not 25 July 1998.

**Mechanism.** On `ci-ready`, before it presses the display key at FreeDOS's start-up menu, the page writes `D:` with `SETDATE.BAT` (`@DATE mm-dd-yyyy`, the visitor's local date in FreeDOS's default US format). The three start-up batch files run `SYNC` and then `IF EXIST D:\SETDATE.BAT CALL D:\SETDATE.BAT` before Quill starts. The time is already the host's. A session that passes midnight keeps counting in DOS. No time zone is involved: DOSBox gives the visitor's local time.

**Cost.** 0. **Tests.** The page with the browser's clock set through Playwright's `clock` to a known day: `DATE` at the prompt and Quill's Insert Date show that day. **Risks.** A FreeDOS country setting other than the US would read the date differently; the disk has none.

### 5. Fit to window and full screen

**For the visitor.** **Fit to window** (a checkbox beside Pointer speed, remembered) makes the screen as large as the window allows at 4 by 3, with an option for whole multiples only (crisper text at 2× and 3×); **Full screen** stays as it is.

**Mechanism.** Page only: the stage's size from the window, `renderAspect` and `imageRendering` unchanged; the pointer module re-reads the picture's size (WP-PTR-3 already watches the stage). The guest's mode does not change: DOS has no mode list to fit to, unlike the ReactOS helper's `ChangeDisplaySettingsA`.

**Cost.** 0. **Tests.** Shots at 1280 by 720, 1920 by 1080 and 390 by 844; no horizontal scroll at 390 pixels; the pointer's level 3 check from WP-PTR-3 still passes when fitted.

### 6. Sound

Skipped. Quill for DOS makes no sound and DOSBox's Sound Blaster and speaker stay silent. Nothing to build.

### 7. Phones and tablets

**For the visitor.** The pointer works as a trackpad (WP-PTR-3); the row of keys stays; a **Keyboard** button opens the phone's own keyboard and what is typed goes to Quill; the chip's menu has **Paste from your computer**, **Copy to your computer** and **Type my clipboard**; **Send a file to the PC** picks a file.

**Mechanism.** The Keyboard button focuses a hidden `<input type="text">` (`autocomplete`, `autocorrect`, `autocapitalize` and `spellcheck` off; `inputmode="text"`), which js-dos ignores; `beforeinput` events become keys through `keys.ts` (`insertText` types the characters, `deleteContentBackward` is Backspace, `insertLineBreak` is Enter); the field is cleared after each event so predictive text has nothing to rewrite. This is the DOS page's form of SITE-46.

**Cost.** 0. **Tests.** Playwright's mobile emulation (390 by 844, touch) types into `COPY CON` through the field; the owner's phone check.

### 8. The display modes and the saved state

- **Modes.** The three start-up batch files (`QUILLG.BAT`, `QUILL16.BAT`, `AUTOEXEC.BAT`) each gain the same two lines and `/W`; every feature works the same in all three. Tests run each flow once per mode.
- **Saved state.** The page keeps js-dos's persistence off (`fsChanges.local: false`, SITE-4): `persist()` stores the whole images, 13 MB, and a new build could not use them. **Later** (WP-DOSWEB-7, off by default): *Keep my documents in this browser* stores `c.img` alone in the origin-private file system, outside js-dos's per-build folder, writes it back before the start-up menu key (the same moment as the date), with a **Forget my documents** button. Because `C:` is the visitor's disk only, it survives a new build.

## Privacy and gestures

The ReactOS page's rules (SITE-34, written by WP-HOST-4) apply unchanged, through the same state machine:

1. **Nothing leaves the tab.** The bridge makes no network request. The emulator's file system is in the tab's memory; files dropped and clipboard contents live there until Restart, Stop or the tab closes, and are never written to browser storage (until *Keep my documents* is turned on, and then only `C:`, in this browser).
2. **The host clipboard is read only on a paste gesture** (Ctrl+V, Shift+Insert or Cmd+V on the selected screen, through the `paste` event) or a click on **Paste from your computer** or **Type my clipboard**. Never on a timer, focus or load; the guest has no way to ask for it.
3. **The host clipboard is written only after the visitor's own copy** in Quill (a gesture under 5 s old, one write per gesture) or a click on **Copy to your computer**.
4. **A visible indicator** (the chip) and **a setting** (**Share the clipboard**, on by default and remembered; `?clipboard=off`; `?bridge=off` turns off the clipboard, the disks' use and the date and leaves today's page).
5. **Reading `C:`** after a gesture is the page reading its own emulator's memory, not the visitor's computer; it happens only after a copy gesture, a pointer-up on the screen or the files panel opening.

## What is in the first demo

The first demo is the ReactOS page (the [launch checklist](first-demo-launch.md)); the DOS page is not pinned, and every merge to `main` deploys it with the next Pages build. So each package must leave the page working on its own, and `?bridge=off` is the way back. The owner wants a complete set; what is cheap enough to land with the launch:

| Item | With the launch | Why |
|---|---|---|
| The date fix and the two disks (WP-DOSWEB-1) | **Yes** | Page and bundle only; fixes a visible 1998 date |
| Files both ways and the text PDF (WP-DOSWEB-2) | **Yes** | Page only. Until `/W` lands, the last sector of a file Quill has just saved can still be in DOSBox's buffer; the panel marks such a file and says to save again (the next write to `C:` flushes the earlier one) |
| Quill's `/W` (WP-DOSWEB-3) | **If WP-ED-5 has merged** in time | Small, but it waits for the editor track |
| The page's clipboard (WP-DOSWEB-4) | **If WP-HOST-4 has merged**; otherwise the week after | It reuses the ReactOS page's clipboard code |
| Keyboard button and Fit to window (WP-DOSWEB-5) | **Yes** if WP-PTR-3 has merged | Page only |
| Keep my documents (WP-DOSWEB-7), a formatted PDF (WP-DOSWEB-8) | Later | Not needed for the demo's story; each is an owner decision |

## Testing

| Level | What runs | Where | Checks |
|---|---|---|---|
| 1. Units | `bun test` in `dos-bridge` | Local and CI (a *DOS bridge tests* job) | `fat.ts` reads images `mtools` made and writes images `mtools` and `fsck.fat -n` accept (goldens under `tests/dos-bridge/`), FAT12 and FAT16, every split of a cluster chain; `names.ts`; `keys.ts`; `pdf-text.ts` (the PDF's objects and offsets); the clipboard flows over a fake `ci` and fake clock, gestures and `ClipboardPort` |
| 2. Quill in DOSBox | `dos/tools/e2e.sh` with a scenario that starts `QUILLDOS.EXE /W` with `C:` and `D:` mounted as DOSBox drives (`mount`, not a booted FreeDOS) | Local and the `ci-dos` image | Copy writes `C:\HOSTCLIP.RTF` equal to the golden; a `D:\PASTE.RTF` with a new `PASTE.SEQ` pastes; without a new number Quill pastes its own copy; the screens equal the host's |
| 3. The page | A local bundle (`scripts/build-try.sh --build`), the page served on 127.0.0.1, headless Chromium (clipboard permissions granted) and Playwright's Firefox (a mocked `navigator.clipboard` and synthetic `paste` events where it would prompt) | Local | Every flow in this plan, in all three display modes; the date; files in and out with their sums; the text PDF; the phone keyboard in mobile emulation; `?bridge=off` gives today's page |
| 4. The owner | [The check list](#owners-check-list) | Real browsers, LibreOffice or Word, a phone | What headless runs cannot show |

Every browser is headless; servers bind 127.0.0.1; every command that starts a browser or DOSBox sets `DISPLAY` with `WAYLAND_DISPLAY` unset; every process is stopped by its own PID.

## Budgets

| What | Budget |
|---|---|
| `QUILLDOS.EXE`, the `/W` code | At most **1,536 bytes**; over it, the builder stops and reports (the fallbacks: the clipboard by typing only, or the file clipboard in a `.DMD` module once the loader exists). Gated by `installer/budgets.toml`, raised once by its measured cost in DOS-121 |
| `dos-bridge` built module | At most 16 KB minified, no runtime dependency |
| The bundle | At most +64 KB zipped for the two images and `SYNC.COM` |
| Copy out, 1 KB of text | Host clipboard written within 1 s of the Ctrl+C keydown (Chromium, graphics 256) |
| Paste in, 1 KB of text | Text in Quill within 1 s of the keydown; 100 KB of RTF within 2 s |
| Reading `C:` | At most 60 ms a read (p95 of 20), at most 4 reads a gesture |
| Typing | 30 characters a second, nothing dropped over 2,000 characters |

## Decisions to record

This plan reserves **SITE-60 to SITE-69** and **DOS-120 to DOS-123** (SITE-54 to SITE-57 and DOS-115 are taken by open pull requests 198 and 196). This plan writes SITE-60 (**Assumed**); each package writes its own rows when it opens its pull request.

| Id | Decision | Written by |
|---|---|---|
| SITE-60 | The DOS page's bridge is two shared hard disks with one writer each (`C:` the guest's, `D:` the page's) and the flush rules; no serial link (js-dos has none) | This plan (**Assumed**); WP-DOSWEB-1 amends it with what it built |
| SITE-61 | The page side is the `dos-bridge` TypeScript package, laid out as `host-link`, reusing `host-link`'s converters and clipboard state machine | WP-DOSWEB-1 |
| SITE-62 | The guest's date comes from the page through `D:\SETDATE.BAT`, written before the start-up menu key | WP-DOSWEB-1 |
| SITE-63 | Files in land in `D:\` under 8.3 names with `NAMES.TXT`; files out are listed from `C:` and downloaded on a click; a printout downloads as a text PDF | WP-DOSWEB-2 |
| SITE-64 | The DOS page's clipboard flows: the caught Ctrl+V into `D:\PASTE.*`, the copy gesture's reads of `C:\HOSTCLIP.*`, **Type my clipboard**, the privacy rules of SITE-34 | WP-DOSWEB-4 |
| SITE-65 | The Keyboard button on touch screens and Fit to window on the DOS page | WP-DOSWEB-5 |
| SITE-66 | Keep my documents (if the owner takes it) | WP-DOSWEB-7 |
| SITE-67 to SITE-69 | Spare, for the owner's check and WP-DOSWEB-8 | WP-DOSWEB-6, WP-DOSWEB-8 |
| DOS-120 | `/W`: Quill serves its clipboard through `C:\HOSTCLIP.*` and `D:\PASTE.*`, resets before it reads `D:` and flushes after it writes `C:` | WP-DOSWEB-3 |
| DOS-121 | The budget raise for `/W`, by its measured cost | WP-DOSWEB-3 |
| DOS-122, DOS-123 | Spare | — |

## Rules for every package

- **One package, one branch, one worktree** under `~/source/worktrees/quill/<topic>`, from `origin/main`. `~/source/experiments/quill16` is used only for `git fetch` and `git worktree add` and `remove`.
- **Browsers and DOSBox** run headless or on a private display (`env -u WAYLAND_DISPLAY DISPLAY=:<n> …` in every command, a display number of your own), with PIDs recorded and stopped by those PIDs; never `pkill` or `killall` by name; never the host desktop. Servers bind 127.0.0.1 on a port of your own; ports 8095 and 8098 are the owner's and are left alone.
- **Disk.** Keep `df -h /home` above 20 GB; check before a bundle build. Bundles and images go under `~/.cache/quill16-try/` or `$QUILL_TRY_DIR`; `site/try/quill.*.jsdos` and `try-manifest.json` are build outputs and never committed. Never `git add -A`.
- **No publishing.** No release, tag, upload or Pages dispatch; push only the package's branch. Remember that every merge deploys the DOS page.
- **Shared files.** `site/try/try.js` belongs to WP-PTR-3 until it merges; after that the bridge adds one hook line (attach on `ci-ready`, detach on stop) and nothing else there. `site/try/index.html` and `site/css/try.css`: one marked block per package. `scripts/build-try.sh`, `scripts/try-dosbox.conf` and `scripts/try-disk/*`: WP-DOSWEB-1, except that WP-DOSWEB-3 adds `/W` to the batch files' Quill line. `dos/quill-dos/src/main.rs`: WP-DOSWEB-3's dispatch lines only, the rest in `bridge.rs`. `host-link`'s files are WP-HOST-4's: the DOS page imports its built module and adds an export there only as an additive change listed in the pull request. `site/try/js-dos/**` is never changed.
- **Evidence** goes to `~/Documents/Liminal HQ/Quill/Web demo/DOS page bridge/<package>/`; the few shots that document a feature are committed under `docs/media/` as `web-dos-*.png`.
- **Gates.** `bash scripts/check-headers.sh`, markdownlint in Docker, `python3 scripts/check-ids.py`, `scripts/check-try.sh`, `python3 -I scripts/stamp-try.py --check`, `python3 -I scripts/check-site.py`, the package's `bun test`, type check and drift check; `make dos` and `dos/tools/size-report.sh` when `dos/**` changes.

## Order and parallelism

| # | Package | Starts when | Runs beside |
|---|---|---|---|
| 1 | **WP-DOSWEB-1** The disks, the date and the package core | Now (the `try.js` hook after WP-PTR-3 merges) | WP-PTR-3, WP-HOST-4 |
| 2 | **WP-DOSWEB-2** Files both ways and the text PDF | WP-DOSWEB-1 merged | WP-DOSWEB-3 |
| 3 | **WP-DOSWEB-3** Quill's `/W` | WP-ED-5 (196) merged; before WP-ED-10 opens the Unicode freeze | WP-DOSWEB-2 |
| 4 | **WP-DOSWEB-4** The page's clipboard | WP-DOSWEB-1 and WP-HOST-4 merged (its level 3 runs with Quill need WP-DOSWEB-3) | WP-DOSWEB-5 |
| 5 | **WP-DOSWEB-5** Keyboard button and Fit to window | WP-PTR-3 and WP-DOSWEB-1 merged | WP-DOSWEB-4 |
| 6 | **WP-DOSWEB-6** The owner's check and its fixes | 2 to 5 merged and served to the owner | — |
| 7 | WP-DOSWEB-7 Keep my documents | The owner says yes | Any |
| 8 | WP-DOSWEB-8 A formatted PDF from DOS | The owner says yes | Any |

**With the cap of three builders** (WP-PTR-3, WP-HOST-4 and one floater running now): WP-DOSWEB-1 takes the next free slot; WP-DOSWEB-3 goes in the slot WP-ED-5's builder leaves, ahead of the freeze; WP-DOSWEB-2 and WP-DOSWEB-4 follow as WP-PTR-3 and WP-HOST-4 finish; WP-DOSWEB-5 fits beside them. The launch's own packages (WP-HOST-3 to WP-HOST-7, WP-WEB-8, WP-LAUNCH-*) keep priority: the DOS page is not on the launch's critical path.

## Work packages

### WP-DOSWEB-1 The disks, the date and the package core

- **Finish line.** The bundle carries `c.img` (8 MB, FAT12, label `QUILLDOCS`) and `d.img` (32 MB, FAT16, label `FROMPC`) with correct MBR geometry (no "suspect partition" warning at boot) and `A:\SYNC.COM`, mounted as C: and D: by `try-dosbox.conf`; the three start-up batch files run `SYNC`, call `D:\SETDATE.BAT` if it is there, make `C:\` the working folder, and start Quill as today (no `/W` yet); `dos-bridge`'s `fat.ts`, `names.ts`, `disks.ts` and `index.ts` pass their unit tests and the type check, and the built module passes its drift check; on `ci-ready` the page writes `D:` with `SETDATE.BAT` before the display key, and FreeDOS's `DATE` shows the browser's day in all three modes; the diagnostics line under the screen (or the build line's title) shows the bridge's state; `?bridge=off` gives today's page.
- **Files owned.** `scripts/build-try.sh` (the two images, `SYNC.COM` from its bytes, the zip), `scripts/try-dosbox.conf`, `scripts/try-disk/AUTOEXEC.BAT`, `QUILLG.BAT`, `QUILL16.BAT`, `QUILL.INI` (the remembered folder, if Quill reads one), `scripts/check-try.sh` (the new images and their sizes), new `site/try/packages/dos-bridge/**` (package files, `src/fat.ts`, `src/names.ts`, `src/disks.ts`, `src/index.ts`, tests), new `tests/dos-bridge/**` (the `mtools` goldens and the script that makes them), new `site/try/lib/dos-bridge.js` (built), new `site/try/bridge.js` (attach, date, state), one hook line in `site/try/try.js` and one script tag in `site/try/index.html` (after WP-PTR-3 merges), `scripts/stamp-try.py` (stamping the two new files), `.github/workflows/ci.yml` (one *DOS bridge tests* job), new `site/try-reactos/harness/check-dos-bridge.ts` (level 3: date, disks), `site/try/js-dos/PROVENANCE.md` (a line saying the page uses `fsReadFile` and `fsWriteFile`, to check after an update), `site/README.md` (*The DOS page's disks*), `docs/decisions.md` (SITE-60 amended from **Assumed** to **Decided** with the built details, SITE-61, SITE-62).
- **Must not touch.** `site/try/js-dos/**` (except that one PROVENANCE line), the pointer code in `try.js`, `site/js/pointer.js`, `site/try-reactos/**` except the new harness script, `dos/**`, `crates/**`, `host-link`'s package.
- **Dependencies.** None to start. The `try.js` hook and the script tag wait for WP-PTR-3 to merge (rebase then).
- **Tests.** Level 1 for the four files; level 3 for the date and the disks in Chromium and Firefox; `check-try.sh`, `stamp-try.py --check`, `check-site.py`, headers, markdownlint, `check-ids.py`.
- **Evidence.** The boot screen with C: and D: and no warning, `DATE` in each mode, the test counts, the bundle's size before and after, into `~/Documents/Liminal HQ/Quill/Web demo/DOS page bridge/WP-DOSWEB-1/`.
- **Parallel.** With WP-PTR-3 and WP-HOST-4.

### WP-DOSWEB-2 Files both ways and the text PDF

- **Finish line.** A file dropped on the stage or picked with **Send a file to the PC** is in `D:\` under its 8.3 name with `NAMES.TXT` updated and its SHA-256 equal (checked by copying it to `C:` at the prompt and reading `C:`); over 32 MiB or over `D:`'s room is refused with the size and the limit; the *Files on the PC* panel lists `C:` and the samples, downloads a saved `.RTF` with its SHA-256 equal, and makes a PDF of a printout that passes `qpdf --check` with `pdftotext -layout` giving the text back; a file still being written is marked so; the help text under the screen explains `C:`, `D:` and `SYNC`.
- **Files owned.** `dos-bridge`'s `src/files.ts`, `src/pdf-text.ts` and tests, the files part of `bridge.js`, the panel's and drop zone's blocks in `index.html` and `try.css`, new `harness/check-dos-files.ts`, `site/README.md` (*Files on the DOS page*), `docs/decisions.md` (SITE-63), `docs/media/web-dos-files.png`.
- **Must not touch.** `fat.ts` and `disks.ts` beyond additive changes with tests, the clipboard, `dos/**`.
- **Dependencies.** WP-DOSWEB-1 merged.
- **Tests.** Level 1 for `files.ts` and `pdf-text.ts`; level 3 for both directions and the PDF.
- **Evidence.** The panel with a download and a PDF, the folder in Quill's Open dialog on D:, timings.
- **Parallel.** With WP-DOSWEB-3.

### WP-DOSWEB-3 Quill's `/W`

- **Finish line.** `QUILLDOS.EXE /W` serves its clipboard through a `FileClipboard` (`C:\HOSTCLIP.RTF` and `.TXT` out, `D:\PASTE.*` in when `PASTE.SEQ` is new, its own copy otherwise), greys Paste by what the files offer, resets before reading `D:` and before its file dialog lists a drive, and flushes (reset, then a one-sector INT 13h read) after Copy, Cut, Save, Save As, Print to a file and autosave; without `/W` nothing changes (every existing DOS scenario's screens are equal); the start-up batch files pass `/W`; the cost is measured and at most 1,536 bytes, and `installer/budgets.toml` is raised by it (rounded up as DOS-70's rule says) in DOS-121.
- **Files owned.** New `dos/quill-dos/src/bridge.rs`, `dos/quill-dos/src/main.rs` (the switch and the dispatch lines only), `dos/quill-dos/src/files.rs` (a flush call after a save, if that is where saves end), new `tests/scenarios/dos-bridge-*.keys` and their goldens, `dos/tools/e2e.sh` (the `C:` and `D:` mounts for those scenarios only), `scripts/try-disk/*.BAT` (`/W`), `installer/budgets.toml`, `docs/dos-graphics.md` or the DOS section of SPEC (the switch), `docs/clipboard.md` (the DOS bridge row), `docs/decisions.md` (DOS-120, DOS-121).
- **Must not touch.** `crates/quill-app/**` and `crates/quill-clip/**` (a needed change there is a question to the editor track), `site/**` except the batch files' `/W`.
- **Dependencies.** WP-ED-5 merged; must merge before WP-ED-10 opens the freeze, or wait until the freeze ends.
- **Tests.** Level 2 (the scenarios), `make dos`, `dos/tools/size-report.sh` before and after, G-HOST for anything shared.
- **Evidence.** The size before and after, the scenario frames, `HOSTCLIP.RTF` opened in LibreOffice.
- **Parallel.** With WP-DOSWEB-2.

### WP-DOSWEB-4 The page's clipboard

- **Finish line.** On a local bundle with `/W`: the flows of [The clipboard](#1-the-clipboard) work in headless Chromium and Firefox in all three modes (level 3); the privacy rules hold as tests over the shared state machine; the chip and its menu (the ReactOS page's, with **Type my clipboard** added) work by keyboard and have screen-reader names; `?clipboard=off`, `?bridge=off` and the setting turn it off; the budgets for copy and paste hold.
- **Files owned.** `dos-bridge`'s `src/clip.ts`, `src/keys.ts` and tests, the clipboard part of `bridge.js`, the key-catching hook (in `bridge.js`, attached to `window` in the capture phase; no change in `try.js` beyond WP-DOSWEB-1's line), the chip's block in `index.html` and `try.css`, new `harness/check-dos-clipboard.ts`, `site/README.md` (*The DOS page's clipboard*), `docs/decisions.md` (SITE-64), `docs/media/web-dos-clipboard.png`; an additive export in `host-link`'s index for the state machine and converters, if it has none, listed in the pull request.
- **Must not touch.** `host-link`'s behaviour, `dos/**`, `fat.ts` beyond additive changes, the pointer code.
- **Dependencies.** WP-DOSWEB-1 and WP-HOST-4 merged; WP-DOSWEB-3 for the runs with Quill (a stub that writes `C:\HOSTCLIP.*` at the prompt stands in until then).
- **Tests.** Levels 1 and 3; the LibreOffice import check of WP-HOST-4 on the DOS page's HTML.
- **Evidence.** The chip in each state, the level 3 table with timings per mode.
- **Parallel.** With WP-DOSWEB-5.

### WP-DOSWEB-5 Keyboard button and Fit to window

- **Finish line.** On a touch screen (Playwright mobile emulation at 390 by 844) a **Keyboard** button types into `COPY CON` and into Quill, Backspace and Enter included, with predictive-text edits not doubling characters; **Fit to window** fills the window at 4 by 3 (and at whole multiples when chosen), remembered, with no horizontal scroll at 390 pixels and the pointer's checks passing when fitted.
- **Files owned.** The keyboard and fit parts of `bridge.js` (using `keys.ts`), their blocks in `index.html` and `try.css`, new `harness/check-dos-touch.ts`, `site/README.md` (*Phones and the window*), `docs/decisions.md` (SITE-65).
- **Must not touch.** The pointer module and its adapter, the clipboard.
- **Dependencies.** WP-PTR-3 and WP-DOSWEB-1 merged.
- **Tests.** Level 3 in mobile emulation and at three window sizes.
- **Evidence.** Shots of the phone layout with the keyboard field focused, and fitted at 1920 by 1080.
- **Parallel.** With WP-DOSWEB-4.

### WP-DOSWEB-6 The owner's check and its fixes

- **Finish line.** Every item of the owner's check list is yes, or has a recorded reason and a fix merged; SITE-67 records what the check changed.
- **Files owned.** The constants and strings of `bridge.js` and `dos-bridge`, the help text, `docs/decisions.md` (SITE-67).
- **Dependencies.** WP-DOSWEB-2 to WP-DOSWEB-5 merged and served to the owner.

### WP-DOSWEB-7 Keep my documents (later)

- **Finish line.** With *Keep my documents in this browser* on, a document saved in Quill is there after a reload and after a new build; **Forget my documents** removes it; off by default; nothing stored when off.
- **Files owned.** `dos-bridge`'s `src/keep.ts`, its part of `bridge.js` and the page, `docs/decisions.md` (SITE-66).
- **Dependencies.** WP-DOSWEB-2 merged; the owner's yes.

### WP-DOSWEB-8 A formatted PDF from DOS (later)

- **Finish line.** **Download as PDF** on a saved `.QLD` gives the PDF Quill for Windows 95 prints, made in the page by `quill-pdfdoc` built for WebAssembly, within a size budget the owner sets.
- **Dependencies.** The owner's yes; `quill-pdfdoc` building for `wasm32-unknown-unknown` (the architecture's wasm gate says the core does).

## Builder briefs

Each brief is the common preamble followed by the package's own part. Paste both.

### Common preamble

```text
You are a Sonnet builder for Quill (repo liminal-hq-labs/quill), working on the DOS page bridge: the host
features of the DOS Try Quill page (site/try/, js-dos 8.5.3, FreeDOS 1.3, QUILLDOS.EXE) through two shared
hard disk images. Read AGENTS.md, docs/plans/dos-page-bridge.md (your package, "The design", "Rules for
every package" and the feature your package serves), docs/plans/host-integration.md ("The page",
"The clipboard flows", "Privacy and security"), site/try/try.js, site/try/js-dos/PROVENANCE.md,
scripts/build-try.sh and the SITE- decisions in docs/decisions.md (SITE-4, SITE-5, SITE-31, SITE-51,
and, once their packages merge, SITE-22 from WP-PTR-3 and SITE-33 to SITE-35 from WP-HOST-4).

Where to work: never touch ~/source/experiments/quill16 except `git fetch` and `git worktree add/remove`.
Create your worktree at ~/source/worktrees/quill/<topic> on branch <branch> from origin/main. Remove it
after the PR merges, with plain commands and full literal paths, one per command (`git worktree remove
<path>`, `git worktree prune`, `git branch -d <name>`); never globs, variables or `find -delete` in a
removal. Keep `df -h /home` above 20 GB (check before a bundle build and stop with a report rather than go
under); check free memory before writing over 1 GB to /tmp (it is RAM).

Rules: Canadian English, real em dashes, sentence case, no hard-wrapped markdown. SPDX header on every new
source, script, TOML and YAML file (TypeScript takes the `//` form). Tabs except YAML. Never `git add -A`
or `git commit -a`; stage named paths. Commits: Conventional Commits, body in markdown with bold labels
and no headings, hard-wrapped at about 100 columns, written to a file and committed with `git commit -F`,
ending with `Co-Authored-By: Claude Sonnet <noreply@anthropic.com>` naming your own model. Push only your
branch; open one PR (title capitalized, imperative, no prefix; body `## Summary` with bold lead-in
bullets, `## Test plan` with the commands run, ending with an unchecked "Not verified:" line). Do not
merge. Every merge to main deploys the DOS page, so the page must work after your PR alone, and
`?bridge=off` must give today's page.

Never publish: no release, no tag, no upload, no Pages dispatch. Bundles and images stay under
~/.cache/quill16-try (or $QUILL_TRY_DIR); site/try/quill.*.jsdos and try-manifest.json are never committed.

Processes: every browser is headless (Chromium at /usr/bin/chromium, or Playwright's Firefox, through
playwright-core 1.65 as site/try-reactos/harness does), every DOSBox run uses a private display, and every
command that starts any of them sets DISPLAY explicitly with WAYLAND_DISPLAY unset (`env -u
WAYLAND_DISPLAY DISPLAY=:<n> ...`, a display number of your own). Servers bind 127.0.0.1 on a port of your
own; never 8095 or 8098 (the owner's). Record every PID you start and stop it by that PID; never pkill or
killall by name. Never show anything on, or send input to, the host desktop. In page scripts, never name a
global `ci`: js-dos.js defines a function of that name.

Stay inside "Files owned". If you need a change in a must-not-touch path, stop and report what and why.
When a choice is not covered, take the reasonable default, record it in your decision row marked
**Assumed**, list it in the PR as "OWNER DECISION: question | default taken | where to swap", and keep
going.

Evidence: into ~/Documents/Liminal HQ/Quill/Web demo/DOS page bridge/<package>/; commit the few shots
that document the feature under docs/media/ as web-dos-*.png. Before the PR: bash scripts/check-headers.sh,
markdownlint in Docker (docker run --rm -v "$PWD:/workdir" davidanson/markdownlint-cli2:v0.17.2 "**/*.md"),
python3 scripts/check-ids.py, scripts/check-try.sh, python3 -I scripts/stamp-try.py --check, python3 -I
scripts/check-site.py, the dos-bridge package's bun test, type check and drift check once it exists, make
dos and dos/tools/size-report.sh if you touched dos/**, and your package's own checks. Update the README
section you own and decisions.md in the same change. Report: PR URL, what was and was not run, the
evidence paths, OWNER DECISION lines, and anything you stopped on.
```

### WP-DOSWEB-1 The disks, the date and the package core (full brief)

```text
Package WP-DOSWEB-1, The disks, the date and the package core. Branch feat/dos-bridge-disks, worktree
~/source/worktrees/quill/dos-bridge-disks.

Read first: docs/plans/dos-page-bridge.md in full ("What was measured" holds the spike's numbers and the
two DOS calls' bytes; "The design" is your contract); scripts/build-try.sh, scripts/make-freedos-image.py,
scripts/try-dosbox.conf, scripts/try-disk/*, scripts/check-try.sh and scripts/stamp-try.py (how the
bundle and the page are built and stamped); site/try/try.js (start, ci-ready, chooseMode, stop: your hook
goes beside chooseMode); the host-link and ac97 packages under site/try-reactos/packages/ (copy their
package.json, tsconfig, README shape, test runner, build and drift check exactly); the "Disks the page and
the guest share" table (the flush rules exist because of it).

Build, in this order, one commit each where it makes sense:

1. The images. In scripts/build-try.sh, after fl.img: c.img, 8 MB (16 cylinders, 16 heads, 63 sectors),
   one primary FAT12 partition from sector 63 with correct start and end CHS in the MBR, formatted with
   mtools (label QUILLDOCS), empty; d.img, 32 MB (64 cylinders), FAT16 (type 06h), label FROMPC, empty.
   Write the MBR yourself in Python (no sfdisk: its end CHS made FreeDOS print "suspect partition"), so
   the bytes are the same on every machine. SYNC.COM on A:\ from its bytes, written by the script: INT 21h
   AH=0Dh, then INT 13h AH=02h one sector of drive 80h into a buffer in its own segment, then ret (about
   20 bytes; put the disassembly in a comment). Add both images to the zip.
   Fixed times keep the zip repeatable. check-try.sh checks the images' geometry and that the bundle grew
   by at most 64 KB.
2. scripts/try-dosbox.conf: `imgmount 2 c.img -size 512,63,16,16 -t hdd -fs none` and `imgmount 3 d.img
   -size 512,63,16,64 -t hdd -fs none` before `boot fl.img`. The three batch files: `SYNC`, then `IF EXIST
   D:\SETDATE.BAT CALL D:\SETDATE.BAT`, then `C:` and start Quill from A:\QUILL with its full path and the
   sample document's full path (check that Quill finds QUILL.INI, QUILL.FNT and EN_CA.DIC beside the
   program, not in the current folder; if it does not, keep `CD \QUILL` and set the remembered folder in
   QUILL.INI to C:\ instead, and say which you did). Quill is started exactly as today otherwise (no /W:
   that is WP-DOSWEB-3).
3. The package, site/try/packages/dos-bridge/: package.json, tsconfig (strict, noUncheckedIndexedAccess),
   README, src/fat.ts (parse an MBR and a FAT12 or FAT16 volume; list a folder; read a file by path;
   build a volume from the empty template and a list of {name, bytes, time} files into the root, with
   FAT copies, directory entries and DOS times; never write outside the image; refuse when full),
   src/names.ts (the 8.3 rules of the plan's "Files into the PC", NAMES.TXT), src/disks.ts (the only file
   that calls ci.fsReadFile and ci.fsWriteFile: readC(), writeD(files), and the date file), src/index.ts
   (attach(ci, options) and the public types). Tests: fat.ts against goldens in tests/dos-bridge/ that a
   script there makes with mtools (FAT12 and FAT16 images with files of 0, 1, 511, 512, 513 bytes, a
   cluster chain over several FAT sectors, a sub-folder); every image fat.ts builds is accepted by
   `fsck.fat -n` and read back by mtools equal; names.ts against a table (accents, spaces, many dots,
   device names, collisions to ~10, empty names). Build one ES module into site/try/lib/dos-bridge.js
   with a drift check, at most 16 KB minified.
4. site/try/bridge.js: on ci-ready (one hook line in try.js, added after WP-PTR-3 has merged: rebase,
   then add it beside chooseMode), write D: with SETDATE.BAT (`@DATE mm-dd-yyyy` from the browser's local
   date) before chooseMode presses the display key (await the write, then let chooseMode run; the menu
   waits 30 s, so there is time); on stop, drop everything. `?bridge=off` skips it all. A short state for
   the build line's title or a hidden diagnostics line ("bridge: on, D: written 12 ms"). index.html: the
   script tag, stamped by stamp-try.py.
5. A level 3 check, site/try-reactos/harness/check-dos-bridge.ts (it drives a locally built page; build
   with scripts/build-try.sh --build): in Chromium and Firefox, each of the three display modes: no
   "suspect partition" on the boot screen; at the prompt after File > Exit, `DATE` shows the browser's
   day (set it with Playwright's clock to 2026-12-25 to prove it is not today by chance); `DIR C:` and
   `DIR D:` work; with ?bridge=off the date is 07-25-1998 as before.
6. CI: one "DOS bridge tests" job (bun test, tsc --noEmit, the drift check), set up as the ac97 job is.
   PROVENANCE.md: one line saying the page now uses ci.fsReadFile and ci.fsWriteFile and to check they
   exist after an update. site/README.md: "The DOS page's disks" (what C: and D: are, SYNC, the flush
   rules in two sentences). decisions.md: SITE-60 (written by the plan as Assumed) amended with what you built, and SITE-61 and
   SITE-62 in the SITE- rows' style, with alternatives (a serial link: js-dos has none; a second floppy: FreeDOS sees a phantom; DOSBox's
   own mounted folders: gone after boot; one shared disk: two writers) and consequences.

Must not touch: site/try/js-dos/** except the PROVENANCE line, the pointer code of try.js,
site/js/pointer.js, site/try-reactos/** except the new harness script, dos/**, crates/**, the host-link
package.

Finish line checks (all must hold): the unit tests, the type check and the drift check pass; the level 3
check passes in both browsers in all three modes; check-try.sh, stamp-try.py --check, check-site.py,
check-ids.py, the headers check and markdownlint pass; the page with ?bridge=off behaves as today.
Evidence into ~/Documents/Liminal HQ/Quill/Web demo/DOS page bridge/WP-DOSWEB-1/ (the boot screen, DATE
in each mode, the bundle's size before and after, the test counts). PR title suggestion: "Give the DOS
page two shared disks and the visitor's date".
```

### WP-DOSWEB-2 to WP-DOSWEB-5

```text
Package <id>, <title from the plan>. Branch <feat/dos-bridge-files | feat/dos-bridge-quill-w |
feat/dos-bridge-clipboard | feat/dos-bridge-touch>, worktree ~/source/worktrees/quill/<same topic>.

Read first: the plan's section for your package, "The design", the feature your package serves under
"The features", "Privacy and gestures" for the clipboard, WP-DOSWEB-1's package as it merged
(site/try/packages/dos-bridge/ and site/try/bridge.js), and the files your package owns.

WP-DOSWEB-2 adds files.ts and pdf-text.ts, the drop zone, the picker and the Files on the PC panel, with
the help text about C:, D: and SYNC. WP-DOSWEB-3 adds bridge.rs and the /W switch to QUILLDOS.EXE (the
FileClipboard over DosSource and DosSink, the reset and flush through dpmi::simulate_int, the points the
plan lists), measures it against the 1,536-byte gate before anything else (stop and report if the first
working version is over), and adds the DOSBox scenarios. WP-DOSWEB-4 adds clip.ts and keys.ts over
host-link's state machine and converters, the capture-phase key hook, the chip and its menu with Type my
clipboard, and the level 3 runs in both browsers and all three modes. WP-DOSWEB-5 adds the Keyboard
button on touch screens and Fit to window.

Build what the plan's Finish line says, in small commits, staying inside Files owned. Finish line,
tests, evidence and decision numbers: the plan's package section. PR title suggestions: "Move files both
ways on the DOS page and print to a PDF", "Serve Quill's DOS clipboard through the page's disks", "Share
the clipboard between the DOS page and Quill", "Add a phone keyboard and Fit to window to the DOS page".
```

## Owner decisions

Each is taken with the default so no package waits; the last column is where to change it.

| Question | Default taken | Where to swap |
|---|---|---|
| Bridge the DOS page with shared disks, since js-dos has no serial link | Yes: `C:` the guest's, `D:` the page's, one writer each | SITE-60 (WP-DOSWEB-1) |
| Spend up to 1.5 KB of `QUILLDOS.EXE` on `/W` (the formatted clipboard both ways and the flushes) while WP-ED-5 leaves 4,608 bytes and Unicode already needs WP-MOD-4 | Yes, gated at 1,536 bytes; over it, typing only and Save As for copying out | DOS-120, DOS-121 (WP-DOSWEB-3); `installer/budgets.toml` |
| Quill's working folder on the page | `C:\` (the visitor's disk), so Save As lands where the panel lists | The start-up batch files or `QUILL.INI` (WP-DOSWEB-1) |
| Print to PDF on DOS | A text PDF made in the page from a printout; a formatted PDF later | `pdf-text.ts`; WP-DOSWEB-8 |
| Keep the visitor's documents across reloads | Not now; later as an opt-in that stores `C:` alone | WP-DOSWEB-7; SITE-66 |
| Clipboard sharing on the DOS page when it opens | On, remembered, the chip always showing it (as the ReactOS page) | `bridge.js`; SITE-64 |
| Ship the DOS bridge with the ReactOS launch | The date and files packages yes; the clipboard when WP-HOST-4 and WP-ED-5 have merged; nothing waits on it | This plan's first-demo table; the launch checklist |
| Fit to window on by default | Off by default, remembered | `bridge.js`; SITE-65 |
| `D:`'s size | 32 MB (about 39 KB on the bundle for both empty disks) | `build-try.sh`, `try-dosbox.conf` (WP-DOSWEB-1) |
| The file size limits | 32 MiB a file in, 64 MiB out, as on the ReactOS page | `files.ts`; SITE-63 |

## Owner's check list

On the owner's desktop (Firefox on Wayland, then Chromium), with LibreOffice and, if at hand, Word, then a phone. Each item is a yes or no; any no gets a note into `~/Documents/Liminal HQ/Quill/Web demo/DOS page bridge/owner/`.

1. Start Quill in each display mode: no warning on the boot screen; Insert > Date shows today.
2. In Quill, select the sample's title, Ctrl+C; paste into LibreOffice Writer: bold and the size are there.
3. Copy a formatted paragraph in LibreOffice; click the screen; Ctrl+V in Quill: bold, italic, size and colour arrive, once, with no stray "v".
4. Copy in Quill, then Paste in Quill: Quill's own text, not the computer's.
5. Edit > Copy with the mouse, wait 10 seconds: the chip offers **Copy to your computer**, and it works.
6. File > Exit; at the prompt, **Type my clipboard** into `COPY CON C:\T.TXT`, then Ctrl+Z, Enter, `SYNC`: the panel lists `T.TXT` and its download has the text.
7. Drag a file from the desktop onto the screen: Quill's Open dialog shows it on D: under its short name.
8. Save the document as `C:\LETTER.RTF` and print to `C:\LETTER.PRN`: both are in the panel; the RTF opens in LibreOffice; the PDF of the printout reads right.
9. Clipboard off: Ctrl+V in Quill pastes what Quill had, and nothing from the desktop.
10. Fit to window and Full screen both look right.
11. Phone: the Keyboard button types into Quill; **Paste from your computer** then Edit > Paste works; a picked file arrives on D:.

## Risks

- **DOSBox's stdio buffering** is what the flush works around; a js-dos update could change it (for better or worse). `PROVENANCE.md` gains a check, and the level 3 test catches it.
- **A visitor at the prompt** sees stale listings of `D:` and leaves unflushed files on `C:` without `SYNC`; the help explains, and the panel marks a file still being written.
- **Two writers by accident**: a program at the prompt that writes `D:` loses its file on the next rebuild (said in the help); the page never writes `C:`.
- **The budget.** `/W` competes with Unicode for the last 4,608 bytes after WP-ED-5; the gate and the typing fallback bound the damage, and WP-MOD-4 is the room the Unicode plan already needs.
- **The freeze.** WP-DOSWEB-3 must merge before WP-ED-10 opens it, or wait for it to close.
- **Shared files.** `try.js` and `index.html` are WP-PTR-3's until it merges; the bridge takes one line and one block.
- **Every merge deploys.** A broken bridge breaks the live DOS page; `?bridge=off` and each package's finish line keep it working alone.
- **Safari and real phones** were not measured.

## What was not verified

- Quill itself on the shared disks: the spike used `COMMAND.COM` on a floppy without a real `QUILLDOS.EXE`, so whether Quill finds its files when started from `C:`, its paste through a file and its flush through DPMI are for WP-DOSWEB-1 and WP-DOSWEB-3.
- A guest reading `D:` while the page rewrites it (the design avoids it; not tested).
- Typing into Quill's editor at 30 characters a second in graphics mode (only `COPY CON` was typed into), and how Quill takes an Alt-and-keypad character.
- The soft keyboard on a real phone, Safari anywhere, and Firefox's own Paste button on **Paste from your computer**.
- The cost of `/W` in bytes (estimated, not built).
- Whether a correct end CHS removes FreeDOS's "suspect partition" warning (the spike's images had none).
