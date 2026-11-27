# Preferences: one settings model and an Options window on every build

The plan for a Preferences window (Tools > Options) on the DOS text and graphics screens, Windows 3.x, Windows 95 and later, the GTK client, the WinUI client and the Linux terminal, over one settings model in the core. Autosave comes first: it gains a mode that saves as you work, its interval and folder become something a person can change, and its defaults change now, ahead of the window. Written against `origin/main` at `89a714d`, with the DOS autosave merged (DOS-47, DOS-48, CORE-69) and the Windows, terminal, GTK and WinUI autosave still to come (the autosave plan's PRs 8, 9, 11, 18 and 19).

It builds on the preferences design of 2026-11-24 (`~/Documents/Liminal HQ/Quill/Quill preferences design.md`, with the owner's answers to its fifteen questions) and on [work-packages.md](../work-packages.md), whose rules, budgets, gates (G-HOST, G-DOS, G-W16, G-W32, G-GTK, G-DISK, G-MD), evidence folders and common brief every package here uses. **This series replaces WP-ED-13 to WP-ED-16** (ARCH-30): the same ground, cut so the model lands early as new files, the autosave rows come first and every build has its own package. Sizes marked **[E]** are estimates for the package to measure; everything else was read in the code.

## Table of contents

- [The answer in short](#the-answer-in-short)
- [What exists today](#what-exists-today)
- [Autosave's default: change it now](#autosaves-default-change-it-now)
- [Autosave as you work](#autosave-as-you-work)
- [The settings](#the-settings)
- [The model: quill-settings](#the-model-quill-settings)
- [Where each build keeps its settings](#where-each-build-keeps-its-settings)
- [How a change applies](#how-a-change-applies)
- [Import, export and reset](#import-export-and-reset)
- [The window on each build](#the-window-on-each-build)
- [Decisions](#decisions)
- [Order and lanes](#order-and-lanes)
- [Budgets](#budgets)
- [Tests and goldens](#tests-and-goldens)
- [Work packages](#work-packages)
- [Briefs](#briefs)
- [Owner decisions](#owner-decisions)

## The answer in short

- **Autosave every 5 minutes, not 10, on DOS and Windows 3.x, now** (WP-PREF-0, CORE-150, approved by the owner on 2026-11-27): Windows 95 stays at 5 and the modern clients at 2. Every existing install picks it up, because `Autosave=` is written only when it differs from the default.
- **Autosave as you work** (WP-PREF-0, CORE-161): a mode that writes the recovery copy after a few seconds without a key, on by default on Windows 95, the terminal, GTK and WinUI, offered but off by default on DOS and Windows 3.x, where a write on a 386 is long enough to feel. `[Quill] AutosaveMode` is `off`, `work` or `minutes`; the minutes stay as the backstop. It is the existing scheduler with two numbers changed, so it lands in WP-PREF-0 with the defaults.
- **One model.** `quill-settings` (base layer, `no_std`, no `alloc`, no floats, no panics) works on byte slices: a compiled, versioned schema table (`QSET`, ARCH-31), a total parser and formatter for each kind of value, the `QUILL.INI` reader and a merging writer that keeps every line it does not change, the `Profile` trait, the layers and the migrations. The file format is written down in `docs/settings-format.md` and carries `[Settings] Version=1` (CORE-151).
- **Seven pages, one list.** General, New documents, Spelling, Save, View, Screen (DOS only) and Advanced (GTK and WinUI only); 32 settings, each with its builds, default and how it applies. A page shows only the rows its build has.
- **Each build in its own style.** A text-cell dialog on the DOS text screens, the same dialog as a Windows 3.1 window on the graphics screen, Word 2.0's category dialog on Windows 3.x, a COMCTL32 property sheet on Windows 95, an `AdwPreferencesDialog` on GTK and a Settings page with a navigation pane on WinUI. Retro builds apply on OK or Apply; GTK and WinUI apply live.
- **Order.** WP-PREF-0 now; WP-PREF-1 (the model, new files) and WP-PREF-4 (the `quill settings` command) beside the ARCH lane now; the window after the Unicode freeze, where WP-ED-13 to WP-ED-16 stood. Owner decision on pulling it before the freeze is at the end.

## What exists today

Read in the code at `89a714d`.

| Part | State |
|---|---|
| Options window | None on any build. No `Command::Options`, no `Dialog::Options`, no Tools menu. The SPEC still says "until the Options page is built" |
| Settings in the editor | `App::each_setting` and `App::apply_setting` in `crates/quill-app/src/lib.rs`, a hand-written match: recent files, `Language`, `AsYouType`, `Folder`, `Autosave` (through `recovery.rs`), `Zoom`, `Blink`, the four chrome bits, `DocFolder` on DOS |
| Reading | `load_settings` reads the first 1,536 bytes of `QUILL.INI` (B3); DOS reads `Rows` and `Graphics` from the first 512 |
| Writing | `write_ini` writes the file whole on DOS, Win16 and the terminal, dropping keys it does not know (B4); Win32 writes one key at a time through `WritePrivateProfileStringA` from a hand-made copy of the key list (`win9x/quill-win9x/src/ui.rs` `ini_load`, `ini_save`, B5) and never deletes one (B1); Win16 writes the file whole in the current folder with `_lcreat` (B2) |
| Registry | Not used. `QUILL32.EXE` imports no registry function |
| GTK | `QUILL.INI` in `$XDG_CONFIG_HOME/quill` through the editor's settings, and `gtk.ini` beside it for the window's size, maximized state and menu bar (`linux/quill-gtk/src/settings.rs`). No GSettings |
| WinUI | A spike only (`winui/spike`) |
| Autosave | Runs on the three DOS screens only: `configure_recovery(10, writer, false)` in `dos/quill-dos/src/main.rs`. The editor keeps the interval, its default and the debug hook (`recovery.rs`: `autosave_minutes`, `set_autosave_minutes`, `rcv_each_setting` writes `Autosave` only when it differs from the default). The host's scripts and `quill-paint`'s shell tests also pass 10 |
| Recovery folder | `[Quill] RecoveryDir`, `RecoveryDir32`, `RecoveryDirLinux`, read by `recovery.rs`; empty means `RECOVER\` beside the program |
| Spelling as you type | Off on DOS and Windows 3.x, on on Windows 95 (D201, D202), set by `[Quill] AsYouType` |
| Zoom | `[Quill] Zoom`, written whenever View > Zoom changes it; Page Width by default, 100 on the DOS graphics screen |

## Autosave's default: change it now

**Decided (the owner, 2026-11-27): change it now, in WP-PREF-0, ahead of the window.** DOS and Windows 3.x go from 10 minutes to 5; Windows 95 stays at 5; the terminal, GTK, WinUI and the web stay at 2. On the builds where autosave as you work is the default, the minutes are the backstop (see the next section).

- **What 10 minutes costs.** An autosave is due when the interval has passed since the document first changed after its last save or autosave, and then waits for two seconds of quiet. So a power cut loses up to the interval plus the wait plus the write. Ten minutes was Word 97's AutoRecover default, which is the shape of the loss the owner describes: a long stretch of typing between copies, discovered only after the machine went down.
- **What 5 minutes costs.** At most twice as many writes, and only while the document is changing. A write never runs while a key is coming: it starts after two seconds of quiet, polls for input every 4 KB and stops at once when a key waits, leaving the old copy. On a 386 the autosave plan estimates 0.2 to 0.6 seconds for a 50 KB document and 1 to 2.5 seconds for 250 KB; the forced write after two cut-short attempts is the only visible pause, so a person whose pauses are all shorter than the write sees one about every five minutes on a large document instead of every ten, and someone who pauses for a few seconds now and then never sees one. Copies from a floppy go to `RECOVER\` on the program's disk (DOS-47), so a floppy is never written more often.
- **Why not 2 on the retro builds.** At 2 minutes a 250 KB document on a 386 would bring a forced pause of 1 to 2.5 seconds every two or three minutes for a person who types in short bursts, and most writes would start in the brief pauses of thinking, the moments a 386 is then slowest to answer the next key. The modern clients keep 2 because their writes are milliseconds and, once their executors land, off the UI thread.
- **Why not wait for the window.** The window is many packages away (it needs the settings model and the Unicode freeze before it). The default protects everyone who never opens it, and the window then lets a person go down to 1 minute, which is what the owner did for himself.
- **Who it changes.** Everyone without an `Autosave=` line, which is everyone: nothing writes the key at the default and no UI can set it yet. A hand-written `Autosave=10` stays 10.

## Autosave as you work

The owner's ask (2026-11-27): save after a short pause in typing, a few seconds of quiet, and let a person turn it off because an older machine may be too slow. **What is saved is the recovery copy, never the document itself** (CORE-161): writing over `LETTER.QLD` at every pause would take away Don't Save and the meaning of Save, and would replace the original on a floppy with every pause. The recovery copy already holds everything a crash needs, and Recover brings it back.

| Mode | `AutosaveMode` | A copy is written | Default on |
|---|---|---|---|
| Off | `off` | Never | — |
| As you work | `work` | After `AutosavePause` seconds without a key or button (3 by default), and no sooner than 10 seconds after the document first changed since the last copy; and in any case once `Autosave` minutes have passed, as the backstop | Windows 95 (W32), the terminal, GTK, WinUI, the web |
| Every N minutes | `minutes` | `Autosave` minutes after the document first changed since the last copy, at the next two seconds of quiet (one on the modern clients), as today | DOS text and graphics, Windows 3.x |

- **Why these defaults.** The Win32 build's floor is a Pentium (D33), where a 250 KB copy is a fraction of a second and stops at the next key; the modern clients write in milliseconds and will write from a worker. On a 386 the same copy is 1 to 2.5 seconds, so writing it at every three-second pause would put a write in front of most of the keys that follow a pause; there the person chooses As you work in Options, and the help line says `On a slow computer, typing may pause while Quill saves.`. OWNER DECISION at the end.
- **It is the scheduler that exists** (CORE-17, CORE-20, CORE-69's questions unchanged). `Probe::due` already asks for an interval since the document first became dirty and for a quiet time; As you work only changes the two numbers it is fed: the interval becomes the 10-second floor and the quiet becomes `AutosavePause`. Every other rule is the same: the job is last in `App::idle`'s order, it runs only in plain editing and never in a second window, it writes through `save_recovery` into the same slot, a waiting key stops the write and keeps the old copy, a failed write backs off and says `Quill cannot autosave: the disk is full.` once a session, and the debug hook `[Debug] AutosaveSeconds` still overrides everything.
- **The forced write.** Today, after two writes cut short, the third is announced and runs to the end. In As you work that would make every third pause a visible stop on a slow machine, so the forced write waits until the backstop's minutes have passed since the document first changed; until then a cut-short write simply tries again at the next pause.
- **Recovery files are unchanged.** One current copy and one set-aside copy per document, the same names, the same `RCOV` chunk (flag 8, autosave), the same list in `QUILL.INI` (written when a document's first copy is made, not at every write), the same lock note. The two-copy rule means each write replaces only the current copy, so the disk holds no more than it does at 5 minutes.
- **What a person sees.** No `Autosaved.` flash in As you work (it would blink at every pause); the flash stays for Every N minutes when the save polish adds it. `Autosaving LETTER.QLD...` shows only for the backstop's forced write.
- **Disk and floppies.** A copy goes beside the document, or to `RECOVER\` for a document on a floppy, so a floppy is never written at every pause. Every write is a `.$$$` file and a rename, so a cut never leaves half a copy.
- **Keys** (CORE-161). `[Quill] AutosaveMode` (`off`, `work`, `minutes`) and `[Quill] AutosavePause` (1 to 30 seconds, default 3). Windows 3.x and Windows 95 share one `QUILL.INI` but take different defaults, so the mode is a family key (`AutosaveMode` for the 8.3 builds, `AutosaveMode32`, `AutosaveModeLinux`, `AutosaveModeWinUI`) and so is the pause; `Autosave` minutes stay shared. A file from before has no mode: `Autosave=0` there reads as Off and any other value as Every N minutes on the builds whose default is minutes, so nobody who turned autosave off finds it on.

## The settings

Builds: **DT** the DOS text screens, **DG** the DOS graphics screen (one `QUILLDOS.EXE` and one `QUILL.INI` with DT), **W16** Windows 3.x, **W32** Windows 95 and later, **TTY** the Linux terminal, **GTK**, **WUI**. Applies: **Live** (on OK or Apply on the retro builds, at once on GTK and WinUI), **New** (new documents only; the open one is changed in its own dialogs and kept in `DOCP`, as the owner decided), **Start** (the next time Quill starts, with a note in the dialog). A row shows on a build only once that build does what the row says; a row marked with a package in Builds waits for it.

| Page | Setting (label) | Key | Values | Default | Builds | Applies |
|---|---|---|---|---|---|---|
| General | At start, open | `[Quill] Startup` | `blank`, `last` | `blank` | all | Start |
| General | Recent files listed, and Clear | `[Quill] RecentCount` | 0 to 4 | 4 | all | Live |
| General | Measurement units | `[Quill] Units` | `in`, `cm` | from the system (`WIN.INI` `iMeasure`, the DOS country, `LC_MEASUREMENT`, `LOCALE_IMEASURE`) | all | Live |
| General | Documents folder | `[Quill] DocFolder` | path | empty | DT, DG | Live |
| General | Blinking caret | `[Quill] Blink` | `1` the system's rate, `0` steady | `1` | W16, W32, GTK, WUI | Live |
| General | Pointer speed | `[Quill] PointerSpeed` | 0 the driver's, 1 to 10 | 0 | DT, DG (WP-PREF-6) | Live |
| General | Double-click speed | `[Quill] DoubleClick` | 200 to 900 ms | 500 | DT, DG (WP-PREF-6) | Live |
| New documents | Font | `[Quill] Font` | a face, at most 31 bytes | `Times New Roman` | all | New |
| New documents | Size | `[Quill] Size` | 4 to 144 points | 12 | all | New |
| New documents | Paper | `[Quill] PaperSize` | `letter`, `a4`, `legal` | `letter` with inches, `a4` with centimetres | all | New |
| New documents | Margins | `[Quill] Margins` | four measures, 0 to 14,400 twips, at least an inch of text | `1800 1800 1440 1440` | all | New |
| New documents | Default tab stops every | `[Quill] DefaultTab` | a measure, 0.1 to 4 in | 720 twips (0.5 in; 1.25 cm in metric) | all (WP-ED-12) | New |
| New documents | Save new documents as | `[Quill] SaveFormat` | `qld`, `rtf`, `txt` | `qld` | all | New |
| Spelling | Check spelling as you type | `[Quill] AsYouType` | 0, 1 | 0 on DT, DG, W16, TTY; 1 on W32, GTK, WUI | all | Live |
| Spelling | Language | `[Spelling] Language` | `en-CA`, `en-US`, `en-GB` | `en-CA` | all | New |
| Spelling | Ignore words in capitals | `[Spelling] IgnoreCaps` | 0, 1 | 1 | all | Live |
| Spelling | Always suggest | `[Spelling] AlwaysSuggest` | 0, 1 | 1 | all | Live |
| Save | Save recovery information: Off, As you work, Every … minutes | `[Quill] AutosaveMode` (family: `AutosaveMode32`, `AutosaveModeLinux`, `AutosaveModeWinUI`) | `off`, `work`, `minutes` | `minutes` on DT, DG, W16; `work` on W32, TTY, GTK, WUI (CORE-161) | DT, DG now; W16, W32, TTY, GTK, WUI with their autosave packages | Live |
| Save | … minutes (and the backstop for As you work) | `[Quill] Autosave` | 1 to 60 (0 still reads as Off) | 5 on DT, DG, W16, W32; 2 on TTY, GTK, WUI (CORE-150) | as the mode | Live |
| Save | Pause before saving as you work | `[Quill] AutosavePause` (family keys as the mode) | 1 to 30 seconds | 3 | as the mode | Live |
| Save | Recovery folder | `[Quill] RecoveryDir` (`RecoveryDir32`, `RecoveryDirLinux`, `RecoveryDirWinUI`) | path; empty is the build's own | empty | as Autosave | Live |
| Save | Always keep a backup copy | `[Quill] Backup` | 0, 1 (`.BAK` beside the document) | 0 | all (WP-PREF-13) | Live |
| View | Toolbar, Formatting bar, Ruler, Status bar | `[Quill] Toolbar`, `FormattingBar`, `Ruler`, `StatusBar` | 0, 1 | 1 | DG, W16, W32, GTK, WUI; Ruler and Status bar on DT, TTY | Live |
| View | Zoom | `[Quill] Zoom` | `Width`, 25 to 200 | `Width`; 100 on DG | DG, W16, W32, GTK, WUI | Live |
| View | Style | `[Quill] Theme` | `system`, `light`, `dark` | `system` | GTK, WUI | Live |
| View | Paper colour | `[Quill] PaperColour` | `white`, `theme` | `white` | GTK, WUI | Live |
| Screen | Text rows | `[Quill] Rows` | 25, 50 | 25 | DT | Start |
| Screen | Start in graphics | `[Quill] Graphics` | 0, 1, 16 | 0 | DT, DG | Start |
| Screen | Colours | `[Quill] Scheme` | `colour`, `mono` | `colour` | DT, TTY | Start (DT), Live (TTY) |
| Advanced | Import, Export, Reset all | — (actions) | — | — | GTK, WUI; `quill settings` everywhere | — |

- **Kept as keys, not shown:** `Window`, `Folder` (the last folder, remembered), `[Recent]`, `[Recent32]`, `[RecentLinux]`, `[Recovery]` lists, `RecoveryDirPrev` (written by Quill, see [How a change applies](#how-a-change-applies)), `PictureMemory`, `Threads` (the Win32 worker was a no-go for Windows 9x, W9X-13, so only the modern clients have threads and they have no choice to make), `[Debug] Trace`, `[Debug] AutosaveSeconds`, `gtk.ini`.
- **The recovery question at start is not a setting.** It is always asked, before a file named on the command line and before `Startup=last`.
- **Page sizes fit the DOS dialog.** `quill-dlg`'s `Focus<16>` holds 16 controls; with the page list and four buttons a page has room for 11. The largest page, New documents, has 9 (four margin fields); Save has 8 (three mode buttons, the minutes, the pause, the folder and its note, the backup).

## The model: quill-settings

`crates/quill-settings`, base layer, already named in `ORDER` in `scripts/check-layers.py`. It knows no `Store`, `Source` or `Files` (ARCH-9): everything comes in and goes out as byte slices and closures, so the same code runs in a 64 KB Windows 3.x data segment, under DOS, on the host and in the modern clients.

| Module | What it holds |
|---|---|
| `schema` | `Schema<'a>(&'a [u8])`: the compiled table read in place, with `entry(id)`, `find(section, key)`, `each(page, build)`. `Id` constants are generated (`generated.rs`) so code names `id::AUTOSAVE`, not a number |
| `value` | `Value<'a>`: `Bool`, `Int(i32)`, `Choice(u8)`, `Measure(i32 twips)`, `Zoom(u16, 0 for Width)`, `Text(&'a [u8])`. `parse(entry, bytes) -> Value` is total (a bad value is the default, numbers clamp); `format(entry, value, out: &mut [u8]) -> usize` |
| `ini` | `parse(bytes, f(section, key, value))` with the Windows profile rules (case-insensitive names, trimmed spaces, first duplicate wins, `;` comments); `rewrite(old, changes, out)` streams the old file to `out`, changing, adding or deleting only the named keys and keeping every other byte; `Writer` for a fresh file |
| `profile` | `trait Profile { fn get(&mut self, section, key, out: &mut [u8]) -> Option<usize>; fn set(&mut self, section, key, value: Option<&[u8]>) -> bool; }`, where `None` deletes |
| `layers` | Build default, then `QUILLDEF.INI` (machine defaults, written by the installer), then `QUILL.INI` (the person's), then the session (`/50`, `/G`, `/T`, `/MONO`, never saved). `effective(id)`, `inherited(id)`; a value equal to what it inherits deletes its key (CORE-152) |
| `migrate` | Version 0 to 1: `[Quill] Language=0..2` becomes `[Spelling] Language=en-CA` and the old key goes on the next write (CORE-154). A table, so later versions add rows |
| `draft` | `Draft<const N: usize, const T: usize>`: the values the dialog edits, numbers in `[u32; N]` and text in `[u8; T]` (font, two paths), with `changed(id)` and `each_changed`. About 120 + 400 bytes; Windows 3.x keeps it on the dialog's stack |
| `transfer` | `export(values, out)` and `import(bytes, f(id, value))` with counts of taken and ignored keys (CORE-155) |

**The schema table (`QSET`, ARCH-31).** `assets/settings/settings.json` is the one description: id, section, key, legacy keys, kind and range, defaults per build (with `locale` meaning the shell supplies it), page and order, label, help line, mnemonic, builds, applies, family key, exported, help id. `emit-resources --settings` compiles it to `assets/settings/settings.qset` (committed, checked for drift) and `crates/quill-settings/src/generated.rs` (the `Id` constants). Every build reads the same bytes: DOS, the host, the terminal, GTK and WinUI with `include_bytes!`, Windows 3.x from an `RT_RCDATA` resource, so its data segment holds only a handle (W16-91). Labels and help lines are a second table (`settings.qstx`), linked where a build draws them from the schema (DOS, the terminal, GTK, WinUI); Windows 3.x and 95 carry theirs in the dialog and string resources.

| `QSET` part | Contents |
|---|---|
| Header, 16 bytes | Magic `QSET`, version (`u16`, 1), entry count (`u16`), build count (`u16`), offsets of the defaults, choices and names pools (`u16` each), flags (`u16`, 0) |
| Entry, 16 bytes each | Id (`u8`), kind (`u8`), page (`u8`), order (`u8`), builds mask (`u8`), applies (`u8`), flags (`u8`: family key, exported, hidden), mnemonic (`u8`), minimum and maximum (`i16` each), section and key name offsets (`u16` each), legacy name offset (`u16`, 0 for none), help id (`u16`) |
| Defaults | Per entry, a default for every build (`i32`, or a names-pool offset for text, `0x7FFFFFFF` for "the shell supplies it") |
| Choices and names | Length-prefixed ASCII names: sections, keys, choice words (`in`, `cm`, `letter`) |

A reader checks the magic, refuses a newer major version and bounds every offset, so a damaged table gives an empty schema, never a panic. About 30 entries make about 1.4 KB **[E]**.

**The `QUILL.INI` format (CORE-151)**, written in full in `docs/settings-format.md` by WP-PREF-1: the grammar above; Windows-1252 everywhere except the modern clients' own family keys (`*Linux`, `*WinUI`), which are UTF-8; lines up to 255 bytes, names up to 31; a 16 KB limit, past which the file is read but not written and the status line says `QUILL.INI is too large for Quill to change.`; `[Settings] Version=1`, written by the first save; a file with no version is version 0 and is migrated; a file with a newer version is read for the keys Quill knows and its version is never lowered; family keys (`Folder`, `Folder32`, `FolderLinux`, `FolderWinUI`, and the same for `RecoveryDir`) so no build reads a path it cannot open.

## Where each build keeps its settings

| Build | File | Through |
|---|---|---|
| DOS (text and graphics) | `QUILL.INI` beside `QUILLDOS.EXE`; `QUILLDEF.INI` beside it | `IniFile` over `Files`, safe save (temporary file, then rename) |
| Windows 3.x | `QUILL.INI` in the Windows directory (B2 fixed, W16-90); `QUILLDEF.INI` in the program's folder | `GetPrivateProfileString` and `WritePrivateProfileString`, already imported |
| Windows 95 and later | The same `QUILL.INI`, sharing it with Windows 3.x; `*32` family keys and `[Recent32]` | The profile functions, one key at a time. Not the registry (W9X-111): the file is shared with the Windows 3.x build, the import list has no registry calls, and an export is the file. On NT without a manifest, Windows' file virtualization keeps a per-user copy when the Windows directory is not writable |
| Linux terminal and GTK | `$XDG_CONFIG_HOME/quill/QUILL.INI` (`[RecentLinux]`, B7 fixed); `gtk.ini` beside it for the GTK window | `IniFile` over `std::fs`; not GSettings (LNX-60), which needs a compiled schema installed system-wide and would split the settings from the file the command and an export read |
| WinUI | `%APPDATA%\Quill\QUILL.INI` (`*WinUI` family keys, `[RecentWinUI]`) | `IniFile` over `std::fs`; not `ApplicationData.LocalSettings`, which needs package identity an unpackaged build lacks (WUI-60) |

## How a change applies

| Style | Builds | Rule |
|---|---|---|
| OK and Cancel | DOS text and graphics, terminal | OK commits the draft: Live rows act at once, New rows wait for the next new document, Start rows say `Takes effect the next time Quill starts.` beside them. Cancel throws the draft away |
| OK, Cancel and Apply | Windows 3.x and 95 | Apply commits as OK does and keeps the dialog open; Cancel after Apply keeps what Apply did (the owner's choice) |
| Live | GTK and WinUI | Each change goes through `App::set` at once; the file is written after 500 ms of quiet and when the window closes. There is no OK or Cancel |

- **Writing** (CORE-152, CORE-157). Only changed keys are written, after a fresh read of the file, so a second Quill window's changes survive; a value equal to the inherited default deletes its key (B1 fixed). A second window keeps the values it started with until its next start.
- **Validation.** A field that cannot hold its value refuses OK with the flow's message (`Type a number from 1 to 60.`, `Type a measurement such as 1.25" or 3.2 cm.`) and puts the focus back on it.
- **Autosave** (CORE-158). A new mode, interval or pause counts from the next idle pass, so lowering it to 1 minute on a document changed six minutes ago writes a copy at the next pause. Off stops new copies and keeps the ones there, which still protect work done; Save and close remove them as before. The minutes and the pause keep their values while the mode is Off, and are greyed. A new recovery folder is checked with a probe file before OK takes it (`Quill cannot write to C:\SAFE.`); the old folder is kept in `RecoveryDirPrev` and read at launch until it holds no copies, so no copy is stranded by the change.
- **Screen settings on DOS** apply at the next start (the owner's choice); a switch on the command line shows `Overridden for this session by /G.`.
- **Help.** F1 (DOS, Windows 3.x), What's This (Windows 95) and the row subtitles (GTK, WinUI) all show the setting's help line from the schema.

## Import, export and reset

- **Export** (CORE-155) writes an INI file in the format above with a comment line, `[Settings] Version=1` and `Exported=` the build, then every exported setting with its effective value. Not exported: recent files, `Folder`, `Window`, the recovery lists and every path (they name one machine's disks).
- **Import** reads such a file (or any `QUILL.INI`), checks each key against the schema, writes the person's layer and says `12 settings imported, 2 ignored.`; a value equal to the default deletes the key, as any write does.
- **Reset** (Defaults on a page, Reset all on Advanced) deletes the keys it covers after a Yes or No question, so machine defaults show through. It keeps recent files, the last folder and the window position (the owner's choice).
- **Where.** In the Advanced page on GTK and WinUI, and everywhere as `quill settings get | set | reset | export | import FILE` in `quill-cli` (WP-PREF-4), which reads and writes any build's `QUILL.INI`. The retro builds have Defaults per page and no import or export in the window (the owner's choice).

## The window on each build

| Build | Opens from | Title | Form |
|---|---|---|---|
| DOS text | Tools > Options..., the Spelling dialog's `< Options >` (at the Spelling page) | Options | A shadowed text-cell box, 72 by 13 cells (at most 76 by 19), built at run time from the schema |
| DOS graphics | The same | Options | The same state drawn as a Windows 3.1 dialog by the overlay |
| Windows 3.x | Tools > Options..., Spelling's Options | Options | A category list and one `RT_DIALOG` holding every page |
| Windows 95 | Tools > Options..., Spelling's Options | Options | A COMCTL32 4.00 property sheet |
| Terminal | Tools > Options... through F10 | Options | The DOS text dialog, drawn by the same painter |
| GTK | Ctrl+, the primary menu's Preferences, Tools > Preferences in the classic menu bar | Preferences | `AdwPreferencesDialog` |
| WinUI | Ctrl+, the gear beside the menu bar, File > Settings | Settings | A page in the window with a `NavigationView` |

**The Tools menu** (CORE-156) sits between View and Help on every build with a menu bar. On the DOS text screens Tools takes column 65 and Help moves to 72, so the bar ends at column 76 of 80 (`MENU_COLS` gains a tenth entry); WP-PREF-5 regenerates every text screen golden in a commit of its own.

### DOS text

The style of the Spelling and file dialogs: a frame with the page list on the left, the page's controls on the right, the buttons under them and the focused setting's help line on the status row. Tab and Shift+Tab move through the ring; Up and Down in the list change the page at once; Ctrl+PgUp and Ctrl+PgDn change it from anywhere; Alt and a mnemonic go to a control; Space toggles; Alt+Down opens a list; Enter is OK and Esc is Cancel; F1 opens the setting's help topic. The box is centred on 80 × 50 and drawn in the mono attributes with `/MONO`. The Save page at 80 × 25, with As you work chosen (on DOS the default is Every 5 minutes):

```text
  ┌────────────────────────────── Options ───────────────────────────────┐
  │ ┌───────────────┐  Save recovery information:                        │
  │ │ General       │    ( ) &Off                                        │
  │ │ New documents │    (•) As you &work, after [ 3] seconds' pause     │
  │ │ Spelling      │    ( ) &Every [ 5] minutes                         │
  │ │▒Save▒▒▒▒▒▒▒▒▒▒│                                                    │
  │ │ View          │  Recovery &folder:                                 │
  │ │ Screen        │  [                                               ] │
  │ └───────────────┘  Empty means C:\QUILL\RECOVER\                     │
  │                    [ ] Always keep a &backup copy                    │
  │                                                                      │
  │       < OK >      < Cancel >      < &Defaults >      < Help >        │
  └──────────────────────────────────────────────────────────────────────┘
 On a slow computer, typing may pause while Quill saves.
```

### DOS graphics

The same `Dialog::Options` state, drawn by `quill-paint`'s overlay as the file dialog's graphics form is (DOS-34, DOS-39): a list box for the pages, check boxes, sunken fields with spin arrows and drop-downs, bevelled buttons that act on release, a dotted focus rectangle. The pointer works every control; the wheel moves the page list or an open drop-down.

### Windows 3.x

Word 2.0's Tools > Options: a **Category** list on the left, the category's controls on the right, OK, Cancel, Apply and Help down the right edge, and a **Description** line at the bottom that follows the focused control. Windows 3.1 has no tab control, so one dialog template holds every page and choosing a category shows and hides groups (`ShowWindow`, `EnableWindow`). Labels live in the dialog resource, help lines in `RT_STRING`, the schema in `RT_RCDATA`, the draft on the dialog procedure's stack; the dialog procedure goes in its own discardable code segment, `OPTIONS` (W16-92). The recovery folder is a text field (Windows 3.1 has no folder picker), checked on OK.

### Windows 95 and later

A property sheet (`PropertySheetA`, the 40-byte V1 structure sizes so retail Windows 95 takes it) with the tabs General, New documents, Spelling, Save and View; OK, Cancel and Apply; any change sends `PSM_CHANGED`, `PSN_KILLACTIVE` validates with the flow's messages, `PSN_APPLY` commits; What's This through `WinHelpA` with `HELP_WM_HELP` and `HELP_CONTEXTMENU` and a generated control-to-topic array (W9X-110). The recovery folder has `Browse...` through `SHBrowseForFolderA` (W9X-112). Controls are standard, each label before its control in the tab order, so MSAA and the keyboard find them.

### GTK

An `AdwPreferencesDialog` with search, pages General, New documents, Spelling, Save, View and Advanced, and rows from the schema (the label is the title, the help line the subtitle): `AdwSwitchRow`, `AdwComboRow`, `AdwSpinRow`, `AdwEntryRow`, a `GtkFontDialogButton` for the font, `GtkFileDialog.select_folder` for the recovery folder, and Import, Export and Reset all as button rows with Reset behind an `AdwAlertDialog` (LNX-60). The autosave rows are an `AdwComboRow` for the mode (Off, As you work, Every few minutes) with `AdwSpinRow`s for the pause and the minutes under it, the one that does not apply greyed. Live apply, as the GNOME guidelines ask; the theme row drives `adw::StyleManager`.

### WinUI

A Settings page in the window, as Windows 11 Notepad has: Back (Alt+Left or Esc) returns to the document; a `NavigationView` with a `Find a setting` box and one item per page; setting cards (corner radius 4, card brushes, minimum height 68) with `ToggleSwitch`, `ComboBox`, `NumberBox` and an `Expander` for the margins; an `InfoBar` for a Start setting; `AutomationProperties.Name` and `HelpText` from the schema; F6 between the pane and the content (WUI-60).

### Later: the web and Desk

The web demo would draw a `<dialog>` from the schema with `QUILL.INI` text in IndexedDB; Desk would implement `Profile` over `desk_setting_get` and `desk_setting_put`. Neither is in this series.

## Decisions

Ranges reserved for this series, above every id on `main` and in the other plans. The Windows 95 range is W9X-110 to W9X-119 because W9X-90 to W9X-94 went to Print to PDF (WP-WEB-3) and W9X-95 to W9X-99 are reserved by the Reveal Codes plan. **DOS-80 to DOS-89 are carved out of the editor track's DOS-70 to DOS-89** (it keeps DOS-70 to DOS-79, where its own are); LNX-60 to LNX-69 and WUI-60 to WUI-69 are the editor track's ranges for a UI slice it builds itself, which these are. The ids WP-ED-13 to WP-ED-16 named (CORE-106 to CORE-109, ARCH-19, DOS-73, W16-62, W16-66, W9X-63) stay spare and are not reused.

| Area | Range |
|---|---|
| CORE | CORE-150 to CORE-169 |
| ARCH | ARCH-30 to ARCH-34 |
| DOS | DOS-80 to DOS-89 |
| W16 | W16-90 to W16-99 |
| W9X | W9X-110 to W9X-119 |
| LNX | LNX-60 to LNX-64 |
| WUI | WUI-60 to WUI-64 |
| INS | INS-8 and INS-9 |

ARCH-30 is recorded in `docs/decisions.md` with this plan; each package records its own.

| Id | Decision | Package |
|---|---|---|
| ARCH-30 | The preferences work is the WP-PREF series of this plan, replacing WP-ED-13 to WP-ED-16; `quill-settings` is created by WP-PREF-1 as new files ahead of WP-ARCH-6, which then moves the recent list and the editor onto it | This plan |
| ARCH-31 | The schema is a compiled, versioned byte table (`QSET`) read in place on every build, from `include_bytes!` or `RT_RCDATA` | WP-PREF-1 |
| ARCH-32 | The SPEC's settings table is generated from the schema and checked by `make-docs --check` | WP-PREF-2 |
| CORE-150 | Autosave every 5 minutes on DOS, Windows 3.x and Windows 95, every 2 on the terminal, GTK, WinUI and the web; 1 to 60, 0 read as Off; decided by the owner on 2026-11-27 | WP-PREF-0 |
| CORE-151 | `QUILL.INI` format version 1: grammar, encodings, limits, `[Settings] Version`, migrations, never lowered, unknown lines kept byte for byte, family keys | WP-PREF-1 |
| CORE-152 | Layers: build default, `QUILLDEF.INI`, `QUILL.INI`, session; a value equal to the inherited one deletes its key; writes merge after a fresh read | WP-PREF-1 |
| CORE-153 | Settings are read and written by id through the schema (`App::get`, `App::set`); the dialog edits a `Draft`; three apply classes | WP-PREF-2 |
| CORE-154 | `[Spelling] Language=en-CA`, migrated once from `[Quill] Language` | WP-PREF-2 |
| CORE-155 | Export, import and reset: what is exported, the report, what Reset keeps | WP-PREF-4 |
| CORE-156 | Tools > Options on the retro builds and the terminal, Preferences on GTK, Settings on WinUI; the pages and their order; Spelling's Options opens the Spelling page | WP-PREF-5 |
| CORE-157 | Apply styles: OK and Cancel on DOS and the terminal, OK, Cancel and Apply on Windows, live on GTK and WinUI; a second window takes changes at its next start | WP-PREF-5 |
| CORE-158 | Autosave rows: a new interval counts from the next idle pass; off keeps existing copies; a new folder is probed, and the old one read at launch until empty (`RecoveryDirPrev`) | WP-PREF-5 |
| CORE-159 | Start-up: a blank document or the last one, after the recovery question, and never over a file on the command line | WP-PREF-5 |
| CORE-160 | `Backup=1` keeps the previous file as `<name>.BAK`, made by the safe save's replace step | WP-PREF-13 |
| CORE-161 | `AutosaveMode`: Off, As you work (a copy after `AutosavePause` seconds of quiet, at least 10 seconds after the first change, the minutes as the backstop and the only forced write) or Every N minutes; As you work by default on Windows 95 and the modern clients, Every 5 minutes on DOS and Windows 3.x; it writes the recovery copy, never the document; family keys for the mode and the pause | WP-PREF-0 |
| DOS-80 | The text-cell Options dialog is built from the schema; the graphics screen and the terminal draw the same state | WP-PREF-5 |
| DOS-81 | `Rows`, `Graphics` and `Scheme` are settings applied at the next start; the switches override them for the session only and are never saved (`Kept` goes) | WP-PREF-2 |
| DOS-82, DOS-83, DOS-85 | Budget raises for WP-PREF-2, WP-PREF-5 and WP-PREF-6, each after savings, by the measured cost rounded up to 4 KB, with the floppy's free bytes (DOS-70) | WP-PREF-2, 5, 6 |
| DOS-84 | Pointer speed through INT 33h function 1Ah (read with 1Bh first and put back at exit) and a scale in the PS/2 reader; the double-click time replaces `DOUBLE_TICKS` | WP-PREF-6 |
| W16-90 | `QUILL.INI` through the profile functions in the Windows directory | WP-PREF-3 |
| W16-91 | The schema from `RT_RCDATA`; the draft on the dialog's stack | WP-PREF-2 |
| W16-92 | One category dialog template with Apply; its code in the `OPTIONS` segment | WP-PREF-8 |
| W9X-110 | A COMCTL32 4.00 property sheet with V1 sizes, Apply and What's This | WP-PREF-7 |
| W9X-111 | Settings stay in `QUILL.INI` through the profile functions, not the registry | WP-PREF-7 |
| W9X-112 | The recovery folder's Browse through `SHBrowseForFolderA` | WP-PREF-7 |
| LNX-60 | `AdwPreferencesDialog`, live apply, writes after 500 ms of quiet, Ctrl+,; `QUILL.INI`, not GSettings | WP-PREF-10 |
| LNX-61 | The terminal reuses the DOS text dialog and keeps `[RecentLinux]` | WP-PREF-9 |
| WUI-60 | A Settings page in the window with a navigation pane and cards, live apply; `%APPDATA%\Quill\QUILL.INI` | WP-PREF-11 |
| INS-8 | The installer presets settings only through `QUILLDEF.INI` in the program's folder | WP-PREF-12 |

## Order and lanes

The lanes are work-packages.md's: the **ARCH lane** (moves), the **ED lane** (one open branch in `crates/quill-app/src`), a **side** slot when the cap of three builders allows, and the **UI track**'s own lane for GTK and WinUI. Waves are that page's.

| When | ED lane | Side | UI track | Waits for |
|---|---|---|---|---|
| Now | WP-PREF-0 autosave defaults and the mode (`recovery.rs`, `idle.rs`; files no open ED package owns) | WP-PREF-1 the model (new files) | — | — |
| After WP-PREF-1 | — | WP-PREF-4 `quill settings` | — | WP-PREF-1 |
| Wave 6 | (WP-ARCH-6 in the ARCH lane adopts WP-PREF-1's `ini` and `Profile`) | — | — | WP-PREF-1, WP-ARCH-5 |
| Wave 9 (after the freeze) | WP-PREF-2 settings by id | — | — | WP-ARCH-6, WP-ARCH-7, WP-ED-11 |
| Wave 10 | WP-PREF-5 Tools > Options, the flow and the DOS text dialog | WP-PREF-3 Windows 3.x profile functions | — | WP-PREF-2, WP-ED-9 |
| Wave 11 | WP-PREF-6 DOS graphics and the pointer | WP-PREF-7 Windows 95 sheet; WP-PREF-9 terminal | WP-PREF-10 GTK | WP-PREF-5 (and WP-GTK-1, merged) |
| Wave 12 | (WP-ED-18 Numbering) | WP-PREF-8 Windows 3.x dialog | WP-PREF-11 WinUI (once WP-WINUI-5 merges) | WP-PREF-3, WP-ARCH-9, WP-ED-5 for WP-PREF-8 |
| Wave 13 | WP-PREF-13 backup copy | WP-PREF-12 installer defaults (when the installer can write INI files) | — | WP-PREF-5; the Liminal Installer INI writes |

- **WP-PREF-0 starts now** (the owner, 2026-11-27). It owns the autosave settings in `crates/quill-app/src/recovery.rs` and the probe's inputs in `idle.rs`, which no open package owns; if another ED-lane branch is open in `quill-app`, both proceed on their own files and the second to merge rebases. WP-ARCH-3 (`quill-session`) must not touch either file and waits for nothing here.
- **Autosave on the other builds is the autosave plan's**, not this series': Windows 3.x (its PR 8), Windows 95 (PR 9), the terminal (PR 11), GTK (PR 18) and WinUI (PR 19). Each of those calls `configure_autosave` with its build's defaults (As you work on Windows 95, the terminal, GTK and WinUI) and turns its build's bit on in the schema's autosave rows, so the Save page grows them the day the build autosaves.
- **WP-ED-12** (default tab stops in the document) adds its row's build bits when it lands; WP-PREF-5 does not wait for it.
- **Three builders at most.** Waves 11 and 12 also hold WP-ED-17 to WP-ED-19 (Page Layout, Numbering, Header and Footer); the orchestrator fills the three slots from both lists in their order, the ED lane's package first, and the UI track's packages run in that track's own slots.

## Budgets

| Limit | Now | This series |
|---|---|---|
| `QUILLDOS.EXE` | About 436,224 of 442,368 bytes, about 6 KB left (DOS-48) | WP-PREF-0 +0.3 to +0.6 KB **[E]** (the mode, the pause and their keys), inside the 6 KB; WP-PREF-2 +2 to +4 KB **[E]** (the schema 1.4 KB, `Values`, the layers, less the hand-written match and `Kept`); WP-PREF-5 +6 to +10 KB **[E]** (the dialog is one painter over the schema, not code per page, plus the labels table about 2.5 KB); WP-PREF-6 +2 to +4 KB **[E]**; WP-PREF-13 +0.5 KB. +10.8 to +19.1 KB, replacing the editor track's +8 to +14 for WP-ED-13 and WP-ED-15. Raises follow DOS-70, each in its own DOS row (DOS-82, DOS-83, DOS-85); the stop at 480 KB or 128 KB free on the floppy holds |
| DOS floppy | About 427,000 bytes free (DOS-48) | The executable packs to about 58 %, so the series costs about 6 to 11 KB of floppy; each package reports `installer/tools/verify-disks.sh` |
| Windows 3.x data segment | About 2 KB free; nothing merges below 1,024 (W16-60) | WP-PREF-2 at most +40 bytes (the schema's handle; the table is in `RT_RCDATA` from the start); WP-PREF-3 0 or less (the whole-file buffer goes); WP-PREF-8 +100 to +300 **[E]**, with 1,024 still free |
| Windows 3.x stack | `check_stack_log.py` margin | At least 2,048 to spare with the Options dialog open and its draft (about 520 bytes) on the stack |
| Windows 3.x code | `segments.json` | The `OPTIONS` segment gets its own budget line (WP-PREF-8) |
| Windows 3.x and 95 floppies | `verify-disks.sh` | Each package reports the disks it grows |
| `QUILL32.EXE` imports | `win9x/tools/imports.allow` | WP-PREF-7 adds `comctl32.dll!PropertySheetA`, `shell32.dll!SHBrowseForFolderA`, `shell32.dll!SHGetPathFromIDListA`, `shell32.dll!SHGetMalloc` (`WinHelpA`, `InitCommonControls` and `GetCaretBlinkTime` are already there) |

## Tests and goldens

- **Autosave** (WP-PREF-0): host tests over `@clock` and `@quiet` for each mode (As you work: no copy at 2 seconds of quiet, a copy at 3 once 10 seconds have passed since the first change, a new copy after more typing and another pause, a cut-short write retried at the next pause and forced only after the backstop's minutes; Every N minutes: as today; Off: none), the INI keys at and away from the defaults and the version 0 reading of `Autosave=0`; DOSBox `m17-autosave-work` (`AutosaveMode=work`, type, pause three seconds, cut off, the next start offers the copy).
- **The model** (WP-PREF-1), 10,000 cases each with `proptest`: `parse(format(v)) == v` for every kind; `ini::rewrite` with no changes is the identity on any bytes, and with changes alters only the named keys; `IniFile` agrees with a map model and with a fake profile API that follows the Windows rules; random bytes never panic the INI reader or the schema reader; a value equal to the inherited one deletes its key; an 8.3 build never reads a `*32`, `*Linux` or `*WinUI` key; migration is idempotent and version is never lowered. Goldens in `tests/golden/settings/`: `settings.qset` and its text dump `schema.txt`, the defaults INI each build would write (`defaults-dos.ini` to `defaults-winui.ini`), a version 0 file and its migration, an export.
- **The schema's sync tests:** `settings.qset` and `generated.rs` equal what `emit-resources --settings` makes from `settings.json`; keys and mnemonics unique per page; every page fits `Focus<16>`; the SPEC's table equals `make-docs --check` (WP-PREF-2).
- **The editor** (WP-PREF-2): `m4-quill.ini` changes once, reviewed (the version line and the language key); a 4 KB INI loads whole (B3); unknown keys and sections survive a DOS save (B4); `/50` is never saved; Windows 95 can go back to Canadian English (B1).
- **Flows** (WP-PREF-5), in `tests/golden/flows/`, with the INI bytes in each transcript: open and cancel, change and OK, Apply then Cancel, Defaults, an invalid number, a Start setting's note, the autosave interval down to 1 minute with a copy written at the next pause (`@clock`), autosave off keeping the copy, a new recovery folder and `RecoveryDirPrev` at the next launch. The DOS text driver gives the same transcript as the native stub.
- **Screens:** every page at 80 × 25, 80 × 50 and mono (`tests/golden/screens/`); the graphics dialog as frame CRCs in 16 and 256 colours; DOSBox runs equal to the host cell for cell; `m18-options-autosave` in real time (set 1 minute in the dialog, type, pause, cut the program off, the next start offers the copy).
- **Windows:** a binding test that every control in the Windows 3.x and 95 pages names a schema id of the right kind; Windows 3.11 play-back through every category; Wine opens every tab, Apply writes a key that reads back, Cancel leaves the file byte-identical; one What's This pass on Windows 95 in DOSBox-X.
- **GTK and WinUI:** a row toggled changes the file within a second; search finds Language; AT-SPI and UI Automation names equal the labels; light and dark shots.

## Work packages

Every package uses work-packages.md's rules (owned files, branch late, rebase, the `dialogs.json` rule) and the common brief. Evidence goes to `~/Documents/Liminal HQ/Quill/Editor features/WP-PREF-n/`.

### WP-PREF-0 — Autosave every 5 minutes, and autosave as you work

- **Finish line:** the editor has `AutosaveMode` (Off, As you work, Every N minutes) and `AutosavePause`, with the scheduling of [Autosave as you work](#autosave-as-you-work), read and written as `QUILL.INI` keys at and away from each build's defaults; a shell gives its defaults with one call; the DOS screens default to Every 5 minutes and take `AutosaveMode=work` from the file; the SPEC gives every build's defaults (5 minutes on DOS, Windows 3.x and Windows 95, 2 on the modern clients; As you work on Windows 95 and the modern clients), so the Windows, terminal, GTK and WinUI autosave packages build them that way; a hand-written `Autosave=10` and an old `Autosave=0` keep their meaning.
- **Owns:** the autosave settings in `crates/quill-app/src/recovery.rs` (`configure_recovery`, a new `configure_autosave`, `autosave_interval`, `autosave_wait`, `rcv_each_setting`, `rcv_apply_setting`); the forced-write rule in `autosave_go` and the probe's interval and quiet in `autosave_pass` in `crates/quill-app/src/idle.rs`; the `configure_recovery` call and its comment in `dos/quill-dos/src/main.rs`; the same call in `crates/quill-host/src/script.rs` and `crates/quill-paint/tests/shell.rs`; `crates/quill-host/tests/autosave_modes.rs` (new); the DOS scenario `m17-autosave-work`; the autosave sentences and rows of `SPEC.md`; the D195 row and sentence in `docs/linux.md`; CORE-150, CORE-161; a progress row.
- **Must not touch:** the job order in `App::idle`; `save_recovery` and the recovery file names; the questions (CORE-69); any Windows, terminal, GTK or WinUI shell (none autosaves yet).
- **Depends on:** nothing. Starts now, in the ED lane, on files no open package owns.
- **Budget:** DOS +0.3 to +0.6 KB **[E]**, inside the 6 KB left; no other build links it differently.
- **Tests:** the autosave bullet of [Tests and goldens](#tests-and-goldens); every existing golden unchanged; G-HOST, G-DOS, G-MD.
- **Evidence:** `QUILL.INI` after runs with the default, with `Autosave=10` and with `AutosaveMode=work`, as text files; the `m17-autosave-work` recovery question on `dos-text-25`.
- **Decisions:** CORE-150, CORE-161.

### WP-PREF-1 — `quill-settings`: the model, the schema and the file format

- **Finish line:** `crates/quill-settings` with the modules of [The model](#the-model-quill-settings); `assets/settings/settings.json` describing all 32 settings; `emit-resources --settings` writing `settings.qset`, `settings.qstx` and `generated.rs`; `docs/settings-format.md` with the INI and `QSET` formats; the property tests and goldens; nothing links it yet.
- **Owns:** `crates/quill-settings/**` (new); `assets/settings/**` (new); the `--settings` mode in `crates/quill-host/src/bin/emit-resources.rs` (a new function and its argument branch only); `docs/settings-format.md` (new); `tests/golden/settings/**` (new); the root `Cargo.toml` members and `Cargo.lock`; the crate row in `docs/architecture.md`; ARCH-31, CORE-151, CORE-152.
- **Must not touch:** `crates/quill-app`; any shell; `recent.rs` (WP-ARCH-6 moves it).
- **Depends on:** nothing.
- **Budget:** none linked.
- **Tests:** the model's property tests and goldens; a `no_std` build for `x86_64-unknown-none`; G-HOST, G-MD.
- **Evidence:** `schema.txt` and the defaults INI for each build, copied to the evidence folder.
- **Decisions:** ARCH-31, CORE-151, CORE-152.
- **Changes WP-ARCH-6:** its finish line stays (every `QUILL.INI` byte-identical), but it adopts this crate's `ini` and `Profile` and moves only `recent` and `keys` in.

### WP-PREF-2 — Settings by id in the editor (replaces WP-ED-13)

- **Finish line:** `App::get(id)` and `App::set(id, value)` over the schema replace `each_setting` and `apply_setting`'s match (both stay as thin wrappers for the shells until each moves); the layers, delete-on-default and the merging write on every build; `[Spelling] Language` with the migration; DOS `Rows`, `Graphics` and `Scheme` as settings with the switches as session overrides; the Win32 key list from the schema (B5); the SPEC's settings table generated; B1, B3, B4, B6 fixed.
- **Owns:** the settings region of `crates/quill-app/src/lib.rs`; `crates/quill-app/src/ini_file.rs`; `rcv_each_setting` and `rcv_apply_setting` in `recovery.rs`; the settings load and save and `Kept` in `dos/quill-dos/src/main.rs`; `ini_load` and `ini_save` in `win9x/quill-win9x/src/ui.rs`; Win16's settings load (`RT_RCDATA` for the schema) in `win16/examples/quill/main.rs`; `make-docs`' settings table; SPEC Settings; ARCH-32, CORE-153, CORE-154, DOS-81, W16-91.
- **Must not touch:** Win16's whole-file write (WP-PREF-3); any dialog; the meaning of any setting beyond the bugs named.
- **Depends on:** WP-PREF-1, WP-ARCH-6, WP-ARCH-7, WP-ED-11.
- **Budget:** DOS +2 to +4 KB **[E]** (DOS-82 if over); Win16 data ≤ +40.
- **Tests:** the editor tests of [Tests and goldens](#tests-and-goldens); G-HOST, G-DOS, G-W16 (data figure), G-W32, G-GTK, G-MD.
- **Evidence:** `QUILL.INI` before and after a language change and back, on DOS, Windows 3.11 and Wine; the spelling language on `dos-text-25` and `win32-wine`.

### WP-PREF-3 — Windows 3.x settings through the profile functions (replaces WP-ED-14, side)

- **Finish line:** `QUILL.EXE` reads and writes through `GetPrivateProfileString` and `WritePrivateProfileString` with a bare name, so the file is in the Windows directory and the program's folder holds none (B2); `[Recent32]` and every other key survive a Windows 3.x session.
- **Owns:** the Win16 `Profile` implementation and the removal of its whole-file write in `win16/examples/quill/main.rs`.
- **Depends on:** WP-PREF-2. Parallel with WP-PREF-5.
- **Budget:** Win16 data ≤ 0; stack gate.
- **Tests:** G-W16 (`C:\WINDOWS\QUILL.INI` holds the keys after a run on 3.11, with a `[Recent32]` section put there first still present), G-MD.
- **Evidence:** a directory listing and the file as text.
- **Decisions:** W16-90.

### WP-PREF-4 — `quill settings` on the command line (side)

- **Finish line:** `quill settings get KEY`, `set KEY VALUE`, `reset [PAGE]`, `export FILE`, `import FILE` and `list`, with `--build dos|w16|w32|linux|winui` and `--file PATH` (default the Linux client's file), over `quill-settings` alone.
- **Owns:** `crates/quill-cli/src/settings.rs` (new), the `settings` branch and usage lines in `crates/quill-cli/src/main.rs`; `tests/golden/cli/settings-*`; CORE-155.
- **Depends on:** WP-PREF-1.
- **Tests:** export then import is a fixed point; unknown and invalid keys reported; an 8.3 `--build` never writes a `*32` key; G-HOST, G-MD.
- **Evidence:** a terminal transcript of each verb.
- **Decisions:** CORE-155.

### WP-PREF-5 — Tools > Options: the flow, the draft and the DOS text dialog (replaces the shared part of WP-ED-15)

- **Finish line:** the Tools menu with Options... on DOS, Windows 3.x and 95 (Windows draws it with nothing behind it until WP-PREF-7 and WP-PREF-8: the item is greyed there); `Command::Options(page)`, `Dialog::Options(page)`, the draft, the flow entry points (`options_start`, `options_changed`, `options_page`, `options_defaults`, `options_apply`, `options_cancelled`) and the host verbs (`options page`, `options set`, `options apply`, `options defaults`, `expect setting`, `dump settings`); the text-cell dialog at 80 × 25, 80 × 50 and mono; Spelling's `< Options >`; the autosave rules of CORE-158; `Startup` (CORE-159); `RecentCount` and Clear.
- **Owns:** the Tools menu in `crates/quill-menu`; the options flows in `crates/quill-flow`; `crates/quill-app/src/options.rs` (new) and its `Mode` and widget entries; `MENU_COLS` and the options drawing in `crates/quill-app/src/paint.rs`; the restart note in `dos/quill-dos/src/main.rs`; the help topics in `assets/help/quill.qhm` (one per page); flows and screen goldens.
- **Depends on:** WP-PREF-2, WP-ED-9.
- **Budget:** DOS +6 to +10 KB **[E]** (DOS-83 after savings).
- **Tests:** the flows; every text screen golden regenerated in a commit of its own for the menu bar, and reviewed; DOSBox equal to the host; `m18-options-autosave`; G-HOST, G-DOS, G-DISK, G-MD.
- **Evidence:** every page on `dos-text-25`, `dos-text-50` and mono; the Tools menu on each.
- **Decisions:** CORE-156 to CORE-159, DOS-80, DOS-83.

### WP-PREF-6 — Options on the DOS graphics screen, and the pointer settings

- **Finish line:** the graphics screen draws `Dialog::Options` as a Windows 3.1 dialog with the pointer working every control; Pointer speed and Double-click speed on the General page of both DOS screens, put into effect at once and the driver's sensitivity put back at exit.
- **Owns:** the options drawing in `crates/quill-paint`'s overlay; `dos/quill-dos/src/mouse.rs` and `ps2.rs` (speed and scale); `DOUBLE_TICKS`' use in `crates/quill-app/src/mouse.rs` (it becomes the setting); graphics goldens.
- **Depends on:** WP-PREF-5.
- **Budget:** DOS +2 to +4 KB **[E]** (DOS-85).
- **Tests:** frame CRCs in 16 and 256 colours; a pointer script through every page; the double-click time in a host mouse test; G-HOST, G-DOS, G-DISK.
- **Evidence:** every page on `dos-gfx-12h` and `dos-gfx-vbe`.
- **Decisions:** DOS-84, DOS-85.

### WP-PREF-7 — The Options property sheet on Windows 95 (replaces half of WP-ED-16, side)

- **Finish line:** the sheet of [Windows 95 and later](#windows-95-and-later) from `dialogs.json` pages whose controls carry `"setting": "<id>"`; the Tools item enabled; Browse for the recovery folder; What's This.
- **Owns:** the Options entries in `assets/dialogs/dialogs.json` (regenerating `generated.rs`); the Options code in `win9x/quill-win9x/src/dialogs.rs`; `res_pack.py` styles for the sheet's pages; `win9x/tools/imports.allow`; W9X-110 to W9X-112.
- **Depends on:** WP-PREF-5.
- **Budget:** the import list as named under [Budgets](#budgets).
- **Tests:** the binding test; G-W32 (Wine opens every tab, Apply writes a key that reads back, Cancel leaves the file byte-identical, the import and Pentium scans); G-DISK; G-MD.
- **Evidence:** every tab on `win32-wine`; the Save tab and What's This on `win95`.

### WP-PREF-8 — The Options dialog on Windows 3.x (replaces the other half of WP-ED-16, side)

- **Finish line:** the category dialog of [Windows 3.x](#windows-3x) with its description line, Apply and F1; the Tools item enabled.
- **Owns:** the Win16 Options entries in `dialogs.json`; the Options code in `win16/examples/quill/dialogs.rs` (new module `options.rs` if the segment rule wants it); the `OPTIONS` line of `win16/examples/quill/segments.json`; the help-line strings in `emit-resources`' Win16 string output; W16-92.
- **Depends on:** WP-PREF-5, WP-PREF-3, WP-ARCH-9, WP-ED-5 (the data reliefs of W16-60).
- **Budget:** Win16 data +100 to +300 **[E]**, 1,024 still free; stack margin ≥ 2,048 with the dialog open.
- **Tests:** the binding test; G-W16 (3.11 play-back through every category, the data and stack figures, the segment check); G-DISK; G-MD.
- **Evidence:** every category on `win16-311`.

### WP-PREF-9 — Options in the Linux terminal (side)

- **Finish line:** `quill-tty` opens the DOS text dialog from Tools > Options; the Screen page shows Colours only; the terminal writes `[RecentLinux]` and reads an old `[Recent]` once (B7).
- **Owns:** `linux/quill-tty/src/session.rs`'s settings and menu handling; its scenario tests; LNX-61.
- **Depends on:** WP-PREF-5.
- **Tests:** the same frames as DOS for each page; the pty scenarios; G-HOST.
- **Evidence:** the Save page in a terminal capture.

### WP-PREF-10 — Preferences on GTK (UI track; replaces the Options item of WP-GTK-7)

- **Finish line:** the `AdwPreferencesDialog` of [GTK](#gtk), live, with Import, Export and Reset all; Ctrl+,, the primary menu and Tools > Preferences.
- **Owns:** `linux/quill-gtk/src/preferences.rs` (new); the action and menu lines in `src/actions.rs` and `src/menu.rs`; `tests/preferences.rs`; LNX-60.
- **Depends on:** WP-PREF-5 (the draft and `App::set`); WP-GTK-1 (merged). The Theme row follows WP-GTK-4 when it lands.
- **Tests:** a row toggled changes `QUILL.INI` within a second; search finds Language; AT-SPI names equal the labels; G-GTK.
- **Evidence:** every page in light and dark, from the UI track's compositor run.

### WP-PREF-11 — Settings on WinUI (UI track)

- **Finish line:** the Settings page of [WinUI](#winui), live, with Import, Export and Reset all, in `%APPDATA%\Quill\QUILL.INI`.
- **Owns:** the settings page files under `winui/quill-winui/`; WUI-60.
- **Depends on:** WP-PREF-5, WP-WINUI-5. The theme rows follow WP-WINUI-8.
- **Tests:** a UI Automation script finds every card by name and changes one; light and dark at 100 % and 150 %.
- **Evidence:** every page in light and dark.

### WP-PREF-12 — Machine defaults from the installer (later)

- **Finish line:** Quill reads `QUILLDEF.INI` (already a layer since WP-PREF-2); the Quill installer writes it from a components page (spelling language, units and paper from the country, the DOS screen), and uninstall removes it and leaves `QUILL.INI`.
- **Owns:** `installer/` project entries; INS-8. Needs the Liminal Installer INI writes for retro targets (the preferences design, section 4).
- **Depends on:** WP-PREF-2 and that installer change.
- **Tests:** installing with British English gives `[Spelling] Language=en-GB` in `QUILLDEF.INI` and Quill starts in it with no `QUILL.INI` key; installer e2e on DOS, 3.11 and 95.

### WP-PREF-13 — The backup copy

- **Finish line:** with `Backup=1`, a save keeps the file it replaces as `<name>.BAK` (8.3 builds: the extension becomes `BAK`; long names: `LETTER.QLD.bak`), made in the safe save's replace step so a failed save still leaves the original; the Save page's row is shown on every build.
- **Owns:** the replace step of the safe save in `crates/quill-app`; its tests; CORE-160.
- **Depends on:** WP-PREF-5.
- **Budget:** DOS +0.5 KB **[E]**; Win16 data ≤ +16.
- **Tests:** the safe-save property tests with the backup on; a full disk leaves the original and no `.BAK` change; G-HOST, G-DOS, G-W16, G-W32.

## Briefs

Each brief follows the common brief of [work-packages.md](../work-packages.md#the-common-brief). The first two are in full.

### WP-PREF-0 (full)

```text
Package WP-PREF-0: autosave every 5 minutes on DOS and Windows 3.x, and autosave as you work. The
owner approved the defaults on 2026-11-27 and wants this now, ahead of the Options window. Plan:
docs/plans/preferences.md ("Autosave's default: change it now", "Autosave as you work",
WP-PREF-0). Read crates/quill-app/src/idle.rs and recovery.rs first, and SPEC "Idle time and
autosave".

Branch feat/autosave-as-you-work from origin/main; worktree
~/source/worktrees/quill/autosave-work. You own the autosave settings in recovery.rs and the
probe's inputs and the forced-write rule in idle.rs; nothing else in crates/quill-app/src. If
another branch is open in quill-app, keep to these files; the second to merge rebases.

1. recovery.rs: add `pub enum AutosaveMode { Off, Work, Minutes }` (Copy, Eq, Debug) and
   `pub fn configure_autosave(&mut self, mode: AutosaveMode, minutes: u8, pause: u8)` that sets
   the value and the default of each (minutes 1 to 60, pause 1 to 30, clamped);
   configure_recovery keeps its signature and sets minutes as today, so existing callers work.
   Getters and setters for mode and pause beside autosave_minutes. Constants with doc comments:
   WORK_FLOOR = 10 (seconds after the first change before an As-you-work copy), PAUSE_DEFAULT = 3.
2. The two numbers the probe gets (autosave_interval, autosave_wait): Off gives interval 0;
   Minutes gives minutes * 60 and today's wait (2 s on the 8.3 builds, 1 on the others); Work gives
   WORK_FLOOR and the pause. The debug hook AutosaveSeconds still wins over all of it, as today.
   Probe and the job order do not change.
3. idle.rs autosave_go: in Work mode the forced write (cut >= ABANDONS) happens only once the
   seconds since the first change reach minutes * 60; before that an abandoned write just counts
   and waits for the next pause. Minutes mode is exactly as today. Say why in a comment.
4. Settings keys (rcv_each_setting, rcv_apply_setting): AutosaveMode (off, work, minutes; case-
   insensitive; anything else is the default) and AutosavePause, each written only when it
   differs from its default, like Autosave. The family key the build reads and writes is
   AutosaveMode / AutosavePause on the 8.3 builds, AutosaveMode32 / AutosavePause32 on Win32,
   ...Linux on the Linux clients, ...WinUI on WinUI: take the family from the existing Style
   (Dos, Long, Dot) the way RecoveryDir does, and leave a WinUI case for its shell to add.
   Autosave stays one shared key, 1 to 60; reading Autosave=0 with no mode key in the file means
   Off (the old meaning), and writing Off writes AutosaveMode=off and leaves Autosave alone.
5. dos/quill-dos/src/main.rs: `const AUTOSAVE_MINUTES: u8 = 5;` with a doc comment citing
   CORE-150, and after configure_recovery(AUTOSAVE_MINUTES, writer, false) call
   configure_autosave(AutosaveMode::Minutes, AUTOSAVE_MINUTES, 3) (CORE-161). Rewrite the comment
   above it: every five minutes unless QUILL.INI says otherwise, or as you work with
   AutosaveMode=work. crates/quill-host/src/script.rs and crates/quill-paint/tests/shell.rs: 10
   becomes 5, same mode, so the host stays the DOS screens' twin.
6. Tests, new crates/quill-host/tests/autosave_modes.rs (SPDX header), with the clock and quiet of
   crates/quill-host/tests/idle.rs: the cases in the plan's "Autosave" tests bullet, plus Minutes at
   4 min 59 s (none) and 5 min with 2 s quiet (one); INI: no autosave lines at the defaults,
   Autosave=10, AutosaveMode=work and AutosavePause=5 written when set and read back, an old
   "[Quill]\r\nAutosave=0\r\n" reads as Off and is not rewritten as on; a Win32-style app
   (Style::Long) reads AutosaveMode32 and ignores AutosaveMode. Every existing golden must stay
   unchanged; if one changes, fix the test's set-up, not the golden, and say so in the PR.
7. DOS scenario m17-autosave-work beside m17-autosave (same harness; QUILL.INI with
   AutosaveMode=work and AutosavePause=2): type, wait three seconds, cut the program off, the next
   start offers the copy, Recover brings the letters back. Host and DOSBox frames equal.
8. Docs in the same change:
   - SPEC.md: "Idle time and autosave" gets the modes (the plan's table and rules: the floor, the
     pause, the backstop and the forced write, no flash in As you work, the recovery copy and
     never the document) and the defaults (5 minutes on DOS, Windows 3.x and Windows 95, 2 on
     Linux; As you work on Windows 95, Linux and WinUI, Every N minutes on DOS and Windows 3.x);
     the DOS "Autosave and recovery" bullet says every five minutes, or as you work with
     AutosaveMode=work; the settings table gets AutosaveMode and AutosavePause rows and the new
     Autosave defaults.
   - docs/linux.md: the D195 row and the autosave sentence, citing CORE-150 and CORE-161.
   - docs/decisions.md: CORE-150 (Decided by the owner, 2026-11-27: the minutes above; why: the
     plan's section) and CORE-161 (Assumed, with "OWNER DECISION: As you work by default on
     Windows 95 and the modern clients, Every 5 minutes on DOS and Windows 3.x; swap in each
     shell's configure_autosave call and the SPEC rows"; alternatives: As you work everywhere, or
     only on the modern clients; writing the document itself; a quiet of 1 or 5 seconds).
   - docs/progress.md: a row with what was run.
9. Gates: G-HOST; G-DOS (dos/tools/size-report.sh before and after; report both and the
   difference, expected +0.3 to +0.6 KB; past the budget stop and report); G-MD. No Windows, terminal
   or GTK shell calls configure_autosave yet; say so.
10. Evidence in ~/Documents/Liminal HQ/Quill/Editor features/WP-PREF-0/: the three QUILL.INI files
   of the package's evidence line, and the recovery question of m17-autosave-work on dos-text-25.
11. PR: "Autosave every five minutes, or as you work". Labels: enhancement, core, dos.

Finish line: the DOS screens write a recovery copy five minutes after the first unsaved change by
default, or three seconds after each pause with AutosaveMode=work; Off, an old Autosave=0 and a
hand-set Autosave=10 keep their meaning; the SPEC gives every build's defaults; the size is within
the budget and every existing golden is unchanged.
```

### WP-PREF-1 (full)

```text
Package WP-PREF-1: create crates/quill-settings, the one settings model, with a compiled versioned
schema, the QUILL.INI codec and the file format written down. New files only: nothing links the
crate yet. Plan: docs/plans/preferences.md ("The settings", "The model: quill-settings", "Where each
build keeps its settings", "Tests and goldens", WP-PREF-1). Also read docs/decisions.md ARCH-9 and
the preferences design in ~/Documents/Liminal HQ/Quill/Quill preferences design.md (sections 0 to 2
and the owner's answers at its end).

Branch feat/quill-settings-model from origin/main; worktree ~/source/worktrees/quill/settings-model.

1. crates/quill-settings/Cargo.toml with the workspace package fields and
     [package.metadata.quill]
     layer = "base"
     no_std = true
     alloc = false
     float = false
   and no dependencies (proptest as a dev-dependency). src/lib.rs: SPDX header, #![no_std], the
   clippy deny block quill-app uses (unwrap, expect, panic, indexing_slicing). quill-settings is
   already in ORDER in scripts/check-layers.py: do not edit that file.
2. Modules, each with doc comments and unit tests: schema (Schema<'a>(&'a [u8]) read in place: the
   QSET layout in the plan's table; a bad magic, a newer major version or any offset out of bounds
   gives an empty schema); value (Value<'a> and total parse/format per kind; numbers clamp, bad text
   is the default); ini (parse with the Windows profile rules; rewrite(old, changes, out) streaming
   the old bytes to a closure, changing, adding (at the end of the section, or a new section at the
   end of the file) or deleting only the named keys; Writer for a fresh file; CRLF out, CR, LF or CRLF
   in); profile (the Profile trait; an IniText<'a> that implements it over a caller's buffer, for
   tests and for IniFile later); layers (four layers, effective, inherited, the delete-on-default
   rule); migrate (version 0 to 1: [Quill] Language=0..2 to [Spelling] Language=en-CA|en-US|en-GB);
   draft (Draft<const N, const T>); transfer (export and import with taken and ignored counts; what
   is not exported per the plan).
3. assets/settings/settings.json: all 32 settings of the plan's table, with every field the plan's
   schema paragraph names (id, section, key, legacy keys, kind, range, defaults per build with
   "locale" where the shell supplies it, page, order, label, help line, mnemonic, builds, applies,
   family, exported, help id). Labels in sentence case, Canadian English, one mnemonic per page,
   unique. Rows that wait for a package (PointerSpeed, DoubleClick, DefaultTab, Backup, and the
   Autosave and RecoveryDir bits for the builds without autosave) carry their builds as they will be
   and a "waits" field naming the package; the schema compiles them with the build bit clear.
4. crates/quill-host/src/bin/emit-resources.rs: a --settings mode (one new function and one argument
   branch; touch nothing else) that writes assets/settings/settings.qset, settings.qstx and
   crates/quill-settings/src/generated.rs (the Id constants and PAGE and BUILD constants), and a
   --check form that fails when any differs. Commit the generated files.
5. docs/settings-format.md (new): the QUILL.INI format version 1 (grammar, encodings, limits, the
   [Settings] Version rule, migrations, family keys, the layers and QUILLDEF.INI, where each build
   keeps the file) and the QSET and QSTX layouts, byte by byte, with a worked example of one entry.
   Link it from SPEC.md's "Settings file" paragraph (one sentence; behaviour does not change).
6. Tests: the property tests of the plan's "The model" bullet at 10,000 cases each; a sync test that
   the committed generated files equal a fresh emit; goldens in tests/golden/settings/ (settings.qset,
   schema.txt, defaults-<build>.ini for dos, w16, w32, linux and winui, migrate-v0.ini and its result,
   export.ini), rewritten with QUILL_UPDATE_GOLDEN=1. Build the crate for x86_64-unknown-none to prove
   no_std (the "bare" gate of make validate does this for listed crates: add it there only if that
   list is a file this package may own; otherwise run it by hand and say so).
7. Docs: crate row in docs/architecture.md; decisions ARCH-31 (the schema is a compiled, versioned byte
   table read in place on every build; alternatives: a Rust const table with RT_RCDATA only on Win16,
   two code paths; JSON at run time, too big), CORE-151 (the INI format version 1) and CORE-152 (the
   layers and the write rules), Status Assumed; a progress row. SPEC.md changes only by the link.
8. Gates: G-HOST (fmt, clippy, nextest, headers, floats, layers: no new violation), G-MD. No shell is
   built differently: say so.
9. Evidence in ~/Documents/Liminal HQ/Quill/Editor features/WP-PREF-1/: schema.txt and the five
   defaults INI files.
10. PR: "Add quill-settings, one settings model with a versioned schema". Labels: enhancement, core.

Finish line: quill-settings builds no_std with no alloc, floats or panics; settings.json, the QSET
table and generated.rs agree; docs/settings-format.md describes every byte; the property tests and
goldens pass; no other crate changed except emit-resources' new mode.
```

### WP-PREF-2

```text
Package WP-PREF-2 (replaces WP-ED-13): settings by id in the editor. Plan: docs/plans/preferences.md
WP-PREF-2. Branch feat/settings-by-id after WP-PREF-1, WP-ARCH-6, WP-ARCH-7 and WP-ED-11 merge.
App::get(id)/App::set(id, value) over quill-settings; each_setting/apply_setting become wrappers;
layers (build default, QUILLDEF.INI, QUILL.INI, session) and delete-on-default; the merging write;
[Spelling] Language with the v0 migration; DOS Rows, Graphics and Scheme as settings, switches
session-only, Kept removed; Win32 ini_load/ini_save from the schema; Win16 reads the schema from an
RT_RCDATA resource (add it to the resource packer; near data at most +40 bytes). Fix B1, B3, B4, B6.
Generate the SPEC settings table with make-docs --check. m4-quill.ini changes once: review it.
Gates: G-HOST, G-DOS (DOS-82 rule if over), G-W16 (data figure), G-W32, G-GTK, G-MD. Evidence in
Editor features/WP-PREF-2. Decisions ARCH-32, CORE-153, CORE-154, DOS-81, W16-91; SPEC Settings.
PR: "Read and write every setting by id from one schema".
```

### WP-PREF-3

```text
Package WP-PREF-3 (Win16 only, replaces WP-ED-14): QUILL.INI through GetPrivateProfileString and
WritePrivateProfileString with a bare name, in the Windows directory (B2). Plan:
docs/plans/preferences.md WP-PREF-3. Branch feat/w16-profile-api after WP-PREF-2. Implement Profile
over the two calls with stack buffers; remove the whole-file write. Before the 3.11 run put a
[Recent32] section in C:\WINDOWS\QUILL.INI and prove it survives. Gates: G-W16 (data Δ ≤ 0, stack),
G-MD. Evidence: directory listing and the file as text. Decision W16-90; SPEC Settings file. PR:
"Keep the Windows 3.x settings in the Windows directory".
```

### WP-PREF-4

```text
Package WP-PREF-4: `quill settings` in quill-cli (get, set, reset, export, import, list; --build,
--file) over quill-settings alone. Plan: docs/plans/preferences.md "Import, export and reset" and
WP-PREF-4. Branch feat/cli-settings after WP-PREF-1. New crates/quill-cli/src/settings.rs; in
main.rs only the command branch and usage lines. Goldens in tests/golden/cli/settings-*. Tests:
export then import is a fixed point; unknown and invalid keys counted; an 8.3 --build never writes a
*32 key. Gates: G-HOST, G-MD. Evidence: a transcript of each verb in Editor features/WP-PREF-4.
Decision CORE-155. PR: "Read and change Quill's settings from the command line".
```

### WP-PREF-5

```text
Package WP-PREF-5: Tools > Options, the flow, the draft and the DOS text dialog. Plan:
docs/plans/preferences.md ("How a change applies", "DOS text", WP-PREF-5). Branch feat/options-dos
after WP-PREF-2 and WP-ED-9. Command::Options(page), Dialog::Options(page), the Draft, the six flow
entry points and host verbs; the Tools menu between View and Help (greyed on the Windows builds
until their dialogs land); the text-cell dialog drawn from the schema and qstx labels at 80x25,
80x50 and mono; Spelling's < Options >; CORE-158's autosave rules (probe a new folder;
RecoveryDirPrev read at launch until empty); Startup; RecentCount and Clear. The Tools menu shifts
every DOS text menu bar: regenerate those goldens in a separate commit and review them. Add
m18-options-autosave (set 1 minute in the dialog, type, pause, cut off, the next start offers the
copy). Measure the size; take savings first, then a DOS-83 raise (DOS-70). Gates: G-HOST, G-DOS,
G-DISK, G-MD. Evidence in Editor features/WP-PREF-5: every page on dos-text-25, dos-text-50, mono.
Decisions CORE-156 to CORE-159, DOS-80, DOS-83; SPEC menus, DOS text mode, Settings. PR: "Add Tools >
Options to the DOS text screens".
```

### WP-PREF-6

```text
Package WP-PREF-6: the Options dialog on the DOS graphics screen, and Pointer speed and Double-click
speed on both DOS screens. Plan: docs/plans/preferences.md ("DOS graphics", WP-PREF-6). Branch
feat/options-dos-gfx after WP-PREF-5. Draw Dialog::Options in quill-paint's overlay as the file
dialog's graphics form does (DOS-34, DOS-39 rules for the pointer). Pointer speed: INT 33h 1Bh to
read and keep the driver's sensitivity, 1Ah to set it, put back at exit; the PS/2 reader scales its
deltas; 0 leaves the driver alone. DoubleClick replaces DOUBLE_TICKS (ms to ticks, rounded). Gates:
G-HOST, G-DOS (DOS-85 rule if over), G-DISK, G-MD. Evidence: every page on dos-gfx-12h and
dos-gfx-vbe. Decisions DOS-84, DOS-85. PR: "Show Options on the DOS graphics screen, with pointer
settings".
```

### WP-PREF-7

```text
Package WP-PREF-7 (Win32): the Options property sheet. Plan: docs/plans/preferences.md ("Windows 95
and later", WP-PREF-7). Branch feat/options-w32 after WP-PREF-5. PropertySheetA with V1 sizes (40
bytes), five tabs from dialogs.json pages whose controls carry "setting": "<id>", PSM_CHANGED,
PSN_KILLACTIVE with the flow's messages, PSN_APPLY, Cancel keeps what Apply did; What's This through
WinHelpA; Browse with SHBrowseForFolderA, SHGetPathFromIDListA and SHGetMalloc. Add the four imports
to imports.allow. dialogs.json rule: append; on a conflict in generated.rs rerun emit-resources
--dialogs. Binding test. Gates: G-W32 (imports, Pentium scan, Wine: every tab, Apply reads back,
Cancel leaves the file byte-identical), G-HOST, G-DISK, G-MD. Evidence: every tab on win32-wine; Save
and What's This on win95. Decisions W9X-110 to W9X-112. PR: "Add the Options property sheet to Windows
95".
```

### WP-PREF-8

```text
Package WP-PREF-8 (Win16): the Options category dialog. Plan: docs/plans/preferences.md ("Windows
3.x", WP-PREF-8). Branch feat/options-w16 after WP-PREF-5, WP-PREF-3, WP-ARCH-9 and WP-ED-5. One
RT_DIALOG with every page, a Category list, a Description line, OK, Cancel, Apply, Help, F1; groups
shown with ShowWindow and EnableWindow; labels in the template, help lines in RT_STRING, the schema
from RT_RCDATA, the Draft on the stack; the dialog procedure in a new OPTIONS segment with its line in
segments.json. Report the data segment before and after (≥ 1,024 free after) and the stack margin
with the dialog open (≥ 2,048). Gates: G-W16 (3.11 play-back through every category), G-HOST, G-DISK,
G-MD. Evidence: every category on win16-311. Decision W16-92. PR: "Add the Options dialog to Windows
3.x".
```

### WP-PREF-9

```text
Package WP-PREF-9 (terminal): Tools > Options in quill-tty through the DOS text dialog; Screen page
shows Colours only and applies it at once; [RecentLinux] with a one-time read of [Recent] (B7). Plan:
docs/plans/preferences.md WP-PREF-9. Branch feat/options-tty after WP-PREF-5. Only
linux/quill-tty/src/session.rs and its tests. Gates: G-HOST (pty scenarios, frames equal to DOS),
G-MD. Evidence: the Save page captured from a pty. Decision LNX-61. PR: "Add Options to the Linux
terminal client".
```

### WP-PREF-10

```text
Package WP-PREF-10 (GTK, UI track): Preferences as an AdwPreferencesDialog from the schema, live,
writes after 500 ms of quiet and on close; Import, Export, Reset all on Advanced; Ctrl+comma, the
primary menu and Tools > Preferences. Plan: docs/plans/preferences.md ("GTK", WP-PREF-10) and
docs/ui-track.md's rules. Branch feat/gtk-preferences after WP-PREF-5. New src/preferences.rs; only
the action and menu lines elsewhere. Tests: a toggled row changes QUILL.INI within 1 s; search finds
Language; AT-SPI names equal labels. Gates: G-GTK, G-MD. Evidence: every page light and dark from the
UI track's compositor run. Decision LNX-60. PR: "Add the Preferences window to the GTK client".
```

### WP-PREF-11

```text
Package WP-PREF-11 (WinUI, UI track): a Settings page in the window with a NavigationView, a Find a
setting box and cards from the schema, live, Import, Export and Reset all, %APPDATA%\Quill\QUILL.INI.
Plan: docs/plans/preferences.md ("WinUI", WP-PREF-11) and docs/winui.md. Branch feat/winui-settings
after WP-PREF-5 and WP-WINUI-5. Tests: a UI Automation script finds every card by name and changes
one; light and dark at 100 % and 150 %. Evidence in Editor features/WP-PREF-11. Decision WUI-60. PR:
"Add the Settings page to the WinUI client".
```

### WP-PREF-12 and WP-PREF-13

```text
Package WP-PREF-12 (later): QUILLDEF.INI from Quill's installer. Plan: docs/plans/preferences.md
WP-PREF-12 and the preferences design's section 4. Needs Liminal Installer INI writes for retro
targets first: report if they are missing and stop. Decision INS-8. PR: "Preset Quill's settings from
its installer".

Package WP-PREF-13: Backup=1 keeps the replaced file as .BAK, made in the safe save's replace step.
Plan: docs/plans/preferences.md WP-PREF-13. Branch feat/backup-copy after WP-PREF-5. Safe-save
property tests with the backup on; a full disk leaves the original and no .BAK change. Gates: G-HOST,
G-DOS, G-W16, G-W32, G-MD. Decision CORE-160; SPEC Files. PR: "Keep a backup copy when saving, if
asked".
```

## Owner decisions

Each default below is taken so no builder waits; the last column is where to swap it.

| Question | Default taken | Where to swap |
|---|---|---|
| Change the autosave default now, and to what? | **Decided** (the owner, 2026-11-27): 5 minutes on DOS, Windows 3.x and Windows 95, 2 on the modern clients, in WP-PREF-0 now | — |
| Autosave as you work by default where? | Windows 95, the terminal, GTK and WinUI; Every 5 minutes on DOS and Windows 3.x, where As you work is offered but off | Each shell's `configure_autosave` call, CORE-161 and the SPEC rows; after WP-PREF-1, `settings.json` |
| How long a pause before saving as you work? | 3 seconds (1 to 30), and at least 10 seconds after the first change | `PAUSE_DEFAULT` and `WORK_FLOOR` in `recovery.rs`, CORE-161 |
| As you work saves the recovery copy, not the document? | Yes (CORE-161) | A fourth mode, "Save the document itself", would be a new decision with its own package |
| The window before or after the Unicode freeze? | After: WP-PREF-2 and WP-PREF-5 take WP-ED-13's and WP-ED-15's waves (9 and 10), keeping "Unicode first under a freeze" | Move WP-PREF-2 and WP-PREF-5 to waves 7 and 8 in [Order and lanes](#order-and-lanes) and work-packages.md; WP-ED-10 then also covers the dialog's text fields |
| DOS-80 to DOS-89 for this series? | Yes, carved out of the editor track's DOS-70 to DOS-89 | The id table in [Decisions](#decisions) and work-packages.md's id table (DOS-90 to DOS-99 are free instead) |
| The schema as one byte table on every build? | Yes (ARCH-31) | WP-PREF-1; the alternative is a Rust const table with `RT_RCDATA` on Windows 3.x only |
| Autosave Off keeps the copies already written? | Yes (CORE-158) | WP-PREF-5's flow |
| Export leaves out paths and recent files? | Yes (CORE-155) | `exported` in `settings.json` |
| Windows 95: `QUILL.INI`, not the registry? | `QUILL.INI` through the profile functions (W9X-111) | WP-PREF-7 |
| GTK: `QUILL.INI`, not GSettings? | `QUILL.INI` (LNX-60) | WP-PREF-10 |
| WinUI: `%APPDATA%\Quill\QUILL.INI`? | Yes (WUI-60) | WP-PREF-11 |
| Offer "open the last document at start"? | Yes, `Startup=blank` by default (CORE-159) | The `Startup` row in `settings.json` |
| Pointer speed on DOS: leave the driver's by default? | Yes, 0 is the driver's (DOS-84) | The `PointerSpeed` default |
| The least autosave interval the window offers? | 1 minute | The `Autosave` range in `settings.json` |
| Include the backup copy (WP-PREF-13)? | Yes, after WP-PREF-5 | Drop the package and the row |
| Names: Options, Preferences, Settings by build? | Options on the retro builds and the terminal, Preferences on GTK, Settings on WinUI (CORE-156) | WP-PREF-5, WP-PREF-10, WP-PREF-11 |
| If the DOS budget reaches the 480 KB stop? | Stop and ask (DOS-70); the first saving offered is the graphics dialog drawn as text cells over the screen, as the preferences design first proposed | WP-PREF-6 |
