# Progress

What is built, how it was shown to work, what is blocked and what was assumed. Updated with every merge. Behaviour is in [SPEC.md](../SPEC.md), the order in [roadmap.md](roadmap.md), the choices in [decisions.md](decisions.md).

## Table of contents

- [Status](#status)
- [Evidence](#evidence)
- [Blocked](#blocked)
- [Assumptions](#assumptions)

## Status

| Step | State | Notes |
|---|---|---|
| T1 Register probes | Done for Windows 3.11 at the fast profile | `win16/` copied from `rust-win16` with provenance; two-task probe program; findings below. Wine, the 386SX profile and Windows ME not run |
| T4 DOS foundation | Done | `dos/` copied with `dos/PROVENANCE.md`; `QUILLDOS.EXE` draws the `quill-grid` hello screen and `dos/tools/e2e.sh` compares it with the host golden |
| M0 Foundations | Done on the host, with two items open | The workspace, `Store` and `FarVec`, the piece table with undo, the document, layout, the `.QLD` reader and writer, `quill-render`. The fuzz smoke job is not set up (needs nightly and `cargo-fuzz`); the property tests include damaged-file and random-byte inputs in its place |

## Evidence

### M0

- `make validate` passes: format, clippy with the portable-crate lint set, 16 tests under `cargo nextest`, headers, float ban.
- Property tests against a `Vec` and a `String` model run with 8-byte and 96-byte chunks, so every few operations cross a chunk edge: after any edit sequence the text and both format layers match the model, undoing every step reaches each earlier state, redoing reaches each later one, and an out-of-memory failure leaves the text as it was.
- `tests/docs/plain.qld` is built by `quill-host`'s `make-docs`, read back, laid out with `GridMeasure` and compared with `tests/golden/layout/plain.grid.txt`; read then write gives the same bytes.
- `tests/golden/pages/plain.grid.png` is the page map (see D61).

### T4

- `make dos` (or `dos/tools/e2e.sh`) builds `QUILLDOS.EXE` (about 34 KB, CWSDPMI inside) with the pinned toolchain and no C compiler, runs it under headless DOSBox Staging 0.83.0 at `cputype = 486`, and compares the screen it read back from video memory (B8000h) with `tests/golden/screens/hello.80x25.txt`: identical, characters and attributes.
- The DOS side needs `-Zbuild-std` through `RUSTC_BOOTSTRAP=1`, as Desk does; it is not part of `make validate`, which stays host-only.

### T1

Run with `win16/tools/run-w31.sh` (DOSBox-X 2025.02.01 in Docker, Xvfb, `machine=svga_s3`, `cputype=pentium`, Windows for Workgroups 3.11 in 386 Enhanced mode, `GetWinFlags=00000C29`). Logs: `docs/evidence/t1-probe-parent.log` and `t1-probe-child.log`. A second copy of the program, started with `WinExec`, yields in a loop with other patterns while the first runs every case.

- **A task switch leaves the other task's high halves in EBX, ESI, EDI and EBP.** Raw calls to `Yield`, `PeekMessage`, `GetMessage` and `DialogBox` returned `ESI=6666BE02` to a task that loaded `2222BE02`: the low half is restored, the high half belongs to the task that ran meanwhile. The child lost all four high halves about 150,000 times in 326,000 rounds. Through the runtime's import stubs, which push and pop the full 32-bit EBX, ESI, EDI and EBP, there were 0 changes in the parent's five cases (twice, with FS and GS set and clear) and 0 in the child's 163,000 stubbed rounds. So the stubs are required and enough; every exported-procedure wrapper must save the four registers too (the template's `export_proc!` does).
- **EBX is scratch in every API,** low half included (`GlobalAlloc`, `PeekMessage`, `GetMessage`, `DialogBox`, `SendMessage` all return a different EBX).
- **FS and GS do not survive** calls that enter the memory manager or the message loop (`GlobalAlloc`, `PeekMessage`, `Yield`, `GetMessage`, `DialogBox` give 0). `GetVersion` and a 12-million-iteration busy loop under timer interrupts keep them. So the far-memory helpers load FS or GS inside each helper and never hold them across a call, as `toolchain-needs.md` says.
- **ESP's high half becomes 0x8001** after the first message-loop call and stays there. It is harmless on a 16-bit stack and the stubs clear it (`movzx esp, sp`).
- **`SendMessage` to Program Manager** (another task, 16-bit code) leaves ESI, EDI and EBP alone: only a target task that uses the high halves leaks.
- **Jump tables:** a four-way `match` compiled to `jmp [ebx*4+0x10]` and its entry landed on `0001:11D0`, the middle of a `call`, which Windows reported as an illegal instruction. Jump tables are off in `win16/.cargo/config.toml` until the cause is found (the table is in the data segment while its entries are code offsets; the packer or the linker script is the suspect). T2 should look at it before multi-segment code, since a jump table inside a segment is exactly the case the plan calls safe.
- A screenshot of the fault dialog on a real Windows 3.11 desktop is in the scratch directory only; the committed screenshots start at M5.

## Blocked

- **T1 under Wine and on Windows ME:** not run. Wine's 16-bit support needs a 32-bit prefix that this machine does not have set up, and the ME VM was left alone until a Win32 program needs it.
- **T1 at the 386SX profile:** not run; the busy loop is sized for the fast profile and would take minutes at 3000 cycles.

## Assumptions

D58 to D62 in [decisions.md](decisions.md) were made while implementing M0.
