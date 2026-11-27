# Spike J13: a Win32 worker thread for autosave (checkpoint A)

Checkpoint A of the autosave and recovery plan asks whether a Win32 `CreateThread` worker can serialize a 1 MB document from an owned copy while the main thread keeps typing and painting, and post its completion back with `PostMessageA`, on the systems Quill32 has to run on. The code is throwaway and lives in `spikes/w32-worker/`, apart from `win9x/` and every workspace; nothing in `QUILL32.EXE` changed. The decision is [W9X-13](decisions.md).

**Status.** The inline path this report recommends has since shipped: `App::idle` runs autosave in abortable slices from the idle loop (CORE and W9X decisions of the same name, #125). The spike code in `spikes/w32-worker/` is unchanged and still stands alone: it shares no code with `quill-app` and is outside `QUILL32.EXE` and `make validate`.

## Table of contents

- [Answer](#answer)
- [What was built](#what-was-built)
- [Checkpoint A, criterion by criterion](#checkpoint-a-criterion-by-criterion)
- [Windows 95 OSR2.5 in DOSBox-X](#windows-95-osr25-in-dosbox-x)
- [Why the worker does not buy typing latency on one CPU](#why-the-worker-does-not-buy-typing-latency-on-one-cpu)
- [ReactOS 0.4.16](#reactos-0416)
- [Wine](#wine)
- [Exit while a worker runs](#exit-while-a-worker-runs)
- [Recommendation for PR 12](#recommendation-for-pr-12)
- [Not proven](#not-proven)
- [Reproducing it](#reproducing-it)

## Answer

**The mechanics work; the latency criterion fails.** A worker made with `CreateThread` took an owned copy of the document, serialized 1 MB, wrote the file and posted `WM_APP` back, 1,400 verified times in a row on Windows 95 OSR2.5, 3,000 on Wine and 1,100 on ReactOS, with no deadlock and no lost message, and the process left cleanly with a worker running on all three. The bytes matched an oracle every time and the worker's copy never changed under it. What fails is criterion 4: on Windows 95 a lowest-priority worker adds **9 to 33 per cent** to the median key-to-paint time while it writes (8.1 per cent with the file writes switched off), against a limit of ±5 per cent, and a normal-priority worker more than doubles it. On one CPU a thread cannot make the work cheaper, and a cooperative job that takes a slice when the queue is empty lands in the same band (5 to 21 per cent), so the thread buys nothing there that PR 8's inline abortable write does not. The same worker is free on a multi-core host (Wine, all cores: +1 per cent).

Windows 98 SE was not run: the test bed has no image for it. ReactOS also showed a file-system failure that is not about threads (`ERROR_USER_MAPPED_FILE` when a file is created again straight after it was closed), which PR 1 and PR 8 should expect.

## What was built

`W32SPIKE.EXE` is a 17 KB Rust `no_std` program on `win9x/win9x.json` (Pentium, no C runtime, subsystem 4.0, `core` built from source), built by `spikes/w32-worker/tools/build.sh` with the same toolchain as `QUILL32.EXE`. It has its own Cargo workspace and its own `.cargo/config.toml` (a `.cargo` folder under `win9x/` would merge in and add Quill32's resource file), so it is outside `make validate`.

- **The document.** 1 MB (configurable) of words, paragraph breaks, braces, backslashes and 8-bit bytes from a fixed generator.
- **The job.** `src/worker.rs` serializes the bytes as an RTF-like body in 4 KB slices (`\par`, `\\`, `\'hh`), checksums the output with CRC-32, writes each slice to `SPIKE.TMP` and looks at a cancel flag between slices. The module's own import block is kernel32 only (`worker.allow` lists its eleven functions, and `tools/check.sh` fails if a twelfth appears). `begin`, `step` and `finish` split the job so the same code runs on a thread (`run`) or from the main thread's idle loop (the `coop` phase).
- **Ownership.** The main thread copies the document into a fresh heap block, hands that block and a job record to the thread, and never touches the copy again. The worker frees the copy; the main thread frees the job record after the completion message and after `WaitForSingleObject` on the thread handle. While a stress job runs the main thread changes bytes of the live document, and the worker's output and the checksums of its input at start and at end must still equal what the main thread computed from the copy beforehand.
- **Completion.** The thread's entry function runs the job, then calls `PostMessageA(hwnd, WM_APP, seq, job)`. `CreateEventA` and `SetEvent` signal the job's end, so a watchdog can tell a lost message from a slow job (3 seconds of a signalled event with no message is a failure) and the exit path can wait on it.
- **Typing.** A separate time-critical thread plays the keyboard driver: it sleeps `keyms`, stamps the performance counter and posts a key message. The main thread edits a row of text and the live document, invalidates the window and paints; the time from the stamp to the end of `WM_PAINT` is the key-to-paint time, so a delay in waking the main thread counts. `paintrep` repeats the paint's `TextOutA` rows until a paint costs about what Quill's does (454 ms at 12,000 DOSBox-X cycles; the plan's 0.64 s was a page of real text).
- **Phases** (`phases=` in `SPIKE.CFG`): `base` (typing alone), `worker` (a normal-priority worker running back to back for the whole phase), `low` (the same at `THREAD_PRIORITY_LOWEST`), `coop` (the cooperative job in the main thread's idle time), `inline` (the same serialization in one go on the main thread, once), `stress` (200 or more start-and-finish cycles with verification) and `exit`.
- **Verdict.** The window is white while it runs, green when every check passed and red when one failed, so a harness that cannot read a file can read a pixel; `SPIKE.LOG` has the numbers.

New imports beyond `win9x/tools/imports.allow` (`spikes/w32-worker/new-imports.allow`): the five J13 lists (`CreateThread`, `CreateEventA`, `SetEvent`, `WaitForSingleObject`, `PostMessageA`), and for measurement only `Sleep`, `SetThreadPriority`, `GetLastError`, `QueryPerformanceCounter`, `QueryPerformanceFrequency` and `PeekMessageA`. All of them are in Windows 95's KERNEL32 and USER32.

## Checkpoint A, criterion by criterion

| # | Criterion (autosave plan) | Result |
|---|---|---|
| 1 | The round trip works 100 times in a row with no hang and no lost `WM_APP` on Wine, Windows 95 OSR2 and 98 SE, ReactOS | **Windows 95 OSR2.5: met** (1,400 verified cycles in seven runs, 1,631 jobs in all). **Wine: met** (3,000 verified cycles in six runs, 13,561 jobs). **Windows 98 SE: not run** (no image). **ReactOS: not met as it stands**: the round trip itself never hung or lost a message, and 1,100 verified stress cycles passed, but the back-to-back 1 MB phases lost 3 or 4 of about 540 jobs to a file-creation error (below) |
| 2 | The worker's code reaches only kernel32 | **Met** for `worker.rs` (`tools/check.sh`). The thread's entry function in `main.rs` makes the one `PostMessageA` call into USER32, as the J13 task specifies; a worker module that must not name USER32 would take a notification callback from the shell |
| 3 | The import and Pentium scans pass with the new imports | **Met** for `W32SPIKE.EXE`: every import is on Quill32's list or on `new-imports.allow`, and `win9x/tools/scan-pentium.sh` finds no MMX, SSE or CMOV. `QUILL32.EXE` was not changed, so its own scans were not run; adding the five J13 names to `imports.allow` is PR 12's job |
| 4 | Median key-to-paint while the worker writes 1 MB is within ±5% of no autosave at 12,000 cycles | **Not met.** +9 to +33% at lowest priority, +104 to +128% at normal priority; the cooperative job is +5 to +21% |

Checkpoint A as written is therefore a **no-go**. The next two sections show where the numbers come from and the last section says what to do with them.

## Windows 95 OSR2.5 in DOSBox-X

DOSBox-X 2025.02.01, Pentium core, `cycles=fixed` (12,000 is the plan's 486DX-33 and 3,000 its 386SX), the installer repository's Windows 95 OSR2.5 test bed (`win95-base.img`, copied for each run and the copy deleted), the program set as the shell with `WIN.INI`'s `run=`, a private Xvfb display, no host input. 640 by 480. The base image is plain OSR2.5; no Windows 98 SE image exists in the test bed, so **98 SE is not proven**. `SPIKE.LOG` is read back from the exchange disk after Windows shuts down; the window's colour is read from screenshots. The key spacing is 2.5 s at 12,000 cycles (7 s at 3,000) so that no key waits for the one before it, and a phase types 20 keys (10 at 3,000).

DOSBox-X at fixed cycles is deterministic: two of the three identical 12,000-cycle runs match to the microsecond, and the third differs slightly. The first table is the same configuration three times.

Median key-to-paint (ms), 1 MB document, each phase 20 keys:

| Run | No worker | Worker, normal priority | Worker, lowest priority | Cooperative idle job |
|---|---|---|---|---|
| 12,000 cycles, 64 MB, run 1 | 454.5 | 1,008.0 (+121.8%) | 604.2 (+32.9%) | not run |
| 12,000 cycles, 64 MB, run 2 | 454.5 | 1,008.0 (+121.8%) | 604.2 (+32.9%) | not run |
| 12,000 cycles, 64 MB, run 3 | 454.5 | 1,036.7 (+128.1%) | 585.8 (+28.9%) | not run |
| 12,000 cycles, 64 MB, later build | 454.5 | 1,021.3 (+124.7%) | 496.9 (+9.3%) | 548.9 (+20.8%) |
| 12,000 cycles, 8 MB | 454.7 | 981.0 (+115.7%) | 495.2 (+8.9%) | not run |
| 12,000 cycles, 8 MB, later build | 454.6 | 966.2 (+112.5%) | 495.6 (+9.0%) | 478.4 (+5.2%) |
| 3,000 cycles, 64 MB | 1,874.2 | 4,026.6 (+114.8%) | 2,163.4 (+15.4%) | not run |
| 3,000 cycles, 64 MB, later build | 1,875.0 | 4,001.7 (+113.4%) | 2,185.8 (+16.6%) | 2,038.0 (+8.7%) |

"Later build" is the spike after the cooperative phase and the diagnostic switches were added; its defaults are unchanged. The first 12,000-cycle runs with 64 MB differ from each other by a few per cent, and the lowest-priority figure moves between +9% and +33% from boot to boot, so no row is a precise number; **no worker figure is within ±5%**, and the one cooperative figure that comes near it (+5.2%) is on the line.

The worker's own numbers at 12,000 cycles: a 1 MB job takes about **4.4 s** back to back (serialization 4.15 s, writing 0.19 s), 230 KB a second; at 3,000 cycles **19.6 s**, 52 KB a second. The same serialization on the main thread without writing takes 3.8 s and 14.6 s. The snapshot copy that gives the worker its bytes is itself a stall on the main thread: **0.18 s** for 1 MB at 12,000 cycles and 0.78 s at 3,000, which is what `CowStore` (PR 10) is for.

The inline baseline, the whole job in one go on the main thread, blocks for **3.75 s** at 12,000 cycles (15.1 s at 3,000), and the key that arrives during it waits for the end of it: p95 key-to-paint 2,246 ms (12,002 ms at 3,000), against 590 to 850 ms for the worst key with a lowest-priority worker.

What moved the lowest-priority figure, at 12,000 cycles and 16 keys a phase:

| Variant | Lowest-priority worker, median (change from 454.0 ms with no worker) |
|---|---|
| As above | 586 to 604 ms (+29% to +33%) |
| No `WriteFile` at all | 490.8 ms (+8.1%) |
| Writes gathered into 64 KB calls | 565.8 ms (+24.6%) |
| `THREAD_PRIORITY_IDLE` instead of `LOWEST` | 555.7 ms (+22.5%) |
| `Sleep(0)` after every slice (normal priority: 741.8 ms, +63%) | 545.1 ms (+20.1%) |

## Why the worker does not buy typing latency on one CPU

The worker's CPU time does not disappear. On the machines Windows 95 runs on there is one processor, so serializing 1 MB costs 3.8 s of it however it is scheduled; a thread only decides who waits. At normal priority the main thread and the worker share time slices and the paint takes about twice as long. At lowest priority the main thread should always win, yet each paint still costs 9 to 33 per cent more: the gap closes by about 20 points when the `WriteFile` calls are removed, so most of it is the file system (Windows 95's VFAT and the emulated IDE disk are driven by the processor, and the writes probably also hold the Win16 lock that the paint's GDI calls need; that was not isolated), and the rest is the scheduler's slice (the minimum key-to-paint rises by 25 to 40 ms while any worker runs). A cooperative job, which does the same slices in the main thread's idle time and so does not compete for the scheduler at all, lands in the same band, because it pays the same file-system cost. None of this is specific to threads: it is the price of writing 1 MB on that machine. What a thread does buy is bounded worst case against a single blocking write (2.2 s against under 0.9 s above), and that is also what PR 8's abortable sliced write buys with no thread, no copy and no extra imports.

## ReactOS 0.4.16

QEMU 11.1.3 with KVM, one virtual CPU, 256 MB, a sparse copy of the golden `reactos16-golden.img` (never written; the copy was deleted), the program in `C:\SPIKE` and a one-line `.BAT` in the All Users Startup folder, no key or pointer input at all, watched through the monitor's `screendump` only, and shut down with `system_powerdown`. The log was read from the disk copy with mtools afterwards.

Median key-to-paint (ms): no worker 76.6 and 76.9; normal-priority worker 79.9 (+4.3%) and 81.5 (+5.9%); lowest priority 77.8 (+1.6%) and 77.7 (+1.0%); one inline write (80 ms block) 79.0 and 78.6; the cooperative job 80.7 and 79.9. The 77 ms floor is the guest's, not the program's. A worker job (1 MB) takes about 90 ms here (11 MB a second), so ReactOS shows what a fast machine does: every figure is within a few per cent.

The failure: in the phases that start a job the instant the last one ends (about 540 jobs), the worker lost 3 or 4 jobs per run because `CreateFileA(CREATE_ALWAYS)` on `SPIKE.TMP`, the file the previous job had just closed, failed with **error 1224, `ERROR_USER_MAPPED_FILE`** (seen in the one run that logged the code, job 165 of 313). ReactOS keeps a section open on a just-closed file for a short while. It is not about threads: the single-threaded cooperative job lost one job in the same way (the code was not logged for it), the stress phase, which creates 300 files with the verification between, never failed, and neither Wine nor Windows 95 ever did. An autosave that rewrites its file every few seconds is unlikely to hit it, but the safe save (PR 1) and the autosave (PR 8) should retry a create or a rename that returns 1224 a few times before they give up. One earlier run, where the paint was far slower than the key interval (so keys queued and the lowest-priority worker starved for 6 minutes behind the main thread's painting), is kept as `results/reactos-saturated-paint.log`; it is a mistake in the test, not a result.

## Wine

Wine 11.21 on Xvfb, a private prefix under `$HOME`, six runs of 60 keys a phase, 500 verified cycles each; and `taskset -c 0` to give Wine one host CPU.

| | No worker | Worker, normal | Worker, lowest |
|---|---|---|---|
| All cores, three runs, median (ms) | 6.39 to 6.48 | 6.45 to 6.52 (+1%) | 6.57 to 6.63 (+2%) |
| One CPU, three runs, median (ms) | 6.39 to 6.52 | 9.3 to 10.6 (+45% to +63%) | 7.9 to 10.0 (+22% to +57%) |

On every core a worker is free, on one CPU it is not, and Wine's priorities do nothing on Linux without privilege, which is why lowest looks like normal here. A 1 MB job takes about 5 ms.

## Exit while a worker runs

Two exits were tried, each with a job running (a slowed job, so that it is certain to be): the **join**, where the main thread sets the cancel flag and waits on the job's event (10 to 57 ms on Windows 95, 1 to 20 ms elsewhere, every run ending with the job's status `cancelled`, so it was running), and the **abrupt** one, `ExitProcess` with the worker still running. The abrupt exit on Windows 95 returned to the desktop with the log written and Windows shut down normally afterwards; on ReactOS and Wine the process ended and the guest shut down or the run returned. The join is what Quill's shutdown guard should do; the abrupt one shows a worker cannot hold the process open.

## Recommendation for PR 12

**No-go for a Windows 9x executor, and PR 12 can be skipped** with the plan's own fallback: `Threads` stays 0 on Win32, autosave is PR 8's inline abortable write, `CowStore` stays on the std builds, and the plan loses about four builder days and no schedule.

1. The measured cost is a no-go by the plan's own rule (criterion 4, ±5%), on every Windows 95 configuration tried (12,000 and 3,000 cycles, 8 and 64 MB).
2. The worker is not better than doing the same slices from the idle loop, which PR 8 will have: both sit in the same 5 to 33 per cent band, because the cost is the file system's and the CPU's, not the scheduler's. The one clear gain, the worst key waiting under 0.9 s rather than 2.2 s, is also what slicing with an abort flag gives.
3. A worker needs the snapshot, which costs 0.18 s of stall on a 486-class machine per 1 MB before the thread starts, and a live snapshot at 8 MB has a memory price that PR 10 would have to pay first.
4. The same ReactOS and Windows 95 runs show nothing that is a blocker for later: the five J13 imports work on Windows 95, the exit paths are safe, and a worker is free on a multi-core host.

If the owner wants threads later, the case that pays is Quill32 on a modern multi-core Windows, where the worker costs the typist nothing (Wine, all cores: +1%). A narrow PR 12 would then enable the worker only when the process reports more than one processor (a `GetSystemInfo` import; Windows 95 and 98 report one), and keep Windows 9x and Me on the inline write. It is a smaller job than the plan's PR 12 and has no urgency: 1 MB documents are rare and the inline write is the safe path.

## Not proven

- **Windows 98 SE and Me.** No image in the test bed; nothing was downloaded or installed.
- **Real hardware or a real 486.** DOSBox-X's cycle counts are a rule of thumb; Windows 95's disk code under DOSBox-X's IDE emulation may cost more or less than a real IDE disk. The numbers show the shape, not a prediction.
- **Quill's real paint.** The spike paints repeated text rows to cost 454 ms; Quill's own paint draws and lays out differently, so the ±5% figures are for this stand-in.
- **A write while the document is being edited.** The spike copies first; a design that writes from the live document and aborts on an edit (PR 8) was not part of this.
- **The exact 1224 case on the cooperative path** (the code was not logged for the one cooperative failure), and whether ReactOS fails on a rename rather than a create.
- **Windows NT 4 and later on real Windows.** Wine and ReactOS stand in for them.

## Reproducing it

```sh
spikes/w32-worker/tools/build.sh                  # dist/W32SPIKE.EXE (set CARGO_TARGET_DIR to keep the build outside the tree)
spikes/w32-worker/tools/check.sh                  # worker imports, import allowlists, Pentium scan
spikes/w32-worker/tools/wine-run.sh [--one-cpu] keys=60 keyms=100 cycles=500 paintrep=50
spikes/w32-worker/tools/w95-run.sh --cycles 12000 [--mem 8] phases=base,worker,low,coop,stress,exit keys=20 keyms=2500 paintrep=100 cycles=200 stressdoc=32768
spikes/w32-worker/tools/reactos-run.sh phases=base,worker,low,inline,stress,exit keys=20 keyms=2500 paintrep=5 cycles=300 stressdoc=65536
```

`SPIKE.CFG` keys: `doc`, `keys`, `keyms`, `cycles`, `stressdoc`, `phases`, `paintrep`, `exit` (`join` or `abrupt`), `exitslow`, `yield`, `idleprio`, `nowrite`, `wchunk`, `jobtimeout`, `hold`. The logs of every run quoted here are in `spikes/w32-worker/results/`. The Windows 95 and ReactOS scripts need Docker with the installer repository's DOSBox-X image, or QEMU, and take their images from `W95_BASE` and `ROS_GOLDEN`; nothing is downloaded.
