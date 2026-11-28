# The host link

The host link is how the Try Quill page and the ReactOS guest it runs talk to each other: the clipboard, files, the clock and the screen's size. It rides on the guest's first serial port (COM1), which v86 exposes to the page as `serial0-output-byte` (guest to page) and `serial_send_bytes(0, …)` (page to guest). This file is the specification; the plan is [plans/host-integration.md](plans/host-integration.md). Two codecs implement it and are held to the same bytes by [the shared vectors](#the-shared-vectors): the guest's `crates/quill-hostlink` (Rust, `no_std`, no allocation, no panics) and the page's `host-link` package (`site/try-reactos/packages/host-link`, TypeScript). Decision SITE-30; the page package is SITE-31.

The key words MUST, SHOULD and MAY are used as in RFC 2119.

## What is on the line

The port carries bytes in both directions with no framing of its own. Two kinds of frame share it, and anything outside a frame is **noise** and ignored (a console prompt, a stray `echo`).

- **`LHQ-FILE`**, the older frame, guest to page only: a file with a size, a name and an optional SHA-256. `SENDHOST.CMD` writes it from a batch file, so it is kept exactly as [web-demo.md](plans/web-demo.md#the-channel-to-the-page) defined it.
- **`LHQ-PKT`**, the packet, both ways: a header line and up to 4,096 bytes of payload, checked by a CRC-32. Messages are carried in packets.

A frame starts at the **start of a line**: the first byte received, any byte after an LF, or the byte after the end of a frame. A frame that begins in the middle of a line is noise.

## Frame grammar

```abnf
pkt-frame    = "LHQ-PKT" SP "1" SP channel SP id SP more SP length SP crc CRLF payload
file-frame   = "LHQ-FILE" SP "1" SP size SP name CRLF content file-end
file-end     = "LHQ-END" [SP sha256] CRLF

channel      = 1*16lcalpha                 ; known: ctl, clip, file, time, display
id           = number                      ; 0 to 4294967295, per direction, wrapping
more         = "M" / "E"                   ; M: more packets of this message follow; E: this one ends it
length       = number                      ; 0 to 4096, the payload's length in bytes
crc          = 8lchex                      ; lowercase
payload      = *OCTET                      ; exactly length bytes
size         = number                      ; 0 to 67108864, the content's length in bytes
name         = 1*128name-octet             ; any octet except 0x00-0x1F and 0x7F; UTF-8 is expected
content      = *OCTET                      ; exactly size bytes
sha256       = 64lchex
number       = "0" / (%x31-39 *9DIGIT)     ; no leading zeros, no sign, at most 10 digits
lcalpha      = %x61-7A
lchex        = DIGIT / %x61-66
CRLF         = %x0D %x0A
```

Rules the grammar cannot say:

- A header line is at most **64 bytes** for `LHQ-PKT` and **160 bytes** for `LHQ-FILE`, its CRLF included; `LHQ-END` is at most 80. The longest `LHQ-PKT` header, `LHQ-PKT 1 display 4294967295 M 4096 xxxxxxxx` and CRLF, is 46 bytes.
- Fields are separated by exactly one space. A line must end in CRLF; an LF alone is not a header.
- The digit after the frame name is the **frame version** and is `1`. A line that begins with either frame name and another version number does not parse (see [Errors](#errors)).
- The **CRC** is CRC-32 as zlib's `crc32` (IEEE 802.3: reflected, polynomial `0xEDB88320`, initial value and final XOR `0xFFFFFFFF`), over the header line from the `L` of `LHQ-PKT` through the last digit of `length` (the space before the CRC and the CRLF are not covered), followed by the payload bytes.
- `LHQ-FILE` content is read by count and is **not scanned**: it may hold anything, including text that looks like a frame. It has no checksum of its own unless `LHQ-END` carries a SHA-256.

### Worked example

A message `op=ok\nseq=1\n\n` (13 bytes) on `clip` as message 4, ending in this packet:

```text
header covered by the CRC   LHQ-PKT 1 clip 4 E 13
payload                     op=ok\nseq=1\n\n
CRC-32 of the two joined    49529bd1
on the line                 LHQ-PKT 1 clip 4 E 13 49529bd1\r\nop=ok\nseq=1\n\n
```

## Messages

A **message** is the payloads of one channel and id, in order, up to the `E` packet. Packets of different channels may interleave; each channel has at most one message open at a time. A packet whose id differs from the channel's open message **abandons** that message without a reply (the sender gave up or the line was restarted), and starts a new one.

Ids count messages **per direction**, from 0, one counter for every channel, wrapping at 2^32.

### Body

```abnf
body   = op-line *field-line LF data
op-line    = "op=" 1*32(lcalpha / DIGIT / "-") LF
field-line = key "=" *value-octet LF
key        = 1*32(lcalpha / DIGIT / "_" / "-")
data       = *OCTET
```

The first key is always `op`. A value is at most 256 bytes and contains no LF. A body has at most 16 fields, `op` included. The empty line ends the fields; everything after it is binary **data** and is not interpreted. Keys are case-sensitive; if a key repeats, the first counts. A body that breaks any rule is **malformed**: the receiver answers `code=malformed` and drops the message.

Text values and text data are UTF-8. Numbers are decimal ASCII with no sign. A `dib` body is a packed DIB (a `BITMAPINFOHEADER`, its colour table if any, the bits), as `CF_DIB` holds it.

### Reassembly

A receiver keeps the bytes of the open message for each channel. When the message would pass its **max** (the `max` of the sender's `hello`, or 8,388,608 bytes without one, and never less than 65,536) it answers `code=too-large`, drops what it has and drops the rest of that message's packets, up to its `E`. An unknown `op` on a known channel is answered `code=unknown` and otherwise ignored; a packet on a channel the receiver does not know is dropped, and the receiver answers `code=unknown` **once for the message** (on its first packet). Every error reply is a `ctl` message (see [Errors](#errors)).

## Channels and messages

`dir` is `g>p` for guest to page, `p>g` for page to guest. A key shown in brackets is optional; `u32` is a decimal number from 0 to 4294967295.

| Channel | `op` | dir | Keys and data |
|---|---|---|---|
| `ctl` | `hello` | both | `proto` u32 (`1`), `role` (`guest` or `page`), `app` (`HOSTLINK/0.1.0` or `host-link/0.1.0`), `caps` (comma list of `clip`, `file`, `time`, `display`), [`window` u32: the bytes the page may have in flight, guest only, default 16384, never less than 4352], [`max` u32: the largest message the sender accepts] |
| `ctl` | `credit` | g>p | `n` u32: bytes the guest has taken off the port |
| `ctl` | `pause`, `resume` | both | `channel`: the channel to hold or release |
| `ctl` | `error` | both | `code` (see [Errors](#errors)), `id` u32 (the message at fault, when known), [`text`: a sentence for the log] |
| `ctl` | `bye` | both | none |
| `clip` | `offer` | g>p | `seq` u32 (the guest's clipboard sequence), [`text`, `rtf`, `dib`: the size of each format present], `source` (`user` or `self`) |
| `clip` | `data` | g>p | `seq` u32, `fmt` (`text`, `rtf` or `dib`); data: the bytes |
| `clip` | `ok` | g>p | `seq` u32 of a `set` |
| `clip` | `refused` | g>p | `seq` u32, `reason` |
| `clip` | `get` | p>g | `seq` u32, `fmt` |
| `clip` | `set` | p>g | `seq` u32, `formats` (comma list); then one `part` message per format |
| `clip` | `part` | p>g | `seq` u32, `fmt`; data: the bytes |
| `file` | `begin` | both | `id` u32 (the file's number, not a message id), `name`, `size` u32, [`sha256`: 64 lowercase hex digits] |
| `file` | `chunk` | both | `id` u32; data: at most 4,000 bytes, so the message is one packet |
| `file` | `end` | both | `id` u32 |
| `file` | `saved`, `failed` | g>p | `id` u32, and `path` or `reason` |
| `time` | `set` | p>g | `local`: the visitor's wall time as `YYYY-MM-DDTHH:MM:SS.mmm`, `utc`: milliseconds since 1970 (up to 16 digits, not a u32), [`zone`: the IANA name, for the log only] |
| `display` | `fit` | p>g | `w`, `h` u32: the stage's size in device pixels |
| `display` | `modes` | g>p | `list`: the modes `EnumDisplaySettingsA` lists, `WxHxBPP` joined by commas |
| `display` | `changed` | g>p | `w`, `h`, `bpp` u32 |

The codecs know the frames, bodies, `ctl` and the credit rules. What the other channels' messages mean is the business of the packages that use them (the clipboard, files, time and display work packages); they are specified here so that both ends agree.

Example, the guest offering its clipboard:

```text
LHQ-PKT 1 clip 41 E 52 <crc>\r\n
op=offer\nseq=17\ntext=23\nrtf=1840\ndib=0\nsource=user\n\n
```

## Hello

The `hello` exchange says who is on the line and starts a session. It is always the first message of a session.

- **The helper** sends `hello` when it opens the port. It answers every `hello` it receives with its own, which also tells the page that it is running after a restore.
- **The page** sends `hello` when it attaches to a machine (a cold boot, or a restored state) and, until it hears one back, again every **5 s** at most. It answers a `hello` it did not ask for (the helper started after the page) with its own, once; a `hello` that arrives while its own is unanswered is the answer and is not answered, so two sides cannot ping-pong.
- A `hello` clears the receiver's reassembly state and, on the page, the credit account (see [Credits](#credits)); bytes the page queued before the save are gone with v86's input array, which a `hello` resets. The page sends its `time set` after every `hello` exchange.
- The helper must work with the defaults if the page's `hello` never reaches it.
- A `hello` is outside the credit account: the page does not know the window until the helper has told it.

## Credits

The page-to-guest direction is paced by **credits**, so that a 30 MB file cannot fill the guest's input buffer faster than the helper empties it. The guest-to-page direction needs none: the page drains every byte as v86 delivers it, and the helper writes at most one packet at a time per channel.

- The helper's `hello` names a `window` (16,384 bytes by default, within the input buffer it asked for with `SetupComm`; never less than 4,352, the longest frame with room to spare). The page **never has more frame bytes outstanding than the window**, counting every byte of every frame it sends (header lines and payloads) except `hello`.
- The helper sends `ctl op=credit n=<bytes>` when it has taken **4,096 bytes or more** off the port since its last credit; `n` is the number it has taken. The page subtracts `n` from its outstanding bytes, never below zero, so a repeated or oversize credit cannot raise the window.
- A frame is sent only if it fits in what remains. If the page has frames to send and no credit has arrived for **5 s**, it stops with a status line (state `stalled`) rather than flooding the guest; the next credit clears it.
- **Priority.** Each side queues messages per channel and sends packets in the order `ctl`, `clip`, `time`, `display`, `file`: whenever a packet can be sent, the highest channel with one waiting goes first, and a channel's own messages go in order. A 30 MB file therefore never holds a paste up for more than one packet (about 14 ms at the measured rate). `pause` stops the sender serving a channel until `resume`.

A timeline with the default window (every number is frame bytes; a full packet is 4,142):

| Time | Event | Outstanding | Available |
|---|---|---|---|
| 0 s | helper `hello window=16384` | 0 | 16384 |
| 0.0 s | page sends three full file packets | 12426 | 3958 |
| 0.0 s | a 62-byte `time set` is sent ahead of the next file packet | 12488 | 3896 |
| 0.1 s | helper `credit n=4096` | 8392 | 7992 |
| 0.1 s | page sends one more full packet | 12534 | 3850 |
| 5.2 s | no credit has come: the page is `stalled` and sends nothing | 12534 | 3850 |
| 6.0 s | helper `credit n=12534` (the guest was busy) | 0 | 16384 |

## Errors

- A line that starts `LHQ-` but does not parse is **skipped**: nothing is replied. The receiver scans for the next `LHQ-` at the start of a line and counts the error (`header`). The line's would-be payload is read as noise.
- A packet whose **CRC is wrong** is dropped with every other packet of its message (the receiver notes the message id and drops packets of it up to its `E`), and the receiver sends `ctl op=error code=crc id=<id>`. A bad CRC is never read as data. Version 1 does **not** resend: every clipboard and time message is safe to repeat, the visitor repeats a copy or a paste, and a file is marked damaged in the list, as `LHQ-FILE` is.
- A message larger than the receiver's `max` is refused with `code=too-large`. An unknown channel or `op` gets `code=unknown`. A malformed body gets `code=malformed`.
- A `hello` with a `proto` higher than the receiver knows gets `code=version` (once), and the receiver then stays quiet. The page then says "The PC's link helper is a different version: clipboard sharing is off".
- After an `LHQ-FILE`'s content, anything other than `LHQ-END` (with an optional SHA-256) is a `trailer` error: the file is damaged, the bytes through the next LF are dropped, and the parser resynchronises.

| `code` | Meaning | `id` is |
|---|---|---|
| `crc` | A packet failed its CRC; the message is dropped | The message's id |
| `too-large` | The message is past the receiver's `max` | The message's id |
| `unknown` | An unknown channel or `op` | The message's id |
| `malformed` | The body breaks the body rules | The message's id |
| `version` | The `hello`'s `proto` is newer than the receiver knows | The `hello`'s id |

v86's serial line is lossless, so an error means a bug or a second writer on the port; the harness's counts of each code are part of every test, and the page's diagnostics *Link* row shows them.

## Versions and restore

- The digit after the frame name is the frame version; `hello`'s `proto` is the protocol version. Both sides speak version 1. A new frame version is a new line shape that older parsers skip; a new protocol version changes the messages. A side that sees a `proto` above its own does not guess.
- **Restore.** A saved state holds the helper already running with the port open. After a restore the page attaches and sends `hello`; the helper answers at once, and the page learns the window again. Credit starts from zero outstanding, because whatever the page had queued went with v86's input array.
- **Cold boot without a helper.** No `hello` ever comes back; the page keeps retrying every 5 s at most, shows "no helper" in the *Link* row, and nothing else changes.

## Limits

| What | Limit |
|---|---|
| Packet payload | 4,096 bytes |
| `LHQ-PKT` header line | 64 bytes with CRLF (the longest is 46) |
| `LHQ-FILE` header line | 160 bytes with CRLF; name 1 to 128 bytes; size up to 67,108,864 (64 MiB) |
| `LHQ-END` line | 80 bytes with CRLF |
| Channel name | 1 to 16 lowercase letters |
| Message id | 0 to 4,294,967,295 per direction |
| Body keys | key 1 to 32 bytes; value up to 256 bytes; up to 16 fields |
| Message size | the receiver's `max`: 8,388,608 by default, never below 65,536 |
| `file chunk` data | 4,000 bytes |
| Credit window | 16,384 by default, never below 4,352 |
| Credit step | the helper sends a credit after 4,096 bytes or more |
| Hello retry | 5 s, until answered |
| Credit timeout | 5 s |
| Per-format limits (enforced by the clipboard and file work packages, not by the codecs) | text 1 MiB, RTF 4 MiB, picture 4 MiB from the guest and 4,096 pixels a side into it, drop 32 MiB, download 64 MiB |

## The guest's codec

`crates/quill-hostlink` is the base-layer crate the helper links: `#![no_std]`, `forbid(unsafe_code)`, no `alloc`, and no `unwrap`, `expect`, `panic` or slice indexing (clippy denies them). It is integer-only.

- `Parser` takes a 4,096-byte buffer from the caller and holds one header line; `feed(bytes, sink)` reports `Event::Packet` (CRC held), `FileBegin`, `FileData`, `FileEnd` or `Error` (`Header`, `Crc`, `Trailer`, each with the byte offset of the frame that failed). A split anywhere gives the same events.
- `Assembler` and `Mux` reassemble messages into buffers the caller owns, with the abandon, poison and `too-large` rules above. `Body` reads a body without copying; `BodyWriter` writes one.
- `encode_packet`, `encode_message`, `encode_file_header` and `encode_file_trailer` write frames into a caller's buffer.
- `Credit` is the sender's account and `CreditReporter` the receiver's count of bytes owed.

## The page's package

`site/try-reactos/packages/host-link` has the same parser, encoders, reassembly and `Credit` in TypeScript (`frame.ts`, `message.ts`, `flow.ts`), and `link.ts`, which adds `hello` and its retry, the per-channel send queues in priority order, the error replies and the statistics. `attach(emulator)` joins a `Link` to v86's COM1. No clipboard, file or time behaviour is in it yet.

## The shared vectors

`tests/host-link/vectors/` holds named cases as a pair: `NAME.bin` (the bytes on the line) and `NAME.json` (what both codecs must report). `tests/host-link/gen_vectors.py` writes them from this specification with zlib's CRC-32, independent of both codecs; `--check` fails when the files differ, so a change to either needs the generator and this file changed with it. Each codec's tests read every vector fed whole, one byte at a time, split in two at every point and split at random, so any chunking must give the same result.

The JSON holds:

- `max`: the receiver's largest message (optional, default 8,388,608).
- `events`: what the frame parser reports, in order.
  - `{"kind": "packet", "channel", "id", "more", "payload_hex"}`, where `channel` is `unknown` for a name that is not one of the five.
  - `{"kind": "file-begin", "size", "name"}`, `{"kind": "file-data", "data_hex"}` (consecutive ones joined, because their split depends on the input's) and `{"kind": "file-end", "sha256"}` (the digits or `null`).
  - `{"kind": "error", "error", "offset"}`: `error` is `header`, `crc` (with `channel` and `id`) or `trailer`; `offset` counts bytes from the start of the vector to the first byte of the frame that failed, so a codec shows where it resynchronised by what it reports next.
- `messages`: what the message layer reports, in order: `{"kind": "message", "channel", "id", "op", "fields", "data_hex"}` (`fields` is every `[key, value]` in order, `op` first) or `{"kind": "error", "code", "channel", "id"}` with `code` one of `crc`, `too-large`, `unknown`, `malformed`.

The vectors cover each message kind, a message over several packets (4,096, 4,096 and the rest), a zero-length packet, the largest header and payload, id wrap, interleaved channels, a new id abandoning an open message, `LHQ-FILE` with and without a SHA-256 and with content that looks like frames, noise before, between and after frames and mid-line, a header split at every point, a bad CRC (single and in the middle of a message), seven kinds of bad header, an unknown channel, a message over `max` and exactly at it, and three malformed bodies.

## Guests

This specification describes the link between the ReactOS demo's page and `HOSTLINK.EXE`, and nothing in it is particular to Windows except the clipboard formats it names. How the same link serves other guests (Linux under X11, terminal-only guests, Windows 3.x, and the DOS page's shared disks as a different transport under the same page rules), which transports come after the serial port, how print jobs ride on the `file` channel, and how the protocol, the codecs and the helpers become a reusable package, is planned in [plans/host-link-guests.md](plans/host-link-guests.md) (SITE-80 to SITE-82). Additions it plans (the `html` and `png` formats, the print keys) are made here by the packages that build them.
