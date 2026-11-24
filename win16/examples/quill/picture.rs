// Pictures on the Windows 3.x page: a BMP already is a device-independent bitmap, so its bytes are copied once from the document's
// picture pool into a block of global memory and GDI scales them to the size they are shown.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Nothing here is decoded and nothing big is in the data segment: the portable decoders keep 40 KB (BMP) to 70 KB (PNG) of rows,
// accumulators and inflate window on the stack, which the 64 KB automatic data segment cannot give. GDI scales with COLORONCOLOR,
// so a shrunk picture drops lines and columns instead of averaging them as the other builds do. PNG is not drawn: it needs the
// inflater's 32 KB window and two rows of its own, and stays a grey box.
use crate::far::FarBlock;
use crate::store::FarStore;
use quill_core::doc::Doc;
use w16::*;

/// How many pictures the cache holds, and the most far memory they may take together (a single picture over the budget is still
/// tried, alone).
const SLOTS: usize = 3;
const BUDGET: u32 = 1 << 20;
/// The most pixels along a side drawn (the decoders' limit, `quill_image::MAX_SIDE`).
const MAX_SIDE: u32 = 4096;

const BI_RGB: u32 = 0;
const BI_RLE8: u32 = 1;
const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC_0020;
const COLORONCOLOR: u32 = 3;

/// Why a picture is a grey box.
#[derive(Copy, Clone, PartialEq)]
pub enum Why {
	/// Not tried yet.
	None,
	/// Drawn.
	Ready,
	/// PNG: no decoder fits the data segment.
	Png,
	/// Not a bitmap this build hands to GDI: another header, a compression other than RLE8, 16 or 32 bits, top-down rows.
	Format,
	/// The file ends before its pixels do, or a size is out of range.
	Damaged,
	/// `GlobalAlloc` refused the block.
	NoMemory,
}

struct Slot {
	idx: u16,
	fp: u32,
	why: Why,
	block: FarBlock,
	/// Where the pixels start in the block, and the bitmap's size in pixels.
	bits: u32,
	w: i32,
	h: i32,
	/// Which use this was, so the least recently used slot goes first.
	used: u32,
	/// The file's size.
	size: u32,
	bpp: u16,
}

impl Slot {
	const EMPTY: Slot = Slot {
		idx: 0,
		fp: 0,
		why: Why::None,
		block: FarBlock::empty(),
		bits: 0,
		w: 0,
		h: 0,
		used: 0,
		size: 0,
		bpp: 0,
	};
}

static mut SLOT: [Slot; SLOTS] = [Slot::EMPTY; SLOTS];
static mut CLOCK: u32 = 0;
/// Why the file named on the command line did not open, for `PICS.LOG`.
pub static mut OPEN_FAILED: Option<quill_formats::FormatError> = None;

fn slots() -> &'static mut [Slot; SLOTS] {
	unsafe { &mut *core::ptr::addr_of_mut!(SLOT) }
}

fn le32(b: &[u8], at: usize) -> u32 {
	let g = |i: usize| u32::from(b.get(at + i).copied().unwrap_or(0));
	g(0) | g(1) << 8 | g(2) << 16 | g(3) << 24
}

fn le16(b: &[u8], at: usize) -> u32 {
	let g = |i: usize| u32::from(b.get(at + i).copied().unwrap_or(0));
	g(0) | g(1) << 8
}

fn cached_bytes() -> u32 {
	slots()
		.iter()
		.filter(|s| s.why == Why::Ready)
		.map(|s| s.block.len())
		.sum()
}

/// Frees the least recently used slot that holds memory. Returns false when none does.
fn evict() -> bool {
	let mut pick: Option<usize> = None;
	for (i, s) in slots().iter().enumerate() {
		if !s.block.is_empty() && pick.is_none_or(|p| s.used < slots()[p].used) {
			pick = Some(i);
		}
	}
	let Some(i) = pick else { return false };
	slots()[i].block.free();
	slots()[i].why = Why::None;
	true
}

/// Reads picture `idx` into a slot: validates the bitmap header, then copies the file after its 14-byte header into global memory.
fn build(doc: &Doc, s: &FarStore, idx: u16, fp: u32, at: usize) {
	let slot = &mut slots()[at];
	slot.block.free();
	slot.idx = idx;
	slot.fp = fp;
	slot.why = Why::Format;
	let Some(p) = doc.pictures.get(s, u32::from(idx)) else {
		slot.why = Why::Damaged;
		return;
	};
	if p.kind != 0 {
		slot.why = Why::Png;
		return;
	}
	let mut head = [0u8; 54];
	let got = doc.picture_bytes(s, idx, 0, &mut head);
	if got < head.len() || head.get(..2) != Some(b"BM") {
		slot.why = Why::Format;
		return;
	}
	let (off_bits, size) = (le32(&head, 10), p.len);
	let (bi_size, width, height) = (
		le32(&head, 14),
		le32(&head, 18) as i32,
		le32(&head, 22) as i32,
	);
	let (planes, bpp, compression, used) = (
		le16(&head, 26),
		le16(&head, 28),
		le32(&head, 30),
		le32(&head, 46),
	);
	// A 40-byte header with bottom-up rows (GDI here does not draw a negative height), plain or RLE8.
	let rle = compression == BI_RLE8 && bpp == 8;
	if bi_size != 40
		|| !(compression == BI_RGB || rle)
		|| planes != 1
		|| !matches!(bpp, 1 | 4 | 8 | 24)
		|| height < 0
	{
		slot.why = Why::Format;
		return;
	}
	if width <= 0 || height == 0 || width as u32 > MAX_SIDE || height as u32 > MAX_SIDE {
		slot.why = Why::Damaged;
		return;
	}
	// The colour table that precedes the pixels (a 24-bit bitmap's, when it has one, is ignored by GDI).
	let colours = if bpp == 24 {
		0
	} else if used == 0 {
		1 << bpp
	} else {
		used.min(1 << bpp)
	};
	let stride = (bpp * width as u32).div_ceil(32) * 4;
	// Run-length rows are as long as the header says.
	let pixels = if rle {
		le32(&head, 34)
	} else {
		stride * height as u32
	};
	if rle && pixels == 0 {
		slot.why = Why::Damaged;
		return;
	}
	if off_bits < 14 + 40 + 4 * colours || off_bits.saturating_add(pixels) > size {
		slot.why = Why::Damaged;
		return;
	}
	// Room for this picture: drop the oldest until it fits the budget, and try again alone when Windows says no.
	let need = size - 14;
	while cached_bytes() + need > BUDGET && evict() {}
	let mut block = FarBlock::alloc(need, false);
	while block.is_none() && evict() {
		block = FarBlock::alloc(need, false);
	}
	let Some(block) = block else {
		slot.why = Why::NoMemory;
		slot.size = size;
		return;
	};
	let mut buf = [0u8; 1024];
	let mut pos = 14u32;
	while pos < size {
		crate::store::sample();
		let n = doc.picture_bytes(s, idx, pos, &mut buf);
		if n == 0 {
			break;
		}
		block.write(pos - 14, buf.get(..n).unwrap_or(&[]));
		pos += n as u32;
	}
	let slot = &mut slots()[at];
	if pos < size {
		slot.why = Why::Damaged;
		let mut b = block;
		b.free();
		return;
	}
	slot.block = block;
	slot.bits = off_bits - 14;
	slot.w = width;
	slot.h = height;
	slot.bpp = bpp as u16;
	slot.size = size;
	slot.why = Why::Ready;
}

/// The slot holding picture `idx`, building it when it is not cached (or when another picture took its index).
fn get(doc: &Doc, s: &FarStore, idx: u16) -> &'static Slot {
	let fp = quill_app::picture::fingerprint(doc, s, idx);
	unsafe { CLOCK += 1 };
	let now = unsafe { CLOCK };
	let all = slots();
	if let Some(i) = all
		.iter()
		.position(|x| x.why != Why::None && x.idx == idx && x.fp == fp)
	{
		all[i].used = now;
		return &all[i];
	}
	// A free slot, else the least recently used.
	let at = all
		.iter()
		.position(|x| x.why == Why::None && x.block.is_empty())
		.unwrap_or_else(|| {
			let mut best = 0;
			for (i, x) in all.iter().enumerate() {
				if x.used < all[best].used {
					best = i;
				}
			}
			best
		});
	build(doc, s, idx, fp, at);
	let all = slots();
	all[at].used = now;
	&all[at]
}

/// Draws picture `idx` with its top left at (`x`, `y`), `w` by `h` pixels. Returns false when the picture is not drawn, and the
/// caller paints its grey box.
pub fn draw(dc: u32, doc: &Doc, s: &FarStore, idx: u16, x: i32, y: i32, w: i32, h: i32) -> bool {
	let slot = get(doc, s, idx);
	if slot.why != Why::Ready || w <= 0 || h <= 0 {
		return false;
	}
	unsafe {
		SetStretchBltMode(dc, COLORONCOLOR);
		let lines = StretchDIBits(
			dc,
			x as u32,
			y as u32,
			w as u32,
			h as u32,
			0,
			0,
			slot.w as u32,
			slot.h as u32,
			slot.block.ptr_at(slot.bits),
			slot.block.ptr_at(0),
			DIB_RGB_COLORS,
			SRCCOPY,
		);
		lines != 0
	}
}

/// Writes PICS.LOG beside the program: one line for each picture the page asked for, and the far memory in use.
pub fn write_log() {
	let log = w16::log();
	log.clear();
	let mut any = false;
	for (i, x) in slots().iter().enumerate() {
		if x.why == Why::None {
			continue;
		}
		any = true;
		log.s("slot ")
			.dec(i as u32)
			.s(" picture ")
			.dec(u32::from(x.idx))
			.s(": ");
		match x.why {
			Why::Ready => log
				.s("drawn ")
				.dec(x.w as u32)
				.s("x")
				.dec(x.h as u32)
				.s(" at ")
				.dec(u32::from(x.bpp))
				.s(" bits, ")
				.dec(x.size)
				.s(" bytes of global memory"),
			Why::Png => log.s("grey box: PNG is not decoded here"),
			Why::Format => log.s("grey box: not a bitmap GDI draws as it is"),
			Why::Damaged => log.s("grey box: damaged or out of range"),
			Why::NoMemory => log
				.s("grey box: GlobalAlloc refused ")
				.dec(x.size)
				.s(" bytes"),
			Why::None => log,
		};
		log.nl();
	}
	if !any {
		log.s("no pictures asked for").nl();
	}
	if let Some(e) = unsafe { OPEN_FAILED } {
		log.s("the file did not open: ").s(match e {
			quill_formats::FormatError::BadFile => "not a document, or damaged",
			quill_formats::FormatError::Newer => "from a newer version",
			quill_formats::FormatError::TooLarge => "over a document limit",
			quill_formats::FormatError::NoMemory => "out of memory",
			quill_formats::FormatError::Write => "write error",
		});
		log.nl();
	}
	let (held, asked, free) = unsafe { crate::store::REFUSED };
	let refusals = unsafe { crate::store::REFUSALS };
	if refusals > 0 {
		log.s("store: ")
			.dec(u32::from(refusals))
			.s(" refused; the first with ")
			.dec(u32::from(held))
			.s(" blocks held, asking ")
			.dec(u32::from(asked))
			.s(" bytes, ");
		if free == 0xFFFF_FFFF {
			log.s("the table full");
		} else {
			log.dec(free / 1024).s(" KB free");
		}
		log.nl();
	}
	let mut path = [0u8; 128];
	crate::c_name(b"PICS.LOG", &mut path);
	write_file(path.as_ptr(), log.bytes());
}
