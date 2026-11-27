# The first demo: launch checklist

One checklist for the first public demo: Quill on the Afterglow ReactOS remix (QAGRR) at `https://labs.liminalhq.ca/quill/try-reactos/`, its saved v86 desktop, and the reactos-liminal v0.1.0 release that it stands on. The owner's wish, 2026-11-27: the first demo carries the PDF printer, sound and the shared clipboard, with downloads, file drop, time sync, the shut-down and turn-on states, the pointer fix, the Afterglow savers, the new icons and the Afterglow wallpaper naming, since "there may be no later demos". Written on 2026-11-27 against Quill `main` at `a004a70` and refreshed against `15218cb` (after #187 to #190 merged), reactos-liminal `main` at `ac2af05`, afterglow-savers `main` at `74147cf`, v86 0.5.473, the [web demo plan](web-demo.md), the [host integration plan](host-integration.md), the [pointer plan](pointer.md), afterglow-savers' `docs/packing.md`, reactos-liminal's `docs/release.md` and `docs/plans/icon-round-4.md`, the owner's decision log and the Quill board's Go-live frame. Where this plan and [decisions.md](../decisions.md) disagree later, the decisions win.

Nothing here publishes anything. Every act that publishes (a tag, a release, an upload, a Pages deployment that carries the data, a post) is the owner's, and each is named below where it falls.

## Table of contents

- [In one page](#in-one-page)
- [What is in the first demo, and what waits](#what-is-in-the-first-demo-and-what-waits)
- [The checklist](#the-checklist)
- [Critical path](#critical-path)
- [Order for the builder slots](#order-for-the-builder-slots)
- [Blocked on the owner](#blocked-on-the-owner)
- [The final build and its checks](#the-final-build-and-its-checks)
- [Test matrix on real browsers](#test-matrix-on-real-browsers)
- [The publishing pipeline](#the-publishing-pipeline)
- [The page at launch](#the-page-at-launch)
- [Licences and attribution](#licences-and-attribution)
- [Release v0.1.0: the procedure](#release-v010-the-procedure)
- [Release notes](#release-notes)
- [Announcement drafts](#announcement-drafts)
- [Smoke test after publishing](#smoke-test-after-publishing)
- [Rollback](#rollback)
- [Risks](#risks)
- [Decisions to record](#decisions-to-record)
- [Owner decisions](#owner-decisions)
- [Sonnet briefs](#sonnet-briefs)
- [What was not verified](#what-was-not-verified)

## In one page

- **Built and merged today:** the clean address (WP-WEB-1, #164), Print to PDF with a text layer (WP-WEB-3, #167), the sound spike that said go (WP-WEB-4, #171), the wallpaper named *Quill Afterglow* (WP-WEB-6, #175 and #180), 32 px icons in the remix (#179), the power state with **Turn on** (SITE-40, #183), the host link's specification and codecs (WP-HOST-2, #181), the pointer module (WP-PTR-1, #177), paste undo and the cut guard (WP-ED-2, #185), the zoom fixes checked on ReactOS (#156, #163, #178), the Windows 3.x shortcut through otvdmw with its start check (#136), icon round 3 in reactos-liminal (#36) and icon round 4's art in Afterglow (afterglow-themes #64).
- **Running now:** the host link spike (WP-HOST-1), sound as a TypeScript package (WP-WEB-5), the rich clipboard in the editor (WP-ED-5), the Cracktro's words and the savers' guest check (WP-SCROLL, then WP-PACK-2), the status bar's first package (WP-BAR-1, not on this list) and a pointer fix for the DOS Try Quill page. In review: icon round 4's first patch package (WP-ICON4-1, reactos-liminal #38), reactos-liminal's pinned savers build (WP-PACK-3, reactos-liminal #39) and the remix pinned at icon round 3 (#193, rebuild 3 built).
- **Not started, and needed:** about 15 packages across four repositories (the checklist lists every one). The longest chain is the shared clipboard with Quill's own rich copy and paste, then the savers, which cross three repositories and two image builds.
- **Then:** one final build (WP-WEB-8), the owner's look in real browsers, the reactos-liminal `v0.1.0` tag, a Quill release holding the demo's data, and one merge that points Pages at it. That merge is the moment the demo goes live.
- **Blocked on the owner today:** disk space (20 to 27 GB free on `/home`, at the builders' floor), the Afterglow icon round 4 look in the image, and eleven decisions below, each with a default already taken.

## What is in the first demo, and what waits

| Item | Owner's wish | First demo | Package and state |
|---|---|---|---|
| PDF printer (File > Print to PDF, text layer) | Yes | **In** | WP-WEB-3 merged (#167); the hand-off to the page's list is WP-HOST-5 (not started) |
| Downloads from the guest (*From the PC* list) | Yes | **In** | WP-HOST-5 (takes over WP-WEB-2), not started |
| Sound (AC'97 from the page, on at 70 %) | Yes | **In** | WP-WEB-4 go (#171); WP-WEB-5 the typed package, in progress |
| Shared clipboard: text both ways | Yes | **In** | WP-HOST-1 running; WP-HOST-3, WP-HOST-4 not started |
| Shared clipboard: rich text and pictures | Yes | **In** (plain text through the W9X-101 stopgap if WP-ED-6 is late) | WP-ED-5, WP-ED-6 (Win32 half), WP-HOST-7, not started |
| File drop or pick into the guest | Yes | **In** | WP-HOST-5, not started |
| Time sync to the visitor's clock | Yes | **In** | WP-HOST-6, not started |
| Fit to window | Not asked | **If ready** before WP-WEB-8; off by default | WP-HOST-6 |
| Shut-down and turn-on states | Yes | **In, done** | SITE-40, #183 merged |
| Pointer fix (Follow, Capture, touch trackpad) | Yes | **In** | WP-PTR-1 merged (#177); WP-PTR-4 (ReactOS page on the module) not started; WP-PTR-5 tuning may follow launch |
| The Try Quill DOS page on the pointer module | — | Waits (may land any time) | WP-PTR-2, WP-PTR-3 |
| Afterglow savers (Collection, Cracktro, Ink, Maze, Twilight) and Quill Ghostwriter, none selected | Yes | **In** | WP-PACK-1 merged (savers #3); WP-PACK-2 in progress, WP-PACK-3 in review (reactos-liminal #39), WP-PACK-4 to WP-PACK-6 not started |
| The Cracktro's scroller words off by default | Yes (owner, 2026-11-27) | **In**, before the savers commit S | WP-SCROLL, in progress |
| New icons, round 3 | Yes | **In** | reactos-liminal #36 merged; the remix pinned at round 3 in #193 (open; rebuild 3 built), and moved again in WP-PACK-6 |
| New icons, round 4 (Folder Options first) | Yes | **In**: WP-ICON4-1; WP-ICON4-2 if merged before the image freeze | WP-ICON4-1 in review (reactos-liminal #38); WP-ICON4-2 not started |
| Afterglow wallpaper naming (*Quill Afterglow* in the remix) | Yes | **In, done** | #180 merged |
| Zoom fixes, Win16 print preview, style box | — | **In, done** | #156, #163, #165, #170, #178 |
| Quill for Windows 3.x under WineVDM | — | **In** (slow, said on the page) | #136, `qagrr/check-win16.sh` |
| The page made public: wording, help, privacy, licences, home-page link | — | **In** | WP-WEB-7 and WP-LAUNCH-1, not started |
| A live smoke check after publishing | — | **In** | WP-LAUNCH-2, not started |
| reactos-liminal v0.1.0 (Afterglow full-image parts, no ISO) | — | **In**, tagged before the demo goes live | WP-LAUNCH-3, then the owner's tag |
| File lists on the clipboard, a tray icon for the helper, resending damaged packets | — | Waits | host integration "Later" |
| Ghostwriter in Quill's installers | — | Waits (not part of the web demo) | WP-PACK-7 |
| One shared savers DLL (`AGCORE1.DLL`) | — | Shelved (owner) | WP-SL-1, 3, 4 |
| The remastered ISO | — | Waits for Setup theming (owner, 2026-11-26) | reactos-liminal |
| Unicode beyond Windows-1252 in Quill | — | Waits (characters outside it paste as `?` into Quill; Notepad keeps them) | WP-ED-10, WP-ED-11 |
| Network in the guest | — | Never in this demo | — |
| The print set (#190), Reveal Codes, Preferences, WinUI, GTK | — | Not part of the demo | — |

## The checklist

Every package and pull request that must merge before the demo goes live, in build order within each lane. *Status* is as of 2026-11-27, refreshed in the evening, from `gh pr list`, `git log origin/main` and the worktrees.

### Lane A: the page and the emulator (Quill)

| # | Package | Plain description | Repo | Status today | Needs |
|---|---|---|---|---|---|
| A1 | WP-WEB-1 | Boot the saved desktop from a manifest at the plain address | Quill | **Merged** (#164) | — |
| A2 | SITE-40 fix | Follow the guest's power: *ReactOS has shut down*, **Turn on** | Quill | **Merged** (#183) | — |
| A3 | WP-WEB-4 | Spike: an AC'97 sound card added to v86 from the page | Quill | **Merged** (#171), verdict go | — |
| A4 | WP-WEB-5 | Sound: the AC'97 device as a typed TypeScript package, the Sound button and slider, the state's audio registers | Quill | **In progress** (a builder has it; the full brief below is what it was given) | A1, A3 |
| A5 | WP-PTR-1 | The shared pointer module, its lab page and its measure | Quill | **Merged** (#177) | — |
| A6 | WP-PTR-4 | Put the ReactOS page on the pointer module: Follow by default, Capture where raw counts exist, touch as a trackpad | Quill | **Not started, unowned** | A5 |
| A7 | WP-WEB-7 | Hosting: the pin file, the Pages steps that fetch v86 and the data, the licences page | Quill | **Not started, unowned** | A1 |
| A8 | WP-LAUNCH-1 | The page made public: wording, help, privacy and accessibility notes, the touch keyboard button, the home-page link, no `noindex` | Quill | **Not started, new in this plan** | A7 (shares the licences link) |
| A9 | WP-LAUNCH-2 | A live smoke check, run by hand against the published address | Quill | **Not started, new in this plan** | A7 |

### Lane B: the host link (Quill)

| # | Package | Plain description | Repo | Status today | Needs |
|---|---|---|---|---|---|
| B1 | WP-HOST-2 | The link's specification, test vectors, the Rust codec and the TypeScript package's core | Quill | **Merged** (#181) | — |
| B2 | WP-HOST-1 | Spike: the clipboard viewer chain, COM1 rates, `SetLocalTime` and display modes in the real guest, and the browsers' paste and write rules | Quill | **In progress** (worktree `host-1`, branch `spike/host-link`) | — |
| B3 | WP-HOST-3 | The guest helper `HOSTLINK.EXE` on the disk, started at logon, owning COM1 | Quill | **Not started** (brief in host-integration.md) | B1, B2 |
| B4 | WP-HOST-4 | The page's clipboard: the flows, the converters, the chip, the privacy rules as tests | Quill | **Not started** (can start now against a fake helper) | B1; its browser runs need B3 |
| B5 | WP-HOST-5 | Files both ways and Print to PDF to the list (`HostPort=LINK`, W9X-100) | Quill | **Not started** | B3 |
| B6 | WP-HOST-6 | Time sync, and Fit to window if ready | Quill | **Not started** | B3 |
| B7 | WP-ED-2 | One Undo takes back a paste; the cut guard | Quill | **Merged** (#185) | — |
| B8 | WP-ED-5 | The rich clipboard in the editor: the clipboard request, the 2,048-byte buffer gone | Quill | **In progress** (brief in work-packages.md) | B7 |
| B9 | WP-ED-6 (Win32 half first) | The rich clipboard on Windows 95: RTF, `CF_TEXT`, `CF_DIB` | Quill | **Not started** | B8 |
| B10 | WP-HOST-7 | Quill's rich copy and paste carried to the host, and the owner's clipboard check | Quill | **Not started** | B4, B9 (or the W9X-101 stopgap) |

### Lane C: the savers and the icons (afterglow-savers, reactos-liminal, Quill)

| # | Package | Plain description | Repo | Status today | Needs |
|---|---|---|---|---|---|
| C1 | WP-PACK-1 | The savers' image build contract and CI | afterglow-savers | **Merged** (#3) | — |
| C2 | WP-SCROLL | The Cracktro's scroller words off by default, with a setting | afterglow-savers | **In progress** (planned in `docs/linux-packages.md`) | — |
| C3 | WP-PACK-2 | The savers checked in a ReactOS guest, settings dialog and sound included; its merge is the savers commit **S** | afterglow-savers | **In progress**, after WP-SCROLL in the same slot (brief in `docs/packing.md`) | C1, C2 (so S has the words off) |
| C4 | WP-PACK-3 | reactos-liminal builds a pinned outside Rust project with several outputs | reactos-liminal | **In review** (reactos-liminal #39) | — |
| C5 | WP-PACK-4 and WP-PACK-5 | The savers wired into the full and compact recipes at S, the images booted, the sizes | reactos-liminal | **Not started** (one pull request) | C3, C4 |
| C6 | Icon round 3 | Round 3's art, the Go button and the NT namespace icons in the image | reactos-liminal | **Merged** (#36) | — |
| C7 | WP-ICON4-1 | Icon round 4: Folder Options first, then everything today's patchers write | reactos-liminal | **In review** (reactos-liminal #38) | afterglow-themes #64 (merged) |
| C8 | WP-ICON4-2 | Icon round 4: patcher changes for 22 and 256 px frames, 24 px groups, the newer strip header | reactos-liminal | **Not started** | C7 |
| C9 | WP-PACK-6 | The remix pins reactos-liminal after C5 and C7, adds Ghostwriter, rebuilds and checks its parts and state | Quill | **Not started** (brief in packing.md); the round 3 pin and rebuild 3 are #193, open | C5, C7 (C8 if the owner waits for it) |

### Lane D: the release and going live

| # | Package | Plain description | Repo | Status today | Needs |
|---|---|---|---|---|---|
| D1 | WP-LAUNCH-3 | Get reactos-liminal ready for v0.1.0: locks, release notes for what is now in the image, the WineVDM release named | reactos-liminal | **Not started, new in this plan** | C5, C7 (C8) |
| D2 | Owner | Dispatch `build.yml` on `main` once as a dress rehearsal and read its job summary | reactos-liminal | Waiting | D1 |
| D3 | Owner | **Tag `v0.1.0`**: the release workflow builds and publishes by itself | reactos-liminal | Waiting | D2, the licence checklist |
| D4 | WP-WEB-8 | The demo build: one image and state with everything, checked end to end on a plain static server, packed and pinned, unpublished | Quill | **Not started** | A4, A6, A7, A8, B3 to B6, B10, C9 |
| D5 | Owner | The look: the final build in real browsers on a local server (the test matrix) | — | Waiting | D4 |
| D6 | Owner | **Create the Quill release** `reactos-demo-v0.1.0` with the tarball, its sums and the source pointer | Quill | Waiting | D3, D5 |
| D7 | Owner | **Merge the pin pull request**: Pages deploys with the data, and the demo is live | Quill | Waiting | D6 |
| D8 | Agent, then owner | Run the live smoke check; the owner posts the courtesy note and the announcements | — | Waiting | D7 |

Merged since the first draft and **not** on this list: Quill #187 (WinUI real window plan), #188 (Unicode plan), #189 (the module interface, WP-MOD-1), #190 (the print set). Open or running and not on this list: #191 (the packed-size gate, WP-MOD-8), the status bar's first package (WP-BAR-1) and the DOS Try Quill page's pointer fix. None blocks the demo. The Unicode plan's freeze (WP-ED-10 and WP-ED-11) must not start in `quill-app` before WP-ED-5 has merged, or the clipboard lane waits behind it.

## Critical path

```text
WP-ED-5 ──► WP-ED-6 (Win32 half) ─────────────────────────┐
WP-HOST-1 (running) ──► WP-HOST-3 ──► WP-HOST-5, WP-HOST-6 ─┤
WP-HOST-4 (starts now) ───────────────────────────────────┼─► WP-HOST-7 ─┐
                                                           │              │
WP-SCROLL ──► WP-PACK-2 = S ─┐                             │              │
WP-PACK-3 ───────────────────┴─► WP-PACK-4/5 ─┐            │              │
WP-ICON4-1 (#38) ──► (WP-ICON4-2) ────────────┴─► WP-PACK-6 ──────────────┼─► WP-WEB-8 ─► owner's look ─► v0.1.0 tag ─► Quill release ─► pin merge = live
                                              └─► WP-LAUNCH-3 ─► build.yml rehearsal ─► v0.1.0 tag     │
WP-WEB-5 ─────────────────────────────────────────────────────────────────┤
WP-PTR-4 ─────────────────────────────────────────────────────────────────┤
WP-WEB-7 ──► WP-LAUNCH-1, WP-LAUNCH-2 ────────────────────────────────────┘
```

**The critical path is the clipboard with Quill's rich copy:** WP-ED-5 → WP-ED-6 (Win32 half) → WP-HOST-7 → WP-WEB-8. WP-ED-5 is a core change across every shell (the request, the store-backed transport, the Win16 data segment), and WP-ED-6 follows it; together they are longer than the helper chain (WP-HOST-1 → WP-HOST-3 → WP-HOST-5) that runs beside them. Two things shorten it: start WP-ED-5 at the next free slot, and take the W9X-101 plain-text stopgap if WP-ED-5 has not merged when WP-HOST-4 does (the host integration plan's rule, kept).

**The second path is the savers:** WP-SCROLL → WP-PACK-2 → WP-PACK-4/5 → WP-PACK-6, across three repositories with two full image builds (WP-PACK-5 builds both Afterglow images, WP-PACK-6 the remix). It has slack only if WP-PACK-3 starts now.

**The image freeze.** WP-PACK-6 sets the reactos-liminal commit the demo is built on. Anything that changes the guest's disk after it (a later icon round, a trim change) means another remix build and state; WP-WEB-8 then pins that commit and nothing else moves. The same reactos-liminal commit is the one tagged `v0.1.0`, so the release and the demo describe one system.

## Order for the builder slots

Three builders and one floater at once, Opus plans and Sonnet builds. Running now: WP-HOST-1, WP-WEB-5, WP-ED-5, WP-SCROLL then WP-PACK-2, WP-BAR-1 and the DOS page's pointer fix; in review: WP-ICON4-1 (reactos-liminal #38), WP-PACK-3 (reactos-liminal #39) and the round 3 remix (#193). As slots free, in this order (items already running are skipped):

1. **WP-ED-5** (critical path).
2. **WP-WEB-5** (sound; touches only its own package and blocks of the page).
3. **WP-PACK-3** (reactos-liminal, Python only, no image build).
4. **WP-SCROLL**, then **WP-PACK-2** in the same slot (afterglow-savers).
5. **WP-HOST-4** (against a fake helper until WP-HOST-3 lands).
6. **WP-HOST-3** as soon as WP-HOST-1's findings are in.
7. **WP-WEB-7**, then **WP-LAUNCH-1** and **WP-LAUNCH-2** in the same slot.
8. **WP-PTR-4**.
9. **WP-ED-6** (Win32 half first), **WP-HOST-5** and **WP-HOST-6** as their dependencies merge.
10. **WP-PACK-4/5**, **WP-ICON4-2**, **WP-LAUNCH-3**, **WP-PACK-6**, **WP-HOST-7**.
11. **WP-WEB-8** alone, last.

Image builds (WP-HOST-3, WP-PACK-5, WP-PACK-6, WP-WEB-8) are one at a time: each needs 4 GB of sparse image and its build files, and `/home` is at the 20 GB floor.

## Blocked on the owner

**Today.**

- **1. Disk space.** `df -h /home` showed 20 to 27 GB free on 2026-11-27, just above the 20 GB floor every builder stops at, with four image builds still to come. Candidates the owner can remove (each a 4 GiB sparse image): `~/.cache/quill-reactos-liminal-work/images/qagrr.before2.img`, `~/.cache/quill-reactos-liminal-work/images/qagrr.img.prev`, the older of `~/.cache/quill-reactos-liminal-work/golden/qagrr-ad090a87cc03ff4e.img` and `qagrr-d622f74f529937e2.img`, and `~/.cache/quill-reactos-liminal-work/dist.prev`. No agent removes them.
- **2. The eleven decisions** in [Owner decisions](#owner-decisions); each has a default so no builder waits.

**Looks, as they come.**

- **3.** Icon round 4 in the booted image (WP-ICON4-1's before and after sheets, Folder Options first).
- **4.** The savers in ReactOS (WP-PACK-2's and WP-PACK-5's shots and the sound levels; the owner said they would judge the Win32 savers' speed there).
- **5.** Sound by ear (WP-WEB-5, on a real speaker).
- **6.** The pointer and clipboard check lists ([pointer.md](pointer.md#the-owners-check-list), [host-integration.md](host-integration.md#owners-check-list)) on a local server, before launch.
- **7.** The final build (WP-WEB-8) through the [test matrix](#test-matrix-on-real-browsers).

**Acts only the owner does.**

- **8.** Dispatch reactos-liminal's `build.yml` on `main` as a rehearsal (the board's item 9).
- **9.** Complete the licence checklist (reactos-liminal `SPEC.md`, items that apply to disk parts as well as the ISO: 1, 2, 3, 6, 7) and the release notes check.
- **10. Tag reactos-liminal `v0.1.0`.** The tag is the publication: there is no second approval (RL-61).
- **11. Create the Quill release** with the demo tarball (publishing ReactOS and WineVDM binaries a second time, inside the remix).
- **12. Merge the pin pull request**, which deploys the demo.
- **13.** Send the courtesy note to the ReactOS project, then the announcements.

Already done by the owner: HTTPS on `labs.liminalhq.ca` (certificate approved for the domain to 2027-02-25, and *Enforce HTTPS* is on: read back from the Pages API on 2026-11-27).

## The final build and its checks

WP-WEB-8, as [web-demo.md](web-demo.md#wp-web-8-the-demo-build-everything-together) and its [host link additions](host-integration.md#wp-web-8-additions) define it, with these launch additions:

1. **Inputs, pinned.** reactos-liminal at the commit that will be tagged `v0.1.0` (WP-PACK-6's pin); Quill's programs from one Quill `main` commit; the savers at S; v86 0.5.473.
2. **Build.** `qagrr/build-with-rl.sh` with fresh programs, `qagrr/check-win16.sh` on the image, `qagrr/build-state.sh` with the AC'97 device registered and the helper running, `qagrr/check-state.sh`, `tools/stage-web.py`, `qagrr/pack-web.sh` (WP-WEB-7).
3. **Checks, headless, on a plain static server** (`python3 -m http.server` under a `quill/` path, not `serve.ts`): `check-clean.ts`, `check-power.ts`, `check-sound.ts`, `check-host-files.ts`, `check-clipboard.ts` and `check-clipboard-ff.ts`, `check-time-display.ts`, `check-quill-clipboard.ts`, `mouse-axis.ts --check` in Chromium and Firefox, and the new `check-demo.ts` that runs the whole story once: no query string, the desktop, Print to PDF in Quill, the PDF in the list and downloaded with an equal SHA-256, a Notepad clipboard round trip, a file dropped in, the clock, a saver's sound counted by the device, the Windows 3.x shortcut opening Quill, Shut Down and **Turn on**.
4. **Throttled once each** at 50 Mbit/s and 10 Mbit/s; time to the desktop and bytes on the first start recorded, which set the page's "about N MB" sentence (WP-LAUNCH-1 takes the number).
5. **Requests.** The run asserts that no request leaves the page's own origin.
6. **Hand-off, unpublished.** `qagrr-web-<hash>.tar`, its line for `web-data.pin` (URL left as the planned release address, marked unpublished), `SHA256SUMS`, the licences page rendered, and the evidence in `~/Documents/Liminal HQ/Quill/Web demo/WP-WEB-8/`. The owner then serves the same tarball locally for the [test matrix](#test-matrix-on-real-browsers): `bash scripts/fetch-web-data.sh --local <tarball>` and the static server, so the owner tests exactly the bytes that will be published.

## Test matrix on real browsers

Run on the WP-WEB-8 tarball served locally before publishing, and the first four rows again on the live address after. *Who* is who can run it: agents have only headless browsers on this machine.

| Browser and device | Who | Secure context | What it proves |
|---|---|---|---|
| Firefox on Linux, Wayland (the owner's desktop) | Owner | `http://127.0.0.1` counts as secure | The owner's main browser; Follow pointer; the 5 s copy rule; Firefox's own Paste button |
| Chromium on Linux | Owner | yes | Capture refused on Linux (Follow); the promised clipboard write; `AudioWorklet` |
| Chromium and Firefox headless | Agents | yes | Every `check-*.ts` above; repeatable numbers |
| Playwright WebKit on Linux | Agents | yes | A stand-in for Safari's engine (not Safari): `AudioContext` start in the click, the promised `ClipboardItem`, no `userActivation` |
| Chrome or Firefox on Windows or macOS | Owner, if a machine is at hand (the Windows dev VM has Edge) | yes | Raw Capture on the first press; Cmd keys on a Mac |
| Safari on macOS | Owner, if reachable | yes | Autoplay inside Start, the clipboard through the button, WebAssembly speed |
| Safari on iPhone or iPad | Owner, if reachable | live address only (HTTPS) | Memory (a 256 MB guest and a 119 MB state in one tab), the touch trackpad, **Paste from your computer**, the touch keyboard button |
| Chrome on Android | Owner, if reachable | live address only | The same as iOS, and speed on a phone CPU |

**What to try in each** (yes or no; a no gets a note and the diagnostics panel's rows into `~/Documents/Liminal HQ/Quill/Web demo/launch/owner/`):

1. The page opens with no query string; Start brings the desktop with Quill open on the letter; the status line's download size is about right.
2. Type in Quill; menus; zoom minus and plus; File > Print to PDF; the PDF appears under the screen and opens after **Download**.
3. A system sound or a saver's music plays; **Sound** mutes; the slider sets the level; the choice survives a reload.
4. Copy in Notepad, paste on the desktop; copy on the desktop, paste in Notepad (no stray "v"); the chip reads *Shared*, then *Off* when switched off.
5. Quill's bold and red text pastes formatted into LibreOffice; LibreOffice's paragraph pastes formatted into Quill; Alt+Print Screen pastes as a picture.
6. Drag a file onto the screen: it appears in *From your computer*.
7. The taskbar clock shows the visitor's time.
8. A diagonal drag on the desktop draws a diagonal rubber band; the pointer stays under the visitor's.
9. Display Properties > Screen Saver lists the five Afterglow savers and Quill Ghostwriter with none selected; each previews; Preview plays its score.
10. Display Properties > Desktop shows *Quill Afterglow* selected; Folder Options shows the round 4 pictures.
11. The *Quill (Windows 3.x, slow)* icon opens Quill for Windows 3.x.
12. Shut Down dims the screen; **Turn on** brings the desktop back; Reset and Stop behave.
13. Full screen and back; Esc lets the mouse go.
14. Phone: the touch trackpad moves and clicks, a two-finger tap right-clicks, the keyboard button types, **Paste from your computer** and **Copy to your computer** work, a picked file arrives.
15. Keyboard only: Tab reaches Start, the chip, Sound, the slider and the diagnostics; a screen reader names each and reads the status line.

## The publishing pipeline

```text
WP-WEB-8 (local) ──► qagrr-web-<hash>.tar + SHA256SUMS + SOURCE.txt + licences.md
                         │  owner: gh release create reactos-demo-v0.1.0 (Quill)        ← publishing
                         ▼
site/try-reactos/web-data.pin (URL, SHA-256, size)  ── pin pull request ── owner merges ← going live
                         │
                         ▼  .github/workflows/pages.yml on main
         tools/fetch-v86.sh (v86 0.5.473, pinned sums)
         scripts/fetch-web-data.sh: download the tarball, check SHA-256 and size, unpack into
           site/try-reactos/data/parts-<hash>/, data/state-<hash>.l22.bin.zst, web-manifest.json
         a check that every file the manifest names exists with its sum; check-site.py --external
         upload-pages-artifact (site/) ──► deploy-pages ──► https://labs.liminalhq.ca/quill/try-reactos/
```

- **Where the data lives.** One tarball as an asset of a Quill release (`liminal-hq-labs/quill`, tag `reactos-demo-v0.1.0`, SITE-42), never fetched by the page: release downloads redirect without `Access-Control-Allow-Origin` (measured). The Pages workflow pulls it by the pinned SHA-256 on every deployment, so the site always carries exactly the pinned bytes. `site/try-reactos/data/` stays out of git.
- **Why a Quill release and not reactos-liminal's.** The remix holds Quill's programs and the remix recipe, which live in Quill; reactos-liminal's `v0.1.0` carries the Afterglow images' own parts. The Quill release links reactos-liminal `v0.1.0`'s corresponding-source archive for the ReactOS part (D3 comes before D6 for that reason).
- **Every Quill push to `main` that touches `site/**` redeploys** and re-downloads the tarball (about 66 MB from GitHub to GitHub). With the placeholder pin the data step is skipped and the page says the demo is not available (WP-LAUNCH-1's wording), which is today's state.
- **The manifest.** `web-manifest.json` (fetched with `cache: 'no-store'` and `?t=`) names content-hashed parts and state, the v86 file hashes, the audio registers and the Quill and reactos-liminal commits (SITE-14). A page and a state from different builds cannot meet.
- **Cache headers.** Pages sends `Cache-Control: max-age=600` for every file and cannot be told otherwise (read on the live page, 2026-11-27). Content-hashed names make that harmless for data; `?v=` stamps cover the page's own files and v86; the only file that can be ten minutes stale is `index.html`, and an old page reads a new manifest correctly while the manifest's `version` stays 1.
- **Range requests and COOP/COEP.** v86 fetches whole 8 MiB parts, so ranges are not needed (Pages answers `206` anyway). v86 uses no `SharedArrayBuffer`, so no cross-origin isolation is needed, and Pages cannot set COOP or COEP.
- **Sizes and limits.** Parts 51.5 MB (512 files, 466 empty), state 14.2 MB, v86 4.4 MB: about 70 MB on the site and 520 files, inside Pages' 1 GB site and 100 MB a file. WP-WEB-8 records the new numbers (the savers add about 0.45 MB to the parts; the helper and the device add under 1 MB to the state). One measured session served 25.9 MB in 19 requests; Pages' soft bandwidth is about 100 GB a month, so about 3,800 full sessions.
- **Analytics: none.** No script, pixel or request outside the page's origin; the smoke test asserts it. GitHub serves the site and keeps its own logs under GitHub's privacy statement; the page says so in one line.

## The page at launch

WP-LAUNCH-1 owns these changes to `site/try-reactos/index.html`, its CSS and `site/index.html`, and WP-WEB-7 the licences page.

- **Title and purpose.** *Quill on ReactOS* stays the title. The lede says what it is in two sentences, then one line: "An unofficial remaster by Liminal HQ Labs, built on ReactOS™ 0.4.16. Not made, endorsed or supported by the ReactOS Project." (the ™ on the first use, as the ReactOS IP guideline asks).
- **Indexing and links.** `noindex` comes off in the pin pull request, not before (until then the page has no data, and a search engine should not list a page whose Start fails). The Quill home page gains a *Try it on ReactOS* link beside the Try Quill (DOS) link in the same pull request.
- **The first start.** The status line's "about 16 MB" becomes the number WP-WEB-8 measured, and a second line says the PC runs in this tab and needs a computer with a few hundred MB of memory free; on a phone it may be slow.
- **Help, under the screen** (the existing hint list, rewritten): select the screen to give it the keyboard and mouse; Esc lets the mouse go; *Mouse: Follow or Capture* and what each does; on a phone, drag to move, tap to click, two fingers to right-click, and the keyboard button; Copy and paste: what the chip means; Print to PDF and *From the PC*; drop a file to send it in; Sound; Shut Down and **Turn on**; the Windows 3.x icon is slow; Reset loads the saved desktop again and nothing typed is kept.
- **Privacy, one short section.** Nothing you type, copy or drop leaves this tab: the PC runs here. The page reads your clipboard only when you paste into the PC and writes it only after you copy in the PC or press **Copy to your computer**; you can switch sharing off. Files move only when you drop or pick them, or press **Download**. There is no analytics and no request to any other site; GitHub, which serves the page, keeps its own logs. The one setting stored is in your browser (sharing, sound, mouse mode).
- **Accessibility, said plainly.** The controls are buttons and fields with names, the status line is a polite live region, and everything works from the keyboard; the PC's screen is a picture, so a screen reader cannot read what is in it. Contrast and focus rings follow the site's CSS; motion is only the guest's own.
- **Touch keyboard button** (SITE-46, default yes): on a touch device a **Keyboard** button focuses a hidden text box and sends what is typed with `keyboard_send_text`, so a phone can type into Quill. If it cannot be made reliable in a day it is left out and the help says a hardware keyboard is needed.
- **Licences link** in the page footer: *Licences and source* to the page WP-WEB-7 writes.
- **Demo paused wording.** With the placeholder pin (or a manifest that fails its checks), the page says "The ReactOS demo is not available right now. The DOS demo is at Try Quill." instead of the builder's "No demo data on this server"; the builder's wording stays in the diagnostics panel.

## Licences and attribution

`site/try-reactos/licences.html` (WP-WEB-7; `site/try/licences.html` is the DOS page's model), linked from the page and from the Quill release. It names, with versions and commits taken from `web-manifest.json` and `build-manifest.json` by the pack script so they cannot drift:

| Component | Licence | Source named |
|---|---|---|
| ReactOS 0.4.16 (kernel, drivers, most of user mode) | GPL-2.0-or-later, with the runtime exception in its `COPYING` | Tag `0.4.16-release`, commit `822e8647792a2c43f033e3e00ed273d59f163c7b`; reactos-liminal `v0.1.0`'s source archive |
| Wine-derived ReactOS DLLs | LGPL-2.1-or-later | as above |
| Third-party code in ReactOS (`media/doc/3rd Party Files.txt`) | their own licences | as above |
| Fonts on the disk | per ReactOS's `media/fonts/doc/` (OFL-1.1, GPL with font exception and others) | as above |
| WineVDM (otvdm) | GPL-2.0 | The exact release ReactOS 0.4.16 installs (named in WP-LAUNCH-3) and its source tag |
| v86 0.5.473 (with zstd and SoftFloat inside its WebAssembly) | BSD-2-Clause (zstd BSD-3-Clause, SoftFloat BSD-3-Clause) | npm `v86@0.5.473`, repository commit `315b2b0` |
| SeaBIOS and SeaVGABIOS 1.16.2 | LGPL-3.0 | `rel-1.16.2`, built by v86's `bios/fetch-and-build-seabios.sh` |
| Afterglow desktop theme | Apache-2.0 OR MIT | `liminal-hq/afterglow-themes` at the pinned commit |
| Afterglow savers and Quill Ghostwriter | Apache-2.0 OR MIT | `liminal-hq-labs/afterglow-savers` at S |
| afterfetch | Apache-2.0 OR MIT | `liminal-hq-labs/afterfetch` at reactos-liminal's pin |
| reactos-liminal (recipe, patchers, settings) | Apache-2.0 OR MIT | the `v0.1.0` tag |
| Quill and the page (including the AC'97 device, written from Intel's documents and not from QEMU) | Apache-2.0 OR MIT | the Quill commit in the manifest |

Also on the page: the **written offer** (ask in Quill's issues; valid three years from receipt and while the demo is up), **what was changed** (reactos-liminal's `CHANGES.txt` on the disk, the QAGRR recipe and its trim list, Quill in `C:\Quill`), and **trademarks** (ReactOS is a trademark of the ReactOS Project; Microsoft and Windows are trademarks of Microsoft; this is unofficial). Not on the disk, so not listed: Wine Gecko (the compact trim removes it with its licence, RL-40) and ReactOS's own wallpapers (removed by the trim, so no CC-BY-NC art ships; to confirm in WP-WEB-8 from the image's file list). The licence texts (GPL-2.0, LGPL-2.1, LGPL-3.0, BSD-2-Clause, Apache-2.0, MIT) are linked from the page and present on the disk in `LICENSES` and beside v86 in `site/try-reactos/v86/`.

## Release v0.1.0: the procedure

Two releases, in this order, on the owner's go. Each step names who does it.

**Before** (agents, in pull requests): every package on the checklist merged; WP-LAUNCH-3 merged in reactos-liminal (locks current for both recipes, `docs/releases/0.1.0.md` updated, the WineVDM release named in `SOURCE.txt` and `NOTICE.txt`); WP-WEB-8's tarball, pin line and licences ready; the owner has run the test matrix on the local server.

**reactos-liminal `v0.1.0`** ([release.md](https://github.com/liminal-hq-labs/reactos-liminal/blob/main/docs/release.md)):

- **Step 1.** Owner: dispatch **Actions → Build** on `main` once and read the job summary (sizes, hashes, pinned inputs); confirm the image's SHA-256 equals the one WP-WEB-8's remix was built over (same reactos-liminal commit).
- **Step 2.** Owner: complete the licence checklist items that apply to disk parts (1, 2, 3, 6, 7; 4's courtesy note can follow the tag, but before the announcements).
- **Step 3.** Owner: `git tag -s v0.1.0 -m "Afterglow for ReactOS v0.1.0"` on `main` and `git push origin v0.1.0` (or dispatch **Release** with `v0.1.0`). The run checks, builds with KVM, archives the source and publishes by itself; there is no approval step (RL-61). To stop it, cancel before *Publish the release* starts.
- **Step 4.** Owner: open the release, check every asset is there and `sha256sum -c SHA256SUMS` on a download.

**Quill `reactos-demo-v0.1.0`** (SITE-42):

- **Step 5.** Owner: `gh release create reactos-demo-v0.1.0 --repo liminal-hq-labs/quill --title "Quill on ReactOS, demo 0.1.0" --notes-file <WP-WEB-8's notes> qagrr-web-<hash>.tar SHA256SUMS SOURCE.txt licences.md`, on the Quill commit WP-WEB-8 built from.
- **Step 6.** Agent: open the pin pull request (`web-data.pin` with the release URL, SHA-256 and size; `noindex` off; the home-page link), CI green. The pull request runs `fetch-web-data.sh` against the real release URL in its own check and boots from it headless.
- **Step 7.** Owner: **merge the pin pull request.** Pages deploys (about five minutes with the DOS bundle build); the demo is live.
- **Step 8.** Agent: the [smoke test](#smoke-test-after-publishing); its result goes to the owner before any post.
- **Step 9.** Owner: the courtesy note to the ReactOS project, then the announcements.
- **Step 10.** Agent: update `docs/progress.md`, the board's Go-live frame and the CD media version (afterglow-themes `VERSION`, owner decision 2 in the log: "stays until the tag, then update").

## Release notes

reactos-liminal's `docs/releases/0.1.0.md` exists; WP-LAUNCH-3 adds the savers (five, none selected; ReactOS's own savers kept in the full image), icon rounds 3 and 4, the card faces, the system colour table and the shortcut audit, and keeps its *Unofficial* and *Known limits* paragraphs.

The Quill release body (draft, WP-WEB-8 fills the numbers):

```markdown
**Quill on ReactOS, demo 0.1.0** is the data behind https://labs.liminalhq.ca/quill/try-reactos/:
the disk parts and saved desktop of the Quill remix of Afterglow for ReactOS, which the site's
Pages build unpacks and serves. You do not need to download anything to try it.

**Unofficial.** Built on ReactOS™ 0.4.16 with Afterglow for ReactOS v0.1.0. Not made, endorsed or
supported by the ReactOS Project. Report problems in Quill's issues, not to ReactOS.

## What is in it

- Quill for Windows 95 (32-bit) open on a welcome letter, and Quill for Windows 3.x under WineVDM.
- File > Print to PDF, with the PDF offered as a download on the page.
- Sound through an emulated AC'97 card, the Afterglow screensavers and Quill Ghostwriter.
- A shared clipboard (text, rich text and pictures), files both ways and the visitor's clock,
  through a small helper in the guest that talks to the page over COM1.
- The Afterglow theme, its icons and the Quill Afterglow wallpaper.

## Files

- `qagrr-web-<hash>.tar`: the parts, the saved state and `web-manifest.json` (N MB).
- `SHA256SUMS`, `licences.md`, `SOURCE.txt` (the ReactOS source by tag and commit, WineVDM,
  SeaBIOS and the written offer).

Built from Quill `<commit>` and reactos-liminal `v0.1.0` (`<commit>`), v86 0.5.473.
```

## Announcement drafts

Posted by the owner only, after the smoke test passes. The courtesy note goes first and alone; the others wait a day for an answer unless the owner decides otherwise.

**ReactOS forum** (the *Off-topic* or *Showcase* board; also the basis for an email to the project's contact if there is no answer):

```text
Subject: An unofficial themed ReactOS 0.4.16 build, and a word processor demo in the browser

Hello all,

We have made an unofficial remaster of ReactOS 0.4.16 called Afterglow for ReactOS: the official
install CD, installed unattended and dressed with our own Afterglow theme (visual style, icons,
pointers, wallpaper and screensavers). It is built from a recipe, every input is pinned, and the
release carries the licences, the list of changed files and the source pointer:
https://github.com/liminal-hq-labs/reactos-liminal/releases/tag/v0.1.0

We also use it for a browser demo of Quill, a small word processor written in Rust that runs on
DOS, Windows 3.x and Windows 95, here on ReactOS inside v86:
https://labs.liminalhq.ca/quill/try-reactos/

Every page and release says it is unofficial and not made or endorsed by the ReactOS Project, and
asks people to report problems to us, not to you. If you would like the name, the wording or
anything else done differently, or if any of our patches would be useful upstream, please tell us
and we will change it.

Thank you for ReactOS. It ran our Win32 and Win16 builds better than we expected.

Scott Morris, Liminal HQ Labs
```

**ReactOS Discord** (the general or showcase channel; short):

```text
Hi! We made an unofficial themed build of ReactOS 0.4.16 (Afterglow for ReactOS, not endorsed by
the project) and use it to demo Quill, a little Rust word processor, in the browser with v86:
https://labs.liminalhq.ca/quill/try-reactos/ (release and source:
https://github.com/liminal-hq-labs/reactos-liminal/releases/tag/v0.1.0). Happy to change anything
the project would prefer.
```

**Liminal HQ** (blog post and social; the blog stack's post goes in as a pull request, not merged):

```text
Try Quill on ReactOS in your browser

Quill is a small word processor written in Rust. The same code runs on MS-DOS, Windows 3.1,
Windows 95 and Linux, and now you can try the Windows 95 build without installing anything: it runs
on ReactOS, an open-source Windows, inside a PC emulated in your browser tab.

Type in the letter, print it to PDF and download it, copy and paste between the PC and your own
computer, drop a file in, and leave it idle long enough to meet the Afterglow screensavers.
Nothing you type leaves your tab.

https://labs.liminalhq.ca/quill/try-reactos/

ReactOS™ is a trademark of the ReactOS Project; this is an unofficial build by Liminal HQ Labs.
```

## Smoke test after publishing

WP-LAUNCH-2's `site/try-reactos/harness/check-live.ts`, run by an agent or the owner against the live address (read only; it publishes nothing and sends no data anywhere but the site):

1. `GET /quill/try-reactos/` is 200 over HTTPS, `http://` redirects to `https://`, and the page holds no `noindex`.
2. `web-manifest.json` loads; every file it names (the parts' `manifest.json`, 512 parts, the state, v86's five files) is 200 with the expected length, and a sample (the state, the first and last non-empty parts, `libv86.mjs`, `v86.wasm`) hashes to the manifest's SHA-256.
3. Headers: `Content-Type` for `.wasm` is `application/wasm`; `.zst` arrives byte for byte (no transfer encoding changes the bytes v86 sees).
4. Headless Chromium and Firefox: Start, the desktop with Quill within 60 s on this connection, the chip reads *Shared*, a Notepad copy reaches the browser's clipboard, Print to PDF lists a PDF, Shut Down and **Turn on**.
5. Every request the run saw went to `labs.liminalhq.ca`.
6. The licences page and the home-page link are 200; `check-site.py --external` passes on the deployed site.
7. The result (pass or the failing step, timings, bytes) goes into `~/Documents/Liminal HQ/Quill/Web demo/launch/smoke-<date>.md` and to the owner.

The owner then repeats the test matrix's rows 1 to 4 on the live address, and a phone if at hand.

## Rollback

| What went wrong | Who | What to do | Time to effect |
|---|---|---|---|
| A page bug (script, wording, layout) | Agent opens, owner merges | Revert the page's pull request on `main`; Pages redeploys with the same data | Pages deploy, plus up to 10 min of cached `index.html` |
| The data is bad (a broken state, a missing file, a bad build) | Owner | Merge a pull request that sets `web-data.pin` back to the placeholder: the page shows the *not available* wording and the DOS demo link. Or back to the previous good release, once there is one | One deploy |
| The demo must come down (a request from the ReactOS Project, a licence or trademark question) | Owner | The placeholder pin and the home-page link removed in one pull request; the Quill release kept (or its tarball removed, which the release page then says) but its source pointer and the written offer kept for three years, as the licences require for anyone who already received the binaries | One deploy |
| reactos-liminal `v0.1.0` is wrong | Owner | Never edit its assets: release `v0.1.1` with the fix. Deleting a release is only for a legal need, and the source archive stays reachable | A release run |
| Bandwidth trouble (GitHub throttles or writes) | Owner | The placeholder pin first; then move the data to a second host (Cloudflare R2), which needs CORS rules and a change to the page's same-origin rule: a new package, not ready today | Hours to days |
| An announcement says something wrong | Owner | Edit the post, or a short follow-up | — |

Two rules make every row cheap: published assets are never edited (a fix is a new release and a new pin), and the pin is the only switch between "live" and "not available".

## Risks

- **Size and first start.** About 70 MB on the site; a visitor's first start fetches the state (14.2 MB) and the parts the guest touches, and v86. On a slow link the desktop takes tens of seconds; the status line says how much is downloading. The saved state rebuilt with the helper and the device may grow; WP-WEB-8 records it.
- **Hosting limits.** Pages' soft 100 GB a month is about 3,800 full sessions; a well-received post could pass it in days. GitHub can also rate-limit a busy Pages site. No second host is ready; the placeholder pin is the brake.
- **v86 on phones.** Measured only on this workstation (5.4 to 5.7 MIPS idle). A phone may run several times slower; the Windows 3.x build under WineVDM, already "emulation inside emulation", may be unusable there. A 256 MB guest plus a 119 MB decompressed state in one tab may be more than iOS gives a tab, and Safari can reload the tab without warning. Not measured on any phone.
- **The Win16 crash history.** Quill for Windows 3.x crashed under WineVDM in the remix (a SEGV at `1017:0812`) when the image carried a stale `QUILL.EXE`; the fix was fresh builds and `qagrr/check-win16.sh` (#136), which checks the shortcut opens the window in QEMU. It is not checked in v86, where timing differs; `check-demo.ts` adds the Windows 3.x shortcut in v86. ReactOS's `win32k` has also crashed under heavy repaint and live scheme switches (the board's risk rk3); the demo's saved desktop does neither, but a visitor can.
- **v86 internals.** The sound device, the power hook and the key catching use v86 0.5.473's internal API (`emulator.v86.init`, `cpu.devices.pci`, `cpu.io.ports`, the keyboard's listener phase); they hold only for that pinned build, and `PROVENANCE.md` says so. A v86 update is a planned change, never a drift.
- **The clipboard lane's length.** Seven packages, two in `quill-app` and the Win32 shell; the stopgap (W9X-101) is the fallback, and the Unicode freeze must not start ahead of WP-ED-5.
- **The image freeze slipping.** A late icon round or trim change after WP-PACK-6 costs a remix build and a new state; WP-ICON4-2 is the likely one (owner decision 3).
- **Disk space.** Near the 20 GB floor (20 to 27 GB free on 2026-11-27); four image builds remain.
- **Browsers change their rules.** The 5 s write window, the promised write and the Paste prompts differ by browser and version; the page decides by feature and keeps the buttons.
- **Licences and trademarks.** No ReactOS trademark policy covers remasters; the courtesy note asks. The WineVDM release ReactOS 0.4.16 installs is not yet named. A complaint is handled by the rollback's third row.
- **Untested Safari.** Safari has not been run on any device; the pages' Safari paths are the documented ones only.

## Decisions to record

This plan reserves **SITE-41 to SITE-50** (`scripts/next-id.sh SITE 10 "First demo launch (WP-LAUNCH)"`). Each package writes its rows in `docs/decisions.md` when it opens its pull request; until then they are planned.

| Id | Decision | Written by |
|---|---|---|
| SITE-41 | The launch order: reactos-liminal `v0.1.0` first, then the Quill data release, then the pin merge, which is the go-live act; one reactos-liminal commit under both | WP-LAUNCH-3 (or the pin pull request) |
| SITE-42 | The demo's data is one tarball on a Quill release `reactos-demo-v0.1.0`, pulled by `pages.yml` by pinned SHA-256; never fetched by the page; a placeholder pin means "not available" | WP-WEB-7 (the web demo plan's SITE-19 was used by the sound decision, so WP-WEB-7 takes this one) |
| SITE-43 | The page goes public: indexed and linked only with real data, the unofficial line, the privacy and accessibility statements, no analytics | WP-LAUNCH-1 |
| SITE-44 | The live smoke check and what it asserts (including no request outside the origin) | WP-LAUNCH-2 |
| SITE-45 | Rollback: published assets are never edited; the pin is the only switch | WP-LAUNCH-1 |
| SITE-46 | The touch keyboard button on touch devices | WP-LAUNCH-1 |
| SITE-47 to SITE-50 | Spare, for what the final build or the owner's look changes | WP-WEB-8 |

WP-WEB-5 uses SITE-16 and SITE-17 from the web demo plan's block, as planned.

## Owner decisions

Each is taken with the default so no package waits; the last column is where to change it.

| Question | Default taken | Where to swap |
|---|---|---|
| Which release holds the demo's data? | A Quill release, `reactos-demo-v0.1.0`, after reactos-liminal `v0.1.0` | SITE-42; `web-data.pin`; the procedure's step 5 |
| Tag reactos-liminal `v0.1.0` before the demo goes live, or together? | Before, and on the same reactos-liminal commit as the remix (one system, and the demo links its source archive) | SITE-41; the procedure |
| Icon round 4: wait for WP-ICON4-2 before the image freeze? | No: WP-ICON4-1 is in; WP-ICON4-2 goes in only if merged before WP-PACK-6 starts | WP-PACK-6's pin; a later remix build |
| The Cracktro's scroller words: must WP-SCROLL land before the savers commit S? | Yes, so the demo's Cracktro has its words off by default | afterglow-savers packing order (WP-SCROLL before WP-PACK-2's merge) |
| Fit to window in the first demo? | Only if WP-HOST-6 has it before WP-WEB-8; off by default | SITE-37; WP-HOST-6 |
| The rich clipboard if WP-ED-6 is late | The W9X-101 plain-text stopgap, only if WP-ED-5 has not merged when WP-HOST-4 does | host-integration.md; W9X-101 |
| WP-PTR-5 (pointer tuning) before or after launch? | After: launch on WP-PTR-4's defaults once the owner's check list passes; tuning ships as a page change | pointer.md; WP-PTR-5 |
| A touch keyboard button | Yes, if it is reliable within a day; otherwise the help says a hardware keyboard is needed | SITE-46; WP-LAUNCH-1 |
| Remove `noindex` and link the page from the Quill home page | Yes, in the pin pull request, not before | SITE-43; the pin pull request |
| The courtesy note before or after the announcements | Before, alone, with a day's wait | This plan's announcements section |
| Analytics | None | SITE-43 |

## Sonnet briefs

Briefs that already exist and stay as written: WP-HOST-3 to WP-HOST-7 ([host-integration.md](host-integration.md#wp-host-3-to-wp-host-7)), WP-ED-5 and WP-ED-6 ([work-packages.md](../work-packages.md)), WP-PTR-4 ([pointer.md](pointer.md#wp-ptr-3-wp-ptr-4-and-wp-ptr-5)), WP-PACK-2 to WP-PACK-6 (afterglow-savers `docs/packing.md`, *Sonnet briefs*), WP-ICON4-2 (reactos-liminal `docs/plans/icon-round-4.md`), WP-WEB-8 ([web-demo.md](web-demo.md#wp-web-5-wp-web-6-wp-web-7-and-wp-web-8) plus [The final build](#the-final-build-and-its-checks) above). Paste the web demo plan's [common preamble](web-demo.md#common-preamble) before each brief below; WP-LAUNCH-3 uses reactos-liminal's house rules instead (its `AGENTS.md`).

### WP-WEB-5 Sound: the AC'97 package (full brief)

```text
Package WP-WEB-5, Sound: the AC'97 device as a typed TypeScript package, the page's controls and the
state's audio registers. Branch feat/web-demo-sound, worktree ~/source/worktrees/quill/web-demo-sound,
from origin/main. Starts now (WP-WEB-1 and WP-WEB-4 are merged).

Read first: docs/plans/web-demo.md "Goal 3", "WP-WEB-4 report" (the verdict and the hand-offs) and
WP-WEB-5; docs/plans/first-demo-launch.md (this checklist); decisions SITE-14, SITE-19, SITE-40;
site/try-reactos/spikes/ac97/ (ac97-model.js, ac97-v86.js, ac97-sink.js, spike.js, run-spike.ts,
ac97-model.test.mjs): your starting point; site/try-reactos/packages/host-link/ (package.json,
build.ts, tsconfig.json, README.md, test/): the layout to copy exactly; site/try-reactos/sound.js
(the empty seam), power.js (attach and detach across Shut Down and Turn on), try-reactos.js (where
the seams are called), index.html, tools/stage-web.py, tools/stamp-web.py, harness/build-state.ts,
harness/check-power.ts, harness/lib.ts; site/try-reactos/v86/PROVENANCE.md and v86.d.ts.

Goal: on the published layout, after Start, ReactOS plays sound through the visitor's speakers, with
a Sound button and a volume slider; a saved state restores with its sound working.

Build, in this order, one commit each where it makes sense:

1. The package. New site/try-reactos/packages/ac97/ laid out as packages/host-link is (private ES
   module, no runtime dependency, bun test, tsc --noEmit, build.ts writing one minified
   site/try-reactos/lib/ac97.js that is committed, and check:built failing on drift). Port the spike's
   model to typed modules: src/regs.ts (the ICH bus-master and codec register maps as consts),
   src/codec.ts (AC'97 2.3 codec: reset, powered-up 0x000F, variable rate with 0x2C honoured only
   when 0x2A bit 0 is set, volumes as gain, vendor ID 0x8384 0x7600), src/busmaster.ts (PCM-out:
   BDL base, CIV, LVI, PIV, SR, PICB, CR; walking with read_blob; IOC and LVBCI; resume on a new LVI
   with the run bit set; level-style interrupt), src/device.ts (PCI function 8086:2415, the two BARs,
   handlers registered at the power-on addresses BEFORE register_device, getState and setState as a
   versioned plain object), src/attach.ts (wrap emulator.v86.init so the device exists before
   restore_state, as the spike proved; the autostart:false route kept as a documented alternative
   for cold boots), src/sink.ts (the AudioWorklet ring buffer the spike measured, 120 ms top-up,
   ScriptProcessorNode fallback where AudioWorklet is missing), src/index.ts. Written from Intel's
   AC'97 2.3 and ICH documents and the spike; never from QEMU's GPL ac97.c. Tests: every spike test
   moved over and typed, plus reset values, a BDL wrap, a halted channel resumed, IOC with the
   interrupt enable off, getState/setState round trip, a state from a newer version refused.
2. The page. sound.js becomes the glue: attach(emulator, ui) and detach() as power.js already calls
   them; the AudioContext is created or resumed inside the Start click handler (and Turn on and
   Reset), never before; one GainNode shared with v86's PC speaker (stop passing disable_speaker:
   true); Stop, Reset and Shut Down suspend the context; a hidden tab keeps playing. A Sound button
   (aria-pressed, "Sound on" or "Sound off") and a volume slider (labelled "Volume", 0 to 100) in the
   player controls, in one marked block of index.html and try-reactos.css; default on at 70 %,
   remembered in localStorage inside try/catch so a private window still works; ?sound=off in the
   address wins for that load. A browser that refuses audio still runs the machine and the status
   line says "Sound is off in this browser". The diagnostics panel gains a Sound row (rate, frames,
   underruns, interrupts; never samples).
3. The state. harness/build-state.ts registers the device for the state build, waits for the guest's
   first codec access (about 21 s after the desktop on a cold boot), saves while no sound plays (the
   channel halted) and writes the device's getState() beside the state. tools/stage-web.py puts it in
   web-manifest.json's "audio" object (versioned) and --check verifies it; a state without "audio"
   boots with the device present but silent, as the spike measured, and the page says so in the
   Sound row. tools/stamp-web.py stamps lib/ac97.js and sound.js.
4. Checks. New harness/check-sound.ts on a locally built state (qagrr/build-state.sh with your
   build-state.ts; check-state.sh still passes): restore; open the Sounds tab through the Run box
   (rundll32 shell32.dll,Control_RunDLL mmsys.cpl,,1), play a sound; count frames and the peak from the
   sink; mute and check the gain node is 0; slider to 30 and check 0.3; Reset and check silence;
   Shut Down, Turn on and play again (power.js's attach and detach); then the same restore with
   ?sound=off. harness/check-power.ts still passes. Headless Chromium and Firefox both.
5. Docs. site/try-reactos/README.md: a Sound section (replace the AC'97 sound spike section's
   "spike" wording; keep its numbers as history); v86/PROVENANCE.md one paragraph on the internal API
   used (emulator.v86.init, cpu.devices.pci.register_device, cpu.io.register_read/write,
   cpu.read_blob, device_raise_irq/lower_irq); docs/decisions.md SITE-16 (the package, its layout,
   the controls and their defaults) and SITE-17 (the audio registers in the manifest, the save rule,
   a state without them); .github/workflows/ci.yml one "AC'97 tests" job (bun test, typecheck,
   check:built) beside "Host link tests". Remove site/try-reactos/spikes/ac97/ in the last commit and
   say so.

Must not touch: host-link.js, packages/host-link/**, manifest.js beyond reading the "audio" object
through stage-web's output, power.js beyond the two calls it already makes, the pointer block of
try-reactos.js, qagrr/** except a comment, guest/**, crates/**, win9x/**, v86's files.

Finish line checks (all must hold):
- bun test, typecheck and check:built pass in packages/ac97; CI's AC'97 job passes.
- check-sound.ts passes in Chromium and Firefox: frames counted at the guest's rate with 0
  underruns, mute and slider exact, silence after Reset, sound after Turn on, ?sound=off silent.
- check-state.sh, check-clean.ts and check-power.ts still pass.
- stamp-web.py --check passes; the headers check and markdownlint pass.
- lib/ac97.js is under 40 KB minified.

Evidence into ~/Documents/Liminal HQ/Quill/Web demo/WP-WEB-5/: the controls (a Playwright shot of
the page), the counts as JSON, a short WAV recorded from the sink of a system sound, the CPU cost
idle and playing. Commit docs/media/web-sound-controls.png. Report: PR URL, what was and was not run
(real speakers are the owner's), OWNER DECISION lines. PR title suggestion: "Play the ReactOS demo's
sound through an AC'97 package".
```

### WP-WEB-7 Hosting: the pin, the Pages steps and the licences page

```text
Package WP-WEB-7, Hosting. Branch feat/web-demo-hosting, worktree ~/source/worktrees/quill/web-demo-hosting.
Read: docs/plans/web-demo.md "Goal 4" and WP-WEB-7; docs/plans/first-demo-launch.md "The publishing
pipeline", "Licences and attribution", "Rollback" and SITE-42; .github/workflows/pages.yml;
scripts/check-site.py; site/try/licences.html; site/try-reactos/tools/stage-web.py and fetch-v86.sh;
site/try-reactos/v86/PROVENANCE.md; reactos-liminal's docs/licences.md (git show origin/main:...).
Build: scripts/fetch-web-data.sh (reads site/try-reactos/web-data.pin: URL, SHA-256, size; a
placeholder line means "no data" and exits 0 with a notice; --local TARBALL for owners and tests;
checks size and SHA-256 before unpacking; refuses members outside data/ and web-manifest.json;
then runs stage-web.py --check); site/try-reactos/web-data.pin as the placeholder; qagrr/pack-web.sh
(stage-web output to qagrr-web-<12 hex>.tar with sorted members and fixed mtimes, SHA256SUMS, the
pin line, licences.md rendered from the same facts as the page); pages.yml gains fetch-v86.sh and
the data step (skipped cleanly on the placeholder) and the manifest check; site/try-reactos/licences.html
with the table in the launch plan, its facts filled from web-manifest.json at pack time (a small
template step in pack-web.sh, so the page and the tarball cannot disagree) and the written offer and
trademark lines; site/README.md's try-reactos paragraph; docs/decisions.md SITE-42.
Must not: create a release, a tag, an upload or a Pages deployment; touch the page's modules, qagrr.toml,
crates/**, win9x/**. Tests: fetch-web-data.sh against a local tarball (good, one byte changed,
truncated, a member escaping the folder, the placeholder); a boot from a plain static server through
check-clean.ts with BASE pointed at it; check-site.py. PR title: "Prepare the ReactOS demo's data for Pages".
```

### WP-LAUNCH-1 The page made public

```text
Package WP-LAUNCH-1, The ReactOS demo page made public. Branch feat/web-demo-launch-page, worktree
~/source/worktrees/quill/web-demo-launch-page. After WP-WEB-7 has merged.
Read: docs/plans/first-demo-launch.md "The page at launch" (the contract), "Rollback", SITE-43, SITE-45,
SITE-46; host-integration.md "Privacy and security" and "The clipboard chip"; pointer.md "Touch and pen";
site/try-reactos/index.html, try-reactos.css, manifest.js (the no-data path), site/index.html (the Try
Quill links), site/css/site.css.
Build: the lede and the unofficial line; the hint list rewritten as the plan lists (Canadian English,
sentence case, no jargon: "the PC", not "the guest"); a Privacy section and an Accessibility line; the
first-start size from WP-WEB-8's number (leave a clearly marked placeholder until then and list it in the
PR); the not-available wording on the placeholder pin, with the builder's wording kept in the panel; the
Licences and source footer link; the touch Keyboard button (pointer: coarse only; a hidden textarea,
keyboard_send_text, Enter and Backspace mapped; announced) or, if it is not reliable in a day, the help
line instead and an OWNER DECISION line. The noindex removal and the home-page link are written as a
second commit on a separate branch feat/web-demo-go-live that the pin pull request carries; do not
merge them here. Check: check-site.py, stamp-web.py --check, a Playwright shot at 390 px and 1280 px
wide in Chromium and Firefox, Tab order and names read from the accessibility tree, check-clean.ts and
check-power.ts. Decisions SITE-43, SITE-45, SITE-46. PR title: "Write the ReactOS demo page for visitors".
```

### WP-LAUNCH-2 The live smoke check

```text
Package WP-LAUNCH-2, The live smoke check. Branch feat/web-demo-smoke, worktree
~/source/worktrees/quill/web-demo-smoke. After WP-WEB-7 has merged.
Read: docs/plans/first-demo-launch.md "Smoke test after publishing" (the seven steps are the contract)
and SITE-44; harness/lib.ts, check-clean.ts, check-power.ts; tools/stage-web.py's manifest fields.
Build: site/try-reactos/harness/check-live.ts BASE (default https://labs.liminalhq.ca/quill/), read only:
the HTTPS and redirect check, the manifest walk with lengths and the sampled hashes, the headers, the
headless boot in Chromium and Firefox (reuse the existing checks' functions; the clipboard and PDF steps
only when the manifest says the helper is present), the same-origin request assertion, and a Markdown
report. It must run today against a local static server with a staged tarball (BASE=http://127.0.0.1:<port>/quill/)
and say clearly "no data published" against the live site while the pin is the placeholder. Not in CI
(it needs the network and real data); a line in README.md says how to run it. Decision SITE-44. PR title:
"Add a smoke check for the published ReactOS demo".
```

### WP-LAUNCH-3 reactos-liminal ready for v0.1.0

```text
Package WP-LAUNCH-3, reactos-liminal ready for v0.1.0. Repo liminal-hq-labs/reactos-liminal (read its
AGENTS.md and CLAUDE.md: they govern), branch docs/release-0.1.0-ready, worktree
~/source/worktrees/reactos-liminal/release-ready. After WP-PACK-4/5 and WP-ICON4-1 (and WP-ICON4-2 if the
owner waits for it) have merged.
Read: docs/release.md ("Before you start"), docs/releases/0.1.0.md, SPEC.md's licence checklist,
docs/licences.md, NOTICE.md, the release workflow's source job; Quill's docs/plans/first-demo-launch.md
"Release v0.1.0: the procedure" and "Release notes".
Build: ./rl lock for both recipes and commit them if changed (./rl verify passes for both); update
docs/releases/0.1.0.md for what is now in the image (the savers, icon rounds 3 and 4, card faces, the
colour table, the shortcut audit), keeping its Unofficial and Known limits paragraphs; identify the
WineVDM (otvdm) release ReactOS 0.4.16 installs (from the installed files' version resources on the
golden disk, read with mtools, and otvdm's release list) and name it with its source tag in SOURCE.txt's
template and NOTICE.txt; a short checklist file for the owner listing licence checklist items 1, 2, 3, 6
and 7 with the evidence for each (paths, sums, commands run). Must not: tag, dispatch build.yml or
release.yml, or publish anything. Tests: the repository's own (ruff, pytest, headers, markdownlint,
actionlint). PR title: "Get the v0.1.0 release ready".
```

## What was not verified

- What WP-HOST-1's spike has found (no pull request yet), and how far WP-WEB-5, WP-ED-5, WP-SCROLL and WP-PACK-2 have got.
- Reactos-liminal #38 and #39 and Quill #193 were read by title and state only, not reviewed.
- Any number for the final build: sizes, first-start bytes and time with sound, the helper and the savers included.
- Any browser other than headless Chromium and Firefox on this machine; any phone; Safari anywhere.
- Pages' behaviour under real load, and whether GitHub's soft bandwidth limit is enforced by throttling or by mail.
- That the compact trim leaves no CC-BY-NC wallpaper and no Gecko on the QAGRR disk (read from reactos-liminal's trim docs, not from the image's file list).
- The WineVDM release on the disk.
- The courtesy post draft the board mentions; none was found in the Afterglow or Quill folders, so the draft here is new.
