#!/usr/bin/env python3
# Writes the host link's golden vectors (tests/host-link/vectors/*.bin and *.json) from the specification, independent of both codecs.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

"""Run as `python3 -I tests/host-link/gen_vectors.py` to write the vectors, or with `--check` to fail when the files on disk differ.

Each case is NAME.bin (the bytes on the line) and NAME.json (what both codecs must report for them). The JSON holds:

  max       the receiver's largest message in bytes (optional, default 8388608)
  events    what the frame parser reports: packet, file-begin, file-data (consecutive ones joined), file-end, error
  messages  what the message layer reports: message, or error (crc, too-large, unknown, malformed)

The expected values are built here from docs/host-link.md, with zlib's CRC-32, so a bug shared by the two codecs cannot hide.
"""

import hashlib
import json
import os
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "vectors")
MAX_PAYLOAD = 4096
DEFAULT_MAX = 8388608
CHANNELS = ("ctl", "clip", "file", "time", "display")


def crc_hex(head, payload):
	return "%08x" % (zlib.crc32(head + payload) & 0xFFFFFFFF)


def packet_bytes(channel, ident, more, payload, ver=1, length=None, crc=None, crc_text=None):
	head = ("LHQ-PKT %d %s %d %s %d" % (ver, channel, ident, "M" if more else "E", len(payload) if length is None else length)).encode()
	text = crc_text if crc_text is not None else (crc if crc is not None else crc_hex(head, payload))
	return head + b" " + text.encode() + b"\r\n" + payload


def body_bytes(op, fields, data):
	text = "op=%s\n" % op + "".join("%s=%s\n" % (k, v) for k, v in fields) + "\n"
	return text.encode() + data


class Vector:
	def __init__(self, name, description, max_message=None):
		self.name = name
		self.description = description
		self.max = max_message
		self.data = bytearray()
		self.events = []
		self.messages = []

	def noise(self, raw):
		self.data += raw
		return self

	def raw_packet(self, channel, ident, more, payload, **kw):
		"""A packet as the caller built it, expected to parse as given (a valid one)."""
		self.data += packet_bytes(channel, ident, more, payload, **kw)
		self.events.append({"kind": "packet", "channel": channel if channel in CHANNELS else "unknown", "id": ident, "more": more, "payload_hex": payload.hex()})
		return self

	def message(self, channel, ident, op, fields=(), data=b"", split=MAX_PAYLOAD, expect=True):
		"""A message sent as packets of at most `split` bytes."""
		payload = body_bytes(op, fields, data)
		parts = [payload[i : i + split] for i in range(0, len(payload), split)] or [b""]
		for i, part in enumerate(parts):
			self.raw_packet(channel, ident, i < len(parts) - 1, part)
		if expect:
			self.messages.append({"kind": "message", "channel": channel, "id": ident, "op": op, "fields": [["op", op]] + [[k, v] for k, v in fields], "data_hex": data.hex()})
		return self

	def message_error(self, code, channel, ident):
		self.messages.append({"kind": "error", "code": code, "channel": channel if channel in CHANNELS else "unknown", "id": ident})

	def event_error(self, error, offset, **extra):
		event = {"kind": "error", "error": error, "offset": offset}
		event.update(extra)
		self.events.append(event)

	def file_frame(self, size, name, content, sha=None, trailer=None):
		self.data += ("LHQ-FILE 1 %d %s\r\n" % (size, name)).encode()
		self.events.append({"kind": "file-begin", "size": size, "name": name})
		self.data += content
		if content:
			self.events.append({"kind": "file-data", "data_hex": content.hex()})
		tail = trailer if trailer is not None else (b"LHQ-END" + (b" " + sha.encode() if sha else b"") + b"\r\n")
		self.data += tail
		self.events.append({"kind": "file-end", "sha256": sha})

	def write(self, check):
		doc = {"name": self.name, "description": self.description}
		if self.max is not None:
			doc["max"] = self.max
		doc["events"] = self.events
		doc["messages"] = self.messages
		text = json.dumps(doc, indent="\t", ensure_ascii=False) + "\n"
		return {self.name + ".bin": bytes(self.data), self.name + ".json": text.encode()}


def build():
	vs = []

	def new(name, description, max_message=None):
		v = Vector(name, description, max_message)
		vs.append(v)
		return v

	new("ctl-hello-page", "The page's hello.").message("ctl", 0, "hello", [("proto", "1"), ("role", "page"), ("app", "host-link/0.1.0"), ("caps", "clip,file,time,display"), ("window", "16384"), ("max", "8388608")])
	new("ctl-hello-guest", "The helper's hello.").message("ctl", 0, "hello", [("proto", "1"), ("role", "guest"), ("app", "HOSTLINK/0.1.0"), ("caps", "clip,file,time,display"), ("window", "16384"), ("max", "8388608")])
	new("ctl-credit", "A credit.").message("ctl", 1, "credit", [("n", "4096")])
	new("ctl-pause-resume", "Pause then resume a channel.").message("ctl", 2, "pause", [("channel", "file")]).message("ctl", 3, "resume", [("channel", "file")])
	new("ctl-error", "An error message.").message("ctl", 4, "error", [("code", "crc"), ("id", "41"), ("text", "bad checksum")])
	new("ctl-bye", "Bye.").message("ctl", 5, "bye")
	new("clip-offer", "The guest offers its clipboard.").message("clip", 41, "offer", [("seq", "17"), ("text", "23"), ("rtf", "1840"), ("dib", "0"), ("source", "user")])
	new("clip-data-text", "Text with UTF-8 after the body.").message("clip", 42, "data", [("seq", "17"), ("fmt", "text")], "Caf\u00e9 \u2014 na\u00efve".encode())
	new("clip-get-set-part", "The page asks, then sets.").message("clip", 7, "get", [("seq", "3"), ("fmt", "dib")]).message("clip", 8, "set", [("seq", "4"), ("formats", "text,rtf")]).message("clip", 9, "part", [("seq", "4"), ("fmt", "text")], b"hello")
	new("clip-ok-refused", "The guest's answers to a set.").message("clip", 10, "ok", [("seq", "4")]).message("clip", 11, "refused", [("seq", "5"), ("reason", "busy")])
	new("clip-three-packets", "One message over three packets: 4096, 4096 and the rest.").message("clip", 90, "data", [("seq", "1"), ("fmt", "rtf")], bytes((i * 7) % 251 for i in range(9000)))
	new("file-messages", "The file channel's ops.").message("file", 1, "begin", [("id", "9"), ("name", "A letter.rtf"), ("size", "5"), ("sha256", "ab" * 32)]).message("file", 2, "chunk", [("id", "9")], bytes(range(256))).message("file", 3, "end", [("id", "9")]).message("file", 4, "saved", [("id", "9"), ("path", "C:\\From your computer\\A letter.rtf")]).message("file", 5, "failed", [("id", "9"), ("reason", "disk full")])
	new("time-set", "The visitor's wall time.").message("time", 1, "set", [("local", "2026-11-27T14:03:09.123"), ("utc", "1795874589123"), ("zone", "America/Toronto")])
	new("display-ops", "Fit, modes and changed.").message("display", 1, "fit", [("w", "1366"), ("h", "768")]).message("display", 2, "modes", [("list", "640x480x8,800x600x8,1024x768x16")]).message("display", 3, "changed", [("w", "1024"), ("h", "768"), ("bpp", "16")])
	v = new("zero-length-packet", "A message whose last packet carries nothing (M, then E with length 0).")
	payload = body_bytes("data", [("seq", "2"), ("fmt", "text")], b"x" * 10)
	v.raw_packet("clip", 5, True, payload).raw_packet("clip", 5, False, b"")
	v.messages.append({"kind": "message", "channel": "clip", "id": 5, "op": "data", "fields": [["op", "data"], ["seq", "2"], ["fmt", "text"]], "data_hex": (b"x" * 10).hex()})
	v = new("largest-packet", "The longest header and the longest payload: display, id 2^32 - 1, M, 4096 bytes.")
	v.raw_packet("display", 4294967295, True, body_bytes("modes", [("list", "1x1x8")], b"\x00" * (4096 - 25)))
	v.raw_packet("display", 4294967295, False, b"")
	v.messages.append({"kind": "message", "channel": "display", "id": 4294967295, "op": "modes", "fields": [["op", "modes"], ["list", "1x1x8"]], "data_hex": (b"\x00" * (4096 - 25)).hex()})
	new("id-wraps", "Ids wrap at 2^32.").message("time", 4294967295, "set", [("utc", "1")]).message("time", 0, "set", [("utc", "2")])
	new("interleaved-channels", "A file packet between the packets of a clip message; each channel keeps its own message.").raw_packet("clip", 1, True, body_bytes("data", [("seq", "1"), ("fmt", "text")], b"ab")).message("file", 2, "chunk", [("id", "1")], b"zz").raw_packet("clip", 1, False, b"cd")
	vs[-1].messages = [
		{"kind": "message", "channel": "file", "id": 2, "op": "chunk", "fields": [["op", "chunk"], ["id", "1"]], "data_hex": b"zz".hex()},
		{"kind": "message", "channel": "clip", "id": 1, "op": "data", "fields": [["op", "data"], ["seq", "1"], ["fmt", "text"]], "data_hex": b"abcd".hex()},
	]
	v = new("new-id-abandons-open", "A new id on a channel drops the open message without a reply; only the second arrives.")
	v.raw_packet("clip", 5, True, body_bytes("data", [("seq", "1"), ("fmt", "text")], b"lost")).message("clip", 6, "ok", [("seq", "1")])

	v = new("file-with-sha", "LHQ-FILE with a SHA-256.")
	content = b"Hello, PC.\r\n"
	sha = hashlib.sha256(content).hexdigest()
	v.file_frame(len(content), "hello.txt", content, sha)
	new_v = new("file-without-sha", "LHQ-FILE with no checksum, as SENDHOST.CMD writes it.")
	new_v.file_frame(len(content), "hello.txt", content)
	new("file-empty", "A file of zero bytes.").file_frame(0, "empty.txt", b"")
	new("file-name-spaces-utf8", "A name with spaces and UTF-8.").file_frame(3, "Caf\u00e9 menu (2).txt", b"abc")
	v = new("file-contains-frame-text", "File bytes are not scanned: this content looks like a packet and a file header.")
	v.file_frame(0 + len(b"LHQ-PKT 1 ctl 1 E 0 00000000\r\nLHQ-FILE 1 3 x\r\n"), "trap.bin", b"LHQ-PKT 1 ctl 1 E 0 00000000\r\nLHQ-FILE 1 3 x\r\n")
	v = new("file-bad-trailer", "The bytes after the file are not LHQ-END: the file is damaged, the parser resyncs at the next frame.")
	start = len(v.data)
	v.data += b"LHQ-FILE 1 3 bad.txt\r\nabcNOPE\r\n"
	v.events += [{"kind": "file-begin", "size": 3, "name": "bad.txt"}, {"kind": "file-data", "data_hex": b"abc".hex()}]
	v.event_error("trailer", start + len(b"LHQ-FILE 1 3 bad.txt\r\nabc"))
	v.message("ctl", 1, "bye")

	v = new("noise-around", "Noise before, between and after frames, including lines that begin like a frame but are not.")
	v.noise(b"Booting...\r\nC:\\>\r\n")
	v.message("ctl", 1, "bye")
	v.noise(b"\r\nLHQ\r\nLHQ PKT 1\r\nxx\n")
	v.message("ctl", 2, "bye")
	v.noise(b"trailing noise without a newline")
	v = new("noise-mid-line", "A frame that does not start a line is noise; the next one, after a newline, is read.")
	v.noise(b"echo LHQ-PKT 1 ctl 1 E 0 00000000\r\n")
	v.noise(b"junk" + packet_bytes("ctl", 1, False, body_bytes("bye", [], b"")) + b"\n")
	v.message("ctl", 2, "bye")

	v = new("bad-crc-single", "A wrong CRC: the packet is dropped and reported; the next message is fine.")
	start = len(v.data)
	payload = body_bytes("data", [("seq", "1"), ("fmt", "text")], b"oops")
	v.data += packet_bytes("clip", 3, False, payload, crc="00000000")
	v.event_error("crc", start, channel="clip", id=3)
	v.message_error("crc", "clip", 3)
	v.message("clip", 4, "ok", [("seq", "1")])
	v = new("bad-crc-middle", "A wrong CRC on a middle packet: the rest of that message, its last packet included, is dropped.")
	v.raw_packet("clip", 8, True, body_bytes("data", [("seq", "1"), ("fmt", "text")], b"aa"))
	start = len(v.data)
	v.data += packet_bytes("clip", 8, True, b"bb", crc="deadbeef")
	v.event_error("crc", start, channel="clip", id=8)
	v.message_error("crc", "clip", 8)
	v.raw_packet("clip", 8, False, b"cc")
	v.message("clip", 9, "ok", [("seq", "1")])
	v = new("bad-crc-uppercase", "A CRC in uppercase hex is not a header: skipped.")
	start = len(v.data)
	v.data += packet_bytes("ctl", 1, False, b"", crc_text="ABCDEF01")
	v.event_error("header", start)
	v.message("ctl", 2, "bye")

	def bad_header(name, description, raw):
		v = new(name, description)
		start = len(v.data)
		v.data += raw
		v.event_error("header", start)
		v.message("ctl", 2, "bye")

	bad_header("bad-header-missing-field", "A header with a field missing: skipped to the next frame.", b"LHQ-PKT 1 ctl 1 E 00000000\r\n")
	bad_header("bad-header-version-2", "A frame version this codec does not know is a line that does not parse.", packet_bytes("ctl", 1, False, b"", ver=2))
	bad_header("bad-header-leading-zero", "A length with a leading zero.", b"LHQ-PKT 1 ctl 1 E 007 00000000\r\n")
	bad_header("bad-header-more-flag", "A flag that is neither M nor E.", b"LHQ-PKT 1 ctl 1 X 0 00000000\r\n")
	bad_header("bad-header-bare-lf", "A header ending in LF alone.", b"LHQ-PKT 1 ctl 1 E 0 00000000\n")
	bad_header("bad-header-too-long", "A header line past 64 bytes with no end: skipped through the next newline.", b"LHQ-PKT 1 ctl 1 E 0 00000000 " + b"x" * 80 + b"\r\n")
	bad_header("bad-header-length-4097", "A length over 4,096 is a bad header (its payload bytes are noise).", packet_bytes("ctl", 1, False, b"zzzz\r\n", length=4097))
	bad_header("bad-header-id-too-big", "An id past 2^32 - 1.", b"LHQ-PKT 1 ctl 4294967296 E 0 00000000\r\n")
	bad_header("bad-header-unknown-lhq-line", "A line that starts LHQ- and is neither frame.", b"LHQ-WHAT 1 2 3\r\n")
	bad_header("bad-file-header-size", "An LHQ-FILE size that is not a number.", b"LHQ-FILE 1 many x.txt\r\n")
	bad_header("bad-file-header-too-big", "An LHQ-FILE size over 64 MiB.", b"LHQ-FILE 1 67108865 x.txt\r\n")

	v = new("unknown-channel", "A channel the codec does not know: reported as unknown, once for each message.")
	v.raw_packet("zzz", 6, True, b"ab").raw_packet("zzz", 6, False, b"cd")
	v.message_error("unknown", "zzz", 6)
	v.message("ctl", 2, "bye")
	v = new("message-over-max", "A message past the receiver's max: refused with too-large, its remaining packets dropped.", 64)
	v.raw_packet("clip", 2, True, body_bytes("data", [("seq", "1"), ("fmt", "text")], b"x" * 20))
	v.raw_packet("clip", 2, True, b"y" * 40)
	v.message_error("too-large", "clip", 2)
	v.raw_packet("clip", 2, False, b"z")
	v.message("clip", 3, "ok", [("seq", "1")])
	v = new("message-at-max", "A message exactly at max is accepted.", len(body_bytes("ok", [("seq", "1")], b"")))
	v.message("clip", 1, "ok", [("seq", "1")])
	v = new("malformed-first-key", "A body whose first key is not op.")
	v.raw_packet("ctl", 3, False, b"seq=1\nop=ok\n\n")
	v.message_error("malformed", "ctl", 3)
	v = new("malformed-no-blank-line", "A body with no empty line.")
	v.raw_packet("ctl", 3, False, b"op=bye\n")
	v.message_error("malformed", "ctl", 3)
	v = new("malformed-empty", "A message whose only packet is empty.")
	v.raw_packet("ctl", 3, False, b"")
	v.message_error("malformed", "ctl", 3)
	return vs


def main():
	check = "--check" in sys.argv
	wanted = {}
	names = set()
	for v in build():
		assert v.name not in names, v.name
		names.add(v.name)
		wanted.update(v.write(check))
	os.makedirs(OUT, exist_ok=True)
	existing = set(os.listdir(OUT))
	bad = []
	for fname, content in sorted(wanted.items()):
		path = os.path.join(OUT, fname)
		if check:
			if not os.path.exists(path) or open(path, "rb").read() != content:
				bad.append(fname)
		else:
			with open(path, "wb") as handle:
				handle.write(content)
	for fname in sorted(existing - set(wanted)):
		bad.append(fname + " (no longer generated)")
	if check and bad:
		print("host-link vectors differ from gen_vectors.py: " + ", ".join(bad), file=sys.stderr)
		return 1
	if not check and bad:
		print("stale files, remove them by name: " + ", ".join(bad), file=sys.stderr)
		return 1
	print("host-link vectors: %d cases" % len(names))
	return 0


if __name__ == "__main__":
	sys.exit(main())
