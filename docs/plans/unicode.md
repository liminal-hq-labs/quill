# Unicode: finishing the core

The owner asked on 2026-11-27 to finish off the core of Quill with Unicode. This page settles what "finished" means: the representation, the rules for characters (clusters, words, line breaks, case, normalization, width, direction, emoji), what each build shows and types, the files, the costs on every build, the tests and the evidence, and the build order as work packages: the existing `WP-ED-4`, `WP-ED-10` and `WP-ED-11` (Unicode U1a, U1b and U2), updated here, and `WP-UNI-1` to `WP-UNI-10` for the rest, with `WP-UNI-11` to `WP-UNI-15` named and parked for later. It builds on the Unicode plan of 2026-11-25 (`Quill Unicode plan.md` in `~/Documents/Liminal HQ/Quill`), whose model the owner chose (UTF-8 in the document on every build) and whose file format design it keeps; where the two differ, this page wins and says why.

Written against `origin/main` at `aaacc62` and brought up to date at `a004a70`. Marks: **[V]** read in the code or docs at that commit, **[E]** an estimate for the package to measure, **[U]** unverified, to be checked on the named platform before a package depends on it. The rules, budgets, gates (G-HOST, G-DOS, G-W16, G-W32, G-GTK, G-DISK, G-MD) and the common brief are those of [work-packages.md](../work-packages.md); this page adds only what is particular to Unicode.

## Table of contents

- [The answer on one page](#the-answer-on-one-page)
- [Where things stand](#where-things-stand)
- [Scope: what v1 is and what is later](#scope-what-v1-is-and-what-is-later)
- [Verdict per build and scope level](#verdict-per-build-and-scope-level)
- [The representation](#the-representation)
- [The migration path and the freeze](#the-migration-path-and-the-freeze)
- [Rules for characters](#rules-for-characters)
- [What each build shows](#what-each-build-shows)
- [Typing on each build](#typing-on-each-build)
- [Files](#files)
- [Search, replace and spelling](#search-replace-and-spelling)
- [PDF](#pdf)
- [How it meets the other plans](#how-it-meets-the-other-plans)
- [Costs and budgets](#costs-and-budgets)
- [Tests](#tests)
- [Evidence](#evidence)
- [Decisions](#decisions)
- [Work packages](#work-packages)
- [Briefs](#briefs)
- [Questions for the owner](#questions-for-the-owner)
- [Found while planning](#found-while-planning)

## The answer on one page

**Finished means: every build holds, edits, saves, copies, finds and prints any Unicode text without losing a character, and shows what it can.** The document is UTF-8 on every build, DOS and Windows 3.x included. A character a build cannot draw is a visible placeholder (a box with its code point, `▒` in a text cell), never a `?` and never a silent loss; the status line names it; Reveal Codes shows its code. Files carry the real characters: `.QLD` 1.1 adds a sparse `UNIC` chunk that 1.0 readers skip, RTF writes `\uN` with a fallback, plain text reads and writes UTF-8 and UTF-16. Every build can type any character with Alt+X; GTK types anything its input method commits.

**The recommended v1** (the minimal complete core, CORE-210):

| In v1 | Later |
|---|---|
| UTF-8 storage with `Pos`, on every build (U1a, U1b, U2) | |
| One cluster rule for the caret, Delete and selection on every build; Backspace by code point | Full UAX #29 for complex scripts |
| Line breaks: spaces, hyphens, dashes, zero-width space, no-break characters, and between ideographs (a UAX #14 subset) | Full UAX #14, Thai and Lao dictionaries |
| Simple case folding for Latin, Greek, Cyrillic, Armenian and Georgian in Find; spelling through the dictionary's code page | Full case mapping, a Change Case command |
| No normalization of stored text; display and Find compose the precomposed letters of Windows-1252 | NFC on paste as an option |
| Placeholders on every build; the status line and Reveal Codes name the character | |
| `.QLD` 1.1 `UNIC`, RTF `\uc1\uN?`, UTF-8 and UTF-16 text, Unicode clipboard flavours | RTF fallbacks in a charset's code page instead of `?` |
| Alt+X on every build; DOS typing through CP437 into real Unicode; GTK's input method unrestricted; the terminal client in UTF-8 | Insert > Symbol; Windows 95, 98 and Me input locales |
| GTK shows every left-to-right script that needs no shaping (Latin, Greek, Cyrillic, Armenian, Georgian, symbols, CJK) through per-glyph font fallback; single-code-point emoji in monochrome where a font has outlines | Right to left, Indic, Thai, colour emoji, emoji sequences as one glyph (GTK and WinUI only) |
| The DOS text screen shows all of CP437 (box drawing, the Greek and maths letters CP437 has) | DOS code pages 850, 852, 866, 737 and strike pages for DOS graphics |
| DOS graphics, Windows 3.x and Windows 95 show Windows-1252 and placeholders | Charset fonts for Central European, Cyrillic and Greek on Windows 95, 98 and Me, then Windows 3.x |
| PDF: placeholders with the real character in the text layer on Windows 95; embedded font subsets on GTK | Embedded subsets on Windows 95 |

**The representation** (CORE-211): positions in memory are **UTF-8 byte offsets that are always on a code point boundary**, carried by `Pos`, which only `quill-core` makes. Positions in files are **code points** (the `.QLD` counts and the recovery caret). **Clusters** are a movement rule, never stored. UTF-16 offsets exist only at the edges that need them (WinUI's text services, the Windows clipboard).

**The cost** [E]: DOS `QUILLDOS.EXE` +14 to +23 KB over the whole series, against 4,608 bytes left today; it fits under DOS-70's 480 KB stop **if the file dialog module (WP-MOD-4) lands first or alongside**, and does not fit together with the rest of the editor ledger without it. Windows 3.x code +11 to +18 KB across segments and **+20 to +116 bytes of data segment** with every table as a resource (1.9 KB free, floor 1,024). Windows 95 +10 to +14 KB and **no new import**. GTK and WinUI: no size limit.

**The order:** two side packages now (the tables and the test document set), U1a when the editing review's packages have merged, the freeze (U1b, U2 and the placeholders) after the last move, then files, typing, the clipboard, Find and spelling, GTK, PDF, and a closing package that saves one multi-script document on every build and compares the bytes.

## Where things stand

- **`WP-UNI-1` is built** (CORE-218): `quill-text` has `cluster`, `breaks`, `word`, `fold`, `compose`, `width`, `script` and `blocks`, generated by `scripts/gen-unicode.py` from UCD 16.0.0, as `const` tables and as `assets/unicode/UNITAB.BIN`. Measured, they are larger than this page's estimates: 13,611 bytes in all (cluster 3,404, breaks 3,332, word 2,416, fold 1,552, width 980, blocks 404 and 1,007 of labels, script 188, compose 232 for 58 letters, short names 96) because they cover all of Unicode; `WP-UNI-3` decides what each build links and whether the tables are cut to the scripts v1 shows (CORE-218's owner decision). Nothing links them yet.
- **U0 is done** (CORE-61, pull request 101) [V]: the RTF reader reads `\'hh` through the font's `\fcharset` or the document's `\ansicpg` (1250 to 1254, 1257), reaches all 27 Windows-1252 characters of 0x80 to 0x9F through `\uN`, joins surrogate pairs and counts what it refuses; the code page tables are `quill-text`'s, served on Windows 3.x from an `RCDATA` resource (`CPAGES.BIN`, id 100) through `ReadStore::to_unicode`.
- **U1a, U1b and U2 are planned, not started** [V]: `WP-ED-4` (CORE-100), `WP-ED-10` (CORE-100 continued) and `WP-ED-11` (CORE-103, CORE-104) in [work-packages.md](../work-packages.md). There is no `Pos` in `quill-core` yet; `Text` reads and edits `u32` offsets and `&[u8]` (`read_byte`, `read`, `for_each_run`, `read_piece`, `insert`) [V `crates/quill-core/src/text.rs`]. `WP-ED-4` waits for `WP-ED-1` to `WP-ED-3`: **`WP-ED-1` (Enter flags, pull request 173) and `WP-ED-2` (paste undo and the cut guard, pull request 185) have merged**, and `WP-ED-3` (End affinity) is not started; the freeze waits for `WP-ARCH-7`; `WP-ARCH-1` and `WP-ARCH-2` have merged and `WP-ARCH-3` to `WP-ARCH-7` are not started.
- **Other work this plan meets** [V at `a004a70`]: the optional modules plan's first package, WP-MOD-1 (the module interface and Desk's `.DMD` toolchain), is open as pull request 189, so WP-MOD-4 (`QUILLDLG.DMD`, the room DOS-112 counts on) is three packages away; the printable media set (pull request 190) is artwork only and touches nothing here; the WinUI real-window plan (pull request 187) reserves WUI-85 to WUI-94, which is why this plan's WinUI ids are WUI-95 to WUI-97.
- **Byte loops outside the core today** [V by search]: `read_byte(` in `spell_live.rs` 7, `lib.rs` 7, `spelling.rs` 5, `print.rs`, `pointer.rs` and `find.rs` 2 each, the GTK client 3, and the test crates; `for_each_run(` and `read_piece(` in `layout.rs`, `qld.rs`, `lib.rs`, `chrome.rs`, `quill-cli` and `quill-clip`; `Key::Char(u8)` in `quill-app`, `quill-dlg`, the terminal's key map, the Windows 95 shell and about 70 test sites. The status line's column is `caret - line.start` in bytes on a device with fine units [V `crates/quill-app/src/paint.rs:476`]. Caret Left and Right are `±1` [V `lib.rs:2117`].
- **What each output path takes** [V]: `quill-grid` cells hold a CP437 byte (`ansi_to_cp437`); the DOS graphics strikes (`QUILL.FNT`) hold the 224 characters of Windows-1252; the Windows 3.x and Windows 95 painters call `TextOut`/`TextOutA` and `GetCharWidth`/`GetCharWidthA` over bytes 32 to 255; `quill-fonts` maps a byte through `cp1252::to_char` and keeps 224 advances per face; the layout's `Measure::widths` fills `[u16; 224]` and `FontCache` keeps eight; `quill-pdf` writes `WinAnsiEncoding` with the standard 14 fonts (W9X-92). `QUILL32.EXE` imports only ANSI entry points and `check-imports.sh` fails on any name ending in `W` [V]; `GetProcAddress` is on the allowlist and already loads `MoveFileExA` at run time [V `win9x/quill-win9x/src/main.rs:207`].
- **The GTK client refuses** what Windows-1252 cannot hold, with `Quill keeps text in Windows-1252: <c>` (LNX-14) [V `linux/quill-gtk/src/doc.rs:113`]. The WinUI client is a spike only [V `winui/`].
- **`.QLD` is 1.0** (`VERSION = (1, 0)`), unknown chunks are skipped at both levels, and no `UNIC` or `PICX` chunk exists [V `crates/quill-formats/src/qld.rs:18`].
- **Budgets** [V]: `QUILLDOS.EXE` 437,760 of 442,368 bytes (4,608 left, DOS-48); the Windows 3.x data segment about 1.9 KB free with a floor of 1,024 (W16-60) and relief of about 4.4 KB planned (WP-ED-5, WP-ARCH-5, WP-ARCH-9); `QUILL32.EXE` 441,344 bytes with no limit beyond its disk.

## Scope: what v1 is and what is later

**What the core must guarantee, on every build, for v1 to be finished:**

1. **Nothing is lost.** Any sequence of Unicode scalar values that reaches the document (a file, a paste, a key) is kept exactly, edited around, undone and redone, saved and read back the same in `.QLD` and RTF and in UTF-8 and UTF-16 text. Only a save to a single-byte format (Windows-1252 or CP437 text, the `TEXT` chunk a 1.0 reader sees, `CF_TEXT`) gives `?`, and it says how many.
2. **Nothing is split.** No offset, selection, deletion, undo record, layout cursor, line break or file position falls inside a code point; the caret and Delete never fall inside a cluster.
3. **Nothing is hidden.** A character the build cannot draw is a placeholder of a fixed width that the caret can stop beside, the status line can name and Reveal Codes can show; an invisible format character (zero-width joiner, zero-width space, the byte order mark inside text, left-to-right and right-to-left marks, variation selectors) draws nothing but is still a character with a code in Reveal Codes.
4. **Anything can be typed.** Alt+X turns the hex digits before the caret into the character and back, on every build.
5. **Find finds it.** Find matches any character literally, folds case beyond ASCII for the alphabetic scripts in scope, and treats a precomposed Windows-1252 letter and its decomposed form as equal.

**What each build shows in v1** is the matrix below; what it does not show is a placeholder. The scripts in scope for real display are the ones a build can draw without shaping or reordering.

| Build | Shown in v1 | Placeholder in v1 | Later |
|---|---|---|---|
| DOS text (25 and 50 rows) | CP437: ASCII, the Latin-1 letters it has, box drawing, the Greek and maths characters it has (`α ß Γ π Σ σ µ τ Φ Θ Ω δ ∞ φ ε ∩ ≡ ± ≥ ≤ ÷ ≈ ° √ ²`) | Everything else as `▒` (one cell a cluster) | CP850, 852, 866 and 737 through the active code page (WP-UNI-12) |
| DOS graphics | Windows-1252 from `QUILL.FNT` | Everything else as a hex box | Strike pages for Central European, Cyrillic, Greek and symbols (WP-UNI-12) |
| Windows 3.x | Windows-1252 | Everything else as a hex box | Charset fonts where a language edition has them (WP-UNI-12) |
| Windows 95, 98 and Me | Windows-1252 | Everything else as a hex box | Charset fonts and input locales (WP-UNI-11) |
| Windows 2000 and later (the same `QUILL32.EXE`) | As Windows 95 | As Windows 95 | Wide GDI calls loaded at run time (WP-UNI-11) |
| Terminal client | Every single-width character, as the terminal is UTF-8 | Wide (East Asian Wide and Fullwidth) characters, combining marks and emoji as `▒` | Two-cell wide characters (WP-UNI-15) |
| GTK | Every left-to-right script with no shaping: Latin (all blocks), Greek, Cyrillic, Armenian, Georgian, Cherokee, the symbol, arrow, maths, box and dingbat blocks, CJK ideographs, kana and Hangul syllables, single-code-point emoji in monochrome where an outline font has them | Right-to-left scripts, scripts that need shaping (Arabic, Indic, Thai, Lao, Khmer, Myanmar, Tibetan, Mongolian), emoji sequences (ZWJ, skin tones, flags, keycaps) as one box, a character no installed font has | Shaping, bidirectional text, colour emoji (WP-UNI-14) |
| WinUI 3 (when built) | As GTK (WUI-95) | As GTK | As GTK |

**Out of scope for v1 on every build:** bidirectional reordering, shaping, vertical text, ruby, Hangul jamo composition, Thai and Lao line breaking by dictionary, NFC or NFD conversion of stored text, Unicode file names (they stay Windows-1252, LNX-4's stand-ins on GTK), Unicode in `QUILL.INI` beyond CORE-151's modern keys, a non-1252 spelling dictionary, and Insert > Symbol.

## Verdict per build and scope level

Scope levels, each including the one above it: **L1** hold, edit and save any Unicode text (UTF-8 core, files, placeholders); **L2** type any character (Alt+X, native input); **L3** show the alphabetic scripts beyond Windows-1252; **L4** show CJK; **L5** right to left, shaping and colour emoji.

| Build | L1 | L2 | L3 | L4 | L5 |
|---|---|---|---|---|---|
| DOS text | **GO** (v1) | **GO** (v1: Alt+X; CP437 keys become real characters) | GO for CP437's own repertoire (v1); other pages later | **NO-GO** (one byte a cell, no double-width code page in scope) | **NO-GO** |
| DOS graphics | **GO** (v1, if WP-MOD-4 or a DOS-70 raise gives the room) | **GO** (v1) | Later: strike pages on a second disk (+35 KB packed each [E]) | **NO-GO** in the box (Unifont is 2 MB); an optional file only if asked | **NO-GO** |
| Windows 3.x | **GO** (v1, tables as resources) | **GO** (v1: Alt+X; `WM_CHAR` stays Windows-1252) | Later: charset fonts exist only on language editions [U] | **NO-GO** (no double-byte path in a Western 3.1) | **NO-GO** |
| Windows 95, 98 and Me | **GO** (v1, no new import) | **GO** (v1: Alt+X); input locales later | Later (WP-UNI-11, after a spike on the 98 and Me VMs) | Later on 2000 and XP through wide calls loaded at run time; **NO-GO** on 9x | **NO-GO** (Uniscribe is out of scope) |
| Terminal | **GO** (v1) | **GO** (v1, UTF-8 keys) | **GO** (v1, the terminal draws) | Later (two-cell characters) | **NO-GO** |
| GTK | **GO** (v1) | **GO** (v1, the input method) | **GO** (v1) | **GO** (v1: fallback fonts and ideograph breaks) | Later (WP-UNI-14) |
| WinUI 3 | **GO** when the client exists | **GO** (TSF) | **GO** | **GO** | Later |

## The representation

### Three kinds of position, one stored

| Kind | Where it is used | Why |
|---|---|---|
| **UTF-8 byte offset on a code point boundary** (`Pos`) | Every position in memory: the piece table, undo records, the caret and selection, `LayoutCursor`, checkpoints, `Damage`, spelling marks, find results, the pointer's word range, Reveal Codes' tokens | Pieces stay `start` and `len` in bytes (16 bytes each, as today); a position lookup is the piece search it is today; libraries on the new builds (`&str`, GTK, shaping) take byte offsets |
| **Code point (scalar) count** | Every position and length in a file: `.QLD`'s `STHD` count and `RUNS` lengths, the recovery caret (`RCOV`), the `UNIC` run positions; GTK's accessible text offsets | One position space in the file that does not depend on the encoding in memory, so `TEXT` keeps one byte per code point for 1.0 readers |
| **Cluster** | A movement and display rule only: Left, Right, Delete, double click, the mouse, the status line's column | Cluster boundaries move when a mark is typed after a letter, so a stored cluster index would go stale; computing them from the text costs a table lookup per character |

UTF-16 code unit offsets are computed at the two edges that need them and nowhere else: the Windows clipboard's `CF_UNICODETEXT` (a copy, not a position) and WinUI's text services, whose positions are UTF-16 (`quill-a11y` converts for UI Automation and TSF in WUI-95).

**Rejected:** scalar indexes in memory (every lookup needs a count index kept in step with edits, and a piece grows to 20 bytes); cluster indexes (unstable, as above); UTF-16 (doubles ASCII, still needs surrogate handling, and DOS and Windows 3.x gain nothing from it); the first draft's marker byte with a side table (the owner chose real UTF-8, 2026-11-25).

### `Pos` and the core's API

As the Unicode plan's U1 sets out, with the names fixed here so every package uses the same ones:

- `Pos(u32)`, `#[repr(transparent)]`, `Copy`, `Ord`; made only by `quill-core`: `Text::start()`, `end()`, `next(s, pos)` and `prev(s, pos)` (one code point), `snap(s, raw: u32) -> Pos` (rounds down to the start of the character), `check(s, raw) -> Option<Pos>`. `Pos::raw(self) -> u32` reads it. `Range<Pos>` for selections and deletions.
- Editing takes text that is valid by type: `insert(s, pos, &str, cfmt, label)`, `delete(s, Range<Pos>, label)`, `set_cfmt(s, Range<Pos>, …)`. The byte path `insert(&[u8])` goes in U1b.
- Reading: `chars(s, Range<Pos>)` (an iterator of `(Pos, char)`, decoding across piece ends), `char_at(s, pos)`, `read_str(s, Range<Pos>, &mut [u8]) -> &str` (cut back to a boundary when the buffer is short).
- Clusters (WP-UNI-3): `next_cluster(s, pos)`, `prev_cluster(s, pos)`, `cluster_at(s, pos) -> Range<Pos>`, built on `quill_text::cluster`.
- `read_byte` stays inside `quill-core` and `quill-formats`; `clippy::disallowed_methods` forbids it elsewhere from U1b.
- A boundary test is one byte read: the byte at the offset is not 0x80 to 0xBF.
- Data from outside (a file position, the recovery caret, an INI value, a shell's `u32`) becomes a `Pos` only through `check` or `snap`. Stored structures keep `u32` and are rebuilt into `Pos` by the core, with a debug assertion on the boundary.

**Bytes with meaning stay one byte**: U+0001 (an inline object), U+0009 (tab), U+000B (line break) and U+000D (paragraph mark) are one byte in UTF-8 as in Windows-1252, so `PARA`, `OBJS`, the pictures model and D58 do not change. U+FFFC is what Quill writes for an object in exported UTF-8 and UTF-16 text and what accessibility text reports; U+0001 and other C0 controls arriving from outside are dropped as today.

### Memory and limits

`MAX_TEXT` is **1,048,575 bytes of UTF-8** (CORE-104) and `MAX_HF_TEXT` 4,096 bytes, so the buffers and `Doc` keep their size and the Windows 3.x data segment pays nothing. That is about a million characters of English, 960,000 of Canadian French and 550,000 of Russian [E]. A file that would pass it is refused with the existing `TooLarge` message.

## The migration path and the freeze

| Step | Package | What changes | What proves it |
|---|---|---|---|
| U1a | `WP-ED-4` | `Pos`, `Range<Pos>` and the checked API inside `quill-core` and `quill-formats`; `read_byte` public but hidden | Every golden unchanged; the boundary property test (trivially true while text is Windows-1252) |
| U1b | `WP-ED-10` (freeze) | Every caller outside the core on `Pos` and `chars`; `Key::Char(char)`; `App::caret_column`; `read_byte` forbidden outside the core | Every golden byte-identical on every build |
| U2 | `WP-ED-11` (freeze) | The buffers hold UTF-8; readers convert into UTF-8 and writers back; layout decodes with an ASCII fast path; the painters transcode runs; **every entry point still narrows to Windows-1252**, so no document can hold anything else yet | Every golden and every saved file byte-identical; the bench gate |
| U3 | `WP-UNI-3` (freeze) | Placeholders on every painter, the cluster rule, line breaks beyond spaces, `Measure::advance` for characters outside the 224, the status line naming a character; still no entry point lets a non-1252 character in, so it is proved by host tests that build UTF-8 documents directly and by test-only entry points | Placeholder goldens on every build; every other golden unchanged |
| Open the doors | `WP-UNI-4` to `WP-UNI-7` | Files, typing, the clipboard, Find and spelling stop narrowing | Their own tests and the corpus |

**The rule that makes the order safe (CORE-210):** no package that lets a character outside Windows-1252 into a document merges before `WP-UNI-3`, because until then some painter or byte loop may draw it wrongly. U2 proves the storage change with the goldens because nothing visible may change; U3 proves the drawing change with new goldens because nothing can arrive from outside yet.

**The freeze is three packages** (rule 4 of [work-packages.md](../work-packages.md#rules-for-every-package), extended): while `WP-ED-10`, `WP-ED-11` or `WP-UNI-3` is open, no other branch is open in `crates/quill-app/src`, `crates/quill-paint/src`, `crates/quill-core/src/layout.rs`, `crates/quill-clip`, `crates/quill-spell`, `crates/quill-grid`, `crates/quill-fonts`, or a shell's key map or page painter (`dos/quill-dos/src/keys.rs`, `win16/examples/quill/{gdi,main}.rs`, `win9x/quill-win9x/src/{gdi,main}.rs`, `linux/quill-tty/src/{keymap,paint}.rs`, `linux/quill-gtk/src/{keys,page,doc}.rs`). `WP-UNI-3` joins the freeze because it touches every page painter and the layout, the same set U2 does; landing a status bar or Reveal Codes package between U2 and U3 would rebase both. Shell-only work outside those files (`WP-ED-6`, the installer, the site) may run during the freeze, and so may `WP-UNI-1` and `WP-UNI-2` if they are late.

## Rules for characters

### Clusters, the caret and deletion (CORE-212)

**One rule on every build**, so the caret behaves the same on DOS as on GTK. This differs from the first draft, which put the old builds on code points: a decomposed `é` (`e` and U+0301) that the caret could split would let Delete leave a stray accent, and on a build that draws the accent as a box the split would be invisible.

- **A cluster** is a base character followed by any number of: combining marks (General Category Mn and Me, and Mc in the scripts in scope), U+200D ZERO WIDTH JOINER with the character after it, variation selectors (U+FE00 to U+FE0F, U+E0100 to U+E01EF), emoji modifiers (U+1F3FB to U+1F3FF), tag characters (U+E0020 to U+E007F); a pair of regional indicators is one cluster; CR is never joined (the paragraph mark is its own character). This is UAX #29's extended grapheme cluster without the rules for Hangul jamo, Indic conjuncts and prepended characters, which only the later scripts need. The table is `quill_text::cluster` (WP-UNI-1), generated from `GraphemeBreakProperty.txt` and `emoji-data.txt`, about 50 ranges [E].
- **Left, Right, Delete, double click, the mouse, Shift selection** go by cluster. **Backspace removes one code point**, as GTK's own text views do, so `é` typed as `e` and a mark can be corrected mark first, **except** that an emoji sequence, a flag and a keycap go as a whole.
- **Typing a combining mark** after a letter joins it to that letter's cluster; the caret ends after the cluster.
- **Up and Down** keep the x position and land on the nearest cluster boundary the layout reports.

### Line breaking (CORE-215)

A UAX #14 subset, small enough for DOS, in `quill_text::breaks`:

- A line may break **after** a space (U+0020, U+1680, U+2000 to U+2006, U+2008 to U+200A, U+205F, U+3000), after U+200B ZERO WIDTH SPACE, after a hyphen (U+002D, U+2010, U+00AD at a break, U+2012 and U+2013), after U+2014 EM DASH and U+2E3A, U+2E3B, and **before and after** a CJK ideograph, kana or Hangul syllable, except before closing punctuation (`、。，．・：；？！）」』】〕〉》` and their halfwidth and fullwidth forms) and after opening punctuation (`（「『【〔〈《`).
- **Never** at U+00A0, U+202F, U+2007 (the no-break spaces), at U+2060 WORD JOINER or U+FEFF, inside a cluster, or before a combining mark.
- A word longer than the line breaks at a cluster boundary, as today a long word breaks at a byte.
- Right-to-left and complex-script text is placeholders in v1, so it breaks only at its spaces.

### Words (CORE-216, extending CORE-45)

CORE-45's classes stay (letters and digits, punctuation, space) and gain a table for everything above ASCII: a character is a **letter** when its General Category is `L*`, `M*` or `Nd` (so `é`, `ж`, `ß` and a combining mark are inside a word), **punctuation** for `P*` and `S*`, **space** for `Z*`; each **CJK ideograph, kana run or Hangul syllable run** is a word of its own for Ctrl+Left and Ctrl+Right and for double click, so a line of Chinese moves one ideograph at a time rather than one line. Find's whole word, the spelling scan and the status bar's word count use the same rule.

### Case (CORE-217)

**Simple case folding** (`CaseFolding.txt` statuses C and S) for Latin (all blocks), Greek, Cyrillic, Armenian, Georgian, Cherokee and the fullwidth Latin letters, as a range table in `quill_text::fold` of about 400 to 600 bytes [E]. Find with Match case off folds both sides; spelling folds to look a word up. `ß` folds to itself (full folding to `ss` is later); Turkish dotless `ı` and dotted `İ` fold by the default rule. There is no Change Case command today; when one comes, it needs upper and lower mapping, not folding, and is later.

### Normalization (CORE-214)

**Quill never normalizes stored text.** What a file or a paste brings is kept code point for code point, so a round trip changes nothing. Two narrow uses of composition instead, from one table of the 64 precomposed letters Windows-1252 has (about 300 bytes [E], `quill_text::compose`):

- **Display:** a cluster that is a base letter and one combining mark that compose to a Windows-1252 letter (`e` and U+0301 to `é`) is drawn as that letter on every build that draws Windows-1252, instead of a letter and a box.
- **Find:** the pattern and the text are compared through the same table, so `é` finds `é` whichever way each was written.

Full NFC (about 6 to 10 KB of tables) is later, as an option on paste for the modern builds.

### Width (CORE-213)

**On a device with pixels**, width is the font's advance; a placeholder's advance is three quarters of the font's em; an invisible format character's is zero. **In text cells** (DOS text, the terminal), a cluster takes one cell in v1; a wide character is drawn as `▒` in that one cell so a column is still a cluster, and the terminal's two-cell characters are later (WP-UNI-15). East Asian Width is read only to choose `▒` in the terminal (`quill_text::width`, about 40 ranges [E]).

### Direction and shaping (CORE-226)

**Right-to-left and complex scripts are kept, in logical order, and drawn as placeholders on every build in v1.** Drawing Hebrew left to right would show the letters reversed, and Arabic or Devanagari without shaping would be wrong letterforms; a box says honestly that the build cannot show them. The scripts so marked (`quill_text::script::needs_shaping_or_bidi`, about 30 ranges [E]): Hebrew, Arabic and its supplements and presentation forms, Syriac, Thaana, NKo, Samaritan, Mandaic, the Indic blocks from Devanagari to Sinhala, Thai, Lao, Tibetan, Myanmar, Khmer, Mongolian, and the bidirectional controls (which draw nothing). Later, on GTK and WinUI only, a shaping engine in `quill-fonts` and `unicode-bidi` (WP-UNI-14), so measuring and drawing stay one engine (LNX-1).

### Emoji (CORE-227)

A single-code-point emoji is a character like any other: GTK draws it in monochrome when an installed outline font has it (Noto Emoji, Symbola, DejaVu Sans for the older ones), and every other build draws a box. **An emoji sequence** (ZWJ, a skin tone, a flag, a keycap, a VS16 presentation) is one cluster, drawn as **one** box labelled with its first code point on every build in v1, since without shaping it would show as its parts. Colour emoji (COLR, CBDT) are later (WP-UNI-14).

## What each build shows

### Repertoire (CORE-213)

`quill_core::repertoire` answers, for a font, a cluster and the build's level, how to draw it:

```rust
pub enum Show {
	Byte(u8),          // the build's own code page byte: Windows-1252, or CP437 in cells
	Glyph(char),       // the font draws the code point (GTK, WinUI)
	Composed(u8),      // a letter and a mark drawn as one Windows-1252 letter (CORE-214)
	Invisible,         // a format character: no ink, no advance
	Placeholder(char), // a box (or ▒ in a cell), labelled with this code point
}
pub enum Level { Cells437, Ansi1252, Unicode } // L0, L1 and L3 of the first draft; pages (L2) and shaping (L4) are later
```

Each shell names its level; GTK's `Measure` reports which code points a face has (`Glyph` or `Placeholder`). The layout asks `Measure::widths` for the 224 as today and a new `Measure::advance(font, char) -> Option<u16>` for anything else, whose default is `None` (the placeholder advance), so the retro builds make no new call.

### The placeholder (CORE-213)

As the first draft, which the owner accepted: **a box at the cap height with the hex code point in two rows of a 3 × 5 pixel digit font** at 12 pixels a line or more (`04` over `16` for U+0416; three digits a row for an astral code point, `01F` over `600`), **a plain box** below 12 pixels, **`▒`** in a text cell. The box is drawn in the text colour at half intensity where the build has one. The digits are one 16 × 5 bitmap (80 bytes), a resource on Windows 3.x.

- **The status line** names the character when the caret is beside a placeholder: `U+0416 Cyrillic: this character cannot be shown here. It is kept.` The block names are `quill_text::blocks` (about 70 names for the blocks a document is likely to hold, `Other` for the rest, about 900 bytes [E]; string resources on Windows 3.x).
- **On open**, once: `This document has 37 characters that cannot be shown here. They are kept.`
- **Printing** on DOS, Windows 3.x and Windows 95 prints the same box; on GTK the page's glyphs.

### Fonts on each build

| Build | Font | What it covers in v1 |
|---|---|---|
| DOS text | The adapter's CP437 font | 256 cells: CP437; `quill_text::cp437::from_unicode` (the inverse of the table the grid already has) |
| DOS graphics | `QUILL.FNT`, 72 strikes of the 224 Windows-1252 characters | Windows-1252; `QUILL.FNT` is unchanged in v1 |
| Windows 3.x | The system's TrueType and raster fonts through `TextOut` | Windows-1252 (`ANSI_CHARSET`) |
| Windows 95, 98 and Me | The same through `TextOutA` and `ExtTextOutA` | Windows-1252 |
| GTK | `quill-fonts` over the system's fonts, with the vendored test fonts last | Per glyph: the named face, then the face's class fallbacks, then any installed face that has the glyph (a cmap scan, cached per code point), then a box |
| Terminal | The terminal's font | What the terminal draws, single width |

## Typing on each build

| Build | v1 | Later |
|---|---|---|
| DOS (both screens) | INT 16h's byte is **CP437** and becomes its real character (`π`, `Σ`, `│` arrive as themselves, where today they become `?`); Alt+X | The active code page through INT 21h AX=6601h (WP-UNI-12) |
| Windows 3.x | `WM_CHAR` as Windows-1252, as today; Alt+X | The stock font's charset for language editions |
| Windows 95, 98 and Me | `WM_CHAR` as Windows-1252; Alt+X | The input locale's code page through `GetKeyboardLayout`, `GetLocaleInfoA` and `MultiByteToWideChar` (all exported by Windows 95, none a `W` name), `WM_INPUTLANGCHANGE` (WP-UNI-11) |
| Terminal | UTF-8 from the terminal, any character | |
| GTK | Anything the input method commits, preedit included; LNX-14's refusal goes | |
| WinUI | TSF and `WM_CHAR` in UTF-16, joined into code points (WUI-95) | |

**Alt+X (CORE-223)**, on every build: with up to six hex digits before the caret (optionally `U+`), Alt+X replaces them with that character, as one undo step labelled `Insert Character`; with no digits before the caret, Alt+X replaces the character before the caret with its `U+XXXX`. A value that is not a scalar (a surrogate, above U+10FFFF, a C0 control other than tab) says `That is not a character Quill can insert.` On the DOS text screen Alt+X is free [U: check the menu accelerators in `keys.rs`]; on Windows 3.x and 95 it is a `WM_SYSKEYDOWN` that no menu takes, as the menu bar has no X mnemonic [U].

## Files

### `.QLD` 1.1 (CORE-219)

The first draft's design, kept whole: **`TEXT` stays one byte per code point** (the Windows-1252 byte, or `?` for a character Windows-1252 lacks), so a 1.0 reader sees the right length, runs, paragraph marks and objects; **`UNIC`**, inside `LIST STRY` after `OBJS` and only when the story has a character Windows-1252 lacks, lists runs of `(position u32, characters u16, bytes u16, UTF-8)` by code point position. A file with no `UNIC` writes `HEAD` 1.0, byte for byte as today; a file with one writes 1.1. The failure rules stand: a run out of order, past the count, or over a `TEXT` byte that is not `?` makes the reader ignore that story's whole `UNIC` and say `Some characters in this document could not be read and show as ?.`; invalid UTF-8 inside a run is U+FFFD, counted; never `BadFile` for `UNIC` alone. The sparse patch is the first draft's recommendation and the default taken (owner question 13).

**Recovery files** are `.QLD` with `RCOV`, so they carry `UNIC` the same way; the `RCOV` caret counts code points and is `snap`ped on recovery. **Reading old files:** every 1.0 file reads as today, and a frozen copy of the 1.0 reader (`crates/quill-formats/tests/compat/qld_1_0.rs`, WP-UNI-2) opens every 1.1 file in the corpus in CI, showing `?`.

### RTF (CORE-220)

- **Write:** `\uc1` in the header; Windows-1252 characters above 0x7F as `\'hh`, as now; any other character as `\uN?` with `N` the signed 16-bit value and an astral one as its surrogate pair (`\u-10179?\u-8704?`); `\ansicpg1252` stays; font names stay Windows-1252.
- **Read:** CORE-61's reader, with `rtf::narrow` removed: `\uN` and `\'hh` in the font's code page go straight into the UTF-8 text. `{\upr{…}{\*\ud{…}}}` takes `\ud`. A lone surrogate is U+FFFD and counted. Double-byte code pages (932, 936, 949, 950, 1361) stay `?` and counted in v1: their `\'hh` pairs could be decoded only with 100 KB-class tables, and Word writes `\u` beside them anyway.
- **Round trip:** every scalar value survives Quill to Quill; LibreOffice reads Quill's `\u` (`scripts/check-rtf-libreoffice.sh` with the corpus); WordPad on Windows 95 shows the `?` fallback [manual, optional].

### Plain text (CORE-221)

- **Reading:** a UTF-8 byte order mark, a UTF-16 LE or BE byte order mark, then the C4 rule (valid UTF-8 is UTF-8, anything else is the build's code page: Windows-1252 on Windows and GTK, CP437 on DOS), which is now lossless both ways. A file that is not valid UTF-8 and has bytes above 0x7F says `This file is not valid UTF-8 and was read as Windows-1252 text.` (or `DOS text`). UTF-16 lone surrogates are U+FFFD, counted.
- **Saving:** **Text (UTF-8)** joins Save As on every build, next to Text (Windows) and Text (DOS); a save to a single-byte type writes `?` for what it lacks and says `12 characters could not be saved as Windows text and were written as ?.`
- **The default plain text type** stays what each build has (DOS: DOS text; Windows: Windows text; GTK: UTF-8); a setting for it is the preferences plan's to add (below).

### Clipboard (CORE-222)

`quill-clip` gains `TextUtf8` and `TextUtf16` flavours beside `Rtf`, `Text1252` and `Dib`. **Windows 3.x:** RTF (now with `\u`) and `CF_TEXT` (Windows-1252, `?`). **Windows 95:** RTF, `CF_UNICODETEXT` (format number 13; reading and writing it needs no new import, and Windows 95 synthesizes `CF_TEXT` from it and back [U on the 95 VM]) and `CF_TEXT`. **GTK:** `text/rtf` and `text/plain;charset=utf-8`. **WinUI:** RTF and `CF_UNICODETEXT`. **DOS and the terminal:** the document store's flavours (WP-ED-5's default transport). An object in exported text is U+FFFC. The ReactOS host link is already UTF-8 (SITE-35).

## Search, replace and spelling

**Find and Replace (CORE-224):** case-sensitive matching compares UTF-8 bytes (self-synchronizing, so a match always starts on a boundary), then rejects a match that starts or ends inside a cluster; case-insensitive matching decodes and folds both sides through `quill_text::fold` and `compose`; `^uNNNN` in the Find field means that code point, on every build, so a placeholder can be found where it cannot be typed; whole word uses CORE-216. Replace inserts `&str`. The Find box itself takes `Key::Char(char)` from U1b, so it holds what the build can type.

**Spelling (CORE-224):** the scanner decodes UTF-8 and splits by CORE-216; a word converts to the dictionary's code page (Windows-1252 for the English dictionaries, from the `.DIC` header's code page byte, 1252 when absent) and **a word that does not convert is skipped**, neither flagged nor offered; suggestions come back in the dictionary's code page and go in as UTF-8. `is_letter` gains the seven Windows-1252 letters it misses (`Š š Ž ž Œ œ Ÿ`) [V `crates/quill-spell/src/lib.rs:204`]. The user dictionary `QUILL.UDC` stays Windows-1252; adding a word it cannot hold says `Quill can add only words in the dictionary's alphabet.`

## PDF

**Windows 95 (W9X-126):** the standard 14 fonts stay (W9X-92). A character WinAnsiEncoding lacks is drawn as the same placeholder box, as path operators, and the real text goes in the text layer through a marked-content span with `/ActualText` (UTF-16BE), so copying and searching the PDF give the character. About +2 to +3 KB of `quill-pdf` [E].

**GTK (LNX-96):** GTK prints and exports through `quill-pdfdoc` (LNX-80), and GTK shows the characters on screen, so its PDF must too: the faces a document uses are **embedded as TrueType subsets** (`CIDFontType2`, `Identity-H`, a `ToUnicode` CMap), glyphs taken from the same font files `quill-fonts` measured with, so the page is laid out and drawn by one engine. A face whose licence forbids embedding (`OS/2.fsType` bit 1) is drawn with the placeholder box and `ActualText`. Embedding on Windows 95 through `GetFontData` is later (it is an ANSI-neutral GDI export [U on 95]).

## How it meets the other plans

| Plan | What Unicode gives it | What it must do | Kind |
|---|---|---|---|
| **Preferences** ([preferences.md](preferences.md)) | The plain text types (UTF-8, Windows, DOS) after WP-UNI-4 | The Files page gains *Save plain text as* (`[Quill] TextEncoding` = `ansi`, `oem` or `utf8`, the build's default as today) in the preferences package that builds that page, once WP-UNI-4 has merged; `QUILL.INI` stays Windows-1252 (CORE-151). The text fields of the Options window take `Key::Char(char)` (WP-ED-10) | Soft |
| **Reveal Codes** ([reveal-codes.md](reveal-codes.md)) | `Pos`, `chars`, `cluster_at`, `repertoire::show`, `quill_text::blocks` | `WP-CODES-1` branches after WP-ED-11 (already a hard dependency); **CORE-228**: a character the build draws as a placeholder appears in the pane as a character token labelled `[U+0416]`; an invisible format character as its short name (`[ZWJ]`, `[ZWSP]`, `[WJ]`, `[LRM]`, `[RLM]`, `[VS16]`, `[BOM]`); a combining mark after its base as `[U+0301]`; Delete on such a token deletes the character. The names are a 12-entry table in `quill_text::blocks` (WP-UNI-1) | Hard for the labels (WP-UNI-1 and WP-UNI-3 before WP-CODES-1's character tokens) |
| **Status bar** ([status-bar.md](status-bar.md)) | `App::caret_column` (WP-ED-10, clusters from WP-UNI-3) | **CORE-228**: Col counts clusters from the line's start on a device with pixels, and cells on the text screens (as today); the word count (WP-BAR-8) uses CORE-216; the placeholder sentence is a message in the lead (CORE-205). WP-BAR-2's status model reads `caret_column` instead of `caret - l.start` | Hard for WP-BAR-2 (after WP-ED-10, which it already follows by waiting for the freeze) |
| **Clipboard** ([clipboard.md](../clipboard.md), WP-ED-5, WP-ED-6) | The flavours above | WP-UNI-6 follows WP-ED-5 and WP-ED-6; `docs/clipboard.md`'s line "UTF-8 and UTF-16 arrive with `quill-text`" becomes the table | Hard (WP-UNI-6 after both) |
| **Accessibility** (`quill-a11y`, WP-ARCH-8, LNX-24) | UTF-8 lines and the real characters, even where a box is drawn | Offsets in code points for GTK, UTF-16 for WinUI; WP-ARCH-8 already waits for WP-ED-11 | Soft |
| **Optional modules** ([modules.md](modules.md)) | — | The Unicode code stays in the EXE (modules plan section 8); its tables are resources on Windows 3.x; **WP-MOD-4 (the file dialog in `QUILLDLG.DMD`) before WP-UNI-5** keeps the DOS EXE under the stop (below) | Soft, budget |
| **Host integration** ([host-integration.md](host-integration.md)) | Characters outside Windows-1252 paste into Quill on the ReactOS page once WP-UNI-6 lands | Its "Characters" limit line changes then | Soft |
| **Web demo** | — | The Try Quill page ships the DOS build; nothing else | None |

## Costs and budgets

### Per package [E]

| Package | `QUILLDOS.EXE` | Windows 3.x code | Windows 3.x data | `QUILL32.EXE` |
|---|---:|---:|---:|---:|
| WP-UNI-1 tables (nothing links them yet; 13.6 KB of tables when linked in full, see CORE-218) | 0 | 0 | 0 | 0 |
| WP-UNI-2 test documents | 0 | 0 | 0 | 0 |
| WP-ED-4 U1a | +0 to +0.5 KB | +0 to +0.5 KB | 0 | +0 to +0.5 KB |
| WP-ED-10 U1b | +0 to +1.5 KB | +0 to +1 KB | 0 | +0 to +1 KB |
| WP-ED-11 U2 | +5 to +8 KB | +4 to +7 KB (auto, VIEW, FILE) | 0 to +16 B | +4 to +6 KB |
| WP-UNI-3 placeholders, clusters, breaks | +3.5 to +5.5 KB (tables about 1.7 KB) | VIEW +1.5 to +2.5 KB | +20 to +60 B | +3 to +4 KB |
| WP-UNI-4 files | +3 to +4 KB | FILE +2.5 to +3.5 KB | 0 to +20 B | +2.5 to +3.5 KB |
| WP-UNI-5 typing | +1 to +1.5 KB | MAIN +0.8 to +1.2 KB | 0 to +10 B | +0.8 to +1 KB |
| WP-UNI-6 clipboard | +0.3 KB | FILE +0.5 to +1 KB | 0 | +0.8 to +1.2 KB |
| WP-UNI-7 Find, spelling, words | +1.5 to +2.5 KB (fold table about 0.5 KB) | EDIT +1.5 to +2 KB | 0 to +10 B | +1.5 to +2 KB |
| WP-UNI-9 PDF | — | — | — | +2 to +3 KB |
| **v1 total** | **+14.3 to +23.3 KB** | **+10.8 to +18.7 KB** | **+20 to +116 B** | **+15 to +22 KB** |

### DOS (DOS-112)

`QUILLDOS.EXE` has 4,608 bytes left of 442,368. The series alone takes it to about 452 to 461 KB [E]. Together with the editor ledger (+31 to +48 KB, which already counts U1a, U1b and U2) and Reveal Codes (+11 to +16.5 KB), the modules plan's projection of 438 to 458 KB **with** `QUILLDLG.DMD` becomes about **447 to 473 KB**, under the 480 KB stop; **without** the module it is about 489 to 517 KB, past it. So:

1. Each Unicode package raises the budget as DOS-70 and DOS-52 say: savings first, then the measured cost rounded up to 4 KB, in a DOS row of its own with the floppy's free bytes. WP-ED-11 is expected to be the first raise.
2. **WP-MOD-4 should merge before WP-UNI-5** (owner question 10). If it has not and the next package would pass 480 KB, that package stops and asks, as DOS-70 says.
3. **The fallback, if room is short:** the tables (fold, cluster, breaks, compose, width, scripts: about 2.8 KB) move to a data file `QUILLUNI.TBL` beside `QUILL.FNT`, read once at start through the same `ReadStore` hook Windows 3.x uses, and the block names give way to the hex box alone (about −0.9 KB). Neither is planned to be needed.
4. The floppy: +23 KB of code costs about 13 KB packed, against about 427 KB free.

**Speed on a 386 and a 486** (the first draft's gate, kept): a key in `tests/bench/w16-type.txt` and the DOS key-to-paint run within 10 % of today on the English sample and within 25 % on a Canadian French sample, measured by WP-ED-11 and again by WP-UNI-3.

### Windows 3.x (W16-116)

The data segment has about 1.9 KB free and nothing merges below 1,024. On Windows 3.x **every `const` table is data segment** (`.rodata` is near data), so the rule is absolute: **no Unicode table is linked as data on Windows 3.x.** `quill-text`'s tables are written to one `RCDATA` resource, `assets/unicode/UNITAB.BIN` (id 101, about 3 KB), beside `CPAGES.BIN` (id 100), and reached through a `ReadStore` hook like `to_unicode`; the block names and the messages are string resources (WP-ARCH-9's mechanism); the 3 × 5 digits are a bitmap resource. Each Windows 3.x package reports the data segment before and after and `llvm-nm -S` of `quill_text` symbols in `.rodata`, which must be zero bytes of tables. The painter's transcoding buffer is on the stack (128 bytes), so `check_stack_log.py` keeps its 2,048-byte margin. Code goes into the segments named above; `check_segments.py` against `segments.json` decides, and a `TEXT` segment for `quill_text::*` is added if auto passes its budget.

### Windows 95 (W9X-125)

**No new import in v1**: `CF_UNICODETEXT` is a format number; UTF-8 to UTF-16 and back is Rust. The input locale work that needs `MultiByteToWideChar`, `GetKeyboardLayout` and `GetLocaleInfoA` is later (WP-UNI-11), and none of those is a `W` name. Wide GDI calls (`ExtTextOutW`, `GetCharWidthW`), if ever, are loaded with `GetProcAddress` at run time on Windows 2000 and later, never imported (owner question 5).

### GTK and WinUI

No size limit. GTK's per-code-point fallback cache is a `HashMap<char, (face, advance)>` per font, cleared with the font cache.

## Tests

### Property tests (proptest, which `quill-core` already uses)

- **The model:** random edits (insert a generated `&str` mixing 1- to 4-byte characters, combining marks, ZWJ sequences, flags and the meaning bytes; delete a random `Range<Pos>`; set formats; undo; redo) against a `String` model: after every step the text equals the model, is valid UTF-8, and every piece boundary, the caret, the selection, every layout cursor and every checkpoint is on a code point boundary (WP-ED-4, strengthened by WP-ED-11).
- **`snap` and `check`:** any raw `u32` snaps to a boundary at or below it; `check` accepts exactly the boundaries.
- **Clusters:** `next_cluster` then `prev_cluster` returns to the start; no cluster boundary falls inside a code point or between a base and its mark; the cluster table agrees with `GraphemeBreakTest.txt` on every line whose characters are in scope (WP-UNI-1, WP-UNI-3).
- **Line breaks:** the break rule agrees with `LineBreakTest.txt` on the lines whose classes the subset covers, and the excluded lines are listed by class in the test (WP-UNI-1).
- **Formats:** any scalar sequence round-trips through `.QLD`, RTF, UTF-8 text, UTF-16 LE and BE text and the clipboard flavours; a single-byte writer loses exactly what it lacks and counts it; the frozen 1.0 reader reads every 1.1 file with `?` in exactly the `UNIC` positions (WP-UNI-4).
- **Find:** a found range starts and ends on cluster boundaries; folding is idempotent; `^uNNNN` finds what Alt+X inserted (WP-UNI-7).

### The Unicode test document set (`tests/docs/unicode/`, WP-UNI-2)

One file per case, each listed in `tests/docs/unicode/manifest.toml` with its source, its scalar count, what each build is expected to show (shown, composed, placeholder) and a `.expect` file of its code points one a line, which a host test reads through `quill-formats` and compares:

- **Scripts:** `fr-ca.rtf`, `fr-ca.txt` (Canadian French with curly quotes and dashes), `pl-cs-1250.rtf` (`\fcharset238`), `ru-1251.rtf` (`\ansicpg1251`), `ru-u.rtf` (`\uc1\u…`), `ru-word97-upr.rtf`, `el-1253.rtf`, `tr-1254.rtf`, `lt-1257.rtf`, `hy-ka.txt` (Armenian and Georgian), `vi-nfc.txt` and `vi-nfd.txt` (the same Vietnamese both ways), `zh-ja-ko.txt` (ideographs, kana, Hangul, CJK punctuation, an Extension B ideograph), `symbols.txt` (arrows, maths, box drawing, dingbats, currency), `he.txt`, `ar.txt`, `hi.txt`, `th.txt` (placeholders, kept).
- **Clusters and invisibles:** `combining.txt` (stacked marks, a mark at a paragraph's start), `emoji.txt` (single, ZWJ family, skin tones, flags, keycaps, VS15 and VS16), `invisible.txt` (ZWJ, ZWNJ, ZWSP, WJ, a byte order mark inside text, LRM, RLM, U+FFFC, U+0001 from outside).
- **Encodings:** `utf8-bom.txt`, `utf16le-bom.txt`, `utf16be-bom.txt`, `invalid-utf8.txt`, `cp1252-valid-utf8.txt` (the C4 caveat), `cp437.txt` (box drawing and Greek, for the DOS reader).
- **RTF edge cases:** `uc0.rtf`, `uc2.rtf`, `u-negative.rtf`, `lone-surrogate.rtf`, `astral-pair.rtf`, `dbcs-932.rtf`.
- **`.QLD`:** `latin-1-0.qld` (a 1.0 file that must stay 1.0 on save), and from WP-UNI-4 `mixed-1-1.qld` and one `unic-damaged-*.qld` per failure mode (made by a script from `mixed-1-1.qld`, so they cannot drift).
- **The sampler:** `sampler.qld`, one page with a line per script and case above, which every build's screenshots use.

### Goldens and scenarios

- Screen goldens: the DOS text screen with `▒` and CP437's Greek (25 and 50 rows); DOS graphics with hex boxes at 8, 12 and 24 points; Windows 3.11 `/PLAY` with boxes and the status line; Windows 95 under Wine; GTK with Cyrillic, CJK and a box (WP-UNI-3, WP-UNI-8).
- Scenarios: `m16-alt-x.keys` (Alt+X both ways), `m16-cluster-caret.keys` (Left, Right, Delete, Backspace over `e` and U+0301 and a ZWJ family), `m16-cp437-keys.keys` (DOS Alt+227 types `π`), `m16-find-fold.keys` (Find `ЖУК` finds `жук`; `é` finds `é`), each played on every build its package names.
- Every existing golden stays byte-identical through WP-ED-4, WP-ED-10 and WP-ED-11.

### Cross-build (WP-UNI-10)

The sampler, typed by a scenario with Alt+X on DOS, Windows 3.11 and Windows 95 and by the input method on GTK, saved as `.QLD` and RTF on each, and the files byte-compared against the host's; then each build opens every other build's file and its screen is compared with its own golden.

## Evidence

Screenshots go to `~/Documents/Liminal HQ/Quill/Unicode/<WP-ID>/`, one PNG per platform and state, named `<platform>-<state>.png` with the platforms of [work-packages.md](../work-packages.md#gates-and-evidence) (`dos-text-25`, `dos-text-50`, `dos-gfx-12h`, `dos-gfx-vbe`, `win16-311`, `win32-wine`, `win95` where asked, `gtk-xvfb`, `tty`), taken by the repository's own scripts on the builder's own display, never the host desktop. The minimum per package is in its section; the closing package (WP-UNI-10) collects the sampler on every build into `~/Documents/Liminal HQ/Quill/Unicode/sampler/` with a `README.txt` saying which commit each came from.

## Decisions

Ids reserved for this plan in [id-ranges.toml](../id-ranges.toml): **CORE-210 to CORE-229, DOS-110 to DOS-114, W16-115 to W16-119, W9X-125 to W9X-129, LNX-95 to LNX-99, WUI-95 to WUI-97.** The existing Unicode ids stay where they are: CORE-61 (U0, done), CORE-100 (U1a and U1b), CORE-103 (U2, supersedes D11) and CORE-104 (`MAX_TEXT` in bytes), in the editor track's block. Each package writes its rows in `docs/decisions.md` when it lands; the rows are not written by this plan.

| Id | Decision | Package |
|---|---|---|
| CORE-210 | The v1 scope and the rule that no door opens before the placeholders (this page's scope and migration sections) | WP-UNI-3 writes it |
| CORE-211 | Positions: UTF-8 bytes on boundaries in memory, code points in files, clusters as a rule only, UTF-16 at the edges | WP-ED-4 writes it beside CORE-100 |
| CORE-212 | One cluster rule on every build; Backspace by code point except emoji sequences | WP-UNI-3 |
| CORE-213 | Repertoire, levels, the placeholder, width | WP-UNI-3 |
| CORE-214 | No normalization of stored text; display and Find compose the Windows-1252 letters | WP-UNI-3 (display), WP-UNI-7 (Find) |
| CORE-215 | Line break opportunities, a UAX #14 subset | WP-UNI-3 |
| CORE-216 | Word classes beyond ASCII (extends CORE-45) | WP-UNI-7 |
| CORE-217 | Simple case folding for the alphabetic scripts in scope | WP-UNI-7 |
| CORE-218 | `quill-text`'s Unicode tables, generated from UCD 16.0.0, resources on Windows 3.x | WP-UNI-1 |
| CORE-219 | `.QLD` 1.1 and `UNIC` (supersedes nothing; extends the format) | WP-UNI-4 |
| CORE-220 | RTF `\uc1\uN?` writing and reading into UTF-8 (revises CORE-61's narrowing) | WP-UNI-4 |
| CORE-221 | Plain text: byte order marks, UTF-16, Text (UTF-8), C4 kept (revises C4 and D190) | WP-UNI-4 |
| CORE-222 | Clipboard flavours and U+FFFC (revises D26, ARCH-4) | WP-UNI-6 |
| CORE-223 | Alt+X on every build | WP-UNI-5 |
| CORE-224 | Find, Replace and spelling beyond ASCII | WP-UNI-7 |
| CORE-225 | The test document set and the frozen 1.0 reader | WP-UNI-2 |
| CORE-226 | Right-to-left and complex scripts as placeholders in v1 | WP-UNI-3 |
| CORE-227 | Emoji in v1 | WP-UNI-3 |
| CORE-228 | Columns count clusters; Reveal Codes labels for characters | WP-UNI-3 (column), WP-CODES-1 (labels) |
| CORE-229 | Spare | |
| DOS-110 | The text screen shows CP437's repertoire; CP437 keys are real characters | WP-UNI-3, WP-UNI-5 |
| DOS-111 | DOS graphics placeholders; `QUILL.FNT` unchanged in v1 | WP-UNI-3 |
| DOS-112 | The Unicode series' budget: DOS-70 raises, WP-MOD-4 first, the `QUILLUNI.TBL` fallback | WP-ED-11 writes it with its raise |
| DOS-113, DOS-114 | Spare (WP-UNI-12 later) | |
| W16-115 | The painter transcodes runs and draws placeholders | WP-UNI-3 |
| W16-116 | No Unicode table as data; `UNITAB.BIN` | WP-UNI-3 |
| W16-117 | `WM_CHAR` stays Windows-1252; Alt+X | WP-UNI-5 |
| W16-118, W16-119 | Spare (WP-UNI-12 later) | |
| W9X-125 | The painter transcodes and draws placeholders; `CF_UNICODETEXT`; no new import | WP-UNI-3, WP-UNI-6 |
| W9X-126 | PDF placeholders and `ActualText` | WP-UNI-9 |
| W9X-127 to W9X-129 | Reserved for WP-UNI-11 (charset fonts, input locales, wide calls) | Later |
| LNX-95 | GTK shows every unshaped left-to-right script through per-glyph fallback; LNX-14's refusal goes | WP-UNI-8 |
| LNX-96 | GTK PDF embeds TrueType subsets | WP-UNI-9 |
| LNX-97 | The terminal client: UTF-8 in and out, single width, `▒` for the rest | WP-UNI-3, WP-UNI-5 |
| LNX-98, LNX-99 | Spare | |
| WUI-95 | WinUI follows GTK's rules; UTF-16 positions for TSF and UI Automation | When the client exists |
| WUI-96, WUI-97 | Spare | |

## Work packages

### Order

| Wave | ED lane (`quill-app`) | Side | UI track | Waits for |
|---|---|---|---|---|
| Now | (WP-ED-3, as planned; WP-ED-1 and WP-ED-2 have merged) | **WP-UNI-1** tables, **WP-UNI-2** test documents | — | — |
| 2 | **WP-ED-4** U1a | — | — | WP-ED-3 (WP-ED-1 and WP-ED-2 merged) |
| 8 (freeze) | **WP-ED-10** U1b, then **WP-ED-11** U2, then **WP-UNI-3** placeholders | (WP-ED-6 if late) | — | WP-ARCH-7, WP-ED-4, WP-UNI-1 |
| 9 | WP-ED-12, then **WP-UNI-5** typing, then WP-PREF-2 | **WP-UNI-4** files (after WP-ED-12) | **WP-UNI-8** GTK | the freeze ended |
| 10 | **WP-UNI-7** Find and spelling, then WP-PREF-5 | **WP-UNI-6** clipboard (after WP-ED-6) | — | WP-UNI-5 for WP-UNI-7 |
| 11 | (WP-PREF-5 and on, as planned) | **WP-UNI-9** PDF | — | WP-UNI-4, WP-UNI-8 |
| 12 | — | **WP-UNI-10** across the builds | — | every WP-UNI package of v1 |

Three builders at most at once: in wave 9, WP-UNI-5, WP-UNI-4 and WP-UNI-8 are the three; WP-ARCH-8 and WP-CODES-1, which work-packages.md also puts in wave 9, take the next free slots. **Later, parked** (not v1, no wave): WP-UNI-11 (Windows 95, 98 and Me charset fonts and input locales, after a spike), WP-UNI-12 (DOS code pages and strike pages, Windows 3.x charset fonts), WP-UNI-13 (Insert > Symbol on every build), WP-UNI-14 (shaping, bidirectional text and colour emoji on GTK and WinUI), WP-UNI-15 (two-cell wide characters in the terminal).

### WP-UNI-1 — Unicode tables in `quill-text`

- **Finish line:** `quill-text` has `cluster`, `breaks`, `word`, `fold`, `compose`, `width`, `script` and `blocks` modules: `no_std`, no `alloc`, no floats, no panics, each a sorted range table with a lookup function, generated by `scripts/gen-unicode.py` from UCD 16.0.0 (`GraphemeBreakProperty.txt`, `emoji-data.txt`, `LineBreak.txt`, `DerivedGeneralCategory.txt`, `CaseFolding.txt`, `EastAsianWidth.txt`, `Scripts.txt`, `Blocks.txt`, `UnicodeData.txt` for the compositions), and `cp437::from_unicode`; every table can also be read from one byte blob (`assets/unicode/UNITAB.BIN`, written by the same script) so Windows 3.x can serve it from a resource; the tests check every table against the UCD test files where they exist (`GraphemeBreakTest.txt`, `LineBreakTest.txt`, in-scope lines) and against Python's `unicodedata` 16.0.0 otherwise. Nothing links the modules yet.
- **Owns:** `crates/quill-text/src/{cluster,breaks,word,fold,compose,width,script,blocks,unitab}.rs` (new), the `mod` lines and the `cp437::from_unicode` addition in `crates/quill-text/src/{lib,cp437}.rs`, `crates/quill-text/tests/unicode.rs` (new), `crates/quill-text/tests/ucd/` (the two test files, trimmed to the lines in scope, with their licence header), `scripts/gen-unicode.py` (new), `assets/unicode/UNITAB.BIN` and `assets/unicode/README.txt` (new), `assets/unicode/ucd-16.0.0.sha256` (new).
- **Must not touch:** every other crate; `codepage.rs` and `cpdata.rs` (CORE-61's); the Win16 store (WP-UNI-3 wires the resource).
- **Depends on:** nothing. Parallel with everything open today.
- **Budget:** 0 on every build (unlinked); the PR states each table's size in bytes and the blob's.
- **Tests:** `crates/quill-text/tests/unicode.rs`; `python3 -I scripts/gen-unicode.py --check` in G-HOST (`make validate`), with the UCD files read from `~/.cache/quill-ucd/16.0.0/` and checked against the committed SHA-256 list, skipped with a message when the cache is absent (CI runs `--check` against the committed outputs, which needs no download); G-HOST, G-MD.
- **Evidence:** none visual; the size table in the PR.
- **Decisions:** CORE-218.

### WP-UNI-2 — The Unicode test document set and the frozen 1.0 reader

- **Status:** built (CORE-225). As built: the host test is `crates/quill-host/tests/unicode_docs.rs`; each document also has `.clusters` and `.breaks` files (a reference for WP-UNI-3, written from CORE-212 and CORE-215 with `unicodedata`) and the manifest counts what each build shows; the sampler is `sampler.txt` until WP-UNI-4 makes `sampler.qld`; a synthetic 1.1 file, `skip-unic-1-1.qld`, stands in for `mixed-1-1.qld` so the frozen reader's skip of `UNIC` is proved now; `crates/quill-formats/tests/compat/qld_1_0.rs.sha256` and `scripts/check-frozen-reader.sh` (in `make validate` and CI) hold the frozen copy. `ru-word97-upr.rtf`'s `\upr` copy holds literal `?` characters, so today's reader refuses none of them.
- **Finish line:** `tests/docs/unicode/` holds the files of [the test document set](#the-unicode-test-document-set-testsdocsunicode-wp-uni-2) that exist before WP-UNI-4 (everything but the `.QLD` 1.1 files), `manifest.toml` and an `.expect` per file of the code points **as the format defines them**; a host test reads every file through today's readers and checks what today's code gives (Windows-1252 with `?`, counted) against an `.today` file, so the later packages flip each `.today` to the `.expect` as they open the doors; `crates/quill-formats/tests/compat/qld_1_0.rs` is a frozen copy of today's `.QLD` reader with a test that it reads `tests/docs/*.qld`; `scripts/gen-unicode-docs.py` writes the generated files (the UTF-16 and invalid ones, the sampler's text) so none is edited by hand.
- **Owns:** `tests/docs/unicode/**` (new), `crates/quill-host/tests/unicode_docs.rs` (new), `crates/quill-formats/tests/compat/**` (new), `scripts/gen-unicode-docs.py` (new).
- **Must not touch:** any `src/` file; the existing `tests/docs` files and goldens.
- **Depends on:** nothing. Parallel with WP-UNI-1.
- **Budget:** 0.
- **Tests:** the new host tests; G-HOST, G-MD.
- **Evidence:** none.
- **Decisions:** CORE-225.

### WP-ED-4 — Unicode U1a (updated)

As [work-packages.md](../work-packages.md#wp-ed-4--unicode-u1a-text-positions-as-pos), with the API names of [`Pos` and the core's API](#pos-and-the-cores-api) and one addition: the boundary property test generates multi-byte text through a test-only `Text::insert_utf8_for_tests`, so the invariant is proved for UTF-8 before the buffers hold it. Decision rows CORE-100 and CORE-211.

### WP-ED-10 — Unicode U1b (updated, freeze)

As [work-packages.md](../work-packages.md#wp-ed-10--unicode-u1b-every-caller-on-pos-and-characters-freeze), plus: `App::caret_column(s) -> u32` (characters from the line's start on a device with pixels, cells in text cells), which the status line and WP-BAR-2 use instead of `caret - l.start`; Find's box and every dialog text field take `Key::Char(char)`; the terminal client's key map hands `char`. No behaviour change.

### WP-ED-11 — Unicode U2 (updated, freeze)

As [work-packages.md](../work-packages.md#wp-ed-11--unicode-u2-utf-8-in-the-buffers-freeze), with the rule made explicit: **every entry point** (keys, paste, the RTF and text readers, `.QLD`) narrows to Windows-1252 exactly as today, so no document holds a character outside it until WP-UNI-3 has merged; the RTF reader produces UTF-8 and calls `rtf::narrow` on the way in. Writes DOS-112 with its budget raise.

### WP-UNI-3 — Placeholders, clusters and line breaks (freeze)

- **Finish line:** `quill_core::repertoire` (`Show`, `Level`) and the cluster API (`next_cluster`, `prev_cluster`, `cluster_at`) exist; Left, Right, Delete, Backspace, double click, the mouse and Shift selection follow CORE-212; layout breaks by CORE-215 and measures characters outside the 224 with `Measure::advance` (default: the placeholder's three quarters of an em); every painter draws `Show`: `quill-paint` (DOS graphics, and GTK's page through it where it shares the code) with the hex box, `paint.rs` and `quill-grid` (DOS text, the terminal) with CP437 and `▒`, Windows 3.x `gdi.rs` and Windows 95 `gdi.rs` by splitting a run at each placeholder and transcoding the rest to Windows-1252 into a 128-byte stack buffer; composed letters draw as their Windows-1252 letter (CORE-214); the status line names the character beside the caret and the open message counts placeholders; Col counts clusters; Windows 3.x reads the tables from `UNITAB.BIN` (id 101) and the names from string resources. Still no entry point accepts a character outside Windows-1252.
- **Owns:** the freeze set; `crates/quill-core/src/{repertoire,cluster}.rs` (new) and `layout.rs`'s measure and break paths; the caret, Delete and Backspace arms and `caret_column` in `crates/quill-app/src/lib.rs`; `crates/quill-app/src/paint.rs`'s page cells and status position; `crates/quill-paint/src/page.rs`'s text runs and a new `placeholder.rs`; `crates/quill-grid` (cell drawing of a char); `win16/examples/quill/{gdi,store}.rs`, the resource script lines for `UNITAB.BIN`, the digits bitmap and the strings; `win9x/quill-win9x/src/gdi.rs`; `linux/quill-tty/src/paint.rs`; the new goldens and `m16-cluster-caret.keys`.
- **Must not touch:** the readers and writers (WP-UNI-4); key maps beyond the Delete and Backspace arms (WP-UNI-5); `quill-fonts` (WP-UNI-8); `linux/quill-gtk` beyond what `quill-paint` changes through its API (the GTK slice is WP-UNI-8).
- **Depends on:** WP-ED-11, WP-UNI-1.
- **Budget:** DOS +3.5 to +5.5 KB (a DOS-70 raise likely); Windows 3.x VIEW +1.5 to +2.5 KB, data +20 to +60 B, zero bytes of `quill_text` tables in `.rodata`; Windows 95 +3 to +4 KB, no new import.
- **Tests:** host tests that build UTF-8 documents with the test-only insert from the corpus files (Cyrillic, CJK, combining, emoji, invisible) and check layout, clusters and `Show`; the bench gate again; placeholder goldens at 8, 12 and 24 points on DOS graphics, `▒` on the DOS text screen and the terminal, Windows 3.11 `/PLAY`, Wine; every other golden unchanged; G-HOST, G-DOS, G-W16, G-W32, G-GTK, G-DISK.
- **Evidence:** the sampler opened through a test entry point on `dos-text-25`, `dos-gfx-12h`, `dos-gfx-vbe`, `win16-311`, `win32-wine`, `tty`, and the caret beside a box with the status sentence, in `Unicode/WP-UNI-3`.
- **Decisions:** CORE-210, CORE-212, CORE-213, CORE-214 (display), CORE-215, CORE-226, CORE-227, CORE-228 (column), DOS-110 (display), DOS-111, W16-115, W16-116, W9X-125 (painter), LNX-97 (display).

### WP-UNI-4 — Unicode in files

- **Finish line:** `.QLD` 1.1 with `UNIC` (read and write, 1.0 byte for byte when not needed, every failure rule); RTF writes `\uc1\uN?` and reads `\u` and code pages into UTF-8 without narrowing; plain text reads byte order marks and UTF-16 and writes Text (UTF-8) from Save As on every build; single-byte saves count and say what they replaced; recovery files carry `UNIC` and a code point caret; the corpus' `.today` files become `.expect`; the damaged `UNIC` files are generated.
- **Owns:** `crates/quill-formats/src/{qld,rtf,text}.rs`; the Save As types in `crates/quill-dlg` (the type table only); the open and save messages in `crates/quill-app/src/lib.rs` (named items only); `tests/docs/unicode/*.qld`, `*.expect`; SPEC's file format, RTF and plain text sections.
- **Must not touch:** the painters, key maps, `quill-clip`.
- **Depends on:** WP-UNI-3, WP-UNI-2, WP-ED-12 (it owns `qld.rs` and `rtf.rs` before this).
- **Budget:** DOS +3 to +4 KB; Windows 3.x FILE +2.5 to +3.5 KB, data 0 to +20 B; Windows 95 +2.5 to +3.5 KB.
- **Tests:** the format round-trip properties; the frozen 1.0 reader over every 1.1 file; `scripts/check-rtf-libreoffice.sh` over the RTF corpus; G-HOST, G-DOS, G-W16, G-W32, G-GTK.
- **Evidence:** `ru-1251.rtf` and `sampler.qld` opened on `dos-gfx-vbe`, `win16-311`, `win32-wine`, `gtk-xvfb`; the 1.0 reader's view of the same file (a text dump) in `Unicode/WP-UNI-4`.
- **Decisions:** CORE-219, CORE-220, CORE-221.

### WP-UNI-5 — Typing any character

- **Finish line:** Alt+X both ways on every build (CORE-223); DOS keys are CP437 characters (DOS-110); the terminal takes UTF-8 keys and writes UTF-8 (LNX-97); Windows 3.x and Windows 95 turn `WM_CHAR` into `char` through Windows-1252 (W16-117); the core's insert path stops narrowing; GTK's refusal (LNX-14) is removed in its key and input-method path, as a slice handed to the UI track if WP-UNI-8 has not merged, or done here if the UI lead agrees.
- **Owns:** the Alt+X arm in `key_edit` and its label in `crates/quill-app/src/lib.rs`; `dos/quill-dos/src/keys.rs`; `linux/quill-tty/src/keymap.rs`; the `WM_CHAR` arms of `win16/examples/quill/main.rs` and `win9x/quill-win9x/src/main.rs`; `m16-alt-x.keys`, `m16-cp437-keys.keys`.
- **Must not touch:** `quill-fonts`, `linux/quill-gtk/src/page.rs`.
- **Depends on:** WP-UNI-3; WP-ED-7 (word keys own `keys.rs` and the key maps before this).
- **Budget:** DOS +1 to +1.5 KB; Windows 3.x MAIN +0.8 to +1.2 KB, data 0 to +10 B; Windows 95 +0.8 to +1 KB.
- **Tests:** key tests for Alt+X (digits, `U+`, none, out of range, at a paragraph's start, undo); the two scenarios on G-DOS, G-W16, G-W32 and the terminal; G-HOST, G-GTK.
- **Evidence:** `ж` typed with Alt+X and `π` typed with Alt+227 on `dos-text-25`, `dos-gfx-vbe`; Alt+X on `win16-311`, `win32-wine`; Cyrillic from the input method on `gtk-xvfb`; in `Unicode/WP-UNI-5`.
- **Decisions:** CORE-223, DOS-110 (keys), W16-117, LNX-97 (keys).

### WP-UNI-6 — The clipboard in Unicode

- **Finish line:** `TextUtf8` and `TextUtf16` flavours; Windows 95 reads and writes `CF_UNICODETEXT` and prefers it over `CF_TEXT`; RTF on the clipboard carries `\u`; U+FFFC for objects in exported text; `docs/clipboard.md` updated.
- **Owns:** `crates/quill-clip/**`; `win9x/quill-win9x/src/clip.rs`; `win16/examples/quill/clip.rs` (the `CF_TEXT` conversion only); `docs/clipboard.md`; SPEC's clipboard section.
- **Depends on:** WP-ED-5, WP-ED-6, WP-UNI-3.
- **Budget:** DOS +0.3 KB; Windows 3.x FILE +0.5 to +1 KB, data 0; Windows 95 +0.8 to +1.2 KB, no new import.
- **Tests:** the clipboard round-trip property over the corpus; `win9x/tools/wine-clipboard.sh` with Cyrillic to and from Notepad; G-HOST, G-W16, G-W32.
- **Evidence:** Cyrillic pasted from Notepad into Quill and back on `win32-wine`, in `Unicode/WP-UNI-6`.
- **Decisions:** CORE-222, W9X-125 (clipboard).

### WP-UNI-7 — Find, Replace, spelling and words beyond ASCII

- **Finish line:** CORE-216, CORE-217 and CORE-224: Find folds case through `quill_text::fold` and composes through `compose`; `^uNNNN`; matches on cluster boundaries; one word rule over characters for Ctrl+arrows, double click, whole word, the spelling scan and the word count; spelling skips words the dictionary's code page cannot hold; `is_letter`'s seven letters; the Windows 3.x fold table from `UNITAB.BIN`.
- **Owns:** `crates/quill-app/src/{find,word,spelling,spell_live}.rs`; `crates/quill-spell/src/lib.rs`; `m16-find-fold.keys`.
- **Depends on:** WP-UNI-5 (the ED lane), WP-UNI-1, WP-ED-7.
- **Budget:** DOS +1.5 to +2.5 KB; Windows 3.x EDIT +1.5 to +2 KB, data 0 to +10 B; Windows 95 +1.5 to +2 KB.
- **Tests:** Find and spelling over the corpus; the word properties of WP-ED-7 over UTF-8 text; G-HOST, G-DOS, G-W16, G-W32.
- **Evidence:** Find `ЖУК` with Match case off highlighting `жук`, and a French word with a decomposed accent found by its precomposed form, on `dos-gfx-vbe`, `win16-311`, `win32-wine`, `gtk-xvfb`, in `Unicode/WP-UNI-7`.
- **Decisions:** CORE-214 (Find), CORE-216, CORE-217, CORE-224.

### WP-UNI-8 — GTK shows every unshaped left-to-right script (UI track)

- **Finish line:** `quill-fonts` measures and draws by `char` (`advance_units(f, char)`, glyph lookup by code point, `Measure::advance`), with per-glyph fallback through the system's faces cached per code point; GTK's page draws `Show::Glyph`; the `Repertoire` for GTK is `Level::Unicode` with CORE-226's scripts and emoji sequences as boxes; LNX-14's refusal is gone; the accessible text (LNX-24's snapshot) gives the real characters with code point offsets.
- **Owns:** `crates/quill-fonts/**`; `linux/quill-gtk/src/{page,doc,keys,a11y}.rs`; `linux/quill-gtk/tests/` additions.
- **Depends on:** WP-UNI-3; before or with WP-UNI-5's GTK slice.
- **Budget:** none (GTK).
- **Tests:** `quill-fonts` tests over the corpus with the vendored fonts and a test font that lacks Cyrillic (the fallback is taken); G-GTK with `chrome-shots.sh` on the sampler.
- **Evidence:** the sampler on `gtk-xvfb` in light and dark, in `Unicode/WP-UNI-8`.
- **Decisions:** LNX-95.

### WP-UNI-9 — PDF beyond WinAnsi

- **Finish line:** W9X-126 (`quill-pdf`: placeholder paths and `ActualText`) and LNX-96 (`quill-pdfdoc` on GTK: embedded `CIDFontType2` subsets with `ToUnicode`, `fsType` respected).
- **Owns:** `crates/quill-pdf/**`, `crates/quill-pdfdoc/**`, GTK's PDF call site.
- **Depends on:** WP-UNI-4, WP-UNI-8; WP-GTK-3b for the GTK half [check its state when starting].
- **Budget:** Windows 95 +2 to +3 KB.
- **Tests:** `pdftotext` on the sampler's PDF gives the sampler's text on both builds; `qpdf --check`; `pdffonts` shows the embedded subsets on GTK; G-HOST, G-W32, G-GTK.
- **Evidence:** the sampler's PDF rendered by `pdftoppm` from both builds, in `Unicode/WP-UNI-9`.
- **Decisions:** W9X-126, LNX-96.

### WP-UNI-10 — Across the builds (the finish line of v1)

- **Finish line:** the cross-build test of [Tests](#cross-build-wp-uni-10) passes on DOS, Windows 3.11, Windows 95 under Wine, GTK and the terminal; SPEC's non-goal "Wide-character (Unicode) Windows APIs and Unicode text" is replaced by the v1 scope (the APIs stay a non-goal on Windows 95); `docs/progress.md` marks Unicode v1 done; the sampler shots are collected.
- **Owns:** `tests/flows/unicode-cross.*` and the script that runs it, SPEC's non-goals and limits, `docs/progress.md`.
- **Depends on:** every v1 package.
- **Budget:** 0.
- **Evidence:** `Unicode/sampler/`.
- **Decisions:** none new.

## Briefs

Each brief follows the common brief of [work-packages.md](../work-packages.md#the-common-brief). Evidence goes to `~/Documents/Liminal HQ/Quill/Unicode/<WP-ID>/` rather than `Editor features/`.

### WP-UNI-1 (full)

```text
You are a Sonnet builder on Quill (liminal-hq-labs/quill). Read AGENTS.md, SPEC.md, docs/decisions.md
(ARCH-3, CORE-61), docs/work-packages.md (rules, budgets, gates, the common brief) and docs/plans/unicode.md
(all of it; your package is WP-UNI-1) before you start.

Where to work: never touch ~/source/experiments/quill16 except `git fetch` and `git worktree add/remove`.
  git -C ~/source/experiments/quill16 fetch origin
  git -C ~/source/experiments/quill16 worktree add -b feat/unicode-tables \
    ~/source/worktrees/quill/unicode-tables origin/main
Build output stays where cargo puts it; if another agent holds the cargo lock, set CARGO_TARGET_DIR under
~/.cache/quill-target/unicode-tables and say so in the PR. /tmp is a ramdisk: keep large files out of it.
The UCD files go in ~/.cache/quill-ucd/16.0.0/ (download from https://www.unicode.org/Public/16.0.0/ucd/
and .../ucd/auxiliary/ and .../ucd/emoji/; record their SHA-256 in assets/unicode/ucd-16.0.0.sha256).
Keep `df -h /home` above 20 GB.

Package WP-UNI-1: the Unicode tables the rest of the series needs, in quill-text, linked by nothing yet.

1. Read crates/quill-text (lib.rs, cp437.rs, codepage.rs, cpdata.rs) and scripts/gen-codepages.py: copy
   their style. Every new module is no_std, no alloc, no floats, no panics, no indexing that can panic
   (use .get()), with the crate's clippy deny block, and SPDX headers.
2. Write scripts/gen-unicode.py (python3 -I, standard library only), modelled on gen-codepages.py: it
   reads the UCD files from ~/.cache/quill-ucd/16.0.0/ after checking them against the SHA-256 list,
   and writes, between generated-code markers, the tables of:
   - cluster.rs: the extend set of CORE-212 (Grapheme_Cluster_Break Extend, ZWJ, SpacingMark limited to
     the scripts in scope, Regional_Indicator, Extended_Pictographic from emoji-data.txt, the emoji
     modifiers, variation selectors and tags). API: `fn joins(prev: char, next: char, ri_odd: bool) ->
     bool` plus `fn is_extend(c)`, `fn is_regional(c)`, `fn is_pictographic(c)`.
   - breaks.rs: CORE-215's subset: `enum Brk { Space, After, Ideo, Close, Open, Glue, Other }` from
     LineBreak.txt classes (SP, BA, HY, B2, ZW → After or Space; ID, H2, H3, JL, JV, JT and CJ → Ideo;
     CL, CP, NS, EX, IS → Close; OP → Open; GL, WJ, ZWJ → Glue; the rest Other), and
     `fn break_between(a: Brk, b: Brk) -> bool` with the rules on the plan's page.
   - word.rs: `enum Class { Letter, Punct, Space, Ideo }` from DerivedGeneralCategory (L*, M*, Nd →
     Letter; P*, S* → Punct; Z* → Space; ideographs, kana and Hangul syllables → Ideo).
   - fold.rs: simple case folding (CaseFolding.txt statuses C and S) for Latin, Greek, Cyrillic,
     Armenian, Georgian, Cherokee and the fullwidth Latin letters, as ranges with a delta (and a
     short exception list where the delta pattern alternates), `fn fold(c: char) -> char`.
   - compose.rs: the 64 precomposed letters of Windows-1252 as (base, mark, letter), from UnicodeData.txt
     decompositions; `fn compose(base: char, mark: char) -> Option<char>`.
   - width.rs: East Asian Width W and F ranges, `fn is_wide(c)`.
   - script.rs: CORE-226's ranges, `fn needs_shaping_or_bidi(c)`, plus `fn is_invisible(c)` for the
     format characters the plan lists.
   - blocks.rs: the block of a code point from Blocks.txt, names only for the about 70 blocks the plan
     lists (Latin-1 Supplement to Halfwidth and Fullwidth Forms, the emoji blocks), `Other` otherwise;
     and the 12 short names for invisible characters of CORE-228 (`ZWJ`, `ZWSP`, `WJ`, `LRM`, `RLM`, `VS16`,
     `BOM`, and the rest the plan names).
   - cp437.rs: `pub fn from_unicode(c: char) -> Option<u8>`, the exact inverse of the existing table.
   - unitab.rs: one byte blob layout (a small header of offsets, then each table), `fn tables(blob:
     &[u8]) -> Option<Tables<'_>>` that validates it once, and lookups that read through it, so a
     build can serve the blob from a resource; the const tables and the blob must give the same
     answers (a test checks every code point 0 to 0x10FFFF for each table).
   It also writes assets/unicode/UNITAB.BIN and has --check, which fails when any output is stale.
   CI has no UCD download: --check compares committed outputs against a regeneration only when the
   cache is present, and otherwise checks UNITAB.BIN against the committed .rs tables, so make
   validate passes on a clean machine. Wire --check into the Makefile's validate target the way
   gen-codepages.py is wired.
3. Tests (crates/quill-text/tests/unicode.rs): the cluster table against GraphemeBreakTest.txt and the
   break rule against LineBreakTest.txt, keeping only lines whose characters are all in scope (copy the
   kept lines into crates/quill-text/tests/ucd/ with the Unicode licence header; list in the test how
   many lines were excluded and why, by class); fold against Python's casefold for single code points
   in scope (generate a golden list with the script); compose's 64 entries; from_unicode's round trip
   for all 256 CP437 bytes; the blob against the const tables.
4. Sizes: put a table in the PR body with each table's bytes and the blob's (expect about 2.8 KB in all;
   if any table is over 1 KB, say why and how it could shrink).
5. Docs: add CORE-218 to docs/decisions.md (Assumed; the tables, their sources, UCD 16.0.0, the blob for
   Windows 3.x, the limits of scope), change its block's status in docs/id-ranges.toml from reserved to
   in use and run `python3 scripts/check-ids.py --write`, and a row in docs/progress.md.
6. Gates: G-HOST (`make validate`, which includes the id check), G-MD. No other build links quill-text's
   new modules, so G-DOS, G-W16 and G-W32 are not needed: say so in the test plan.

Rules: Canadian English; no hard-wrapped markdown; tabs except YAML; SPDX header on every new source, script,
TOML and YAML file. Stage explicit paths, never `git add -A` or `git commit -a`; no wildcard deletes. Commit
with a Conventional Commit subject (feat(text): ...), a bold-label markdown body hard-wrapped at about 72
columns, written to a file and committed with `git commit -F`, ending with a `Co-Authored-By:` trailer naming
your own model. Push only your branch; open one PR with `gh pr create`, labels enhancement: the title "Add
the Unicode tables the editor will need to quill-text", `## Summary` with bold-lead bullets, `## Test plan`
with `- [x]`/`- [ ]` items naming the commands run, ending with an unchecked "Not verified:" line. Do not
merge.

If a decision is needed and nobody answers, take the default the plan gives, make it easy to swap, record it
as an Assumed decision with "OWNER DECISION:" in its text, and keep going. Report: the PR URL, the gates run
and their results, the table sizes, and what was not verified.
```

### WP-UNI-2

```text
Package WP-UNI-2 (docs/plans/unicode.md): the Unicode test document set and the frozen .QLD 1.0 reader.
Branch test/unicode-docs, worktree ~/source/worktrees/quill/unicode-docs, from origin/main now. Write
scripts/gen-unicode-docs.py (python3 -I) that generates every file of the plan's list that it can
(UTF-16, invalid UTF-8, the BOM files, the sampler's text, RTF edge cases) into tests/docs/unicode/ with
manifest.toml (SPDX header) and one .expect per file (code points one a line, as the format defines
them) and one .today (what today's readers give: Windows-1252 with ? and the refused count). Hand-written
files (fr-ca.rtf, ru-word97-upr.rtf) get their source in the manifest. crates/quill-host/tests/
unicode_docs.rs reads every file through quill-formats and checks it against .today; later packages flip
.today to .expect. Copy today's .QLD reader into crates/quill-formats/tests/compat/qld_1_0.rs (a frozen
copy; a header comment says never to edit it) with a test that reads tests/docs/*.qld. No src/ file
changes. Gates: G-HOST, G-MD. Decision CORE-225 (and its block's status to in use with check-ids.py
--write). PR: "Add a Unicode test document set and freeze the 1.0 reader".
```

### WP-ED-4, WP-ED-10, WP-ED-11

The briefs in [work-packages.md](../work-packages.md), with these lines added: **WP-ED-4** "Use the API names of docs/plans/unicode.md (Pos, snap, check, chars, read_str); add a test-only insert of UTF-8 text so the boundary property is proved on multi-byte text; decisions CORE-100 and CORE-211." **WP-ED-10** "Add App::caret_column and use it in the status line; the Find box and dialog text fields take Key::Char(char); the terminal key map hands char." **WP-ED-11** "Every entry point narrows to Windows-1252 as today (the RTF reader builds UTF-8 and narrows it); write DOS-112 with the budget raise."

### WP-UNI-3

```text
Package WP-UNI-3 (docs/plans/unicode.md): placeholders, clusters and line breaks on every build. FREEZE:
tell the orchestrator before you branch; the freeze set of the plan's "migration path and the freeze"
section stays closed to everyone else until you merge. Branch feat/unicode-show, worktree
~/source/worktrees/quill/unicode-show, after WP-ED-11 and WP-UNI-1. Build quill_core::repertoire and the
cluster API; caret, Delete, Backspace, selection by CORE-212; layout breaks by CORE-215 and
Measure::advance (default None, the placeholder advance); every painter draws Show (quill-paint hex box
with the 3x5 digits, paint.rs and quill-grid CP437 and ▒, Win16 gdi.rs and Win32 gdi.rs split runs at
placeholders and transcode the rest into a 128-byte stack buffer); compose for display; status sentence
and open message; Col counts clusters. Win16: tables from UNITAB.BIN as RCDATA id 101, names as string
resources, digits as a bitmap; report the data segment before and after and `llvm-nm -S` of quill_text
symbols in .rodata (must be no tables). No entry point may accept a character outside Windows-1252:
prove it with host tests that build UTF-8 documents from tests/docs/unicode through a test-only insert.
Bench gate again (10 % English, 25 % French). New goldens reviewed frame by frame in a commit of their
own; every other golden unchanged. Gates: G-HOST, G-DOS, G-W16, G-W32, G-GTK, G-DISK, G-MD. Evidence in
Unicode/WP-UNI-3. Decisions CORE-210, 212, 213, 214 (display), 215, 226, 227, 228 (column), DOS-110,
DOS-111, W16-115, W16-116, W9X-125, LNX-97. PR: "Show every character as itself or as a placeholder".
```

### WP-UNI-4

```text
Package WP-UNI-4 (docs/plans/unicode.md): Unicode in files. Branch feat/unicode-files, worktree
~/source/worktrees/quill/unicode-files, after WP-UNI-3, WP-UNI-2 and WP-ED-12. .QLD 1.1 UNIC exactly as
the plan's Files section and every failure rule; 1.0 byte for byte when no character needs UNIC (every
existing golden unchanged); RTF \uc1\uN? writing and \u into UTF-8 reading (remove rtf::narrow); plain
text BOMs, UTF-16, Text (UTF-8) in Save As on every build, the count messages; RCOV caret in code
points. Flip the corpus' .today files to .expect; generate the damaged UNIC files from mixed-1-1.qld with
the docs script. Run the frozen 1.0 reader over every 1.1 file and check-rtf-libreoffice.sh over the RTF
corpus. Gates: G-HOST, G-DOS, G-W16 (FILE segment, data figure), G-W32, G-GTK, G-MD. Evidence in
Unicode/WP-UNI-4. Decisions CORE-219, CORE-220, CORE-221; SPEC file format, RTF and plain text sections.
PR: "Keep every character in .QLD, RTF and text files".
```

### WP-UNI-5

```text
Package WP-UNI-5 (docs/plans/unicode.md): typing any character. Branch feat/unicode-keys, worktree
~/source/worktrees/quill/unicode-keys, after WP-UNI-3 and WP-ED-7, in the ED lane. Alt+X both ways
(CORE-223, one undo step "Insert Character", the refusal sentence); DOS keys are CP437 characters
(DOS-110); the terminal reads and writes UTF-8 (LNX-97); WM_CHAR through Windows-1252 into char on
Windows 3.x and 95 (W16-117); the core's insert path stops narrowing. GTK: remove LNX-14's refusal only
if WP-UNI-8 has merged or the UI lead agrees; otherwise write the slice in the PR. Check first that Alt+X
is free on every build's menus and accelerators; if not, stop and ask. Scenarios m16-alt-x.keys and
m16-cp437-keys.keys on every build. Gates: G-HOST, G-DOS, G-W16, G-W32, G-GTK, G-MD. Evidence in
Unicode/WP-UNI-5. PR: "Type any character with Alt+X and the DOS code page".
```

### WP-UNI-6

```text
Package WP-UNI-6 (docs/plans/unicode.md): the clipboard in Unicode. Branch feat/unicode-clip, worktree
~/source/worktrees/quill/unicode-clip, after WP-ED-5, WP-ED-6 and WP-UNI-3. TextUtf8 and TextUtf16
flavours in quill-clip; CF_UNICODETEXT (format 13) read and written on Windows 95 with no new import,
preferred over CF_TEXT; RTF on the clipboard with \u; U+FFFC for objects; docs/clipboard.md. Extend
win9x/tools/wine-clipboard.sh with Cyrillic both ways with Notepad. Gates: G-HOST (round-trip property
over the corpus), G-W16, G-W32 (check-imports.sh unchanged), G-MD. Evidence in Unicode/WP-UNI-6.
Decision CORE-222. PR: "Copy and paste any character through the clipboard".
```

### WP-UNI-7

```text
Package WP-UNI-7 (docs/plans/unicode.md): Find, Replace, spelling and words beyond ASCII. Branch
feat/unicode-find, worktree ~/source/worktrees/quill/unicode-find, after WP-UNI-5 (ED lane), WP-UNI-1 and
WP-ED-7. Find folds through quill_text::fold and compose, matches on cluster boundaries, ^uNNNN; one word
rule over characters (CORE-216) for Ctrl+arrows, double click, whole word, spelling and the word count;
spelling skips words the dictionary's code page cannot hold; is_letter's seven letters; Windows 3.x
reads the fold table from UNITAB.BIN. Scenario m16-find-fold.keys. Gates: G-HOST, G-DOS, G-W16 (EDIT
segment, data figure), G-W32, G-MD. Evidence in Unicode/WP-UNI-7. Decisions CORE-214 (Find), 216, 217,
224. PR: "Find and check words in any alphabet Quill knows".
```

### WP-UNI-8

```text
Package WP-UNI-8 (docs/plans/unicode.md), UI track: GTK shows every unshaped left-to-right script. Branch
feat/gtk-unicode, worktree ~/source/worktrees/quill/gtk-unicode, after WP-UNI-3. quill-fonts by char with
per-glyph fallback through the system's faces (cmap scan, cached per code point), Measure::advance,
Show::Glyph on GTK's page, Level::Unicode with CORE-226's scripts and emoji sequences as boxes; remove
LNX-14's refusal; the accessible text gives real characters with code point offsets. Your own Xvfb
(DISPLAY set in every command, WAYLAND_DISPLAY unset). Gates: G-HOST, G-GTK with chrome-shots.sh on
tests/docs/unicode/sampler.qld, G-MD. Evidence in Unicode/WP-UNI-8 (light and dark). Decision LNX-95.
PR: "Show Greek, Cyrillic, CJK and symbols on the GTK page".
```

### WP-UNI-9

```text
Package WP-UNI-9 (docs/plans/unicode.md): PDF beyond WinAnsi. Branch feat/pdf-unicode, worktree
~/source/worktrees/quill/pdf-unicode, after WP-UNI-4 and WP-UNI-8 (and WP-GTK-3b for the GTK half; if it
has not merged, do the Windows 95 half and say so). quill-pdf: placeholder boxes as paths with
/ActualText spans (W9X-126). quill-pdfdoc on GTK: embedded CIDFontType2 subsets, Identity-H, ToUnicode,
fsType respected (LNX-96). Check with pdftotext, qpdf --check and pdffonts. Gates: G-HOST, G-W32, G-GTK,
G-MD. Evidence in Unicode/WP-UNI-9. PR: "Print characters beyond Windows-1252 to PDF".
```

### WP-UNI-10

```text
Package WP-UNI-10 (docs/plans/unicode.md): the v1 finish line across the builds. Branch
test/unicode-cross, worktree ~/source/worktrees/quill/unicode-cross, after every other v1 WP-UNI package.
A flow that types the sampler with Alt+X on DOS, Windows 3.11 and Windows 95 (Wine) and the input method
on GTK, saves .QLD and RTF, byte-compares each against the host's, then opens every build's file on every
other build and compares the screen with its golden. Replace SPEC's Unicode non-goal with the v1 scope;
mark Unicode v1 done in docs/progress.md; collect the sampler shots in Unicode/sampler with README.txt.
One heavy emulator at a time, your own displays. Gates: G-HOST, G-DOS, G-W16, G-W32, G-GTK, G-MD. PR:
"Prove every build keeps the same characters".
```

## Questions for the owner

Each default below is taken so no builder waits; the last column is where to swap it.

| # | Question | Default taken | Where to swap |
|---|---|---|---|
| 1 | Which scripts does v1 **show**, per build? | Windows-1252 everywhere, CP437 on the DOS text screen, every unshaped left-to-right script on GTK (and WinUI); placeholders for the rest | CORE-210 and [Scope](#scope-what-v1-is-and-what-is-later); pull WP-UNI-11 or WP-UNI-12 into v1 |
| 2 | Bidirectional text in v1? | No: kept in logical order, placeholders on every build; later on GTK and WinUI only | CORE-226; WP-UNI-14 |
| 3 | CJK in v1? | Stored everywhere; shown and typed on GTK with ideograph line breaks; placeholders on the retro builds; no vertical text or ruby | CORE-210, LNX-95 (drop the CJK fallback to make them boxes) |
| 4 | Emoji in v1? | Single emoji in monochrome on GTK where a font has outlines; sequences as one box everywhere; colour later | CORE-227; WP-UNI-14 |
| 5 | Windows 95, 98 and Me: charset fonts and input locales (Central European, Cyrillic, Greek on 98 and Me) in v1? Wide calls through `GetProcAddress` on 2000 and later? | Later (WP-UNI-11, after a spike on the 98 and Me VMs); no `W` import ever | The order table; W9X-127 to W9X-129 |
| 6 | The caret on the old builds: by cluster, as on GTK, or by code point, as the first draft said? | By cluster on every build (one rule); Backspace by code point | CORE-212 |
| 7 | Normalization? | None stored; display and Find compose the 64 Windows-1252 letters; NFC on paste later | CORE-214 |
| 8 | Should the placeholders package join the freeze (three packages instead of two)? | Yes | Rule 4 of work-packages.md and the wave 8 row |
| 9 | Unicode ahead of Preferences in the ED lane after the freeze? | Yes: WP-UNI-5 goes before WP-PREF-2 in wave 9 and WP-UNI-7 before WP-PREF-5 in wave 10, so each preferences package waits for one Unicode package | Waves 9 and 10 in work-packages.md and preferences.md |
| 10 | The DOS budget: land WP-MOD-4 (the file dialog module) before WP-UNI-5, or raise the stop past 480 KB? | WP-MOD-4 first; DOS-70 raises meanwhile; the `QUILLUNI.TBL` fallback only if short | DOS-112; modules.md's order |
| 11 | PDF: embedded subsets on GTK in v1? | Yes on GTK; placeholders with `ActualText` on Windows 95 | LNX-96, W9X-126 |
| 12 | DOS graphics and Windows 3.x beyond Windows-1252 at all (strike pages on a second disk, charset fonts)? | Later (WP-UNI-12), not v1 | The order table |
| 13 | The file's Unicode chunk: the sparse `UNIC` patch or a full UTF-8 copy? (the first draft's question 1) | The sparse patch | CORE-219 |
| 14 | A text file with a few bad UTF-8 sequences: keep C4 (read as the code page) or UTF-8 with U+FFFD? (the first draft's question 4) | Keep C4, with the message | CORE-221 |
| 15 | `QUILL.UDC` stays Windows-1252? (the first draft's question 5) | Yes | CORE-224 |

## Found while planning

- **CORE-101 and CORE-102 are not Unicode ids.** `docs/work-packages.md` says the Unicode plan's U1 and U2 "take CORE-100 to CORE-104", but its own packages give CORE-101 to the rich clipboard (WP-ED-5) and CORE-102 to Edit menu state (WP-ED-9); the Unicode ids in that block are CORE-100, CORE-103 and CORE-104. This page corrects the sentence in work-packages.md's *Found while planning*.
- **The first draft's ids CORE-61 to CORE-85** were mostly taken by other work; only CORE-61 (U0) kept its meaning. This plan's own block is CORE-210 to CORE-229.
- **Windows 3.x `.rodata` is the data segment.** Any `const` table a package adds to a crate Windows 3.x links costs the data segment, which is why W16-116 is absolute and why WP-UNI-1 builds the blob form from the start.
- **The status line's column counts bytes on a device with pixels** (`app.caret - l.start`, `crates/quill-app/src/paint.rs:476`); under Windows-1252 that is characters, under UTF-8 it would not be. WP-ED-10 gives it `caret_column`.
- **The WinUI ids moved.** This plan first reserved WUI-85 to WUI-87, which the WinUI real-window plan (pull request 187) had reserved first as WUI-85 to WUI-94; this plan now takes WUI-95 to WUI-97. Pull request 187 has not merged, so the registry on `main` does not show its block yet; whichever of the two merges second rebases the registry with `python3 scripts/check-ids.py --write`.
