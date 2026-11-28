# The Host Link Kit: guest tools, the other way round

A plan for turning the host link and the virtual sound card into one small, coherent kit of integration features between the browser page and the guest it runs: what VMware Tools, VirtualBox Guest Additions and the QEMU guest agent are to their hypervisors, built from the page's side. The owner, 2026-11-28: "Could host link work with the sound card tool and get data about it and such, can this work like a GUEST TOOLS kit, but instead it's a HOST LINK kit? (See what I did there)". Written on 2026-11-28 against `main` at `ad2e9b1`, with the guest helper (WP-HOST-3, pull request 200), the page clipboard (WP-HOST-4, pull request 204), the AC'97 recording package (pull request 198), the AC'97 reuse plan (pull request 214) and the Windows 95 module loader (pull request 215) open, and v86 0.5.473 as vendored. Where this plan and [decisions.md](../decisions.md) disagree later, the decisions win.

## Table of contents

- [Recommendation](#recommendation)
- [What was checked](#what-was-checked)
- [The architecture](#the-architecture)
- [The capability model](#the-capability-model)
- [The channels](#the-channels)
- [Audio and device telemetry](#audio-and-device-telemetry)
- [Volume in step](#volume-in-step)
- [How the sound card and the link meet](#how-the-sound-card-and-the-link-meet)
- [The sound card tool in the guest](#the-sound-card-tool-in-the-guest)
- [A modular guest helper](#a-modular-guest-helper)
- [The kit's shape and name](#the-kits-shape-and-name)
- [The `v86-host-link` repository](#the-v86-host-link-repository)
- [Migration from Quill](#migration-from-quill)
- [Edits to the other plans](#edits-to-the-other-plans)
- [Versions and compatibility](#versions-and-compatibility)
- [Privacy](#privacy)
- [Testing](#testing)
- [Budgets](#budgets)
- [The first demo](#the-first-demo)
- [Decisions to record](#decisions-to-record)
- [Rules for every package](#rules-for-every-package)
- [Order and parallelism](#order-and-parallelism)
- [Work packages](#work-packages)
- [Builder briefs](#builder-briefs)
- [The lead's brief](#the-leads-brief)
- [Owner decisions](#owner-decisions)
- [Risks](#risks)
- [What was not verified](#what-was-not-verified)

## Recommendation

| Question | Recommendation | Why, in one line |
|---|---|---|
| Is a "host link kit" the right shape? | **Yes.** One capability model (named channels with versions, advertised in `hello`), one helper core per guest family with optional channel modules, one page glue that joins the link to the devices, and one installable bundle per guest family, shipped as a **Host Link Kit** disc the page can mount | The pieces already exist separately (the link, `HOSTLINK.EXE`, the card, the power watcher); the kit is the contract that ties them together and the box they come in |
| The capability model | `hello`'s `caps` keeps its comma list; a token is a channel name (version 1), `name.N` (version N, written beside the bare name so older readers still see it), or `name-feature` (a feature flag). Each side uses a channel only when both list it, at the highest version both list | Additive to protocol 1: an older page or helper reads exactly what it reads today |
| New channels | **`audio`, `power`, `sys`, `log`** (and later `pointer`) join `ctl`, `clip`, `file`, `time` and `display`; both codecs learn the names, and the vectors gain them | The codecs' channel lists are closed (`Channel` in `quill-hostlink`, `PRIORITY` in `frame.ts`); a name they do not know is dropped with `code=unknown` |
| Audio telemetry | **Most of it needs no helper.** The page reads the card itself: the guest's rate and format, playing or not, buffers, starved frames, underruns at the output, and the guest's own master, PCM and record volumes and mutes, which the guest's driver writes into the codec's registers. The helper adds what only the guest knows: its mixer as Volume Control shows it, its wave devices and driver version, and a way to set its volume | `Ac97.gains()` already applies the codec's master (`0x02`) and PCM-out (`0x18`) registers to the output; reading them is a getter |
| Volume sync | The page's slider stays the **speaker** (the visitor's side, always works); the guest's own volume is shown beside it ("Muted in the PC" when the guest has muted); **Keep in step** (off by default) makes the slider set the guest's master through the helper and holds the speaker at full | Two gains in series must never both follow the slider, or the level is squared; the guest's mute is the most common reason a visitor hears nothing |
| The sound card integration | **A thin page glue, `site/try-reactos/kit.js`**, subscribes to the card's `onChange` and a new `onCodec`, and to the link's messages, and writes the Diagnostics rows; the `ac97` and `host-link` packages never import each other | The packages stay reusable in `v86-devices`; the page decides what joins what |
| The sound card tool | **`HLTRAY.EXE`**, a tray icon for the kit ("Host link connected: clipboard, files, audio"), with a **Sound card** window: the card's rate, buffers, underruns, the microphone's state, the volume, and a **Play a test sound** button. A Control Panel page later | A tray icon is what Guest Additions taught people to look for; Windows 95 and ReactOS both have `Shell_NotifyIconA` |
| Guest helper size | **Modular.** `HOSTLINK.EXE` stays the one owner of COM1 and grows only a small module loader and a broker; channels beyond the clipboard live in DLLs (`HLAUDIO.DLL`, `HLSYS.DLL`) loaded through `quill-w32rt`'s loader, and the tray tool is a separate program that talks to the helper by `WM_COPYDATA` | Only one process can open COM1, so separate helpers cannot each speak to the page; DLLs keep the core within budget and make every channel optional |
| Name and repository | Product: **Host Link Kit** (the owner's name; on the disc, in Add/Remove Programs and on the tray icon). Code: **its own repository, `v86-host-link`**, built locally on its `main` at `~/source/liminal-hq-labs/v86-host-link` with no remote until the owner says (**decided by the owner**, 2026-11-28, SITE-129). It holds the protocol, the vectors, the page library, the Rust guest crates, the helpers per guest family and the kit's installers. `v86-devices` (pull request 214) keeps `v86-device-kit` and `v86-ac97`; the two meet only in a page's glue | The host link rides on v86's own serial port and needs none of the device kit; one repository per concern keeps each releasable on its own |
| Who builds it | **An Opus lead** owns the new repository end to end from this plan ([The lead's brief](#the-leads-brief)) and brings the result back into the Quill Remix by vendoring | The lead-agent pattern: a large, self-contained product with packages that build on each other |
| Bundles | **Windows 95 and later**: a Liminal Installer setup (the Win32 runtime, `SETUP.EXE`) with the helper, optional channel modules, the tray tool, a Startup entry, a Start menu group and an uninstaller. **Linux**: a tarball with `hostlinkd`, an OpenRC service and an install script, later an Alpine package. **Terminal guests**: nothing to install. All on one **Host Link Kit CD** (an ISO under 1.44 MB, so also a floppy image) that the page mounts with v86's `set_cdrom` | One disc per kit, as guest tools always came; the page can insert it without a download |
| Time | **Set the zone, not the clock.** `SetLocalTime` does nothing in v86 (WP-HOST-1); `SetTimeZoneInformation` changes the bias and makes the guest re-read the RTC, which already runs on the host's UTC, so the taskbar shows the visitor's time | The one call that was measured to work |
| The first demo | **Page-only items**: the card's telemetry rows in Diagnostics and the "Muted in the PC" badge (WP-KIT-1). Nothing in the guest changes for the launch | No guest image rebuild, no codec change, no helper change before WP-WEB-8 |
| Packages | WP-KIT-1 (page telemetry, now), WP-KIT-2 (the capability model and the new channels in the spec and codecs), WP-KIT-3 (the helper's module loader and broker), WP-KIT-4 (audio), WP-KIT-5 (power, sys and log), WP-KIT-6 (the tray tool and the sound card window), WP-KIT-7 (the Windows installer and the kit disc), WP-KIT-8 (the Linux kit), WP-KIT-9 (pointer feedback) | See [Order and parallelism](#order-and-parallelism) |

## What was checked

All by reading on 2026-11-28; no emulator, browser or server was started for this plan.

- **The link.** `docs/host-link.md` on `main` (five channels, `hello` with `caps`, credits, errors, the vectors); `crates/quill-hostlink/src/lib.rs` (`enum Channel { Ctl, Clip, File, Time, Display, Unknown }` and `Channel::PRIORITY`); the page package's `frame.ts` (`PRIORITY = ["ctl", "clip", "time", "display", "file"]`), `link.ts` (`CAPS = ["clip", "file", "time", "display"]`, `send(channel, op, fields, data)`, `on(event, handler)`, `stats()`); `site/try-reactos/host-link.js` (the *Link* row of the Diagnostics panel).
- **The helper** on `feat/host-guest-helper` (pull request 200): `win9x/hostlink/src/main.rs` (one thread, `MsgWaitForMultipleObjects`, a hidden window of class `LHQHostLink`, static lanes per channel), the WP-HOST-3 report in `docs/plans/host-integration.md` on that branch: `HOSTLINK.EXE` is **22,016 bytes** of a 24 KiB budget guarded in `win9x/build.sh`, with 46 ANSI imports on `win9x/tools/hostlink-imports.allow`; the WP-HOST-1 report: `SetLocalTime` and `SetSystemTime` return TRUE in v86 and the clock does not move, while `SetTimeZoneInformation` changes the bias and makes the guest re-read the RTC (host UTC); `ChangeDisplaySettingsA(&mode, 0)` switches without a restart (GO).
- **The other guests** plan (pull request 213, merged as `docs/plans/host-link-guests.md`): `hostlinkd` for Linux under X11, terminal guests through OSC 52, Windows 3.x, the `Transport` seam, the `clip-html` and `clip-png` feature tokens, print jobs as files with `intent=print`, and `v86-host-link` as the package name (SITE-80 to SITE-89).
- **The card.** `site/try-reactos/packages/ac97/src/index.ts` (`attachAc97(emulator, { state, volume, muted, context, onChange, trace })`; the handle's `stats()` with `guestRate`, `playing`, `interrupt`, `counters`, `drainedFrames`, `output`, `sink`, `volume`, `muted`, `gain`; `sinkStats()` with `out`, `silent`, `peak`, `underrunEvents`, `underrunFrames`, `dropped`, `quanta`; `getAc97State()`); `model.ts` (the codec's power-on values: master `0x02` and PCM out `0x18` start **muted**; the STAC9700 vendor id; `rate` from `0x2C` with variable rate audio; `gains()` applies master and PCM out to the output; the counters `dmaFrames`, `buffersDone`, `iocs`, `starvedFrames`, `irqRaised`); `sound.js` (the Sound button, the slider, the *Sound* row).
- **The reuse plan** (pull request 214, branch `docs/ac97-reuse-plan`): `liminal-hq-labs/v86-devices` with `v86-device-kit`, `v86-ac97`, later `v86-usb`, and `v86-host-link` joining as `packages/host-link` (since overtaken: the owner made `v86-host-link` its own repository, SITE-129); the microphone contract (SITE-103: `startMicrophone()` only from a click, an `onMicrophone(state)` hook for the indicator); USB from UHCI and a HID tablet (SITE-106).
- **The page's power watcher** (`site/try-reactos/power.js`, SITE-40): wraps the ACPI PM1a control port `0xB004` and reports the guest's own power-off.
- **v86 0.5.473** (the fetched `libv86.mjs` in the sound demo worktree and `v86/v86.d.ts`): `set_cdrom`, `eject_cdrom`, `set_fda`, `eject_fda`; the page passes `disable_speaker: true`; a PC speaker with `pcspeaker-update`, `pcspeaker-enable` and `pcspeaker-disable` bus events; a VMware mouse port with `absolute_pointer_enabled` and `push_absolute` strings.
- **Windows 95 modules** (`docs/plans/modules.md` section 19 and pull request 215): `win9x/quill-w32rt` with `runtime!(dll)`, `dll_entry!` and `quill_w32rt::module::load` (full path from `GetModuleFileNameA`, `SetErrorMode`, two exports by name, build-id check, free on refusal); a no-CRT DLL of 4,096 to 11,776 bytes importing only `KERNEL32!ExitProcess`.
- **Liminal Installer** (`SPEC.md` and `docs/retro-win32.md` in `liminal-hq-labs/liminal-installer` at `083b99a`): the Win32 runtime performs `mkdir`, `copy`, `shortcut` (a `.lnk` in a Start menu folder named for the product), `file_type`, `{windows}` and `{system}` files, Add/Remove Programs and a kept uninstaller; it has **no general `registry.set` and no Startup-folder shortcut**, and "Launch after setup" is a check box that does nothing yet. Media (`[[media]]`, `kind = "cdrom"`, `autorun`) are described and built by `li-media`.
- **The mixer investigation.** `~/Documents/Liminal HQ/Quill/Web demo/Sound demo/README.md` (the sound preview of 2026-11-28) has no mixer findings yet; the empty Volume Control is being diagnosed by another builder. This plan does not wait on it: WP-KIT-4 reads its results first.

## The architecture

```text
 Page (the browser tab)                                           Guest (ReactOS, Windows 95+, Linux)
┌──────────────────────────────────────────────────────┐        ┌───────────────────────────────────────────┐
│ UI: player row (Sound, slider, Mic) · clipboard chip │        │ HLTRAY.EXE  tray icon, Sound card window  │
│     Diagnostics: Link · Sound · Audio · System · Log │        │      │ WM_COPYDATA (local broker)         │
│            ▲                     ▲                   │        │      ▼                                    │
│ kit.js  ── the page glue: privacy switches, rows,    │        │ HOSTLINK.EXE  owns COM1; clip, file, time,│
│            volume in step, test sound, insert kit CD │        │   display; ctl and caps; module loader    │
│     │ onChange/onCodec        │ on(message)/send     │        │   ├─ HLAUDIO.DLL  winmm mixer, wave caps, │
│     ▼                         ▼                      │ COM1   │   │               PlaySound test          │
│ ac97 package            host-link package            │◄══════►│   └─ HLSYS.DLL    power, sys, log         │
│ (stats, sinkStats,      (frames, credits, hello,     │ serial │                                           │
│  codec registers, mic)   caps, channels)             │        │ hostlinkd (Linux): the same channels over │
│     │ PCI 8086:2415           │ serial0 bytes        │        │   ttyS1: ALSA ctl, /proc/asound, /proc    │
│     ▼                         ▼                      │        │                                           │
│ v86: PCI · I/O · DMA     v86: UART 16550 (COM1)      │        │ ac97.sys / snd-intel8x0 ── drives the card│
│ power.js: ACPI 0xB004    set_cdrom: Host Link Kit CD │        │ Host Link Kit CD: SETUP.EXE, install.sh   │
└──────────────────────────────────────────────────────┘        └───────────────────────────────────────────┘
        devices (the card, the port, the disc)  ◄── the guest's drivers see ordinary hardware ──►
```

Three rules hold the shape: the packages do not know each other (only `kit.js` imports both); the guest's helper is the only owner of the serial port, so every guest-side feature goes through it; and everything the page can learn from the device itself is learnt there, so the kit degrades to "page-only" with no helper at all.

## The capability model

### Tokens in `caps`

`hello`'s `caps` is today a comma list of channel names. The kit keeps it and gives each token a shape:

```abnf
caps-value = token *("," token)          ; at most 256 bytes, the body rule's value limit
token      = name ["." version] / name "-" feature
name       = 1*16lcalpha                 ; a channel name, as in the frame grammar
version    = "2" / ... / "99"            ; no ".1": the bare name is version 1
feature    = 1*16(lcalpha / DIGIT)
```

- `audio` alone means the `audio` channel at version 1. A side that speaks version 2 lists both, `audio,audio.2`, so an older reader that tests for `audio` still finds it. Both sides use the **highest version both list**; with no common version the channel is off.
- `clip-html`, `clip-png` (planned by WP-HLG-2), `audio-set`, `audio-play`, `power-shutdown` and `sys-cpu` are **features**: optional abilities within a channel version. A feature is used only when both list it, except a feature that is only a sender's ability (`audio-play`), which needs only the receiver's token.
- A token a side does not know is ignored. `proto` stays `1`; a new `proto` is reserved for a change to frames, bodies or credits, never for a new channel.
- The page lists only the channels the visitor has left on (see [Privacy](#privacy)); turning a channel off sends a new `hello`, which the helper answers, so both sides agree at once.
- The `app` key names the helper (`HOSTLINK/0.2.0`); a new optional key **`mods`** lists the loaded channel modules with their versions (`HLAUDIO/0.1.0,HLSYS/0.1.0`) for the *Link* row. It is diagnostics only.

The longest helper `caps` the kit plans, `clip,clip-html,clip-png,file,print,time,time-zone,display,audio,audio-set,audio-play,power,power-shutdown,sys,sys-cpu,log,pointer`, is 128 bytes, half the limit.

### New channel names in the codecs

The codecs know five channels and drop any other name with `code=unknown`. The kit adds **`audio`, `power`, `sys`, `log`** and **`pointer`** to `quill-hostlink`'s `Channel`, to `frame.ts`'s list and to the vectors' known names (WP-KIT-2), with this priority order:

`ctl`, `clip`, `power`, `pointer`, `time`, `display`, `audio`, `sys`, `log`, `file`

`power` sits high because a shutdown notice must not wait behind a file; `pointer` because it is small and late is useless; `log` and `file` last. A helper or page built before WP-KIT-2 never receives the new channels, because the other side sends a channel only when its peer listed it; a stray packet would still be answered `code=unknown` once and dropped, which is the specified behaviour today.

### Message sizes

Every new message is a body of a few fields and fits one packet (at most 4,096 bytes; most are under 300). None has binary data except `log`, whose data is one UTF-8 line of at most 1,024 bytes. State messages are sent on change, debounced to at most 10 a second per channel; requests are answered once.

## The channels

The verdict column is for ReactOS 0.4.16 and Windows 95 and later in v86 unless it says otherwise. **GO** is measured, **GO on paper** read but not run, **NO-GO** measured or impossible.

| Channel | Direction | What | Win32 guest gets it from | Linux guest | Terminal guest | Page UI | Verdict |
|---|---|---|---|---|---|---|---|
| `clip` | both | The clipboard (built: WP-HOST-3, WP-HOST-4) | The viewer chain, `CF_UNICODETEXT`, `Rich Text Format`, `CF_DIB` | X11 selections (`hostlinkd`, WP-HLG-3) | OSC 52 through the page (WP-HLG-5) | The chip | **GO** (built, open pull requests 200 and 204) |
| `file` | both | Files in and out (WP-HOST-5) | `C:\From your computer\`, `/send`, `WM_COPYDATA` | 9p or the link | — | The *From the PC* list, the drop zone | GO on paper (planned) |
| `file` + `print` | g>p | Print jobs (WP-HOST-8, WP-HLG-7) | Quill's Print to PDF, the spool folder | `lpr` shim | — | Print... beside Download | GO on paper (planned) |
| `display` | both | Fit to window (WP-HOST-6) | `EnumDisplaySettingsA`, `ChangeDisplaySettingsA(&mode, 0)` | `xrandr` if the frame buffer can change modes (fbdev cannot; `bochs-drm` on v86's VGA is untested) | — | Fit to window switch | **GO** on ReactOS (WP-HOST-1); Linux untested |
| `time` | p>g | The visitor's time | `SetTimeZoneInformation` (feature `time-zone`); `SetLocalTime` kept for QEMU and real machines | The zone through the optional root service (`/etc/localtime`) | — | None; the taskbar clock | `SetLocalTime` **NO-GO in v86**; the zone **GO** (measured in WP-HOST-1) |
| `power` | both | Guest shutdown notice; page asks for a clean shutdown or restart | `WM_QUERYENDSESSION`, `WM_ENDSESSION`; `ExitWindowsEx(EWX_SHUTDOWN \| EWX_POWEROFF)`, with `AdjustTokenPrivileges` for `SeShutdownPrivilege` found by `GetProcAddress` on NT and ReactOS | `SIGTERM` from init; `poweroff` through the root service | — | "The PC is shutting down"; **Shut down the PC** button beside Stop | GO on paper; the ACPI watcher (SITE-40) stays the proof of power-off |
| `pointer` | both | Where the guest's cursor is (to correct Follow), and a warp after a mode change | `GetCursorPos` when it changes, at most 30 a second; `SetCursorPos` | X11 `QueryPointer`, `WarpPointer`; or v86's VMware absolute mouse with the kernel's `vmmouse` support, no helper | — | None visible; the pointer module uses it | GO on paper; later (WP-KIT-9) |
| `audio` | both | The guest's mixer, devices, rate, microphone state, test sound | `winmm`: `mixerGetNumDevs`, `mixerOpen` with `CALLBACK_WINDOW`, `mixerGetLineInfoA`, `mixerGetLineControlsA`, `mixerGetControlDetailsA`, `mixerSetControlDetails`, `MM_MIXM_CONTROL_CHANGE`; `waveOutGetNumDevs`, `waveOutGetDevCapsA`, `waveInGetDevCapsA`; `PlaySoundA` with `SND_MEMORY` | ALSA control ioctls on `/dev/snd/controlC0` (`SNDRV_CTL_IOCTL_ELEM_READ`, `ELEM_WRITE`, `SUBSCRIBE_EVENTS`), `/proc/asound/card0/pcm0p/sub0/hw_params` and `status` | The card's own data only, page-side | Sound and Audio rows, the badge, Keep in step, Play a test sound | GO on paper; **ReactOS's mixer may be empty** (the investigation) |
| `sys` | both | Telemetry: memory, uptime, CPU idle, OS and helper versions, driver versions, the guest's clock | `GlobalMemoryStatus`, `GetTickCount`, `GetVersionExA`, `GetLocalTime`, `GetTimeZoneInformation`; `GetSystemTimes` by `GetProcAddress` (absent on Windows 95: no CPU idle there); driver versions from `WAVEOUTCAPS.vDriverVersion` | `/proc/meminfo`, `/proc/uptime`, `/proc/stat`, `uname` | — | The *System* row | GO on paper |
| `log` | g>p | The helper's own log lines | The helper and its modules | `hostlinkd`'s log | — | The *Log* row (last 200 lines, Copy) | GO on paper |

Device presence is not a channel of its own: the page knows whether the card is attached (`stats().registered`, `late`), and the guest says what it sees in `audio op=devices` (below). v86 has no PCI hot plug, so "attached" and "detached" are page states shown to the guest, never a device that appears and goes.

### The messages

`dir` as in the specification; `[key]` is optional; `pct` is 0 to 100; `db10` is tenths of a decibel as a signed decimal (the only signed number in the kit, written with a leading `-`, which is a value byte the body allows).

| Channel | `op` | dir | Keys and data |
|---|---|---|---|
| `power` | `ending` | g>p | `kind` (`shutdown`, `logoff` or `end`: Windows 9x cannot tell shutdown from restart) |
| `power` | `shutdown`, `restart` | p>g | none; answered with `ending` or `ctl op=error code=refused` (a new code, see below) |
| `pointer` | `at` | g>p | `x`, `y` u32 (screen pixels) |
| `pointer` | `warp` | p>g | `x`, `y` u32 |
| `audio` | `devices` | g>p | `out`, `in`, `mixers` u32 (counts); [`outname`, `inname`: the first device's name]; [`driver`: `MAJOR.MINOR`] |
| `audio` | `mixer` | g>p | [`master`, `wave`, `mic`, `rec` pct]; [`mastermute`, `wavemute`, `micmute` `0` or `1`]; [`lines` u32: how many lines the mixer has, `0` for an empty one]; `source` (`user` or `self`) |
| `audio` | `set` | p>g | [`master` pct], [`mastermute` `0` or `1`]; answered with `mixer` (`source=self`) or `ctl op=error code=refused` |
| `audio` | `play` | p>g | `sound` (`test`: a 1 kHz tone of 1 s at 22,050 Hz the helper makes in memory; `asterisk`, `exclamation`, `hand`: the system sounds) |
| `audio` | `played` | g>p | `sound`, `ok` (`0` or `1`) |
| `audio` | `host` | p>g | `output` (`on`, `off`, `suspended`, `muted`), `mic` (`off`, `on`, `denied`, `unavailable`), `volume` pct |
| `audio` | `stream` | g>p | Linux only: `rate` u32, `format` (`S16_LE` and so on), `period` u32, `state` (`RUNNING`, `XRUN`, …), [`program`: the owner's `comm`] |
| `sys` | `get` | p>g | none |
| `sys` | `info` | g>p | `os` (`ReactOS 0.4.16`, `Windows 4.0.950`, `Linux 6.6.0`), `app`, `uptime` u32 (seconds), `memtotal`, `memfree` u32 (KiB), [`idle` pct over the last second], `clock` (`YYYY-MM-DDTHH:MM:SS`), `bias` (minutes, signed), [`zone`] |
| `log` | `line` | g>p | `level` (`error`, `warn`, `info`), `src` (`hostlink`, `audio`, …); data: the line |
| `log` | `level` | p>g | `level` |

`ctl op=error` gains the code **`refused`** (the request was understood and declined: no privilege, no mixer, the channel turned off in the guest), additive to the five codes.

### What each guest family gets

| Family | Channels | Helper |
|---|---|---|
| ReactOS, Windows 95 to XP | All; `sys` without `idle` on Windows 95 and 98; `audio` only once the guest has a driver for the card (ReactOS: built in; Windows 98, Me, 2000: a vendor driver the user brings; Windows 95: none, AC'97 NO-GO) | `HOSTLINK.EXE` with `HLAUDIO.DLL`, `HLSYS.DLL`, `HLTRAY.EXE` |
| Linux under X11 | All; `audio stream` with the playing program | `hostlinkd` with its `audio`, `sys`, `power`, `log` modules (Rust features, one binary) |
| Linux, terminal only | No helper: the page's card telemetry; a headless `hostlinkd --no-x11` on `ttyS1` gives `sys`, `audio`, `power` and `log` if the image carries it | Optional |
| Windows 3.x | `clip`, `file`, `sys`, `log`, `power` (`ExitWindows`); no `audio` (no AC'97 driver; MMSYSTEM has no mixer API) | `HOSTLNK.EXE` (WP-HLG-6), later |
| DOS (js-dos page) | None of the link's; the DOS page bridge's disks (SITE-60) | — |

## Audio and device telemetry

### Where each fact comes from

| Fact | From the card, page-side (no helper) | From the guest helper | Shown |
|---|---|---|---|
| Card present, slot, late attach | `stats().registered`, `pciId`, `late` | — | *Sound* row |
| The guest's chosen rate and format | `stats().guestRate` (`0x2C` with variable rate audio); always 16-bit stereo on PCM out | Linux `hw_params` confirms | *Audio* row: "22,050 Hz, 16-bit stereo" |
| Playing now, interrupt | `stats().playing`, `interrupt` | — | *Audio* row |
| Buffers done, frames, starved frames | `counters.buffersDone`, `dmaFrames`, `starvedFrames` | — | *Audio* row |
| Output underruns, dropped, silent | `sinkStats()`: `underrunEvents`, `underrunFrames`, `dropped`, `silent`, `peak` | — | *Audio* row |
| Guest master, PCM and record volume and mute | **The codec's registers** (`0x02`, `0x18`, `0x1C`, `0x0E` mic, `0x1A` record select), which the guest's driver writes | The mixer as the guest's Volume Control shows it (`audio mixer`), which may differ when the driver keeps a software volume | The badge; *Audio* row "PC: master 80 percent, wave 100 percent" |
| Which program plays | — | Windows: **NO-GO** (no API names the process holding a wave device before Vista's sessions); Linux: the `status` file's `owner_pid` and `/proc/PID/comm` | *Audio* row on Linux only |
| The guest's devices and driver | — | `audio devices` | *Audio* row: "The PC sees AC'97 Sound Card (driver 5.1)" |
| Microphone state | The page's own (`onMicrophone`, SITE-103) | — | The page's indicator; told to the guest by `audio host` |

So **WP-KIT-1 needs no guest change at all**: two small additions to the `ac97` package (a `codec()` snapshot and an `onCodec` callback when the guest writes a mixer register) and the rows. It is also the tool the mixer investigation lacks: whether the driver writes the codec's volume registers when the visitor moves the guest's slider is visible at once.

### The badge

The codec powers on with master and PCM out muted, and a guest that never unmutes them is silent however loud the page's slider is. When the card plays and master or PCM out is muted, the player row shows **Muted in the PC** beside the Sound button. With the `audio-set` feature it is a button that sends `audio set mastermute=0`; without a helper it is a hint ("Unmute in the PC's Volume Control").

### Beeps

The page passes `disable_speaker: true`, so a guest's PC speaker beep (`Beep`, or `MessageBeep` on a guest without a sound driver) is silent today. v86 emits `pcspeaker-update` on its bus; the glue could play it through the card's output node (a square wave at the PIT's frequency, under the same volume and mute). **Later and optional**: ReactOS with the card plays its system sounds as waves, and whether v86 still emits the event with the speaker disabled is not verified. The page never routes the visitor's own notification sounds into the guest.

## Volume in step

Two gains are in series: the guest's (the codec registers, applied in `Ac97.gains()`) and the page's (the sink's `GainNode`, from the slider). The kit keeps them distinct:

| Mode | The page's slider sets | The guest's volume | Who can change what |
|---|---|---|---|
| **Separate** (default) | The speaker gain, as today | Shown read-only in the *Audio* row; the badge when muted | The visitor moves the page's slider; the guest's Volume Control moves the guest's |
| **Keep in step** (needs `audio` and `audio-set`) | The guest's master, through `audio set`; the speaker gain is held at 1 | Follows the slider; a change in the guest's Volume Control (an `audio mixer source=user`, or a codec write the page sees) moves the slider | Either side; the last change wins |

- **No loops.** The page ignores a `mixer` with `source=self` (the helper's own write) and never answers a `mixer` with a `set`. The page compares a codec write with its last `set` within 500 ms and treats an equal value as its own echo. The helper stamps writes it made itself, as it does for the clipboard.
- **Mute stays the visitor's.** The Sound button always mutes the speaker gain, in either mode, at once and without the guest; Keep in step never unmutes the speaker because the guest did.
- **The taper.** The page's slider is perceptual (the gain is the square); the guest's mixer is linear in the mixer API's 0 to 65,535 range and the codec steps 1.5 dB. The glue maps slider percent to the mixer's unsigned value through the same square, so 50 percent sounds the same in both modes.
- **Without a mixer** (an empty Volume Control: `audio mixer lines=0`), Keep in step is greyed with "The PC's sound driver has no volume control". Writing the codec's registers from the page behind the driver's back is **NO-GO**: drivers keep a shadow copy of the codec's registers and rewrite it, and the guest's own Volume Control would disagree with what plays.

## How the sound card and the link meet

The page glue is the only place that knows both. Neither package imports the other, so the card stays a plain package of `v86-devices` and the link a plain package of `v86-host-link`, each in its own repository.

```text
attachAc97(emulator, { …, onChange, onCodec })        attach(emulator) → link
          │                                                    │
          ▼                                                    ▼
   kit.js ── card.stats(), card.sinkStats(), card.codec()   link.on("message"), link.send(…), link.stats().peer.caps
          │  rows: Sound, Audio, System, Log
          │  badge, Keep in step, Play a test sound, Insert the kit CD
          └─ sends audio host on every change of output, mic or volume; audio set in step; sys get while the panel is open
```

- **The `ac97` package gains** (additive, inside its budget): `codec(): { master, pcm, record, mic, recordSelect, rate, vra }` (raw words, and a decoded `pct` and `muted` per line using the codec's 1.5 dB steps), and an `onCodec?: (register: number, value: number) => void` option called after a guest write to a mixer register (`0x02` to `0x1E`), at most once per register per 50 ms. No v86 change.
- **The `host-link` package gains** (WP-KIT-2): the token parser (`parseCaps`, `hasChannel(peer, "audio")`, `channelVersion`, `hasFeature`), the new channel names and the `refused` code. No knowledge of the card.
- **`kit.js`** (new, `site/try-reactos/kit.js`, beside `sound.js`, `host-link.js` and `power.js`) receives both handles from `try-reactos.js` and owns the Diagnostics rows *Audio*, *System* and *Log*, the badge, the Keep in step switch, the test sound button and the kit CD button. Its rules are tested in a small pure module (`kit/rules.ts` in the page package, or plain functions in `kit.js` with a `bun test` beside it) with fake handles. In `v86-host-link` the same glue is the demo page's example of "using the card and the link together" (the demo loads the `v86-ac97` module beside it when it is there), documented in its README, still not a dependency of either package.

## The sound card tool in the guest

**`HLTRAY.EXE`**, started from the Startup entry after `HOSTLINK.EXE`, a client of the helper's broker (below), and the kit's visible face in the guest.

- **The tray icon.** `Shell_NotifyIconA` with four states: connected (green), connecting or no page (grey), the page's microphone **on** (red dot), a channel error (amber). The tooltip: "Host link connected: clipboard, files, audio" (the channels both sides listed). After Explorer restarts it re-adds the icon on the `TaskbarCreated` message (`RegisterWindowMessageA`).
- **Its menu.** Host link status…, Sound card…, Insert into the clipboard log (off), About the Host Link Kit, Exit.
- **The Sound card window.** One small dialog from a resource, refreshed twice a second while open: the card (name and driver from `audio devices`), the guest's rate, format and whether it plays, buffers done and starved frames, the page's output state and underruns (sent by the page in `audio host` and a new `audio stats` message the page sends only while this window is open, at most twice a second), the microphone's state ("The page's microphone is off"), the guest's master and wave volume, and **Play a test sound**, which plays a 1 kHz tone through `PlaySoundA(SND_MEMORY | SND_ASYNC)` so the whole chain (the guest's driver, the card, the page's output) is tested from inside the guest. The page counts the tone's frames and pitch in its *Audio* row.
- **Not in it.** No volume slider of its own (Volume Control is the guest's), no recording, no settings that change the guest.
- **Size.** At most 16 KiB with its icons (four 16 by 16 icons at 16 colours are under 1.5 KiB) and one dialog template; imports from `KERNEL32`, `USER32`, `SHELL32` (`Shell_NotifyIconA`) and `GDI32`, all on Windows 95's own DLLs.
- **A Control Panel page** (`HLKIT.CPL`, a DLL exporting `CPlApplet`) is the later swap: the same dialog, found where Guest Additions' users expect settings. Not first: a `.CPL` has a third export that `check-dll.sh` does not allow today.
- **Linux.** No tray tool in the first Linux kit (the window managers in a small image have no tray); `hostlinkd --status` prints the same facts in a terminal.

## A modular guest helper

### Why not separate helpers

Only one process can open COM1, and the link is one stream with one credit account. A second program cannot speak to the page by itself; it must go through the owner of the port. So the choices are where channel code lives and how other programs reach it:

| Shape | For | Against | Verdict |
|---|---|---|---|
| Everything in `HOSTLINK.EXE` | One file, nothing to load | Over the 24 KiB budget with `winmm` and `shell32` imports and three channels (about 9 to 14 KiB more [E]); every guest installs everything | No |
| **Channel modules as DLLs**, loaded by `HOSTLINK.EXE` through `quill-w32rt`'s loader | The core stays small; a missing module is a channel not listed in `caps`; the loader, the build-id check and the DLL checks exist (pull request 215) | A module's panic is the helper's (as in Quill); one more ABI table | **Yes, for headless channels** (`HLAUDIO.DLL`, `HLSYS.DLL`) |
| **A local broker**: other programs register a window with the helper by `WM_COPYDATA` and receive the channels they ask for | Tools are ordinary programs with their own windows and budgets; a crash does not take the link down | Messages cross a process boundary (cheap on Windows: `WM_COPYDATA` copies) | **Yes, for tools with a window** (`HLTRAY.EXE`; Quill's own Print to PDF hand-off is already this shape) |
| One DLL per channel *and* the tray in the helper | — | The tray needs `SHELL32` and a dialog in the helper's process | No |

### The module contract (ARCH-50)

The modules use `quill-w32rt`'s runtime, entry stub and loader unchanged, with a contract of their own beside Quill's: `quill-modabi`'s kinds are Quill's features (pictures, PDF, dialogs), and the host link is not Quill.

- **A crate `hostlink-modabi`** (`no_std`, `#[repr(C)]`, layout tests): `ChanInfoV1 { size, abi_major, abi_minor, build_id, name: [u8; 16], version: u16, features: [u8; 64] }` and `ChanTableV1 { size, start(host: *const HostV1) -> Status, message(op, fields, data, len) -> Status, window(msg, wparam, lparam) -> Status, tick(now_ms) -> Status, stop() }`, where `HostV1` is what the helper offers: `send(channel, op, fields, data, len) -> Status`, `log(level, text)`, `hwnd` (the helper's hidden window, for `CALLBACK_WINDOW`), `now_ms`. Two exports by name, `HostLinkModuleInfo` and `HostLinkModuleStart`, as Quill's modules export two.
- **Found** as `HL*.DLL` beside `HOSTLINK.EXE` (full path, never a bare name), at start only; a refused module is freed and its channel is not listed. The `mods` key in `hello` names what loaded.
- **Bases** `0x62000000` (`HLAUDIO`), `0x62100000` (`HLSYS`), clear of Quill's `0x61…` modules and the savers' `0x63000000`.
- **Called** only from the helper's one thread; a module starts no thread and allocates nothing (its working memory is its own `.bss`).

### The broker

- A tool finds the helper with `FindWindowA("LHQHostLink", NULL)` and sends `WM_COPYDATA` with `dwData = 0x484C4B01` ("HLK" and 1) and a body in the link's own body format (`op=subscribe`, `channels=audio,ctl`); the helper sends that window a copy of each message on those channels, and the page's `audio host` and `audio stats`, as `WM_COPYDATA` with `dwData = 0x484C4B02`. A tool may send `op=send` with a channel and a body to the page, but only on channels its subscription names and never `clip` (the clipboard stays the helper's own).
- The helper drops a subscriber whose window is gone (`IsWindow`), keeps at most four, and tells them `op=link state=up|down caps=…` when the page's `hello` comes or goes.
- Cost in the helper: about 600 to 900 bytes [E]; `IsWindow` is the one new import.

### What the core costs

| Item | Bytes [E] |
|---|---|
| `HOSTLINK.EXE` today | 22,016 (measured) |
| The module loader (`quill_w32rt::module`: path, `LoadLibraryA`, `GetProcAddress`, `SetErrorMode`, `GetLastError`, `FreeLibrary`, the info check) | 1,000 to 1,600 |
| The broker | 600 to 900 |
| The caps tokens and the new channel names (in `quill-hostlink` and the helper) | 300 to 500 |
| **Total** | **about 24,000 to 25,000**: over the 24 KiB budget by up to 1 KiB |

So WP-KIT-3 either raises the budget to **26 KiB** (the default, OWNER DECISION) or moves `time` and `display` out of the core into `HLSYS.DLL`, which frees about 1.5 to 2.5 KiB [E] and makes the core clipboard, files and the loader only.

## The kit's shape and name

### Names

| Level | Name | Where it comes from |
|---|---|---|
| What the visitor and the guest's user see | **Host Link Kit** ("guest tools, from the host's side") | The owner's name; on the disc label, the setup's title, Add/Remove Programs (`Liminal Host Link Kit`), the Start menu group and the tray tooltip |
| The repository | **`v86-host-link`**, its own repository, local at `~/source/liminal-hq-labs/v86-host-link` (later `liminal-hq-labs/v86-host-link` on the owner's go) | The owner, 2026-11-28 (SITE-129); replaces "a package of `v86-devices`" in SITE-89 and SITE-100 |
| The page library | `v86-host-link` (npm name owner-gated) | SITE-89 |
| The Rust crates | `host-link-wire` (from `quill-hostlink`), `host-link-guest` (from `quill-hostlink-guest`), `hostlink-modabi` (new) | This plan |
| The kit | `kit/` in the same repository: the installer projects, the Linux install script, the disc's layout and its build script | This plan |

Not a separate kit repository: the kit is how `v86-host-link`'s helpers reach a guest, and a repository of only installers would version apart from the helpers it installs. The layout is in [The `v86-host-link` repository](#the-v86-host-link-repository).

### One bundle per guest family

| Family | Bundle | What it installs | Start-up | Uninstall |
|---|---|---|---|---|
| Windows 95 and later, ReactOS | `WIN32\SETUP.EXE`, a Liminal Installer project (`win32-classic`, the Win32 runtime) | Component **Host link** (required): `HOSTLINK.EXE`; **Sound card and audio** (default on): `HLAUDIO.DLL`; **System information, power and log** (default on): `HLSYS.DLL`; **Tray icon and sound card tool** (default on): `HLTRAY.EXE`; a Start menu group (Host Link status, Sound card, Uninstall) | A Startup-folder shortcut for `HOSTLINK.EXE` and one for `HLTRAY.EXE` (see the installer gap below) | Add/Remove Programs; `SETUP.EXE` asks `HOSTLINK.EXE /quit` first so its files are not in use |
| Linux | `LINUX/hostlink-kit.tar.gz` with `install.sh` | `hostlinkd` (static i686) to `/usr/local/bin`, its OpenRC service and an `~/.xinitrc` line, the `lpr` shim, the optional root service for `time` and `power` | OpenRC, or the `.xinitrc` line | `install.sh --remove`, which reads the list it wrote |
| Terminal guests | None | — | — | — |
| Windows 3.x (later) | `WIN31\SETUP.EXE`, the Windows 3.1 runtime | `HOSTLNK.EXE` (WP-HLG-6) | The StartUp group | The kept uninstaller |

**The installer gap.** Liminal Installer's Win32 runtime makes Start menu shortcuts but not Startup ones, has no general `registry.set` (only its own Add/Remove key) and its "Launch after setup" box does nothing. The kit needs **one** of: a shortcut place `startup` (a `.lnk` in the Startup folder, recorded as an `S` record like any shortcut, the Windows 3.1 runtime's StartUp group for the same name), or `registry.set` for `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. The Startup shortcut is the default (INS-30): it is visible to the person, undone by the existing `S` record, and the same word works on Windows 3.1. It is a change in `liminal-hq-labs/liminal-installer`, planned there, which WP-KIT-7 waits for; until it lands, the kit's setup writes nothing to start the helper and its last page says "Restart the PC, or run Host Link from the Start menu". The QAGRR disk keeps its own Run value (`qagrr/quill-run.reg`) and never runs the kit's setup.

### The Host Link Kit CD

- **One ISO** (`hostlink-kit.iso`, ISO 9660 with Joliet, under 1.44 MB so the same files also make a floppy image), built by `kit/build-disc.sh` from the helpers' release builds: `AUTORUN.INF` (`open=WIN32\SETUP.EXE`, `icon=WIN32\HLKIT.ICO`), `WIN32\`, `LINUX\`, `README.TXT`, `LICENSE.TXT` and the source offer.
- **The page mounts it.** A **Insert the Host Link Kit CD** button in Diagnostics calls `emulator.set_cdrom({ url: "kit/hostlink-kit.iso" })`; **Eject** calls `eject_cdrom()`. The ISO is served beside the page, fetched only on the click, and never uploaded. The page already has a CD-ROM field for a disc of the visitor's own.
- **Auto-run.** Windows 95 to XP run `AUTORUN.INF` when a disc arrives; ReactOS 0.4.16's AutoPlay is not verified, so the button's hint says "Open the CD in My Computer and run SETUP.EXE" until a check shows auto-run works. "Auto-run on first boot" in the strict sense (a fresh install that finds the disc in the drive) is the same `AUTORUN.INF` on Windows; on Linux it is the image builder's job (WP-HLG-4 installs `hostlinkd` itself).
- **Liminal Installer's media** (`[[media]]` with `kind = "cdrom"`, `autorun`) can build the Windows half of the disc; the Linux files are `extra` files on the medium. Using it is the default; a plain `xorriso` script is the fallback if `li-media` cannot add a second family's folder.
- **A disc label.** The printable media set (INS-23 to INS-25: floppy labels and CD faces drawn as vector masters by `assets/media/gen_media.py`) could draw a **Host Link Kit** disc face and floppy label in the same style: a fun extra for the owner, not a package here.

## The `v86-host-link` repository

**Decided by the owner, 2026-11-28:** the host link and its kit are their own repository, `v86-host-link`, built locally on its `main` at `~/source/liminal-hq-labs/v86-host-link` before it ever reaches GitHub: no remote, no push, no release, no npm publish and no Pages deploy until the owner says. The CI files are written and kept ready, not run on GitHub.

### Layout

```text
v86-host-link/
├── README.md               the pitch, the ten-line page example, the guests table, Status: experimental
├── SPEC.md                 the protocol (today's docs/host-link.md) with every channel, caps tokens, versions
├── AGENTS.md, CLAUDE.md    the house rules (CLAUDE.md a summary of AGENTS.md)
├── LICENSE-APACHE, LICENSE-MIT, PROVENANCE.md, CREDITS.md
├── assets/hero.svg         the hero banner master (its own palette; GNOME-style chrome: a page and a guest
│                           window joined by a cable), rendered down by scripts/render-hero.sh
├── docs/                   decisions.md (prefixes below), progress.md, how-it-works.md, guests.md,
│                           privacy.md, conformance.md (how to test a new helper), kit.md, releasing.md,
│                           migration-from-quill.md, one page per helper
├── vectors/                frame and message vectors, gen_vectors.py (standard library only), --check
├── sessions/               conformance sessions, gen_sessions.py, tools/hostlink-conformance/
├── page/                   the TypeScript page library: frame, message, flow, link, caps, transport, clip
│   ├── src/                (state, rtf, dib, browser), files, print, audio, sys, power, log, pointer
│   ├── test/               bun test, the vectors, the privacy tests
│   └── dist/               the committed built module (drift-checked), host-link.js and .d.ts
├── crates/                 Cargo workspace, rust-toolchain.toml pinned as Quill's
│   ├── host-link-wire/     from quill-hostlink: no_std, no alloc, no panics
│   ├── host-link-guest/    from quill-hostlink-guest: the helper's decisions, Port and Clipboard traits
│   └── hostlink-modabi/    the channel module contract
├── helpers/
│   ├── win32/              HOSTLINK.EXE, mods/hlaudio, mods/hlsys, tray/ (HLTRAY.EXE), HLCLIP.EXE (test),
│   │                       its own target spec, build.sh, the import allowlists, check-dll.sh, the Win32
│   │                       runtime (a pinned copy of quill-w32rt with PROVENANCE.md, see below)
│   ├── linux/              hostlinkd (Rust std, static i686 musl)
│   └── win16/              later: HOSTLNK.EXE
├── kit/
│   ├── win32/              the Liminal Installer project (installer.toml, components, flows), liminst.rev
│   ├── linux/              install.sh
│   └── build-disc.sh       hostlink-kit.iso and the floppy image
├── demo/                   a static page: v86 with a guest and the link, the Diagnostics rows; it can take
│                           the v86-ac97 module beside it to show the audio glue (no dependency)
├── harness/                headless Chromium and Firefox checks, the Wine and QEMU runs
└── .github/workflows/      one job per check, ready and unpushed
```

**The Win32 runtime.** The helpers need `quill-w32rt` (the no-CRT runtime, the DLL entry and the module loader of pull request 215). It is Quill's crate; `v86-host-link` carries a pinned copy under `helpers/win32/w32rt/` with a `PROVENANCE.md` (Quill's commit and the files' SHA-256), refreshed by one script, as Quill's `dos/` copies Desk's modules. The owner may later move it to a shared Labs crate; that is outside this plan.

### Decision prefixes in the new repository

The lead keeps `docs/decisions.md` with numbered rows, one prefix per area and one line of why: **`HL-`** the protocol and the vectors, **`PAGE-`** the page library, **`GUEST-`** the Rust guest core and the module contract, **`WIN-`** the Win32 helpers, **`LNX-`** the Linux helper, **`KIT-`** the installers and the disc, **`CI-`** the checks. The first rows restate the Quill decisions the repository inherits, citing them (SITE-30, SITE-31, SITE-52, SITE-80 to SITE-82, SITE-91, SITE-120 to SITE-129, ARCH-50, INS-30), so the new repository reads on its own.

### How it relates to `v86-devices`

- **`v86-devices`** (pull request 214) keeps `v86-device-kit` and `v86-ac97` (and later `v86-usb`). **`v86-host-link` is separate** and has **no package dependency** on either, in either direction.
- The two meet only in a page: the page creates the card with `attachAc97(emulator, { onChange, onCodec })` and the link with `attach(emulator)`, and its own glue (Quill's `kit.js`, or `v86-host-link`'s demo page) joins them. The audio channel's message shapes are in `v86-host-link`; the card's data are read from the card by the page.
- **Where they would touch:** the USB CDC serial and printer devices of `v86-usb` (WP-USB-3, WP-USB-4) are transports into the link and its print jobs. They stay in `v86-devices`; `v86-host-link`'s `Transport` seam takes them as one more transport the page passes in.

## Migration from Quill

Quill keeps the source of truth until the new repository can build and test the same bytes; then Quill vendors the built results back, as the AC'97 plan does for `v86-ac97` (SITE-104).

| What | In Quill today | Moves to `v86-host-link` | Quill keeps |
|---|---|---|---|
| The specification | `docs/host-link.md` | `SPEC.md` | A short page that links to the vendored copy's version |
| The vectors and sessions | `tests/host-link/` | `vectors/`, `sessions/` | Nothing (the vendored module is checked by hash) |
| The page library | `site/try-reactos/packages/host-link/` (and `src/clip/` from pull request 204) | `page/` | `site/try-reactos/vendor/v86-host-link/`: `host-link.js`, `host-link.d.ts`, `PROVENANCE.md` (the local commit, later the tag, the SHA-256) |
| The codec | `crates/quill-hostlink` | `crates/host-link-wire` | Nothing once `HOSTLINK.EXE` is vendored (the crate has no other user in Quill; checked before removal) |
| The helper's decisions | `crates/quill-hostlink-guest` (pull request 200) | `crates/host-link-guest` | Nothing |
| The Win32 helper | `win9x/hostlink/` (pull request 200) | `helpers/win32/` | `qagrr/` takes the vendored `HOSTLINK.EXE` (and the modules, the tray) with `PROVENANCE.md` and a hash check in `qagrr/qagrr.toml`'s build |
| The page's glue | `host-link.js`, `sound.js`, `power.js`, `kit.js` | Nothing: the glue is Quill's page | All of it |
| Quill's own rich clipboard wiring | WP-ED-6 (Quill's RTF on Windows 3.x and 95), WP-HOST-7 (Quill's rich clipboard end to end), Quill's `[Web] HostPort=LINK` hand-off | Nothing | **All of it**: it is Quill's behaviour, and it talks to the helper only through the clipboard and `WM_COPYDATA` |
| The harness | `harness/check-host-link.ts`, `host-link-boot.ts`, `host-link-disk.sh` | Their protocol and helper checks | The page and state checks (`check-state.ts`) |

**Steps.**

1. **Seed** the repository from Quill at a named commit after pull requests 200 and 204 merge, with history (`git filter-repo` over the paths above into the new layout), so blame survives. Before that, the lead may start from the open branches and rebase.
2. **Prove the same bytes.** `HOSTLINK.EXE` built in the new repository has the same normalised digest as Quill's (`win9x/tools/pe-digest.py`'s method); the page module passes the same vectors and Quill's `check-host-link.ts` with the built module swapped in.
3. **Vendor back.** One Quill pull request replaces the moved paths with `site/try-reactos/vendor/v86-host-link/` and the helper binaries under `qagrr/` with provenance, a hash check in CI and a `tools/fetch-v86-host-link.sh <commit>` that copies from the local repository (from a release asset once one exists). The Quill Remix picks up the kit's helpers through that.
4. **Afterwards,** fixes go to `v86-host-link` first and come back by a refresh; Quill never edits the vendored files.

Until step 3, packages that change the host link's code (WP-HLG-2, WP-KIT-2 onward) run in `v86-host-link`, not in Quill; only WP-KIT-1 (the card's telemetry, page-side) and the launch packages (WP-HOST-5 to WP-HOST-8) run in Quill, and the lead seeds from whatever has merged.

## Edits to the other plans

Pull requests 213 (merged) and 214 (open) say `v86-host-link` is a package of `v86-devices`. The exact edits:

| File | Where | Now | Becomes |
|---|---|---|---|
| `docs/plans/host-link-guests.md` (on `main`) | *Recommendation*, row "Package boundary" | "A family of small packages named `v86-*` … in one Labs repository if the AC'97 plan picks one, otherwise their own" | "**`v86-host-link`, its own repository** (decided by the owner, 2026-11-28, SITE-129), beside `v86-devices` (`v86-device-kit`, `v86-ac97`); see [host-link-kit.md](host-link-kit.md#the-v86-host-link-repository)" |
| the same | *The package boundary*, the paragraph "If the AC'97 reuse plan … chooses one repository for the family …" | The host link joins `v86-devices` under its layout | "The owner chose a repository of its own for the host link (SITE-129); `v86-devices` and `v86-host-link` share the layout, licence and banner style but not a repository or a dependency. The layout is in host-link-kit.md." |
| the same | *Decisions to record*, SITE-89's row | "The package family's name and repository" | "The package family's name and repository: **`v86-host-link`, its own repository** (SITE-129)" |
| the same | *Owner decisions*, row "The package name and repository" | "`v86-host-link`, beside `v86-ac97`; one family repository if the AC'97 plan prefers it" | "**Decided**: `v86-host-link`, its own repository, local until the owner's go (SITE-129)" |
| the same | WP-HLG-8 | "Extraction into `v86-host-link` … `packages/v86-host-link/` in this repository is the new repository's tree" | "Done by the Opus lead in `~/source/liminal-hq-labs/v86-host-link` ([host-link-kit.md](host-link-kit.md#migration-from-quill)); no `packages/v86-host-link/` tree in Quill" |
| `docs/plans/v86-ac97-reuse.md` (pull request 214) | *Recommendation* item 3 and *The package boundary and the repository*, the paragraph "**The host link joins the same family.**" | `v86-host-link` is another package of `v86-devices`, at `packages/host-link` | "**The host link is its own repository.** The owner chose `v86-host-link` as a repository of its own (SITE-129). It uses no part of the device kit and has no dependency on `v86-ac97`; a page joins the two in its own glue. The USB CDC serial and printer devices (WP-USB-3, WP-USB-4) stay here and are transports a page hands to the link." |
| the same | The repository's description | "Virtual hardware and host integration for the v86 emulator" | "Virtual hardware for the v86 emulator" |
| the same | SITE-100's row in its `docs/decisions.md` | "… with `v86-device-kit` and `v86-ac97` as its first packages and `v86-host-link` joining" | "… with `v86-device-kit` and `v86-ac97` as its first packages; `v86-host-link` is its own repository (SITE-129)" |

The first table's edits are in this pull request (the file is on `main`). The second table's are for pull request 214's author or the orchestrator to make on `docs/ac97-reuse-plan` before it merges.

## Versions and compatibility

- **Protocol 1 throughout.** Every change in this plan is a new token, a new channel name, a new key or a new error code; no frame, body or credit rule changes. `proto` stays `1`.
- **Older helpers keep working.** A page with the kit and `HOSTLINK.EXE` 0.1.0 (pull request 200) sees `caps=clip,file,time,display` and uses only those; the kit's rows show "Not offered by this helper" and the page-side card telemetry still shows. A kit helper and an older page see the page's old `caps` and send nothing new.
- **Missing modules** are channels not listed; a module from another build is refused and logged (`QS_VERSION` from the loader), never half-used.
- **Channel versions.** A message change that an older reader would misread is a new channel version (`audio.2`); a new optional key or `op` within a version is a feature token. Each channel's table in `docs/host-link.md` says which version added each row.
- **The conformance material.** WP-KIT-2 adds frame and message vectors for each new channel and the `refused` code, and caps-negotiation cases to the conformance sessions WP-HLG-2 builds: a page with and without each token, a helper with and without each module, a `.2` token on one side only, a token list at the 256-byte limit, an unknown token, and a channel turned off mid-session by a new `hello`.

## Privacy

The host link's rules (the host integration plan's and the guests plan's) hold; the kit adds:

1. **Every channel is the visitor's choice on the page.** The Diagnostics panel has a switch per channel group (Clipboard, Files, Audio, System and log, Power, Pointer); a switch off removes the token from the page's `caps` and sends a new `hello`. Defaults: Clipboard, Files, Power on (they need a gesture anyway); **Audio on** (it carries only levels and device names); **System and log on, but asked only while the Diagnostics panel is open**: the page sends `sys get` once a second while it is open and never otherwise, and `log` lines are kept only while it is open.
2. **The guest sees what the page shares.** The tray tooltip lists the channels in use; the microphone's state is shown in the guest by the tray icon whenever the page's microphone is on, so a program in the guest that records knows the visitor can hear it as much as the visitor knows the guest can hear them.
3. **Nothing leaves the tab.** No channel's data is uploaded, stored beyond the tab or sent anywhere but the guest and the panel; the network-free scan stays a test. Log lines are copied only by the visitor's **Copy** button.
4. **No guest can read the host.** No kit message asks the page for anything the visitor has not chosen to send: `audio host` carries the page's output and microphone states and the slider's level, never a device name, a microphone sample or the host's volume settings; `sys` goes guest to page only.
5. **The guest cannot act on the host.** `power` from the guest is a notice; the page never closes or reloads itself because of one. `audio set` and `power shutdown` go page to guest only, from the visitor's click or the Keep in step switch.
6. **Which program plays** (Linux only) is shown in the panel, never logged by default.

## Testing

| Level | What runs | Checks |
|---|---|---|
| 1. Units | `bun test` for the page package (caps tokens: parse, negotiate, version choice, features; the new channels' vectors), the `ac97` package (`codec()` decoding every step, `onCodec` debouncing, nothing new allocated per sample), `kit.js`'s rules (the badge, Keep in step's echo rules, the panel's request cadence, the switches' `caps`); `cargo nextest` for `quill-hostlink` (the names, the vectors), `hostlink-modabi` (layouts) and `quill-hostlink-guest` (caps and the broker's decisions) | The shared vectors; Keep in step never loops on 1,000 seeded sequences of user, self and codec changes |
| 2. Conformance sessions | WP-HLG-2's sessions plus the caps cases above, against `HOSTLINK.EXE` with and without each module under Wine | Every helper passes every session its `caps` cover |
| 3. Guest | ReactOS 0.4.16 in v86 in headless Chromium with the QAGRR disk and a kit build on a second disk or the kit CD; QEMU for `SetLocalTime` and `ExitWindowsEx` | `audio devices` and `mixer` arrive; moving the guest's master slider (through the guest's own Volume Control, driven by v86's keyboard API) moves the *Audio* row; `audio set` changes Volume Control; Play a test sound is counted at the probe (frames and pitch within 0.5 percent); the tray icon is drawn (a canvas crop); `power shutdown` ends in the ACPI write; the setup installs, starts after a restart and uninstalls |
| 4. The page | Headless Chromium and Playwright's Firefox | The Diagnostics rows with and without a helper; the badge on a muted guest; every switch changes the `hello`; the kit CD mounts and ejects; no request leaves the origin |
| 5. The owner | Real speakers | The test sound, Keep in step, the badge |

Every browser headless, every server on `127.0.0.1` with a PID file and never on ports 8095 to 8098, `DISPLAY` set explicitly with `WAYLAND_DISPLAY` unset in every command that starts Wine, Xvfb or QEMU, every process stopped by its own PID.

## Budgets

| What | Budget |
|---|---|
| `HOSTLINK.EXE` | 26 KiB (raised from 24 KiB, owner decision), or 24 KiB with `time` and `display` moved to `HLSYS.DLL` |
| `HLAUDIO.DLL` | 8 KiB; imports `WINMM` (`mixer…`, `waveOutGetNumDevs`, `waveOutGetDevCapsA`, `waveInGetNumDevs`, `waveInGetDevCapsA`, `PlaySoundA`) and `KERNEL32` only |
| `HLSYS.DLL` | 8 KiB |
| `HLTRAY.EXE` | 16 KiB with icons |
| The kit's Windows setup | Liminal Installer's `SETUP.EXE` (about 130 KB) and the files; the whole disc under 1.44 MB |
| The page package | Under its 40 KB minified budget with the caps parser and the new names (at most 1 KB added) |
| The `ac97` package | `codec()` and `onCodec` within its 36 KB budget (at most 600 bytes added) |
| `kit.js` | At most 8 KB unminified, no dependency |
| The page's work | `sys get` at most once a second, only while the panel is open; the *Audio* row redrawn at most twice a second; no timer at all while the panel is closed and Keep in step is off |
| The guest's work | No polling: the mixer through `MM_MIXM_CONTROL_CHANGE`, the pointer only on change, `sys` only on request |

## The first demo

| Item | First demo | Why |
|---|---|---|
| The card's telemetry rows (*Audio*: rate, format, playing, buffers, starved frames, underruns; the guest's master and PCM volume and mute from the codec) | **Yes, if WP-KIT-1 is ready before WP-WEB-8** | Page-only, small; it also serves the mixer investigation |
| **Muted in the PC** badge (hint form, no helper) | **Yes**, with WP-KIT-1 | Turns the most likely "no sound" into a sentence |
| Anything needing a new helper, module, codec change or guest disk change | No | The launch's disk and state are fixed by WP-WEB-8 |
| The kit CD, the tray tool, Keep in step, Play a test sound | No; after the launch | Each needs WP-KIT-2 and WP-KIT-3 |

## Decisions to record

The next SITE ids are taken up to SITE-109 on `main` and the open branches (SITE-51 to SITE-57, SITE-60 to SITE-73, SITE-80 to SITE-91, SITE-100 to SITE-109). This plan reserves **SITE-120 to SITE-129**, **ARCH-50 and ARCH-51** and **INS-30 and INS-31** by hand, above every block on `main` and in the open branches (the highest are ARCH-42 and INS-28). The rows marked *this plan* are written now as **Assumed**.

| Id | Decision | Written by |
|---|---|---|
| SITE-120 | The Host Link Kit: one capability model, named channels with versions, a helper core with optional modules per guest family, a page glue, and one bundle per family on one disc | This plan |
| SITE-121 | `caps` tokens: a bare name is version 1, `name.N` is version N listed beside the bare name, `name-feature` is a feature; use the highest common version; `proto` stays 1 | This plan |
| SITE-122 | Audio telemetry comes from the card on the page first; the helper adds only what the guest alone knows; `ac97` and `host-link` never import each other and `kit.js` joins them | This plan |
| SITE-123 | The page's slider is the speaker; the guest's volume is shown and can be kept in step (off by default); the page never writes the codec's registers behind the driver | This plan |
| SITE-124 | Every channel is a switch on the page; system information and log lines are asked only while the Diagnostics panel is open; nothing leaves the tab | This plan |
| SITE-125 | The new channels `audio`, `power`, `sys`, `log`, `pointer`, their priority and the `refused` code | WP-KIT-2 |
| SITE-126 | Time sync in v86 sets the zone with `SetTimeZoneInformation`, not the clock | WP-HOST-6 (an amendment to its brief) |
| SITE-127 | The power channel: the guest's shutdown notice and the page's Shut down the PC button | WP-KIT-5 |
| SITE-128 | The Host Link Kit CD: its layout, the page's Insert button, auto-run | WP-KIT-7 |
| SITE-129 | `v86-host-link` is its own repository, built locally until the owner's go; `v86-devices` keeps the card; Quill vendors the built results back | **Decided by the owner** (2026-11-28); written now |
| ARCH-50 | Host link channel modules are Win32 DLLs on `quill-w32rt`'s runtime and loader with their own `hostlink-modabi` contract; tools with a window reach the helper through a `WM_COPYDATA` broker | This plan |
| ARCH-51 | Spare (the Linux helper's module shape, if `hostlinkd`'s Rust features are not enough) | WP-KIT-8 |
| INS-30 | The kit's Windows setup starts the helper from a Startup-folder shortcut, which Liminal Installer's Win32 and Windows 3.1 runtimes gain as the shortcut place `startup` | This plan |
| INS-31 | The kit's setup project and the disc build (`li-media` or the fallback script) | WP-KIT-7 |

## Rules for every package

The host link guests plan's [rules](host-link-guests.md#rules-for-every-package) hold word for word (one branch and worktree under `~/source/worktrees/quill/<topic>`, the process and display rules, disk above 20 GB, nothing published, launch first, the gates), with the evidence folder `~/Documents/Liminal HQ/Quill/Web demo/host-link-kit/<package>/` and committed shots under `docs/media/` as `web-kit-*.png`. Also:

- **`sound.js`, `host-link.js`, `power.js` and `try-reactos.js`** are edited only by one line each that hands a handle to `kit.js`, in a marked block, until WP-WEB-8 has merged.
- **The `ac97` package** is edited additively and only after pull request 198 has merged (it owns `model.ts` and `index.ts` until then); once the package moves to `v86-devices` (WP-AC97-1), the same change goes there.
- **The codecs** (`crates/quill-hostlink`, `frame.ts`, `link.ts`, the vectors) are not edited by two packages at once: WP-HLG-2 and WP-KIT-2 run one after the other.
- **`win9x/hostlink/**`** belongs to WP-HOST-5 and WP-HOST-6 while they run; WP-KIT-3 starts after both have merged or rebases onto whichever is open, never edits their modules.

## Order and parallelism

| # | Package | Starts when | Runs beside |
|---|---|---|---|
| 0 | Pull requests 198, 200 and 204 merge; WP-HOST-5 to WP-HOST-7 and WP-WEB-8 keep their order; the mixer investigation reports | — | — |
| 1 | **WP-KIT-1** The card's telemetry in Diagnostics and the badge (page-only) | Pull request 198 merged | Any launch package; WP-HLG-1 |
| 2 | **WP-KIT-2** The capability model and the new channels in the specification, both codecs and the vectors | WP-HLG-2 merged | WP-HOST-8 |
| 3 | **WP-KIT-3** The helper's module loader, the broker and the budget | WP-KIT-2, pull request 215, WP-HOST-5 and WP-HOST-6 merged | WP-KIT-8 |
| 4 | **WP-KIT-4** The audio channel: `HLAUDIO.DLL`, Keep in step, Play a test sound | WP-KIT-3 merged; the mixer investigation's findings read | WP-KIT-5 |
| 4 | **WP-KIT-5** Power, system information and log: `HLSYS.DLL`, the rows, Shut down the PC | WP-KIT-3 merged | WP-KIT-4 |
| 5 | **WP-KIT-6** The tray icon and the sound card window: `HLTRAY.EXE` | WP-KIT-4 merged | WP-KIT-7's project work |
| 6 | **WP-KIT-7** The Windows setup and the Host Link Kit CD | WP-KIT-4, WP-KIT-5 and WP-KIT-6 merged; Liminal Installer's `startup` shortcut place released and pinned | WP-KIT-8 |
| 7 | **WP-KIT-8** The Linux kit: `hostlinkd`'s audio, sys, power and log, `install.sh` | WP-HLG-3 merged and WP-KIT-2 merged | WP-KIT-3 to WP-KIT-7 |
| 8 | **WP-KIT-9** Pointer feedback for Follow | WP-KIT-3 merged, WP-PTR-4 merged | Anything |
| 1 | **The `v86-host-link` seed** (the lead's phase 1) | Pull requests 200 and 204 merged (or their branches, rebased later) | WP-KIT-1, the launch |
| — | **Vendoring back into Quill** (the lead's phase 5) | WP-KIT-6 merged in `v86-host-link`, and after WP-WEB-8 | — |

With the cap of three builders, the launch packages come first; WP-KIT-1 can take a free slot as soon as pull request 198 has merged, because it touches only the card package additively, a new `kit.js` and marked lines. Everything from WP-KIT-3 on waits for the launch unless the owner says otherwise. WP-KIT-4 and WP-KIT-5 own different modules and can run together.

## Work packages

WP-KIT-1 runs in Quill. **WP-KIT-2 to WP-KIT-9 run in `v86-host-link`** under its lead: the paths below are written as they are in Quill today, and each maps to the new layout (`crates/quill-hostlink` to `crates/host-link-wire`, `win9x/hostlink/` to `helpers/win32/`, the page package to `page/`, `kit.js`'s parts to the repository's `demo/` and, after vendoring, to Quill's `kit.js`, `guests/linux/hostlinkd` to `helpers/linux/hostlinkd`, `kit/` to `kit/`). Quill-side glue a package needs (a row, a switch, the Insert button) is a small Quill pull request after the vendoring step.

### WP-KIT-1 The card's telemetry in Diagnostics, and the badge

- **Finish line.** The `ac97` package's handle has `codec()` and the `onCodec` option, tested; a new `site/try-reactos/kit.js` adds an *Audio* row to the Diagnostics panel (the guest's rate and format, playing or idle, buffers done, starved frames, the output's underrun events and frames, dropped and silent frames, and the guest's master, PCM-out and record volume and mute decoded from the codec), refreshed at most twice a second while the panel is open and not at all while it is closed; the player row shows **Muted in the PC** (a hint, with the guest's Volume Control named) whenever the guest's master or PCM out is muted while the card plays or within 2 s of playing; level 4 checks show the row and the badge on the saved desktop and on a cold boot (where the codec starts muted). SITE-122 and SITE-123's page half, as **Assumed** rows already written by this plan, are updated with what was built.
- **Files owned.** `site/try-reactos/packages/ac97/src/index.ts` (the two additions), `model.ts` (a register-write hook only), their tests, the built `lib/ac97.js` and its stamp; new `site/try-reactos/kit.js` and its test; one line each in `sound.js` and `try-reactos.js` (marked blocks) handing the card's handle to `kit.js`; the badge's element in `index.html` and its rule in `try-reactos.css` (marked blocks); new `site/try-reactos/harness/check-kit-audio.ts`; `site/try-reactos/README.md` (*Diagnostics: Audio*).
- **Must not touch.** `sink.ts`, `capture.ts`, `resample.ts`, the host-link package, `host-link.js`, `power.js`, `qagrr/**`, `win9x/**`, the saved state.
- **Dependencies.** Pull request 198 merged.
- **Budgets.** The `ac97` module within 36 KB (at most 600 bytes added); `kit.js` at most 8 KB; no timer while the panel is closed.
- **Tests.** Level 1 (every codec step decoded, `onCodec` at most once per register per 50 ms, mute bit, the badge's rule); level 4 in headless Chromium and Firefox.
- **Evidence.** The panel and the badge on the saved desktop and on a cold boot; the row while Critical Stop plays.
- **Parallel.** With any launch package.

### WP-KIT-2 The capability model and the new channels

- **Finish line.** `docs/host-link.md` gains the caps token grammar, the five new channels and their tables (version 1 rows), the priority order, the `refused` code and the `mods` key; `tests/host-link/gen_vectors.py` writes vectors for each new channel and for `refused`; `quill-hostlink`'s `Channel` and the page's `PRIORITY` know the names; the page package exports `parseCaps`, `hasChannel`, `channelVersion` and `hasFeature`, and its `hello` lists only the channels the page's options turn on; the conformance sessions gain the caps cases of [Versions and compatibility](#versions-and-compatibility); `HOSTLINK.EXE` 0.1.0 still passes every existing session; SITE-125.
- **Files owned.** `docs/host-link.md`, `tests/host-link/**`, `crates/quill-hostlink/**`, the page package's `src/frame.ts`, `src/link.ts`, new `src/caps.ts`, their tests and the built module, `crates/quill-hostlink-guest/**` (parsing the page's caps only), `docs/decisions.md` (SITE-125).
- **Must not touch.** `win9x/**`, the page's glue files, `src/clip/**`.
- **Dependencies.** WP-HLG-2 merged.
- **Budgets.** At most 1 KB added to the page package; the crates stay `no_std`, no `alloc`, no panics.
- **Tests.** Levels 1 and 2; `make validate`.
- **Parallel.** With WP-HOST-8.

### WP-KIT-3 The helper's module loader, the broker and the budget

- **Finish line.** `crates/hostlink-modabi` (layouts tested); `HOSTLINK.EXE` loads `HL*.DLL` beside itself through `quill_w32rt::module`, lists a module's channel in `caps` only when it loaded, names it in `mods`, and answers a broker subscription by `WM_COPYDATA`; a test module `HLTEST.DLL` (an `echo` of `sys get`) and a test client prove both under Wine; `check-dll.sh` and the import checks pass for each file; the size within the budget the owner chose; ARCH-50 updated with what was built.
- **Files owned.** New `crates/hostlink-modabi/**`, `win9x/hostlink/src/modules.rs` and `broker.rs` and their dispatch lines in `main.rs`, `win9x/hostlink/mods/hltest/**`, `win9x/tools/hostlink-imports.allow` (the loader's functions and `IsWindow`), the budget line in `win9x/build.sh`, new `win9x/tools/wine-hostlink-modules.sh`.
- **Must not touch.** `clip.rs`, `files.rs`, `time.rs`, `display.rs` (WP-HOST-5 and WP-HOST-6's), `quill-w32rt` beyond a reported fix, `quill-modabi`.
- **Dependencies.** WP-KIT-2, pull request 215, WP-HOST-5 and WP-HOST-6 merged.
- **Tests.** Wine: 200 load and free cycles, a missing, another-build and non-PE module each refused with no box, a subscriber window that dies; level 3 on ReactOS in v86 with `HLTEST.DLL`.
- **Parallel.** With WP-KIT-8.

### WP-KIT-4 The audio channel

- **Finish line.** First, a short probe report section in this plan answering, on ReactOS 0.4.16 in v86 with the card: `mixerGetNumDevs`, the lines and controls `mixerGetLineInfoA` and `mixerGetLineControlsA` list, whether `mixerSetControlDetails` changes the codec's registers and Volume Control, whether `MM_MIXM_CONTROL_CHANGE` arrives, `waveOutSetVolume`'s effect, and `waveOutGetDevCapsA`'s name and driver version (reading the mixer investigation's results first and not repeating what it measured). Then `HLAUDIO.DLL` with `audio devices`, `mixer`, `set`, `play` and `played`; `kit.js`'s Keep in step switch, the badge as a button with `audio-set`, **Play a test sound** with the tone counted at the probe, and `audio host` on every change of output, microphone or volume; the echo rules tested.
- **Files owned.** New `win9x/hostlink/mods/hlaudio/**`, its imports list, `kit.js` (the audio parts), the page package's `src/audio.ts` (message shapes only, no card), `harness/check-kit-audio.ts` (extended), the probe section of this plan, `docs/decisions.md` (SITE-123 updated).
- **Must not touch.** The `ac97` package (a needed change is reported), `HOSTLINK.EXE`'s own files.
- **Dependencies.** WP-KIT-3 merged.
- **Budgets.** `HLAUDIO.DLL` 8 KiB.
- **Tests.** Levels 1, 3 and 4; Keep in step's 1,000 seeded sequences.
- **Parallel.** With WP-KIT-5.

### WP-KIT-5 Power, system information and log

- **Finish line.** `HLSYS.DLL` with `power ending` (from the helper's hidden window's `WM_QUERYENDSESSION` and `WM_ENDSESSION`, passed to the module), `power shutdown` and `restart` (`ExitWindowsEx`, the privilege by `GetProcAddress`), `sys info` (without `idle` where `GetSystemTimes` is absent) and `log line`; the page's *System* and *Log* rows, the switch per channel group, **Shut down the PC** beside Stop (a confirmation, then `power shutdown`, then the ACPI watcher ends the machine as today); SITE-127.
- **Files owned.** New `win9x/hostlink/mods/hlsys/**`, its imports list, `kit.js` (the system, log and power parts), `power.js` (one marked line: the notice's text), the page package's `src/sys.ts`, the switches' block in `index.html` and `try-reactos.css`, `docs/decisions.md` (SITE-127).
- **Must not touch.** The ACPI watcher's logic, `HOSTLINK.EXE`'s own files.
- **Dependencies.** WP-KIT-3 merged.
- **Budgets.** `HLSYS.DLL` 8 KiB; `sys get` at most once a second, only while the panel is open.
- **Tests.** Levels 1, 3 (Shut down the PC ends in the ACPI write within 30 s) and 4.
- **Parallel.** With WP-KIT-4.

### WP-KIT-6 The tray icon and the sound card window

- **Finish line.** `HLTRAY.EXE` as [The sound card tool](#the-sound-card-tool-in-the-guest) describes, a broker client, with the four icon states drawn from one SVG master rendered down by script, the Sound card window, the microphone indicator and Play a test sound; the page sends `audio stats` only while the window is open.
- **Files owned.** New `win9x/hostlink/tray/**`, its icon masters and render script, its imports list, `kit.js` (`audio stats`).
- **Dependencies.** WP-KIT-4 merged.
- **Budgets.** 16 KiB.
- **Tests.** Wine (the window's texts from a fake broker); level 3 (the icon in the tray on ReactOS, a canvas crop).
- **Evidence.** The tray icon in each state; the window while Critical Stop plays.

### WP-KIT-7 The Windows setup and the Host Link Kit CD

- **Finish line.** A Liminal Installer project `kit/win32/` with the components of [One bundle per guest family](#one-bundle-per-guest-family), built against the pinned `liminst` with the `startup` shortcut place; `kit/build-disc.sh` makes `hostlink-kit.iso` (under 1.44 MB, `AUTORUN.INF`, Joliet names) and a floppy image of the same files; the page's **Insert the Host Link Kit CD** and **Eject**; level 3: in a cold-booted ReactOS without the helper, insert, run `SETUP.EXE` (by auto-run or from My Computer), restart, the tray says connected, uninstall from Add/Remove Programs, restart, no helper; SITE-128, INS-31.
- **Files owned.** New `kit/**` (in this repository until the extraction), `installer/liminst.rev` (only if the pin must move for the `startup` place), the button's block in `index.html` and `kit.js`.
- **Must not touch.** `qagrr/**` (the demo disk keeps its own Run value), the helpers' sources.
- **Dependencies.** WP-KIT-4, WP-KIT-5 and WP-KIT-6 merged; Liminal Installer's `startup` place released.
- **Tests.** Level 3 as above; the disc's file list and sizes checked by the script.
- **Evidence.** Each setup page, the tray after the restart, the disc face if the owner asks for one.

### WP-KIT-8 The Linux kit

- **Finish line.** `hostlinkd` gains the `audio` (ALSA control ioctls with change events, `hw_params` and `status` for `stream`), `sys`, `power` and `log` channels as Rust features; `install.sh` and `--remove`; level 3 in the WP-HLG-4 image (or WP-AC97-3's test image, which has the card's driver): the *Audio* row shows the guest's mixer and the playing program; ARCH-51 if the features do not suffice.
- **Files owned.** `guests/linux/hostlinkd/src/{audio,sys,power,log}.rs`, `kit/linux/**`.
- **Dependencies.** WP-HLG-3 and WP-KIT-2 merged.
- **Budgets.** `hostlinkd` stays within 512 KB stripped with every feature.

### WP-KIT-9 Pointer feedback for Follow

- **Finish line.** `pointer at` from `HOSTLINK.EXE` (or a module) when the cursor moves, at most 30 a second; the pointer module corrects its estimate of the guest's cursor from it and warps after a mode change; the direction-error measure of the pointer plan improves or stays; a note on v86's VMware absolute mouse for Linux guests (measured or left untested).
- **Files owned.** The module's `pointer` part, the pointer module's link adapter (with the pointer plan's owner).
- **Dependencies.** WP-KIT-3 and WP-PTR-4 merged.

## Builder briefs

The common preamble is the host integration plan's ([Builder briefs](host-integration.md#common-preamble)), with "the host link of the ReactOS web demo" read as "the Host Link Kit", this plan in place of that one, and the evidence folder `~/Documents/Liminal HQ/Quill/Web demo/host-link-kit/<package>/`. Paste the preamble, then the package's part.

### WP-KIT-1 The card's telemetry in Diagnostics, and the badge (full brief)

```text
Package WP-KIT-1, the card's telemetry in Diagnostics and the Muted in the PC badge. Branch
feat/kit-audio-telemetry, worktree ~/source/worktrees/quill/kit-audio-telemetry, from origin/main
after pull request 198 (the AC'97 recording package) has merged. Page only: no guest, disk, state,
helper or codec change.

Read first: docs/plans/host-link-kit.md ("Audio and device telemetry", "Volume in step", "How the
sound card and the link meet", "Privacy", "Rules for every package" and this package);
site/try-reactos/packages/ac97/src/index.ts (AttachOptions, Ac97Stats, Ac97Handle, stats, sinkStats)
and src/model.ts (resetCodec's power-on values: master 0x02 and PCM out 0x18 start muted; gains(),
which decodes 1.5 dB steps: master bits 12-8 and 4-0 with unity 0, PCM out with unity 8; namWrite,
where a guest writes the mixer registers); site/try-reactos/sound.js (describe(), the Sound row) and
host-link.js (how a row is drawn); site/try-reactos/harness/check-sound.ts (how the page is driven
and Critical Stop is played); docs/decisions.md SITE-51 to SITE-57 and SITE-120 to SITE-124.

Build:

1. In the ac97 package, additively: `codec()` on the handle, returning null before the card is
   registered, else { master, pcm, record, mic, recordSelect } as { raw, left, right, muted } with
   left and right as percent of the line's range (master and PCM out five bits, record gain four),
   plus { rate, vra }. An `onCodec?: (register, value) => void` option, called from namWrite for
   registers 0x02 to 0x1E after the write is stored, coalesced to at most once per register per 50 ms
   with the last value (a timer only while writes are pending). No allocation per sample; nothing in
   sink.ts, capture.ts or resample.ts. Tests for every step of each line, the mute bit, the
   coalescing, detach clearing the timer. Rebuild lib/ac97.js and its stamp (tools/stamp-web.py).
2. New site/try-reactos/kit.js: `attachCard(handle)` and `detachCard()`. It draws an "Audio" row in
   the Diagnostics panel (a new <dl class="diag-readout" data-audio-readout> after the Sound row, in a
   marked block of index.html): "22,050 Hz, 16-bit stereo, playing. 177 buffers, 39,045 frames, 0
   starved. Output: 0 underruns, 0 dropped. PC volume: master 100 percent, wave 100 percent, record 0
   percent; master muted." Refresh at most twice a second, and only while the panel (<details
   data-diag>) is open; when it closes, stop the timer. Keep the rules (wording, the badge's
   condition, the refresh gate) in plain exported functions with a bun test beside the file.
3. The badge: a small element in the player row, after the Sound button (marked block of index.html
   and try-reactos.css), hidden by default, shown with "Muted in the PC" and a title "The PC has muted
   its own sound: open its Volume Control (the speaker in the taskbar, or Control Panel > Sounds and
   Audio Devices) to unmute" when master or PCM out is muted and the card is playing or played within
   the last 2 s. It is updated from onCodec and onChange, without the panel being open. It is text,
   not a button, in this package (the button form needs the audio channel, WP-KIT-4).
4. One marked line in sound.js passing the handle to kit.js after attachAc97 (and detachCard on
   detach), and onCodec wired through the attach options; one marked line in try-reactos.js if the
   import order needs it. Nothing else in those files.
5. harness/check-kit-audio.ts: on the saved desktop and on a cold boot (?state=), with the panel open
   and closed: the row's text appears only while open and updates within 1 s of Critical Stop
   starting; on the cold boot the badge shows while the codec is muted and the driver has not
   unmuted it (if the driver unmutes before any sound, say so and test the badge by writing the mute
   bit through the model in the test only); no request off the origin; the page's own timers stop
   when the panel closes (count setInterval calls with an init script).

Must not touch: sink.ts, capture.ts, resample.ts, the host-link package and host-link.js, power.js,
qagrr/**, win9x/**, the saved state and its build scripts.

Budgets: lib/ac97.js within its 36 KB budget and at most 600 bytes larger; kit.js at most 8 KB; no
timer while the panel is closed.

Finish line: as docs/plans/host-link-kit.md WP-KIT-1 says; update SITE-122 and SITE-123's rows in
docs/decisions.md with what was built (still Assumed), the README's "Diagnostics" section, and
docs/progress.md's Host Link Kit row. Gates: the ac97 package's bun test, typecheck and check:built,
the page's harness checks you touched (check-sound.ts must still pass), stage-web.py --check and
stamp-web.py --check, check-headers.sh, markdownlint. Evidence: the panel and the badge on the saved
desktop and the cold boot, the row while Critical Stop plays, into ~/Documents/Liminal HQ/Quill/Web
demo/host-link-kit/WP-KIT-1/. PR title suggestion: "Show the sound card's state in Diagnostics and
say when the PC is muted" (labels: site, audio if it exists; do not merge).
```

### WP-KIT-2 to WP-KIT-9

```text
Package <id>, <title from the plan>. Branch <feat/host-link-caps | feat/hostlink-modules |
feat/hostlink-audio | feat/hostlink-sys | feat/hostlink-tray | feat/host-link-kit-disc |
feat/hostlinkd-kit | feat/hostlink-pointer>, worktree ~/source/worktrees/quill/<same topic>.

Read first: docs/plans/host-link-kit.md (your package, "The capability model", the section your
package serves, "Privacy", "Rules for every package"), docs/host-link.md and the vectors as merged,
docs/plans/host-link-guests.md ("The layering", "Rules for every package"), docs/plans/modules.md
section 19 for WP-KIT-3 to WP-KIT-6, and the files your package owns.

Build what the plan's Finish line says, in small commits, staying inside Files owned. Finish line,
tests, budgets, evidence and decision numbers: the plan's package section. Until WP-WEB-8 has merged,
touch the ReactOS page only in marked blocks, and rebase onto the launch packages rather than ask
them to wait. PR title suggestions: "Name the host link's channels and versions in hello", "Load
host link channel modules and serve local tools", "Share the PC's volume and devices with the page",
"Tell the page about the PC's power, system and log", "Show the host link in the PC's tray", "Build
the Host Link Kit setup and CD", "Give the Linux helper the kit's channels", "Tell the pointer where
the PC's cursor is".
```

## The lead's brief

```text
You are the Opus lead for v86-host-link, Liminal HQ Labs' host link and Host Link Kit: guest tools,
the other way round. You own the product end to end: design, plan, split into work packages, brief
Sonnet builders (at most three at once), integrate, and bring the result into the Quill Remix by
vendoring. You write little code yourself.

Where: a new local repository at ~/source/liminal-hq-labs/v86-host-link, created with `git init -b
main`, worked on topic branches merged into its main with merge commits. NO remote, no push, no
GitHub repository, no release, tag, npm publish or Pages deploy until the owner says. Worktrees under
~/source/worktrees/v86-host-link/<topic>, removed when merged. Quill (~/source/experiments/quill16,
remote liminal-hq-labs/quill) is read through `git fetch` and worktrees only; changes to Quill go
through Quill pull requests from worktrees under ~/source/worktrees/quill/<topic>, never merged by
you.

Read first, in Quill on origin/main: docs/plans/host-link-kit.md (this plan: the capability model,
the channels, the repository's layout, the migration, the work packages WP-KIT-2 to WP-KIT-9),
docs/plans/host-link-guests.md (the guest families, transports, printing, WP-HLG-1 to WP-HLG-9),
docs/plans/host-integration.md (the WP-HOST-1 and WP-HOST-3 reports), docs/host-link.md,
docs/plans/modules.md section 19 and pull request 215 (quill-w32rt), and pull request 214's
docs/plans/v86-ac97-reuse.md (the sibling repository you do not depend on). The Liminal Installer
(~/source/liminal-hq-labs/liminal-installer) builds the kit's Windows setup: read its SPEC.md and
docs/retro-win32.md; the `startup` shortcut place (INS-30) is a change there, which you request
from its owner's orchestrator, not make yourself.

The repository is the memory: create and keep SPEC.md (the protocol, from Quill's docs/host-link.md,
plus every channel of this plan), docs/decisions.md (numbered, prefixes HL-, PAGE-, GUEST-, WIN-,
LNX-, KIT-, CI-, one line of why each; the first rows restate the Quill decisions you inherit, with
their Quill ids), docs/progress.md (phase log, newest first: done and next), one doc per topic,
AGENTS.md and CLAUDE.md (the house rules: Canadian English, sentence case, no hard-wrapped
markdown, SPDX headers "(c) Copyright 2026 Liminal HQ, Scott Morris" and "Apache-2.0 OR MIT", tabs
except YAML, Conventional Commits with markdown bodies via git commit -F, never git add -A), the
licence files, the hero banner as an SVG master with a render script (its own palette, GNOME-style
chrome), and .github/workflows with one job per check, ready and unpushed. A fresh agent must be able
to resume from these alone.

Phases (each leaves evidence in ~/Documents/Liminal HQ/v86-host-link/<phase>/, saying what it shows
and what it does not verify):
  1. Seed: the layout of host-link-kit.md, history kept with git filter-repo from Quill at a named
     commit after pull requests 200 and 204 merge (start from their branches if they have not, and
     rebase later); the gates green locally: vectors --check, bun test, tsc, the built module's drift,
     cargo nextest, clippy, fmt, the helper's build and size, the import and DLL checks, headers,
     markdownlint. HOSTLINK.EXE's normalised digest equal to Quill's.
  2. WP-HLG-2's seams and sessions, then WP-KIT-2 (caps tokens and the new channels).
  3. WP-KIT-3 (modules and broker), then WP-KIT-4 and WP-KIT-5 together, then WP-KIT-6.
  4. WP-KIT-7 (the setup and the Host Link Kit CD) when the installer's startup place exists.
  5. Vendor into Quill: one Quill pull request with site/try-reactos/vendor/v86-host-link/, the
     helpers under qagrr/ with PROVENANCE.md and hash checks, the moved Quill paths removed, Quill's
     kit.js rows and switches wired; the Quill Remix's state rebuilt by its own scripts.
  6. Later: WP-HLG-3/4 and WP-KIT-8 (Linux), WP-KIT-9 (pointer), WP-HLG-6 (Windows 3.x).

Budgets and golden outputs are CI checks, not promises: HOSTLINK.EXE (26 KiB unless the owner says
otherwise), the modules 8 KiB each, HLTRAY.EXE 16 KiB, the page module 40 KB minified, the disc
under 1.44 MB. Processes: browsers headless, Wine, Xvfb and QEMU on a private display with DISPLAY
set explicitly and WAYLAND_DISPLAY unset in every command, servers on 127.0.0.1 with a PID file and
never on 8095 to 8098, every process stopped by its own PID, never pkill by name, never the host
desktop. Keep df -h /home above 20 GB (it was 16 GB when this plan was written: report before any
image build). Removals: one plain command per literal path.

When a call is the owner's, take the default in host-link-kit.md, make it a one-line swap, and report
"OWNER DECISION: question | default taken | where to swap", collected in docs/decisions.md and in
~/Documents/Liminal HQ/Decisions for the owner.md. Report per phase: done, evidence, not verified,
owner decisions, next.
```

## Owner decisions

Each is taken with the default so no package waits; the last column is where to change it.

| Question | Default taken | Where to swap |
|---|---|---|
| The repository | **Decided by the owner**: `v86-host-link`, its own repository, local at `~/source/liminal-hq-labs/v86-host-link` with no remote until the owner says; `v86-devices` keeps the card and the device kit | SITE-129 |
| The kit's name | **Host Link Kit** for people; `kit/` inside `v86-host-link` for code | SITE-120; the names table |
| A separate kit repository | No: the kit is the guest-facing part of `v86-host-link` | SITE-120 |
| How Quill takes it back | Vendored built files with `PROVENANCE.md` and hash checks, no npm or git dependency | SITE-129; [Migration from Quill](#migration-from-quill) |
| `quill-w32rt` in the new repository | A pinned copy with provenance under `helpers/win32/w32rt/` | The layout; a shared Labs crate later |
| Decision prefixes in the new repository | `HL-`, `PAGE-`, `GUEST-`, `WIN-`, `LNX-`, `KIT-`, `CI-` | The lead's `docs/decisions.md` |
| `HOSTLINK.EXE`'s budget with the loader and broker | Raised from 24 KiB to **26 KiB** | `win9x/build.sh`'s budget line; WP-KIT-3 (the alternative: move `time` and `display` into `HLSYS.DLL`) |
| Channel modules: DLLs or everything in one EXE | DLLs through `quill-w32rt`'s loader, with a `hostlink-modabi` contract | ARCH-50; WP-KIT-3 |
| The sound card tool's form | A tray program, `HLTRAY.EXE`, with a Sound card window; a Control Panel page later | WP-KIT-6 |
| The page's slider and the guest's volume | Separate by default; **Keep in step** as a switch, off | SITE-123; `kit.js` |
| Audio channel on by default | On (levels and device names only) | SITE-124; the switches' defaults in `kit.js` |
| System information and log | On, but asked only while Diagnostics is open | SITE-124 |
| Which program is playing | Shown on Linux only, never logged | SITE-124; WP-KIT-8 |
| PC speaker beeps through the card's output | Not now (the page keeps `disable_speaker: true`) | `kit.js`; a later item |
| How the setup starts the helper | A Startup-folder shortcut (a new `startup` shortcut place in Liminal Installer), not a Run value | INS-30; Liminal Installer's shortcut places |
| The disc | One ISO under 1.44 MB with Windows and Linux folders and `AUTORUN.INF`; the page's Insert button; also a floppy image | SITE-128; `kit/build-disc.sh` |
| The disc's builder | Liminal Installer's media (`li-media`), an `xorriso` script as the fallback | INS-31; WP-KIT-7 |
| A Host Link Kit disc face and floppy label in the print set | Not planned; offered | `assets/media/gen_media.py` |
| Time sync in v86 | Set the zone (`SetTimeZoneInformation`), not the clock | SITE-126; WP-HOST-6 |
| The kit in the first demo | Only WP-KIT-1 (page-only telemetry and the badge) | The launch checklist |

## Risks

- **ReactOS's mixer may be empty.** If `mixerGetNumDevs` is 0 or the mixer has no lines (the empty Volume Control the investigation is looking at), `audio mixer`, `audio set` and Keep in step have nothing to work with on the demo's guest; the page-side telemetry and the badge still work, and Play a test sound needs only `PlaySoundA`. WP-KIT-4's probe settles it first.
- **The driver may not write the codec's volume registers** when the guest's slider moves (a software volume in the kernel's mixer instead). Then the badge sees only the driver's own unmute at start-up, and the *Audio* row says "PC volume: not reported by the card" rather than a number that does not change.
- **The helper's budget.** A 1 KiB overshoot is likely; the two remedies are in [What the core costs](#what-the-core-costs).
- **One more ABI.** `hostlink-modabi` beside `quill-modabi` is a second contract to keep; it reuses the loader and the checks so the new code is the table and its tests.
- **Liminal Installer's gap.** Without the `startup` place the kit's setup cannot start the helper; WP-KIT-7 waits, and its fallback (a last-page instruction) is weak.
- **Auto-run on ReactOS** is not verified; the button's hint covers it.
- **Telemetry as a side channel.** Which program plays and the log could reveal what the guest's user does; the defaults keep them in the panel, visible only while it is open, and never stored.
- **Scope.** The kit is a family of small things; each package is small, but nine of them are a lot of review. Only WP-KIT-1 is near the launch.

## What was not verified

- Anything in a guest: the `winmm` mixer calls, `MM_MIXM_CONTROL_CHANGE`, `PlaySoundA(SND_MEMORY)`, `ExitWindowsEx` with the privilege, `GetSystemTimes` and `Shell_NotifyIconA` on ReactOS 0.4.16 in v86; whether ReactOS's `ac97.sys` writes the codec's master and PCM registers when Volume Control moves; auto-run of a CD on ReactOS.
- The size estimates marked **[E]** (the loader, the broker, the modules, the tray program).
- v86's `pcspeaker-update` with `disable_speaker: true`, and its VMware absolute mouse with a Linux guest's `vmmouse` support.
- `bochs-drm` on v86's VGA for Fit to window under Linux.
- ALSA's control events and `/proc/asound` in a v86 Linux image with the card (no such image exists yet: WP-AC97-3 builds one).
- Liminal Installer's media builder with a second family's folder on the same disc.
- The mixer investigation's findings, which had not been written when this plan was.
