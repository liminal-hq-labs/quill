// The loader for optional modules: find a `.DMD` file, check it, place it in memory, bind its services, start it, hand back its table.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The format is Desk's (`desk-module`, `dos/module/format`; docs/plans/modules.md section 5); this is the hosted loader Desk's own
// roadmap lists as M5. The interface a module speaks, its status codes and the build-id check are `quill-modabi`'s (CORE-90, CORE-94).
//
// **Where a module lives (DOS-54).** In memory the program allocates from the DOS allocator's own heap, which is DPMI memory above the
// first megabyte; the program image sits in one DPMI block of its own and the stub gives CS and DS the same base, so a module's address
// is a near offset from that base (the value of a pointer), exactly like every other pointer in the program. Memory is zeroed, the image is
// copied in and relocated for the address it landed at, and the block is freed when the `Module` is dropped, so loading and freeing are
// a heap allocation and a heap free: 200 cycles leave the heap's total unchanged. `Place::Conventional` puts the image below the first
// megabyte (DPMI 0100h) and `Place::Arena` (the `module-arena` feature) in a static arena inside the program's own `.bss`; both exist
// so the same module can be tested at addresses that wrap and at addresses CS already covers.
//
// **CS (DOS-53).** The stub's code selector reaches only the image's own block, so code in a heap block is outside it. Before the first
// module runs, `widen_code_segment` checks that CS and DS share a base (DPMI 0006h), raises the limit of CS to 4 GiB (0008h) and reloads
// CS with a far return. The check and the widening are one-time; without the base check a module's near offsets would mean different
// bytes to code and data. Arena modules sit inside the image block, which CS already covers, and skip it.
//
// **Handshake.** The file's checks (header, layout, every relocation and import, CRC-32) run before a byte is written; then `app_start(1,
// &ModStart, size)` takes the host's ABI version, kind and build id and returns 0 or a status, filling in the kind's table (whose first
// two words are `size` and `info` in every kind), and the host calls `info(&ModInfo)` and runs `quill_modabi::check_info`: in a
// `Policy::Release` program only its own build id is accepted, in `Policy::Development` another build is accepted with a log line.
//
// **Failure.** Every refusal is a `Failure` (a `quill_modabi` status and a reason), logged once, with whatever was allocated freed.
// Nothing here panics on a bad file, a short read, a bad module or a full heap, and nothing waits.
use crate::{cleanup, dos, dpmi, env, io};
use alloc::alloc::{alloc, dealloc};
use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::mem::size_of;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use desk_module::{load_into, FormatError, LoadError, Module as Image, KIND_MODULE};
pub use quill_modabi::Policy;
use quill_modabi::{self as abi, Accepted, ModInfo, ModStart, PicTableV1, Status};

/// The service table version this loader binds (`desk_log`, `desk_panic`, `desk_alloc`, `desk_free`: Desk's SDK 1).
pub const SDK: u16 = 1;

/// The largest module file read (docs/plans/modules.md section 5.3: a module is at most 256 KB in Quill).
pub const MAX_FILE: u32 = 256 * 1024;

/// How much of the one-byte status the module may hand back from `app_start` and still be taken as a status.
const LAST_STATUS: i32 = abi::QS_VERSION as i32;

/// The size of the arena, shared by every module placed in it (`module-arena`).
#[cfg(feature = "module-arena")]
pub const ARENA_SIZE: usize = 64 * 1024;

/// Where a module's image goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
	/// The heap: DPMI memory above the first megabyte. The default.
	Extended,
	/// Conventional memory below the first megabyte (DPMI 0100h): at most 1 MB less 16 bytes, and a near offset that wraps below the
	/// image's own base.
	Conventional,
	/// The static arena in `.bss` (the `module-arena` feature); CS already reaches it, so CS is not widened.
	#[cfg(feature = "module-arena")]
	Arena,
}

/// What the program tells the loader about itself and about what it wants.
#[derive(Clone, Copy, Debug)]
pub struct Options {
	/// The kind the caller wants (`quill_modabi::KIND_PIC`, ...), told to the module and checked against its `info`.
	pub kind: u16,
	/// The program's build id; a `Policy::Release` program accepts only modules with this id.
	pub build_id: u32,
	pub policy: Policy,
	pub place: Place,
	/// The ABI the host claims. `quill_modabi`'s own version unless a test says otherwise.
	pub abi_major: u16,
	pub abi_minor: u16,
	/// Widen CS before the module runs (always, outside the one test that shows what happens without it).
	pub widen: bool,
}

impl Options {
	/// Options for a program of build `build_id` that wants a module of `kind`, with the defaults of a shipped build.
	pub fn new(kind: u16, build_id: u32, policy: Policy) -> Options {
		Options {
			kind,
			build_id,
			policy,
			#[cfg(feature = "module-arena")]
			place: Place::Arena,
			#[cfg(not(feature = "module-arena"))]
			place: Place::Extended,
			abi_major: abi::ABI_MAJOR,
			abi_minor: abi::ABI_MINOR,
			widen: true,
		}
	}
}

/// Why a module was not loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
	/// The file is not there (DOS error 2 or 3).
	Missing,
	/// DOS refused to open or read the file (the error code).
	Dos(u16),
	/// The file has fewer bytes than its length said.
	Short { want: u32, got: u32 },
	/// The file is longer than `MAX_FILE`.
	TooBig(u32),
	/// No memory of this many bytes for the file or the image.
	NoMemory(u32),
	/// CS and DS do not share a base, or the host refused to widen CS.
	NoAlias,
	/// `desk-module` refused the file.
	Format(FormatError),
	/// The file's kind is not a module, or its image does not fit where it was put.
	Placement,
	/// The module imports a service the loader does not have.
	Import(String),
	/// The module's start entry returned this.
	Start(i32),
	/// The table the module handed back is not inside the module, or too short.
	BadTable,
	/// The module's `info` was refused by `quill_modabi::check_info`.
	Info,
}

/// A refusal: a `quill_modabi` status the program can branch on and the reason the log gets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
	pub status: Status,
	pub reason: Reason,
}

impl Failure {
	fn new(status: Status, reason: Reason) -> Failure {
		Failure { status, reason }
	}

	/// The reason in words, for the log (not for the person: see `missing_message`).
	pub fn describe(&self) -> String {
		let mut out = String::from(abi::status_name(self.status));
		out.push_str(": ");
		match &self.reason {
			Reason::Missing => out.push_str("file not found"),
			Reason::Dos(code) => out.push_str(&alloc::format!("DOS error {code:#x}")),
			Reason::Short { want, got } => out.push_str(&alloc::format!("short read: {got} of {want} bytes")),
			Reason::TooBig(len) => out.push_str(&alloc::format!("{len} bytes is over {MAX_FILE}")),
			Reason::NoMemory(len) => out.push_str(&alloc::format!("no memory for {len} bytes")),
			Reason::NoAlias => out.push_str("CS and DS differ or CS cannot be widened"),
			Reason::Format(e) => out.push_str(&alloc::format!("{e}")),
			Reason::Placement => out.push_str("not a module, or it does not fit"),
			Reason::Import(name) => {
				out.push_str("no service ");
				out.push_str(name);
			}
			Reason::Start(code) => out.push_str(&alloc::format!("start returned {code}")),
			Reason::BadTable => out.push_str("the table is not inside the module"),
			Reason::Info => out.push_str("another kind, major version or build than this program"),
		}
		out
	}
}

/// Where a loaded module's memory came from, so it can be given back.
enum Region {
	Heap {
		ptr: *mut u8,
		layout: Layout,
	},
	Conventional {
		selector: u16,
	},
	#[cfg(feature = "module-arena")]
	Arena {
		ptr: *mut u8,
	},
}

/// A module that is loaded, started and accepted. Dropping it runs its stop entry (if it has one) and frees its memory.
pub struct Module {
	region: Region,
	base: u32,
	mem_size: u32,
	kind: u16,
	started: bool,
	stop: Option<u32>,
	table: u32,
	table_len: u32,
	info: ModInfo,
	accepted: Accepted,
}

impl Module {
	/// The near offset of the first byte of the image.
	pub fn base(&self) -> u32 {
		self.base
	}

	/// The bytes of memory the module holds (the image and its `.bss`).
	pub fn mem_size(&self) -> u32 {
		self.mem_size
	}

	/// The kind the module reported.
	pub fn kind(&self) -> u16 {
		self.kind
	}

	/// What the module's `info` entry said about itself.
	pub fn info(&self) -> &ModInfo {
		&self.info
	}

	/// Whether another build or a newer minor version was accepted.
	pub fn accepted(&self) -> Accepted {
		self.accepted
	}

	/// The near offset of the kind's table and its length in bytes.
	pub fn table(&self) -> (u32, u32) {
		(self.table, self.table_len)
	}

	/// The picture table, if this is a picture module whose table is the whole of `PicTableV1` with every entry set and inside the
	/// module. Valid until the module is dropped.
	pub fn pic(&self) -> Option<&PicTableV1> {
		if self.kind != abi::KIND_PIC || (self.table_len as usize) < size_of::<PicTableV1>() {
			return None;
		}
		// SAFETY: `table` was checked to lie inside the module's own memory, which lives as long as `self`, and is 4-byte aligned.
		let table = unsafe { &*(self.table as usize as *const PicTableV1) };
		let inside = |address: u32| address >= self.base && address - self.base < self.mem_size;
		let all = [
			table.info.map(|f| f as usize),
			table.open.map(|f| f as usize),
			table.rows.map(|f| f as usize),
			table.close.map(|f| f as usize),
		];
		all.iter()
			.all(|entry| matches!(entry, Some(a) if inside(*a as u32)))
			.then_some(table)
	}
}

impl Drop for Module {
	fn drop(&mut self) {
		if self.started {
			if let Some(stop) = self.stop {
				// SAFETY: `stop` is the module's `app_stop` entry, bound and relocated, and CS still reaches it.
				let f: extern "C" fn(u32) = unsafe { core::mem::transmute::<usize, extern "C" fn(u32)>(stop as usize) };
				f(1);
			}
		}
		release(&mut self.region);
	}
}

// ---------------------------------------------------------------------------------------------------------------------------
// Where modules are found and what the person is told
// ---------------------------------------------------------------------------------------------------------------------------

/// The folder the program was started from, with its trailing backslash (from the environment block, CORE-95), or `None` when DOS did not
/// leave the path there.
pub fn program_folder() -> Option<String> {
	let path = env::program_path()?;
	let end = path.rfind(['\\', '/', ':'])?;
	Some(String::from(&path[..=end]))
}

/// The file name of a module kind's `.DMD` (`QUILLPIC.DMD`), or `None` for a kind this program does not know.
pub fn file_name(kind: u16) -> Option<String> {
	let name = abi::name_of(kind)?;
	let mut out = String::from(name);
	out.push_str(abi::DOS_EXTENSION);
	Some(out)
}

/// What the status line says when a module is missing or refused (CORE-93): the file, why it might be so, and the way out.
pub fn missing_message(kind: u16) -> String {
	let file = file_name(kind).unwrap_or_else(|| String::from("a module"));
	let mut out = String::new();
	match kind {
		abi::KIND_PIC => {
			out.push_str("This picture needs ");
			out.push_str(&file);
			out.push_str(", which is missing or from another build of Quill.");
		}
		abi::KIND_DLG => {
			out.push_str("Quill cannot show this dialog: ");
			out.push_str(&file);
			out.push_str(" is missing or from another build. Run Setup again to repair Quill.");
		}
		_ => {
			out.push_str("Quill cannot use ");
			out.push_str(&file);
			out.push_str(", which is missing or from another build. Run Setup again to repair Quill.");
		}
	}
	out
}

// ---------------------------------------------------------------------------------------------------------------------------
// The host's services (SDK 1)
// ---------------------------------------------------------------------------------------------------------------------------

/// The most a log line or a panic message from a module is taken as.
const LINE_MAX: usize = 512;

static SINK: AtomicUsize = AtomicUsize::new(0);

/// Where `desk_log` lines (and the loader's own) go: the program's log. The default writes `module: TEXT` to standard output.
pub fn set_log_sink(sink: fn(&[u8])) {
	SINK.store(sink as usize, Ordering::Relaxed);
}

/// Puts the default sink (standard output) back.
pub fn reset_log_sink() {
	SINK.store(0, Ordering::Relaxed);
}

fn log(text: &[u8]) {
	let sink = SINK.load(Ordering::Relaxed);
	if sink != 0 {
		// SAFETY: the value was stored from a `fn(&[u8])` by `set_log_sink`.
		let f: fn(&[u8]) = unsafe { core::mem::transmute::<usize, fn(&[u8])>(sink) };
		f(text);
	} else {
		let _ = dos::write_all(1, b"module: ");
		let _ = dos::write_all(1, text);
		let _ = dos::write_all(1, b"\r\n");
	}
}

fn log_str(text: &str) {
	log(text.as_bytes());
}

extern "C" fn service_log(text: *const u8, len: usize) {
	if !text.is_null() {
		// SAFETY: the module passes `len` readable bytes; the length is cut so a corrupt call cannot read far.
		log(unsafe { core::slice::from_raw_parts(text, len.min(LINE_MAX)) });
	}
}

/// A panic in a module is a panic of the program (DOS-54): the clean-up list runs, the message is written, the program exits with 3.
extern "C" fn service_panic(text: *const u8, len: usize) -> ! {
	static PANICKING: AtomicBool = AtomicBool::new(false);
	if PANICKING.swap(true, Ordering::SeqCst) {
		dos::exit_now(3);
	}
	cleanup::run_all();
	io::write_str("RT PANIC: module: ");
	if !text.is_null() {
		// SAFETY: as `service_log`.
		let _ = dos::write_all(1, unsafe { core::slice::from_raw_parts(text, len.min(LINE_MAX)) });
	}
	io::write_str("\r\n");
	dos::exit_now(3)
}

extern "C" fn service_alloc(size: usize, align: usize) -> *mut u8 {
	match Layout::from_size_align(size, align.max(1)) {
		// SAFETY: the layout has a nonzero size, which `alloc` requires.
		Ok(layout) if layout.size() != 0 => unsafe { alloc(layout) },
		_ => core::ptr::null_mut(),
	}
}

extern "C" fn service_free(ptr: *mut u8, size: usize, align: usize) {
	if let Ok(layout) = Layout::from_size_align(size, align.max(1)) {
		if !ptr.is_null() && layout.size() != 0 {
			// SAFETY: the module hands back a block `service_alloc` gave it, with the layout it asked for.
			unsafe { dealloc(ptr, layout) };
		}
	}
}

/// The address of the service named `name` (Desk's four, SDK 1), or `None`.
fn service(name: &str) -> Option<u32> {
	let address = match name {
		"desk_log" => service_log as *const () as usize,
		"desk_panic" => service_panic as *const () as usize,
		"desk_alloc" => service_alloc as *const () as usize,
		"desk_free" => service_free as *const () as usize,
		_ => return None,
	};
	Some(address as u32)
}

// ---------------------------------------------------------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------------------------------------------------------

#[cfg(feature = "module-arena")]
mod arena {
	use super::ARENA_SIZE;
	use core::cell::UnsafeCell;
	use core::sync::atomic::{AtomicU32, Ordering};

	#[repr(C, align(4096))]
	struct Bytes(UnsafeCell<[u8; ARENA_SIZE]>);
	// SAFETY: single-threaded, like the rest of the runtime.
	unsafe impl Sync for Bytes {}
	static BYTES: Bytes = Bytes(UnsafeCell::new([0; ARENA_SIZE]));

	const SLOTS: usize = 4;
	static START: [AtomicU32; SLOTS] = [const { AtomicU32::new(0) }; SLOTS];
	static LEN: [AtomicU32; SLOTS] = [const { AtomicU32::new(0) }; SLOTS];

	fn fits(start: u32, len: u32) -> bool {
		if start as u64 + len as u64 > ARENA_SIZE as u64 {
			return false;
		}
		(0..SLOTS).all(|i| {
			let (s, l) = (START[i].load(Ordering::Relaxed), LEN[i].load(Ordering::Relaxed));
			l == 0 || start + len <= s || s + l <= start
		})
	}

	/// The first place from the start of the arena or the end of a held slot where `len` bytes at `align` fit, and a free slot to note it.
	pub fn take(len: u32, align: u32) -> Option<*mut u8> {
		let free = (0..SLOTS).find(|&i| LEN[i].load(Ordering::Relaxed) == 0)?;
		let ends = (0..SLOTS).map(|i| START[i].load(Ordering::Relaxed) + LEN[i].load(Ordering::Relaxed));
		for end in core::iter::once(0).chain(ends) {
			let start = end.checked_add(align - 1)? & !(align - 1);
			if fits(start, len) {
				START[free].store(start, Ordering::Relaxed);
				LEN[free].store(len, Ordering::Relaxed);
				// SAFETY: `start + len` is inside the arena, checked by `fits`.
				return Some(unsafe { BYTES.0.get().cast::<u8>().add(start as usize) });
			}
		}
		None
	}

	pub fn give(ptr: *mut u8) {
		let start = (ptr as usize - BYTES.0.get() as usize) as u32;
		for i in 0..SLOTS {
			if LEN[i].load(Ordering::Relaxed) != 0 && START[i].load(Ordering::Relaxed) == start {
				LEN[i].store(0, Ordering::Relaxed);
			}
		}
	}
}

fn round_up(x: u32, a: u32) -> u32 {
	(x + a - 1) & !(a - 1)
}

/// Gets `size` zeroed bytes at `align` from `place`.
fn reserve(place: Place, size: u32, align: u32) -> Result<(Region, *mut u8), Failure> {
	let no_memory = || Failure::new(abi::QS_TOO_LARGE, Reason::NoMemory(size));
	let align = align.max(16);
	let (region, ptr) = match place {
		Place::Extended => {
			let layout = Layout::from_size_align(size as usize, align as usize).map_err(|_| no_memory())?;
			// SAFETY: the size is nonzero (a module has a header and an image).
			let ptr = unsafe { alloc(layout) };
			if ptr.is_null() {
				return Err(no_memory());
			}
			(Region::Heap { ptr, layout }, ptr)
		}
		Place::Conventional => {
			let want = size.checked_add(align).ok_or_else(no_memory)?;
			let paragraphs = u16::try_from(want.div_ceil(16)).map_err(|_| no_memory())?;
			let (segment, selector) = dpmi::alloc_dos_memory(paragraphs).map_err(|_| no_memory())?;
			let linear = round_up((segment as u32) << 4, align);
			(Region::Conventional { selector }, dpmi::linear_to_ptr::<u8>(linear))
		}
		#[cfg(feature = "module-arena")]
		Place::Arena => {
			let ptr = arena::take(size, align).ok_or_else(no_memory)?;
			(Region::Arena { ptr }, ptr)
		}
	};
	// SAFETY: the region is at least `size` bytes at `ptr`.
	unsafe { core::ptr::write_bytes(ptr, 0, size as usize) };
	Ok((region, ptr))
}

fn release(region: &mut Region) {
	match region {
		// SAFETY: `ptr` and `layout` are what `reserve` allocated, released once (a dropped module is not used again).
		Region::Heap { ptr, layout } => unsafe { dealloc(*ptr, *layout) },
		Region::Conventional { selector } => {
			let _ = dpmi::free_dos_memory(*selector);
		}
		#[cfg(feature = "module-arena")]
		Region::Arena { ptr } => arena::give(*ptr),
	}
}

// ---------------------------------------------------------------------------------------------------------------------------
// CS
// ---------------------------------------------------------------------------------------------------------------------------

static WIDENED: AtomicBool = AtomicBool::new(false);

/// Whether CS has been widened (by `widen_code_segment`).
pub fn code_segment_widened() -> bool {
	WIDENED.load(Ordering::Relaxed)
}

/// Raises the limit of CS to 4 GiB so code in a heap block can run (DOS-53), after checking CS and DS have the same base. Done once;
/// later calls return at once. Returns `QS_NOALIAS` if the bases differ or the host refuses either call.
pub fn widen_code_segment() -> Result<(), Status> {
	if WIDENED.load(Ordering::Relaxed) {
		return Ok(());
	}
	let cs: u16;
	// SAFETY: reading a segment register has no effect.
	unsafe { core::arch::asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags)) };
	let cs_base = dpmi::get_segment_base(cs).map_err(|_| abi::QS_NOALIAS)?;
	if cs_base != dpmi::image_base() {
		return Err(abi::QS_NOALIAS);
	}
	dpmi::set_segment_limit(cs, 0xFFFF_FFFF).map_err(|_| abi::QS_NOALIAS)?;
	// A far jump to the next instruction with the selector CS already holds, so the CPU reloads the descriptor and its new limit. (A far
	// return would do, but `retf` assembles with a 16-bit operand size here and leaves four bytes on the stack.)
	let mut target: [u32; 2] = [0, cs as u32];
	// SAFETY: the jump lands on the label that follows it, in the same segment; `target` is the offset and selector it reads.
	unsafe {
		core::arch::asm!(
			"lea {t}, [2f]",
			"mov [{p}], {t}",
			"jmp fword ptr [{p}]",
			"2:",
			t = out(reg) _,
			p = in(reg) target.as_mut_ptr(),
			options(preserves_flags)
		);
	}
	WIDENED.store(true, Ordering::Relaxed);
	Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------------------------------------------------------

/// Loads the module of `kind` from the program's own folder (CORE-95): `<folder>QUILLPIC.DMD` for `KIND_PIC`, never the current folder.
pub fn load(options: &Options) -> Result<Module, Failure> {
	let (Some(folder), Some(file)) = (program_folder(), file_name(options.kind)) else {
		return Err(fail("module", Failure::new(abi::QS_MISSING, Reason::Missing)));
	};
	let mut path = folder;
	path.push_str(&file);
	load_file(&path, options)
}

/// Loads the module in the file `path`.
pub fn load_file(path: &str, options: &Options) -> Result<Module, Failure> {
	let bytes = read_file(path).map_err(|failure| fail(path, failure))?;
	load_bytes(path, &bytes, options)
}

/// Loads a module from bytes already in memory; `name` is for the log.
pub fn load_bytes(name: &str, bytes: &[u8], options: &Options) -> Result<Module, Failure> {
	place_and_start(name, bytes, options).map_err(|failure| fail(name, failure))
}

/// Appends `value` in decimal without `core::fmt`, which a program that avoids it (QUILLDOS.EXE) would otherwise carry for the loader.
fn push_dec(out: &mut String, mut value: u32) {
	let mut digits = [0u8; 10];
	let mut at = digits.len();
	loop {
		at -= 1;
		digits[at] = b'0' + (value % 10) as u8;
		value /= 10;
		if value == 0 {
			break;
		}
	}
	for &digit in &digits[at..] {
		out.push(digit as char);
	}
}

/// Appends `value` as eight hex digits, as `push_dec` does.
fn push_hex(out: &mut String, value: u32) {
	for shift in (0..8).rev() {
		out.push(char::from(b"0123456789ABCDEF"[(value >> (shift * 4)) as usize & 15]));
	}
}

/// Logs the refusal in one line (`NAME: refused: STATUS: reason`) and hands it back. Built without `core::fmt`.
fn fail(name: &str, failure: Failure) -> Failure {
	let mut line = String::from(name);
	line.push_str(": refused: ");
	line.push_str(abi::status_name(failure.status));
	line.push_str(": ");
	match &failure.reason {
		Reason::Missing => line.push_str("file not found"),
		Reason::Dos(code) => {
			line.push_str("DOS error ");
			push_dec(&mut line, *code as u32);
		}
		Reason::Short { want, got } => {
			line.push_str("short read: ");
			push_dec(&mut line, *got);
			line.push_str(" of ");
			push_dec(&mut line, *want);
		}
		Reason::TooBig(len) => {
			line.push_str("file of ");
			push_dec(&mut line, *len);
			line.push_str(" bytes is too large");
		}
		Reason::NoMemory(len) => {
			line.push_str("no memory for ");
			push_dec(&mut line, *len);
			line.push_str(" bytes");
		}
		Reason::NoAlias => line.push_str("CS and DS differ or CS cannot be widened"),
		Reason::Format(_) => line.push_str("not a usable module file"),
		Reason::Placement => line.push_str("not a module, or it does not fit"),
		Reason::Import(name) => {
			line.push_str("no service ");
			line.push_str(name);
		}
		Reason::Start(code) => {
			line.push_str("start returned ");
			push_dec(&mut line, *code as u32);
		}
		Reason::BadTable => line.push_str("the table is not inside the module"),
		Reason::Info => line.push_str("another kind, major version or build than this program"),
	}
	log_str(&line);
	failure
}

/// Reads the whole file into the heap, without panicking when the heap is full or the file is short.
fn read_file(path: &str) -> Result<Vec<u8>, Failure> {
	let dos_error = |e: dos::DosError| match e.0 {
		2 | 3 => Failure::new(abi::QS_MISSING, Reason::Missing),
		code => Failure::new(abi::QS_MISSING, Reason::Dos(code)),
	};
	let mut file = dos::File::open(path, dos::Mode::Read).map_err(dos_error)?;
	let len = file.len().map_err(dos_error)?;
	if len > MAX_FILE {
		return Err(Failure::new(abi::QS_TOO_LARGE, Reason::TooBig(len)));
	}
	let mut bytes: Vec<u8> = Vec::new();
	bytes
		.try_reserve_exact(len as usize)
		.map_err(|_| Failure::new(abi::QS_TOO_LARGE, Reason::NoMemory(len)))?;
	bytes.resize(len as usize, 0);
	let mut got = 0usize;
	while got < bytes.len() {
		let n = file.read(&mut bytes[got..]).map_err(dos_error)?;
		if n == 0 {
			break;
		}
		got += n;
	}
	if got < bytes.len() {
		return Err(Failure::new(
			abi::QS_CORRUPT,
			Reason::Short {
				want: len,
				got: got as u32,
			},
		));
	}
	Ok(bytes)
}

/// The status a `LoadError` is reported as.
fn load_status(error: &LoadError) -> Status {
	match error {
		LoadError::MissingImport { .. } => abi::QS_VERSION,
		LoadError::KindNotLoadable { .. } => abi::QS_UNSUPPORTED,
		_ => abi::QS_CORRUPT,
	}
}

/// The status a `FormatError` is reported as: a newer format or service table than this program is another version, the rest are a
/// damaged or foreign file.
fn format_status(error: &FormatError) -> Status {
	match error {
		FormatError::UnsupportedVersion(_) | FormatError::UnsupportedAbi(_) | FormatError::SdkTooNew { .. } => abi::QS_VERSION,
		_ => abi::QS_CORRUPT,
	}
}

fn place_and_start(name: &str, bytes: &[u8], options: &Options) -> Result<Module, Failure> {
	let image = Image::parse(bytes, SDK).map_err(|e| Failure::new(format_status(&e), Reason::Format(e)))?;
	let header = *image.header();
	let (mut region, ptr) = reserve(options.place, header.mem_size, header.align)?;
	// A near offset is the value of a pointer: CS and DS share the program's base.
	let base = ptr as usize as u32;
	// SAFETY: `reserve` returned `mem_size` zeroed bytes at `ptr`, which nothing else uses.
	let dest = unsafe { core::slice::from_raw_parts_mut(ptr, header.mem_size as usize) };
	let placed = match load_into(&image, KIND_MODULE, dest, base, &|import| service(import)) {
		Ok(placed) => placed,
		Err(error) => {
			release(&mut region);
			let status = load_status(&error);
			let reason = match error {
				LoadError::MissingImport { name, .. } => Reason::Import(String::from(name)),
				_ => Reason::Placement,
			};
			return Err(Failure::new(status, reason));
		}
	};
	let mut module = Module {
		region,
		base: placed.base,
		mem_size: header.mem_size,
		kind: 0,
		started: false,
		stop: placed.stop,
		table: 0,
		table_len: 0,
		info: ModInfo {
			size: 0,
			abi_major: 0,
			abi_minor: 0,
			kind: 0,
			build_id: 0,
			arena: 0,
			limit: 0,
		},
		accepted: Accepted {
			other_build: false,
			newer_minor: false,
		},
	};
	#[cfg(feature = "module-arena")]
	let widen = options.widen && options.place != Place::Arena;
	#[cfg(not(feature = "module-arena"))]
	let widen = options.widen;
	if widen {
		if let Err(status) = widen_code_segment() {
			return Err(Failure::new(status, Reason::NoAlias));
		}
	}

	let mut start = ModStart {
		size: size_of::<ModStart>() as u16,
		abi_major: options.abi_major,
		abi_minor: options.abi_minor,
		kind: options.kind,
		build_id: options.build_id,
		table: 0,
		table_len: 0,
	};
	// SAFETY: `placed.start` is the module's `app_start`, relocated for where it landed; the file's checks put it inside the image and
	// CS now reaches it. It takes the application id, a pointer to the block and its size, and returns 0 or a status.
	let entry: extern "C" fn(u32, *mut ModStart, u32) -> i32 =
		unsafe { core::mem::transmute::<usize, extern "C" fn(u32, *mut ModStart, u32) -> i32>(placed.start as usize) };
	module.started = true;
	let code = entry(1, &mut start, size_of::<ModStart>() as u32);
	if code != 0 {
		module.started = false;
		let status = if (1..=LAST_STATUS).contains(&code) {
			code as Status
		} else {
			abi::QS_INTERNAL
		};
		return Err(Failure::new(status, Reason::Start(code)));
	}
	// The module wrote the table through the pointer it was given; read it back as bytes the compiler cannot have assumed.
	// SAFETY: `start` is a live local the module wrote through, and the read is volatile.
	let (table, table_len) = unsafe {
		(
			core::ptr::read_volatile(&start.table),
			core::ptr::read_volatile(&start.table_len),
		)
	};
	let inside =
		|offset: u32, len: u32| offset >= module.base && (offset - module.base) as u64 + len as u64 <= module.mem_size as u64;
	// Every kind's table begins with its size and its `info` entry, so the host can ask the module about itself.
	if table_len < 8 || table % 4 != 0 || !inside(table, table_len) {
		return Err(Failure::new(abi::QS_CORRUPT, Reason::BadTable));
	}
	// SAFETY: the table is inside the module's memory (checked) and 4-byte aligned.
	let info_entry = unsafe { core::ptr::read((table as usize + 4) as *const u32) };
	if !inside(info_entry, 1) {
		return Err(Failure::new(abi::QS_CORRUPT, Reason::BadTable));
	}
	module.table = table;
	module.table_len = table_len;
	let mut info = ModInfo {
		size: 0,
		abi_major: 0,
		abi_minor: 0,
		kind: 0,
		build_id: 0,
		arena: 0,
		limit: 0,
	};
	// SAFETY: `info_entry` is the module's `info`, inside it; it fills the block it is given and returns a status.
	let ask: extern "C" fn(u32) -> Status =
		unsafe { core::mem::transmute::<usize, extern "C" fn(u32) -> Status>(info_entry as usize) };
	let status = ask(&mut info as *mut ModInfo as usize as u32);
	if status != abi::QS_OK {
		return Err(Failure::new(status, Reason::Info));
	}
	let accepted = abi::check_info(&info, options.kind, options.build_id, options.policy)
		.map_err(|status| Failure::new(status, Reason::Info))?;
	module.kind = info.kind;
	module.info = info;
	module.accepted = accepted;
	if accepted.other_build {
		let mut line = String::from(name);
		line.push_str(": another build accepted (development): module ");
		push_hex(&mut line, info.build_id);
		line.push_str(", program ");
		push_hex(&mut line, options.build_id);
		log_str(&line);
	}
	Ok(module)
}
