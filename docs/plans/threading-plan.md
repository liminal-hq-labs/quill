# Quill threading plan: one job model for single-core and threaded builds

Written 2026-11-24 by an Opus planning agent (read only), saved by the orchestrator. Based on `origin/main` at ee39d9f, the `feat/spell-live` worktree (e5aed32 plus uncommitted edits), Desk at 4a021a7, and the planning documents in this folder. Where the plan depends on the web and layering addendum (the request outbox, the `quill-session` driver, the single-threaded web build) it worked from a summary, because the addendum was not yet saved.

Nothing was built or run. **[V]** means checked in code or a document. **[U]** means not checked. **[G]** means a guess or an estimate.

Status: reference plan. Quill is to be declared done enough after the GTK4 and WinUI 3 clients, so only the early, shared steps (J0 to J5, reflow) are candidates before that. The threaded executors (J12 onward) are later work. Decision ids D250–D259 are legacy numbers kept as written; under the new prefix scheme they would be `CORE-` decisions.

## 0. The answer in one paragraph

The most important finding is not about threads. On every build, `App::lines` (`crates/quill-app/src/lib.rs:909`) runs `layout_story` over the whole body, from the first character, every time anything asks about lines. One paint asks several times: `caret_line`, then two or three `lines` passes in `paint.rs:386,433,438` [V]. The spell-live benchmark on Windows 95 measured one page paint at **0.64 s** at 12,000 DOSBox cycles (about a 486DX-33) and a median key-to-paint of **1.08 s**. Its notes say "each paint lays the document out more than once" (`wt-spell-live/docs/progress.md:139`, uncommitted) [V]. A thread does not fix that on a one-CPU machine with no threads, so the first work is to stop doing it: a resumable layout cursor and a paragraph checkpoint cache.

After that, the model is:

- **The document, the undo log, `App` and every mutation stay on one thread and stay deterministic.**
- **Everything else is a budgeted job:** spelling, background layout, word count, dictionary load, pictures, autosave, export and print. A job is a resumable `step(budget)` in abstract work units, not milliseconds.
- **One scheduler runs the jobs either way.** Each shell either runs it inline in idle slices (DOS, Windows 3.x, the web main thread, Windows 95 on small machines) or hands detached jobs to worker threads (Windows 95 on larger machines, GTK, WinUI, a Web Worker, Desk).
- **Workers never touch the live document.** They read an immutable snapshot (a copy-on-write store and a copy of the document's header).
- **Results are caches or files, never edits.** The UI thread applies them only if the document revision they were made for still holds.

## 1. Inventory: work that blocks the UI thread today

| # | Work | Where | What happens today | Cost the repo gives | Off-UI candidate? |
|---|---|---|---|---|---|
| 1 | **Layout and reflow** | `quill-core/src/layout.rs:287` `layout_story`; `quill-app/src/lib.rs:909` `lines` | Full layout from position 0 for every query: `caret_line`, `line_for`, `line_at`, `line_count`, `scroll`, each paint pass, pointer hit tests (`pointer.rs:45,218`) and view geometry (`view.rs:505,530`) [V]. `lines` also clones the measure each call (for `OutlineMeasure` that clones a `FontSet`) and builds a new `FontCache` asking for 224 widths per font again [V for code; cost U] | One page paint 0.64 s, key-to-paint 1.08 s median at 12,000 cycles on Windows 95 (spell-live bench, uncommitted). SPEC: "a 1 MB document is possible but slow to lay out on a 386SX" (`SPEC.md:544`) | **First fix: incremental reflow with checkpoints.** Then layout of off-screen pages as a job (page count, scroll bar, Page Count field) |
| 2 | **Open and parse RTF, QLD, text** | `lib.rs:834` `App::open`; `quill-formats` | Parses into a fresh `Doc` in the same store, then swaps (`lib.rs:845-860`) [V]. Runs to completion with no progress and no Cancel | No timing in the repo [U]. 1 MB limit (`MAX_TEXT` 1,048,575) | Threaded: file I/O on a worker, parse on the UI thread. Cooperative: a progress hook (D259) |
| 3 | **Spell-check (F7)** | `spelling.rs:217` `spell_next` | Scans from the caret to the next unknown word; on a clean document, the whole text in one call [V] | 2.7 ms a word at 12,000 cycles; 0.6 to 1.35 µs on the host. A clean 170,000-word document is about 7.7 minutes at 12,000 cycles [G, arithmetic] | Inline job with "Checking…" status and Esc |
| 4 | **Spelling as you type** (`feat/spell-live`) | `spell_live.rs:555` `spell_idle(s, budget)` | Already a budgeted inline job: N words a call, a 64-mark table in one `Store` chunk, edits shift marks. Windows 95 drives it with a `WM_TIMER` (D190, D191) [V] | 16 ms for a six-word batch at 12,000 cycles; about 5% added to key-to-paint, mostly drawing the waves [V, uncommitted] | The template for everything else (see §3h) |
| 5 | **Dictionary load** | `spelling.rs:112` → `Dict::load` | Reads 300,086 bytes into `Store` chunks at first use, in one call [V] | About 1 s from a 386 disk (estimate); 270 ms from a DOSBox image | Resumable, one 32 KB chunk per step (about 10 steps) |
| 6 | **Suggestions** | `quill-spell` `suggest`, `BUDGET = 40_000` nodes (`spell_live.rs:28`) | Already a deterministic budget in nodes (`testing.md:28`) [V] | — | Precedent for budgets in work units |
| 7 | **Picture decode, scale, dither** | `picture.rs:64` `render`; `quill-image`; `quill-inflate` | Push callbacks that run to completion; DOS graphics decodes on every paint (`page.rs:338-359`) [V]; Win9x caches three (`gdi.rs:160`); Win16 does BMP only through `StretchDIBits` | PNG working set about 126 KB, BMP about 41 KB. On a 386SX with 4 MB and paging, pictures up to 1024 × 768 drew in about 7 minutes of wall clock against 30 s [V] | Detached job into the picture cache. Cooperative: one whole picture per idle slice, once, instead of on every paint |
| 8 | **PNG inflate** | `quill-inflate` | 32 KB window from the caller; not resumable mid-stream [V] | — | As item 7; a state-machine inflate is a separate later PR |
| 9 | **Save and safe save** | `lib.rs:876` `save`, `:1676` `do_save` | Writes run to completion; safe save is missing on DOS, Win16 and Win9x [V] | 50 KB to a 386's hard disk 0.2 to 0.5 s; a floppy 30 to 50 KB/s (estimates) | Retro: synchronous with a progress hook. Modern: snapshot, write on a worker, rename, mark clean at that revision |
| 10 | **Autosave and recovery** (planned) | Linux plan Part B item 5 | Not built | As item 9 | Retro: inline, sliced by RIFF chunk, abandoned when a key arrives. Modern: snapshot on a worker |
| 11 | **Print (text)** | `print.rs:96` `print_pages` | **Lays out the whole document once per page** (`print.rs:119-142`) [V]: a 100-page document is 100 full layouts | [U] | Fix with the layout cache (item 1); then one job, one page per step |
| 12 | **Print (GDI) and Print Preview** | Win9x `main.rs:704` `print_job`; banded pictures (`gdi.rs:258-283`) | Synchronous; "Not done: a Cancel dialog between pages" (`progress.md:18`) [V] | [U] | Cooperative page steps with a Cancel dialog (the SPEC already asks for one). Workers may not call GDI |
| 13 | **PDF export and print** (GTK G6, WinUI W10, `quill-cli to-pdf`) | Planned; `quill-raster` (PX1) | Not built | [U] | Detached job over a snapshot; pages are independent, so they can run in parallel |
| 14 | **Font loading** (Linux) | `quill-fonts/src/set.rs:113` `FontSet::system` | Reads and parses every font file under the system folders at once (`set.rs:131-150`) [V] | [U]; probably hundreds of ms or more [G] | A worker scan plus an index cache before the first layout needing a non-vendored face |
| 15 | **Glyph outlines and rasterizing** | `quill-fonts` measure; PX1 raster surface | Widths only on main; glyph drawing is on the PX1 branch | [U] | A glyph cache first; threads second |
| 16 | **Tile painting of large pages** | `quill-paint` `paint_page(…, clip)` | One painter, clipped [V]. An RGBA page at 150% is about 7.8 MB [G, arithmetic] | [U] | Fork-join bands with `std::thread::scope` on std shells only, after a measurement says it is needed |
| 17 | **Find and Replace All** | `find.rs:112` `search`, `:215` `replace_all` | Linear scan through 32 KB reads; Replace All edits in a loop [V] | [U] | Find Next: inline scan job, selection applied at the same revision. **Replace All stays synchronous**, because it edits |
| 18 | **Word count** (planned) | Linux plan item 4 | Not built | 1 to 2 s on a 386 for 1 MB (estimate) | Inline job; a status-bar field on modern builds only |
| 19 | **Directory listing** (DOS file dialog) | `dos/rt` `find_each` | Synchronous | 512 entries about 1/8 s of overhead at 3,000 cycles; a whole-tree walk 1.57 s [V] | Inline job that fills the list as it goes |

Four things the inventory shows:

- The **document is single-threaded by construction.** `Text` holds `cursor: Cell<(u32, u32)>` (`text.rs:252`) [V], so `Doc` and `App` are not `Sync`. That is fine, because they should never be shared.
- The text **bytes are append-only.** `ByteBuf` has no in-place writes on the editing path [V]. The format tables are append-only interning. Only the piece list, the undo and redo logs and the picture records change in place. A snapshot therefore costs a copy of a few kilobytes of headers plus the chunks written while it lives, not a copy of the document.
- **Layout can resume at a paragraph boundary.** Between paragraphs, `layout_story` keeps only `(pos, para, page, y)` and a font cache (`layout.rs:298-301`) [V].
- **Budgets in work units already exist** twice: the suggestion node budget and `spell_idle`'s word budget.

## 2. Platform capability matrix

| Platform | Threads | Ways to get work into gaps | Limits | What it means for jobs |
|---|---|---|---|---|
| **DOS** (CWSDPMI, DPMI 0.9) | None | The main loop blocks in INT 16h AH=00 (`keys.rs:14-17`) [V]. It can poll AH=01 for a waiting key and run steps until one arrives, releasing the slice with INT 2Fh AX=1680h when idle [U on CWSDPMI] | Target is 4 MB; HAL forbids real work in interrupt handlers (`dos/hal/hal/src/irq.rs:13-15`) [V] | **Inline only.** The PIT tick can only end a budget early, never run a job. BIOS tick resolution is 54.9 ms |
| **Windows 3.x** | None (cooperative tasks) | `GetMessage` loop today; `PeekMessage` is bound; `WM_TIMER` (D191); `GetInputState` to stop a slice when input waits [U: not imported yet] | Data segment 2,660 bytes free after #51; stack 29,696 with 6,042 spare; code segments 6 and 8 within 250 bytes of budget [V]. One `SS`: no fibers | **Inline only**, under 100 bytes of static state per job, tables in far `Store` chunks. Never work in a `TimerProc` (ticks dirty ESP's high half); use `WM_TIMER` through the loop |
| **Windows 95/98/Me** | Real threads | `WM_TIMER` slices (D191); `GetInputState` [U] | `i586`, no C runtime, no thread-local, 64-bit atomics (`win9x.json`) [V]; the process heap is serialized and thread-safe; imports limited by `imports.allow`; `PostMessageA` not yet allowed | **Inline or one worker.** `CreateThread` is correct without a C runtime [G]. Workers must never call USER or GDI: the Win16Mutex serializes them and a worker in GDI stalls the UI thread [G] |
| **Linux tty** | std threads | `crossterm::event::poll(timeout)` | Plentiful | Inline is enough; a pool for autosave and dictionary load if wanted |
| **Linux GTK4** | std threads; GLib main loop | `glib::idle_add_local` (idle priority runs after GDK's redraw) [G]; the spike already uses `timeout_add_local_once` [V] | GTK objects are not `Send` | Inline slices for small jobs; a std pool for detached jobs; fork-join band painting possible |
| **WinUI 3** | Free threads; UI-thread affinity | `DispatcherQueue.TryEnqueue` at Low priority [U whether Reactor exposes it]; `SwapChainPanel` `Rendering` events [V] | The s0b spike's `LocalSender` is UI-thread only [V], so a `Send` sender is needed [U] | As GTK |
| **Web** (planned) | Main thread only; no `SharedArrayBuffer` on Pages | `requestIdleCallback` where present, else `MessageChannel` or `setTimeout(0)` [U on Safari] | Wasm panics trap | Inline on the main thread. Owned-bytes tasks may go to a Web Worker with `postMessage` and transferable buffers; a trap there falls back to inline |
| **Desk OS** (design only) | Native: preemptive 1:1 kernel threads, 10 ms slice. Hosted: cooperative green threads behind the same API (`docs/APP-MODEL.md:34-36,77-91`) [V]. "Nothing in it is built" | `desk_timer` events; Quill's Desk page asks for an idle or low-priority event | Messages of 32-byte records; workers default to priority 12; basic priority inheritance on mutexes | Same model: a low-priority worker on native, a green thread yielding after each step on hosted. Desk's "long work goes on a worker thread … `yield_now()`" is exactly `step(budget)` |

## 3. The model

### 3a. Rules

1. **One owner.** `App`, `Doc`, the piece table, the undo and redo logs, the caret and selection, the store's mutating calls, dialogs and flows, and clipboard ownership live on the UI thread. `App` is never `Send` or `Sync`.
2. **Jobs never edit the document.** A job produces a cache (spelling marks, layout checkpoints, the page count, picture bits, a word count) or a file (autosave, export, print output). Commands that edit (Replace All, Change All, a spelling replacement) stay synchronous. So the document after any sequence of inputs is the same whatever the scheduling, and the goldens stay valid.
3. **Budgets are work units, not time.** Units are words, paragraphs, picture rows, store chunks or pages. A shell turns its time slice into units by measuring (units per ms, smoothed), so tests are deterministic and machines adapt.
4. **Every result carries the revision it was made for.** `Doc::revision()` is a `u32` bumped by every mutation. A stale result is rebased (marks shifted by the edit log, as `spell_note_edit` does today) or dropped.
5. **Cancellation is cheap and always allowed.** An inline job is simply not stepped again. A detached job sees a cancel flag between steps.
6. **The UI thread never waits on a worker.** It polls or is woken; it never joins on the input path.

### 3b. Two kinds of job, one scheduler

**Inline jobs** run on the UI thread against the live document. Each is a small state machine inside `App`, stepped by `App::idle`:

```rust
// quill-app (L2): one entry point every shell calls when input is quiet.
pub fn idle<S: Store + ?Sized, F: Files>(&mut self, s: &mut S, f: &mut F, b: &mut Budget) -> Idle;
pub enum Idle { Done, More, Wait }        // Wait: waiting on a detached result
pub fn take_damage(&mut self) -> Option<(u32, u32)>;   // generalises spell_take_changed
```

The order inside `idle` is fixed, so it is deterministic:

1. the reflow of the edited paragraph's tail;
2. spelling of visible words;
3. layout of the visible pages;
4. layout of the rest (page count, scroll bar);
5. word count, if a dialog or status field wants it;
6. resumable dictionary load;
7. picture cache prefetch requests;
8. autosave preparation.

Static cost: one small struct per job, under 100 bytes each, with tables in `Store` chunks (D190's rule).

**Detached jobs** own their inputs and can run anywhere. `App` asks for them through the outbox (`App::take_request()`, from the addendum) and is told the result through `App::complete(s, Completion)`:

```rust
// quill-job (L1 base): no_std, no alloc in its core.
pub struct Budget { left: u32 }
impl Budget { pub fn take(&mut self, n: u32) -> bool; pub fn spent(&self) -> bool; }

pub enum Poll<T> { Ready(T), Pending }

/// A job that reads a document view and owns everything else it needs.
pub trait Task<R: ReadStore + ?Sized> {
	type Out;
	fn step(&mut self, view: DocView<'_, R>, b: &mut Budget) -> Poll<Self::Out>;
}

#[derive(Copy, Clone)]
pub struct Ticket { pub kind: u8, pub slot: u8, pub rev: u32 }
pub struct Cancel(core::sync::atomic::AtomicU32);   // a generation; 32-bit atomics exist on every target
```

The same `Task` code runs two ways:

- **Inline executor** (every build). The shell keeps a fixed array of task slots, using its own enum because there is no `Box<dyn>` without `alloc`. In idle time it steps them with `DocView { doc: &app.doc, store: &live_store }`. The executor checks `ticket.rev` against `app.revision()` before each step, and either cancels the task or lets it continue if the task survives edits. A picture decode survives edits, because the picture pool is append-only and the result is keyed by fingerprint (`picture.rs:43`).
- **Thread executor** (feature `alloc` with a shell-supplied spawn, or feature `std`). The task is built with an owned `Snapshot` (§3c), sent to a worker, stepped in slices (so cancellation and Desk's `yield_now` work), and its result is posted back. The UI thread applies results at a defined point between input events, in ticket order.

Two fallbacks for work that cannot be made resumable yet:

- **Progress hook (D259)**, for run-to-completion operations on cooperative builds: open, a user's Save, print and Find Next. These take `&mut impl Progress` with `fn tick(&mut self, done: u32, total: u32) -> bool`. The shell's implementation peeks only for Esc or the Cancel button and repaints a status line from data it already holds. It never re-enters `App`. This is the classic Windows 3.x pattern, without its re-entrancy hazards.
- **One-shot inline**: a task whose single step is the whole job, such as a PNG decode. On DOS this means one stall per picture, once, into the cache, instead of a decode on every paint as now. A resumable inflate comes later.

### 3c. Snapshots, `Send`/`Sync` and the `Store` trait

`Store` today mixes reading and writing (`quill-core/src/store.rs:13-25`) [V]. The plan:

- **D253: split out `ReadStore`.** Move `read(&self, id, offset, out)` into `pub trait ReadStore`, make `Store: ReadStore`, and relax every pure reader to `S: ReadStore + ?Sized`: layout, `walk_line`, the writers `qld::write`, `rtf::write` and `text::write`, `Dict::known` and `suggest`, `picture::render`, the painter and `print`. Every existing `impl Store` gains a separate `impl ReadStore` block, and no call site changes.
- **`DocView<'a, R>`**: `&'a Doc` plus `&'a R`. Because a view has no `&mut Store`, the compiler rejects any mutating call through it.
- **`CowStore` in `quill-heap`** (feature `alloc`): chunks are `Arc<[u8]>`-backed. `write` calls `Arc::make_mut`, so a chunk is copied only when a snapshot shares it. `snapshot(&self) -> SnapView` copies the chunk table (one `Arc` clone per chunk; a 1 MB document with pictures is about 150 chunks). `SnapView: ReadStore + Send + Sync`. It is used by Win9x (threaded mode), GTK, WinUI, tty, cli, Desk native and web. DOS and Win16 keep `HeapStore` and `FarStore` and never take snapshots.
- **`Doc::header_copy()`**: a shallow copy of the `Doc` struct (`ChunkId` arrays and counts, a few kilobytes [G]). The copy has its own `Cell` cursor, so the `!Sync` `Cell` is never shared. `Snapshot { doc: Doc, store: SnapView, rev: u32 }` is `Send`.
- **Copy-on-write cost while a snapshot lives**: typing writes the add buffer's last chunk, one piece-list chunk and one undo chunk, at most about 3 × 32 KB copied once per snapshot [G].
- **Rule:** at most one live snapshot on machines under 16 MB.

### 3d. What stays single-threaded and what is parallelised

| Stays on the UI thread (always) | Runs as an inline job (every build) | Detached: on a worker where threads exist, inline otherwise | Fork-join on std builds only |
|---|---|---|---|
| Piece table, undo and redo, `Doc` mutations, `App` commands and keys, flows and dialogs, clipboard ownership, Replace All and Change All, applying results | Reflow tail, spell-live, visible and background layout, F7's scan to the next word, word count, dictionary load (per chunk), Find Next scan, DOS directory listing, print pagination | Picture decode, scale and dither into `quill-picture`; autosave (snapshot to temp file to rename); export to RTF, PDF or text; PDF and print page rendering; dictionary file I/O; Linux font scan and index | Band painting in `quill-raster` (`std::thread::scope`, disjoint bands, each with its own `Doc::header_copy()` over a `SnapView`); parallel PDF pages; word count over chunk ranges for very large documents |

### 3e. Crates, layers and dependency direction

- **`quill-job` is a new L1 base crate.** It knows no document, only `ReadStore` and `DocView`'s shape through a type parameter. Its core is `no_std` without `alloc`. Feature `alloc` adds an `Arc`-based single-producer mailbox (Win9x and Desk). Feature `std` adds a `Pool` (GTK, WinUI, tty, cli, host). It has two users the day it lands (`quill-app` and `quill-host`'s simulator), which satisfies the two-user rule.
- **Scheduling policy lives in `quill-session`** (L2 top), not in `quill-job`: which jobs, how many units, inline or worker, and the memory admission rules. Every shell drives one `session.pump(now_ms, input_waiting)` instead of copying the D191 timer logic. `quill-job` stays mechanism only.
- **`DocView` and `Snapshot`** are defined in `quill-core`, because they need `Doc`. `quill-job`'s `Task` is generic over the view type, so `quill-job` does not depend on `quill-core`.
- Dependency direction: shells → kit (`quill-raster`, `quill-picture`) → `quill-session` → `quill-app` → `quill-core` → `quill-base`/`quill-heap`; `quill-app`, `quill-session` and `quill-base` use `quill-job`.

### 3f. Alternatives, judged on `no_std`, 16-bit targets, determinism and wasm

| Approach | Verdict |
|---|---|
| **Budgeted resumable jobs (this plan)** | **Recommended.** `no_std`, state in `App` and `Store`, no extra stacks, deterministic by units, works on the web main thread with a worker for owned-bytes tasks |
| `async`/`await` with a static executor (embassy-style) | Not now. Futures hold live locals across awaits: inflate's tables and the 32 KB window would sit in each future, in near memory on a 64 KB data segment with code segments 250 bytes from budget [G on sizes]. Backtraces through state machines also hurt `ne_addr2sym.py` debugging. Worth a second look for decoders on std builds only |
| Message-passing actors (the document as an actor) | Every key would become a round trip, which breaks the synchronous `App::key` contract all shells and goldens rely on. Use only at the edge (worker pool, Web Worker, Desk messages) |
| Scoped threads (`std::thread::scope`) | Yes for band painting and PDF pages in L3 std crates; never in L1 or L2 |
| Rayon-style data parallelism | Not needed: a few bands with `scope` do the same without a large dependency |
| Green threads or fibers | Rejected for Quill: Win16 has one `SS` and 6 KB of spare stack. Desk's own hosted threads can still run Quill's tasks |
| Work in timer interrupts (DOS) or `TimerProc` (Win16) | Rejected. The HAL forbids it, and Win16 timer ticks dirty ESP's high half. Ticks may only end a budget |
| `PeekMessage` pumping inside long operations | Only as the restricted progress hook (D259). Re-entrancy into `App` through the static `SHELL` pointer is undefined behaviour |

### 3g. Memory on 4 MB machines

- **DOS and Win16**: no snapshots, no worker stacks, no second store. Static job state under 100 bytes each. Layout checkpoints are 16 bytes a paragraph in `Store` chunks (`{pos, para, page, y}`): a 1 MB document of about 5,000 paragraphs is about 80 KB [G], against roughly 640 KB for a full line table [G]. Lines are laid out on demand from the nearest checkpoint and only a screenful is kept near.
- **Win9x**: threaded mode only when `GlobalMemoryStatus` reports at least 8 MB of physical memory and `[Quill] Threads` is not 0. One worker with a 64 KB stack [G]. One live snapshot, copy-on-write at most about 96 KB. A picture job waits if the picture memory budget would be passed. Otherwise it runs inline exactly like Windows 3.x.
- **Modern builds**: pool size `min(available_parallelism - 1, 4)`; one snapshot per revision shared by all tasks of that revision.

### 3h. How spell-live (`feat/spell-live`) adopts this

It is already the right shape: `spell_idle(s, budget) -> bool`, state under 100 bytes, marks in one `Store` chunk, an edit log that shifts marks, a Win95 timer driver, random-budget property tests, and a quiet-after-each-key golden directive `@live` [V]. Don't change course. Three small requests for the builder now, so adoption later is a move:

1. Keep the budget as a parameter in units, and keep the "more work?" return.
2. Keep all shell timing (300 ms arm, 55 ms tick, D191) in the shell, none of it in `App`.
3. Keep `spell_note_edit`'s edit description (`pos`, `removed`, `inserted`) a plain struct, because the layout checkpoints will read the same edit log.

Later, in J7: `spell_idle` becomes step 2 of `App::idle` with `Budget` in place of `u32`; D190's "a different length or story drops every mark" heuristic gives way to `Doc::revision()`; `spell_take_changed` becomes `take_damage`; Win95's `TIMER_SPELL` becomes the session's single idle timer. Spell-live stays inline even on threaded builds, because lookups are 1.35 µs on the host and a thread would cost more than it saves.

### 3i. Test strategy

- **Deterministic scheduler `quill_job::Sim`** (seeded): random budgets for inline jobs, random delays, orders and staleness for detached completions, random cancellations. The same seed gives the same trace, printed on failure.
- **Interleaving properties** (`quill-host/tests/jobs.rs`): for random edit scripts × random schedules, after `drain()` the document bytes, the marks, the checkpoint table, the page count, the picture-cache keys and the autosave bytes must equal the eager oracle ("run every job to completion after each event").
- **Cache-equals-recompute**: checkpoint layout must match full `layout_story` line for line on every `tests/docs` file after random edits; resumable `layout_from` split at random paragraph points must match `layout_story`.
- **Goldens unchanged**: `quill-host` calls `drain()` before each frame dump, so every existing screen, layout, flow and page golden is byte-identical. A new `@sched seed=N` directive replays any scenario under a random schedule, and CI runs each scenario under three seeds.
- **Thread executors**: host `Pool` stress tests at 10,000 random cancels; `loom` tests for the mailbox and `Cancel` [U whether `loom` builds here]; `[Quill] Threads=0` forces inline on every build for bisecting.
- **Platform measures** in each PR: Win95 key-to-paint through `w95-spell-bench.sh` at 3,000 and 12,000 cycles; DOS `e2e.sh` byte-equal plus an AH=01 latency log; Win16 `STACK.LOG` margin ≥ 2 KB and data-segment delta; Wine e2e; GTK and WinUI frame-time logs.

### 3j. Risks

| Risk | Where | Mitigation |
|---|---|---|
| Priority inversion | Win9x serialized process heap; the Win16Mutex if a worker touches GDI; Desk mutexes | Workers allocate up front and never call USER or GDI; worker at below-normal priority [U: `SetThreadPriority` not on the allowlist]; Desk worker priority 12 or lower; the UI never blocks on a worker |
| Caret latency | Every inline build | The keystroke path never runs jobs. A slice stops at the first `GetInputState` or AH=01 hit. A slice is at most one 55 ms tick on 386s and at most 8 ms on Pentiums [G]. The 0.64 s paint is the real latency, so J4 comes first |
| Memory spikes | Snapshots, decoders (126 KB for PNG), worker stacks | One snapshot under 16 MB; admission by picture memory; pool sizes |
| Stale results | All threaded builds | Revision tickets; a property test with stale completions injected by `Sim` |
| Debuggability | Threads, wasm traps | `Threads=0`; a 64-entry trace ring written by the emergency-save path; wasm worker traps fall back to inline and are logged |
| Code size | Win16 segments within 250 bytes | Measure `quill-job`'s inline core (target under 1 KB [G]); the thread executor is never linked on Win16 |
| Wine versus real Windows 95 | Win9x threads | A real Windows 95 OSR2.5 run in DOSBox-X in J13's pass criteria, not Wine alone |
| The addendum's outbox API differs | J12 | J12 waits for the addendum to merge; the inline jobs (J1 to J10) do not need it |

## 4. PRs (each sized for one Sonnet builder)

The usual gates apply to each PR that touches linked code: `make validate`, `check-layers.py`, the size report, the Win16, DOS and Win9x gates, and an empty `git diff tests/golden` unless stated.

| PR | Scope | Measured pass criteria | After |
|---|---|---|---|
| **J0 spike** (report plus throwaway code) | (a) Count `layout_story` calls and paragraph visits per key in `m2-letter` and a generated 1 MB document, with host timings. (b) Prototype `layout_from(cursor)` and a checkpoint cache. (c) Prototype `CowStore`, snapshot a 1 MB document and write it on a std thread while 1,000 random edits run. (d) A Win95 `CreateThread` worker that posts `WM_APP`, under Wine and on Windows 95 in DOSBox-X, with the import and Pentium scans | A report with per-key layout calls before and after (target: from about N × document to at most 3 paragraphs [G]). Checkpoint lines equal full layout for every `tests/docs` file. The snapshot write's bytes equal a write of the pre-edit document. The Windows 95 worker round trip works on real Windows 95 | Now (new files only) |
| **J1** `Doc::revision()` | `u32`, wrapping, bumped by every mutating `Text`/`Doc` API | Property test: every mutating call bumps it and no reading call does. Win16 data delta ≤ +8 B | J0 |
| **J2** `ReadStore` split (D253) | Mechanical: `Store: ReadStore`; readers relaxed | Pure-move rule; Win16 code delta within ±64 B | J1 |
| **J3** `LayoutCursor` and `layout_from` in `quill-core` | Resumable at paragraph boundaries; `layout_story` becomes a loop over it | Property test: concatenating random splits equals `layout_story`; layout goldens unchanged | J2 |
| **J4** Checkpoint cache in `quill-app` (D255) | `lines`, `line_for`, `line_at`, `line_count` and the painters go through the cache; the edit log invalidates forward until a checkpoint converges; no measure clone per call | Paragraph visits per typed key in `m2-letter` ≤ 3. Cache equals full layout after 10,000 random edits. Goldens unchanged. Win95 key-to-paint median at 12,000 cycles ≤ 0.4 s against 1.08 s [target G]. Win16 `STACK.LOG` ≥ 2 KB, data delta ≤ +64 B | J3 |
| **J5** Linear print | `print_pages` uses the cache: one pass, not one per page | Print goldens unchanged; layout passes per print = 1 (counter) | J4 |
| **J6** `quill-job` crate (D254) | `Budget`, `Poll`, `Task`, `Ticket`, `Cancel`, `Inline<N>`, `Sim`; metadata `layer = "base"` | `no_std` build for `x86_64-unknown-none`; `Sim` gives the same trace for the same seed; no `alloc` without the feature | J2 |
| **J7** `App::idle` and spell-live under it | Fixed job order; `take_damage`; `drain()` and `@sched` in `quill-host` | All goldens unchanged; the `@sched` property over every scenario under three seeds; spell-live proptests pass unchanged | J4, J6, `feat/spell-live` merged |
| **J8** Shell idle drivers | DOS: AH=01 poll plus 1680h. Win16: `WM_TIMER` plus `GetInputState`. tty: `event::poll`. Win9x: D191's timer generalised to `session.pump` | DOS e2e byte-equal; Win16 `m1-edit` golden on Windows 3.11; import check passes with `GetInputState` added; a logged key wait during a slice ≤ one tick at 3,000 cycles | J7 |
| **J9** Inline jobs: background layout, word count, resumable dictionary load | Progress shown in the status line | Word count equals the oracle on `tests/docs`; the dictionary loads in ≤ 10 steps; no step over its unit budget (counter) | J7 |
| **J10** Progress hook (D259) for open, save, print and Find Next on DOS, Win16 and Win9x inline | Esc and Cancel; no re-entry into `App` | Flow transcripts unchanged; a new flow `cancel-print` shows the document unchanged and no file written; the SPEC's print Cancel dialog on Win9x | J8 |
| **J11** `CowStore`, `SnapView`, `Doc::header_copy` (D256) | In `quill-heap` (`alloc`) | Property test: a snapshot reads the pre-edit document after random edits; bytes copied per typed key ≤ 3 chunks; leak check back to baseline | J2 |
| **J12** Detached tasks through the outbox: autosave, picture decode | `Request::{Autosave, Decode}`, `App::complete`, the std `Pool`, `quill-picture` fill | `Sim` interleaving property including stale and cancelled completions; autosave bytes equal `qld::write` at the ticket's revision; recovery flow script | J11, the outbox, P2 |
| **J13** Win9x worker executor (D257) | `CreateThread`, `CreateEventA`, `SetEvent`, `WaitForSingleObject`, `PostMessageA` on `imports.allow`; threads on at ≥ 8 MB | Import and Pentium scans and Wine e2e unchanged; on Windows 95 in DOSBox-X with 8 MB, autosave of a 1 MB document while typing without a rise in median key-to-paint (±5%); with 4 MB it runs inline | J12 |
| **J14** Modern executors | GTK: `idle_add_local` plus pool and channel. WinUI: DispatcherQueue plus pool. Web: Worker transport | Each client's frame-time log shows no idle slice over 8 ms; a worker trap on the web falls back to inline | J12 and each client's series |
| **J15** Band painting in `quill-raster` (optional) | `std::thread::scope` over bands | Only if a measurement shows a full page paint over 8 ms at 2×. Pixel goldens identical at scales 1.0 to 3.0 | PX1, J11 |

### Decisions D250–D259

- **D250**: The core stays single-threaded and deterministic; jobs never edit the document; results are caches or files.
- **D251**: Budgets are counted in work units, not time; shells convert milliseconds to units.
- **D252**: `Doc::revision()`; results are applied only at their revision (rebased through the edit log or dropped); the clean mark is a revision.
- **D253**: The `ReadStore` split; pure readers take `ReadStore`.
- **D254**: `quill-job` in L1 base (mechanism), with policy in `quill-session` (L2 top).
- **D255**: Incremental reflow through paragraph checkpoints in `Store` chunks, not a full line table.
- **D256**: Snapshots are `CowStore` plus `Doc::header_copy`; at most one live snapshot under 16 MB.
- **D257**: Executors by platform: inline on DOS, Win16 and the web main thread; a Win9x worker at ≥ 8 MB with an INI override; std pools on GTK, WinUI and tty; a Web Worker for owned-bytes tasks; Desk threads (low priority native, yielding hosted).
- **D258**: Testing: `drain()` before frames, `Sim` interleaving properties, `@sched`, and `[Quill] Threads=0`.
- **D259**: The progress hook for run-to-completion operations on cooperative builds: Cancel only, no re-entry into `App`.

## 5. What improves for users

| Platform | What gets better |
|---|---|
| **DOS** | Typing and scrolling stop re-laying out the whole document (J4), the largest gain anywhere. Pictures in graphics mode decode once, not on every paint. Esc cancels a long print, open or spelling scan. The file dialog list fills as it reads. Under Windows or OS/2, Quill gives back idle CPU |
| **Windows 3.x** | The same reflow gain inside 4 MB (checkpoints about 80 KB for a 1 MB document [G]). Spelling as you type becomes affordable when turned on. Printing gets its Cancel dialog. The page count and word count arrive in idle time instead of stalling |
| **Windows 95/98/Me** | Key-to-paint drops from about 1 s toward a few tenths on a 486 [target]. On ≥ 8 MB machines, autosave, picture decode and export run on a worker without pausing typing. Print Cancel between pages. On 4 MB machines it behaves like Windows 3.x, no worse |
| **Linux (tty, GTK)** | The font scan moves off startup; autosave every 2 minutes with no hitch; PDF export and print on a worker with progress; spelling and word count live in the status bar; band painting at HiDPI if measured necessary |
| **WinUI 3** | As GTK: worker export, print and autosave, no UI-thread stalls under the dispatcher, and a later option to present from a render thread |
| **Web** | The main thread never freezes on open, export or pictures; a crash in the Worker falls back to inline instead of killing the page |
| **Desk OS** | Quill fits Desk's own rule: low-priority workers on native, yielding steps on hosted, the same code |

## Decisions needed from you

1. **Do reflow before threads.** J3 to J5 come before any worker work, which puts them ahead of the GTK4 and WinUI build-out in the queue.
2. **Jobs never edit the document.** Replace All and Change All stay synchronous, with a progress hook.
3. Windows 95 threads: **on automatically at ≥ 8 MB physical memory, `[Quill] Threads=0/1` to override**, or always off.
4. **A new crate, `quill-job` (L1), with policy in `quill-session`**, rather than all of it inside the session driver.
5. A user's Save on GTK and WinUI: **asynchronous from a snapshot**, with the clean mark at its revision. On retro builds Save stays synchronous with a progress line.
6. Linux fonts: **a worker scan plus an index cache in `$XDG_CACHE_HOME`**, or fontconfig. That decision belongs to the Linux plan too.
7. Web: **stay single-threaded plus an optional Worker**; do not use the service-worker trick for COOP/COEP.
8. Resumable inflate (a state machine): **later**, after the picture cache, since only DOS graphics would gain from it.
9. Confirm the decision range D250–D259 (or move to the new prefix scheme).

Open checks before anything relies on it: whether Reactor exposes a `Send`-able DispatcherQueue handle [U]; glib 0.22's channel API [U]; whether CWSDPMI accepts INT 2Fh 1680h [U].
