# Documentation plan

How the finished Quill is documented as a published artifact, and what it hands to the blog series *Cross-compiling to the past* on the Liminal HQ website. The series is written separately; this plan lists what Quill's build has to produce for it.

## Who reads it

Blog posts will link here, so the project's own pages are written for a curious general reader first and a contributor second:

- The README and the artifact page explain what Quill is in plain words before any toolchain detail, and say what a term means the first time it appears (a *selector* is "a 16-bit handle that the CPU turns into a memory address").
- Every technical page opens with one paragraph a non-programmer can follow, then gets specific.
- Screenshots carry the story; tables carry the numbers; prose stays short.
- The tone is the series' tone: concrete, a little playful about how strange 1992 was, never sneering at it.

## The story

One sentence: *a word processor written in 2026 Rust for a 1992 PC, and the same program for DOS and Windows 95, from one core.*

The artifact page tells it in five beats:

1. **The finished thing.** Quill on Windows 3.11, a formatted letter on screen, the About box with its free system resources. Before anything is explained, show that it is real.
2. **Three front ends, one core.** The same letter on DOS text mode, Windows 3.1 and Windows 95, side by side, and the shared-core diagram.
3. **What 1992 costs.** The 64 KB data segment budget, far memory, code segments and veneers, told through the things they made hard (a picture that does not fit, a function call that has to change segments).
4. **What degrades where.** The build differences table, illustrated: bold as bright white in text mode, a photo dithered to 16 colours, a help file opened in the real WinHelp.
5. **Shipping it.** The floppy images, the installer, Program Manager's new group.

## Screenshots

Captured by the test harnesses (see [testing.md](testing.md)) at native resolution, PNG, no scaling, cropped to the program window or the desktop, then also written at 2× nearest-neighbour for high-density screens. Names are `docs/media/<milestone>-<scene>.png`. The same document, `tests/docs/letter.rtf` (a one-page letter with a heading, a bulleted list, a numbered list, a tabbed table of prices, a small picture, a header with the date and a footer with page numbers), is the subject wherever a comparison is made.

| # | Scene | Caption | From |
|---|---|---|---|
| 1 | Program Manager with Quill's group and icon | "Quill installed from its floppy, in Windows for Workgroups 3.11." | M11 |
| 2 | Quill on Windows 3.11 with `letter.rtf` | "The letter in Quill on Windows 3.11: TrueType fonts, a ruler with tab stops, and a toolbar drawn the way 1992 drew them." | M6 |
| 3 | The Character menu open | "Menus follow Windows 3.1 Write, with the keyboard shortcuts people still use." | M6 |
| 4 | `ChooseFont` with TrueType fonts | "Fonts come from Windows' own common dialog." | M6 |
| 5 | Ruler with a hanging indent and a decimal tab being dragged | "Indents and tab stops are set on the ruler, as in Word for Windows 2.0." | M6 |
| 6 | Print preview, two pages, header and footer visible | "Print preview uses the printer's own measurements, so what you see is what prints." | M6 |
| 7 | The printed page (rendered `.PRN`) beside the host reference render | "Left: printed through a Windows 3.1 printer driver. Right: Quill's reference renderer on Linux. Same line breaks." | M6 |
| 8 | A photo inserted, at 16 colours and at 256 | "One PNG, dithered for a 16-colour VGA and for a 256-colour palette." | M7 |
| 9 | The placeholder under a 4 MB configuration | "When a picture will not fit in memory, Quill says so and keeps the picture safe in the file." | M7 |
| 10 | The Spelling dialog with suggestions | "The spelling checker speaks Canadian English by default." | M8 |
| 11 | WinHelp showing Quill's contents, then a popup | "Quill's help file, compiled by a help compiler written for this project, in the real WinHelp of 1993." | M9 |
| 12 | DOS text mode with the letter, colour scheme | "The same letter in DOS text mode: bold is bright white, italic is cyan, the list numbers are real." | M2 |
| 13 | DOS text mode, Character menu pulled down | "Pull-down menus in 80 columns, in the spirit of WordPerfect 5.1 and Word for DOS." | M2 |
| 14 | DOS graphics mode with the letter | "DOS graphics mode, with Desk's fonts at 640 × 480 in 16 colours." | M10 |
| 15 | Quill on Windows ME (Win32 build) with the letter | "The native 32-bit build on Windows ME, with long file names." | M4 |
| 16 | The Win16 build running on Windows ME | "And the Windows 3.1 build, because Windows 95 still runs it." | M6 |
| 17 | The About box with free memory and system resources | "Free system resources: the number every 1992 user watched." | M6 |
| 18 | The three release disk images in a file manager, and the DOS installer | "One floppy per platform, each with its own installer." | M11 |
| 19 | The RTF from Windows 3.11 open in LibreOffice today | "And the letter opens in a word processor from this decade." | M2, M7 |

The three-way comparison (12 or 14, 2 and 15) is the capstone post's lead image: one strip, the same letter, three platforms.

## Diagrams

Drawn in mermaid in the docs, and exported as SVG for the artifact page and the blog:

| Diagram | Shows | Source |
|---|---|---|
| Shared core | The portable crates and the three platform layers | [architecture.md](architecture.md#the-shape-of-it), Figure 1 |
| Build pipeline | Rust to ELF to NE, PE and go32 | [architecture.md](architecture.md#build-pipeline), Figure 3 |
| DGROUP map | The 64 KB data segment as a bar, to scale, from the build's size report | Generated by the size check |
| Segment map | Each Win16 code segment's size against its budget, from the size report | Generated |
| A cross-segment call | The veneer and the shadow stack | [architecture.md](architecture.md#code-segments), Figure 2 |
| Where a picture lives | Original bytes, display DIB, print bands | New, from the pictures section |
| The test ladder | Host to emulators to VMs | [testing.md](testing.md), Figure 1 |

## The capstone post

Part 4 of the series, *Quill: one word processor for DOS, Windows 3.1 and Windows 95*, needs these from the build, all produced by M11:

- **The comparison strip:** the same letter on DOS text mode, DOS graphics mode, Windows 3.11 and Windows ME, captured at the same document position.
- **The degradation table:** the spec's build differences table, with a thumbnail per row where it helps (bold in text mode, a dithered picture, the help viewer).
- **Sizes and budgets:** each executable's size, each Win16 segment against its budget, the DGROUP map, and the release disks' fill, straight from the size reports.
- **Memory:** peak memory for the letter and for a 100-page document on each platform, from the trace.
- **The shared-core diagram** and a line count per crate, so the reader sees how thin each platform layer is.
- **Timings** at 386SX-class cycles: open, scroll to the end, spell check, print preview.

Earlier parts get their material from the milestones listed in the roadmap's [series table](roadmap.md#material-for-the-blog-series).

## README and hero

- The README's first line is the pitch in one sentence, then a line that links the series: "Quill is the companion application to the *Cross-compiling to the past* series on the Liminal HQ blog." The link goes in when the posts are published; until then it names the series without a link.
- The hero banner (brief in [hero-brief.md](hero-brief.md)) carries the same description and a small line naming the series.
- The status line stays honest about which platforms run what, milestone by milestone.

## The installer

The finished Quill installs with Liminal Installer. `installer/` is a Liminal Installer project, laid out as that project's architecture describes:

```text
installer/
	installer.toml      product identity (Quill, Liminal HQ Labs), version from the workspace, targets: win16, win32-classic, dos-x86
	components.toml     components and their files
	flows/
		install.json      welcome, licence, folder, components, language, ready, copy, done
		uninstall.json
	themes/
		quill-classic/    a Windows 3.1 grey theme with Quill's icon, and a blue text theme for DOS
	disks/
		dos.toml          QUILL-DOS.IMG: label, files, pack
		w31.toml          QUILL-W31.IMG
		w95.toml          QUILL-W95.IMG
```

| Component | Files | Required |
|---|---|---|
| Quill | `QUILL.EXE`, `QUILL32.EXE` or `QUILLDOS.EXE` for the target; `QUILL.HLP` or `QUILL.HLX`; `QUILL.FNT` on DOS | Yes |
| Spelling dictionaries | `EN_CA.DIC`, with `EN_US.DIC` and `EN_GB.DIC` chosen on the Language page | `EN_CA.DIC` yes, the others no |
| Sample documents | `SAMPLES\LETTER.QLD`, `SAMPLES\LETTER.RTF`, `SAMPLES\WELCOME.QLD` | No |
| Read me | `README.TXT` | Yes |

On Windows the installer creates a Program Manager group (or a Start menu folder on Windows 95), associates `.QLD` with Quill in the registration database and adds `QUILL.EXE /P` as the print command. On DOS it copies the files and offers to add the directory to `PATH`. Both write the installer's receipt so the uninstaller removes exactly what was installed.
