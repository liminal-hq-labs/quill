# host-link

The page's end of the host link: `LHQ-PKT` and `LHQ-FILE` frames over the guest's COM1, as v86 delivers them. It finds frames in noise and resynchronises after an error, checks CRCs, reassembles messages per channel, paces its sending with the helper's credits, sends packets in priority order, answers errors and counts everything. The protocol is [`docs/host-link.md`](../../../../docs/host-link.md) (SITE-30); this package is SITE-31. It has no runtime dependency, and no clipboard, file or time behaviour yet: those are built on the `message` event and `send` by later packages.

## API

```ts
import { attach } from "./lib/host-link.js";

const host = attach(emulator);              // listens on serial0, sends hello, retries it every 5 s until answered
host.on("state", (state) => {});            // "waiting" | "ready" | "stalled" | "version" | "closed"
host.on("message", (m) => {});              // a message on clip, file, time or display: { channel, id, op, fields, data }
host.on("file", (e) => {});                 // LHQ-FILE frames: file-begin, file-data, file-end
host.on("peer-error", (m) => {});           // the helper reported an error
host.send("clip", "get", [["seq", 1], ["fmt", "dib"]]);   // the message's id, or null when the helper has not answered
host.stats();                               // state, peer, bytes, packets, messages, errors by code, credit, queued
host.restart();                             // after a restore or a reset: hello again, everything in flight forgotten
host.detach();
```

`attach(emulator, { tickMs, max, app })` takes anything with `add_listener`, `remove_listener` and `serial_send_bytes` (v86's `V86`, or a fake bus). The layers underneath are exported for tests and for the Rust codec's twin: `FrameParser`, `encodePacket`, `encodeMessage`, `encodeFileHeader`, `encodeFileTrailer`, `parseBody`, `writeBody`, `Assembler`, `Mux`, `Credit` and the `Link` class, which takes a `write` function instead of an emulator.

## Layout

| File | What |
|---|---|
| `src/frame.ts` | The frame parser (both kinds, any split, resync) and the encoders |
| `src/message.ts` | Bodies (`key=value` lines, an empty line, data), reassembly, the once-per-message `unknown` reply |
| `src/flow.ts` | The credit account |
| `src/link.ts` | `hello`, credits, per-channel queues in priority order, error replies, statistics |
| `src/index.ts` | `attach` and the public types |
| `test/` | `bun test`: the shared vectors, a random-input test, the fake-bus tests |
| `build.ts` | Builds `../../lib/host-link.js` (`--check` fails on drift; 40 KB budget) |

## Use

```sh
bun install
bun test
bun run typecheck
bun run build        # writes site/try-reactos/lib/host-link.js; commit it
bun run check:built  # what CI runs
```

The shared vectors are in `tests/host-link/vectors/` at the repository root and are read by the guest's Rust codec (`crates/quill-hostlink`) too; `tests/host-link/gen_vectors.py` writes them from the specification.

## Tested with

- v86 0.5.473 (`site/try-reactos/v86/PROVENANCE.md`): the attach, a cold boot with no disk and no helper, and Stop, in headless Chromium 156 driven by `playwright-core` (the diagnostics *Link* row read *no helper* after Start with one 119-byte `hello` sent, then *off* after Stop, with no page errors). Firefox and Safari have not been run; the package uses only `queueMicrotask`, `setInterval`, `TextEncoder` and private class fields, which all three have.
- Bun 1.4.4 for the tests and the build, TypeScript 5.9.3 for the type check.
- The page has not yet met a real helper: that is WP-HOST-3.
