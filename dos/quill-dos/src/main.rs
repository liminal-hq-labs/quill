// QUILLDOS.EXE: Quill for DOS.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The editor in 80 by 25 text mode, or 80 by 50 with `/50` or `Rows=50` under `[Quill]` in `QUILL.INI`. `QUILLDOS.EXE [FILE]` opens a file. Set `QUILL_DUMP=D:\DUMP.TXT` to write every screen
//! shown, read back from video memory, as golden frames: how the headless tests compare it with the host's.
//!
//! `/G` (or `/GFX`, or `Graphics=1`) runs in graphics instead: a VBE 640 by 480 mode with 256 colours if the card has one, else VGA mode
//! 12h; `/G16` (or `Graphics=16`) takes mode 12h only; `/T` takes text mode back over the setting. The page is drawn with the bitmap
//! strikes of `QUILL.FNT` (`QUILL_FNT` names another file). If the font or the mode is not there, the editor runs in text mode and says so
//! on leaving. In graphics `QUILL_DUMP` writes `frame N crc32 X video Y` per screen (the shadow's CRC-32 and the CRC-32 of the video
//! memory read back), and `QUILL_TIMES` a line of microseconds per screen, to measure with, and `QUILL_KEYS` types the characters it holds instead of reading the keyboard.
#![no_std]
#![no_main]

extern crate alloc;

mod files;
mod gfx;
mod keys;
mod measure;
mod mouse;
mod video;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use measure::ScreenMeasure;
use quill_app::mouse::{Ctx, Pointer, TextCells};
use quill_app::picture::WINDOW;
use quill_app::script::Step;
use quill_app::{App, Key, Scheme};
use quill_grid::{Grid, GridMeasure};
use quill_heap::HeapStore;
use quill_mouse::Ev;
use quill_paint::raster::{Linear8, Planar4};
use quill_paint::shell::{self, Graphics};
use quill_strike::{StrikeMeasure, Strikes};
use rt::dos::{File, Mode};
use rt::env;

rt::entry!(main);

/// Draws the screen. `frame` writes it to the dump as a golden frame; the pointer's cell goes on after the dump, so the frames are the
/// host's.
fn show<const ROWS: usize>(
	app: &App<ScreenMeasure>,
	s: &HeapStore,
	dump: &mut Option<File>,
	n: &mut u32,
	pointer: Option<(i32, i32)>,
	frame: bool,
) {
	let mut g = Grid::<ROWS>::new(0);
	let cursor = quill_app::draw(app, s, &mut g);
	video::present(&g);
	video::cursor(cursor.map(|(x, y)| (x as u8, y as u8)));
	if cursor.is_some() {
		video::cursor_shape(app.overtype());
	}
	if frame {
		if let Some(f) = dump {
			let mut text = String::new();
			*n += 1;
			let _ = video::read_back::<ROWS>().dump(*n, &mut text);
			let _ = f.write_all(text.as_bytes());
		}
	}
	if let Some(at) = pointer {
		video::pointer(at, ROWS);
	}
}

/// The test script `QUILL_MOUSE` names: keys and pointer actions in order, played in place of the keyboard and the mouse driver. A
/// run that holds (`QUILL_HOLD`) goes on to the keyboard when the script ends, which is how a screenshot is taken of its last frame.
fn script_text() -> Option<String> {
	let path = env::var("QUILL_MOUSE")?;
	let bytes = File::open(&path, Mode::Read).ok()?.read_to_vec(0).ok()?;
	String::from_utf8(bytes).ok()
}

/// The rows `QUILL.INI` asks for (`Rows=` under `[Quill]`), or 25.
fn rows_setting(ini: &[u8]) -> usize {
	let mut rows = 25;
	for line in ini.split(|b| *b == b'\n') {
		let line = line.strip_suffix(b"\r").unwrap_or(line);
		if let Some(v) = line.strip_prefix(&b"Rows="[..]) {
			rows = if v.starts_with(b"50") { 50 } else { 25 };
		}
	}
	rows
}

/// The lines of `QUILL.INI` that `save_settings` does not write and the shell does: the 50-row choice and the graphics setting. Put
/// back at the end of the file after each save.
struct Kept {
	rows_50: bool,
	graphics: Option<Vec<u8>>,
}

impl Kept {
	fn of(ini: &[u8], rows_50: bool) -> Self {
		let mut graphics = None;
		for line in ini.split(|b| *b == b'\n') {
			let line = line.strip_suffix(b"\r").unwrap_or(line);
			if line.starts_with(b"Graphics=") {
				graphics = Some(line.to_vec());
			}
		}
		Kept { rows_50, graphics }
	}

	/// Appends what was kept to the settings file the app just wrote.
	fn put_back(&self) {
		if !self.rows_50 && self.graphics.is_none() {
			return;
		}
		let kept = File::open("QUILL.INI", Mode::Read)
			.ok()
			.and_then(|mut f| f.read_to_vec(0).ok());
		if let (Some(mut text), Ok(mut f)) = (kept, File::open("QUILL.INI", Mode::Create)) {
			if self.rows_50 {
				text.extend_from_slice(b"Rows=50\r\n");
			}
			if let Some(g) = &self.graphics {
				text.extend_from_slice(g);
				text.extend_from_slice(b"\r\n");
			}
			let _ = f.write_all(&text);
		}
	}
}

fn main() -> i32 {
	let mut ini = [0u8; 512];
	let n = File::open("QUILL.INI", Mode::Read)
		.ok()
		.and_then(|mut f| f.read(&mut ini).ok())
		.unwrap_or(0);
	let mut rows = rows_setting(&ini[..n]);
	let from_ini = rows == 50;
	let mut graphics = Graphics::from_ini(&ini[..n]);
	for a in &env::args() {
		if a.as_str() == "/50" || a.as_str() == "-50" {
			rows = 50;
		}
		if let Some(g) = Graphics::from_arg(a.as_str()) {
			graphics = g;
		}
	}
	let kept = Kept::of(&ini[..n], from_ini);
	let mut note = None;
	if graphics != Graphics::Text {
		match run_gfx(graphics, &kept) {
			Ok(code) => return code,
			Err(why) => note = Some(why),
		}
	}
	// The screen decides: a card with no 50-row mode keeps 25.
	let rows = video::enter(rows);
	let code = if rows == 50 { run::<50>(&kept) } else { run::<25>(&kept) };
	if let Some(why) = note {
		rt::println!("Quill: graphics mode was not used ({why}); it ran in text mode.");
	}
	code
}

/// Everything the two screens share: an empty document, the settings, and the command line (`/MONO`, a file to open).
fn start(app: &mut App<ScreenMeasure>, store: &mut HeapStore, files: &mut files::DosFiles) -> bool {
	if app.reset(store).is_err() {
		return false;
	}
	app.load_settings(files);
	// The test runs (`QUILL_DUMP`) keep one fixed line so the DOS screens match the host's; otherwise About says what it runs on.
	if env::var("QUILL_DUMP").is_some() {
		app.set_about("Host test run");
	} else {
		let (dos_major, dos_minor) = rt::dos::dos_version();
		let (dpmi_major, dpmi_minor, ..) = rt::dpmi::version();
		let mem = rt::dpmi::free_memory().unwrap_or_default();
		let free = (mem.free_pages != u32::MAX).then(|| mem.free_pages.saturating_mul(4));
		let mut text = [0u8; 96];
		let n = quill_app::platform::dos_about(
			&mut text,
			(dos_major, dos_minor),
			(dpmi_major, dpmi_minor),
			mem.largest_block / 1024,
			free,
		);
		app.set_about(core::str::from_utf8(text.get(..n).unwrap_or(&[])).unwrap_or(""));
	}
	true
}

fn open_arguments(app: &mut App<ScreenMeasure>, store: &mut HeapStore, files: &mut files::DosFiles) {
	for a in &env::args() {
		if a.eq_ignore_ascii_case("/mono") || a.eq_ignore_ascii_case("-mono") {
			app.set_scheme(Scheme::Mono);
		} else if !a.starts_with('/') && !a.starts_with('-') {
			let _ = app.open(store, files, a.as_bytes());
		}
	}
}

fn run<const ROWS: usize>(kept: &Kept) -> i32 {
	let mut store = HeapStore::new();
	let mut app = Box::new(App::with_measure(ScreenMeasure::Grid(GridMeasure)));
	let mut files = files::DosFiles;
	if !start(&mut app, &mut store, &mut files) {
		video::leave();
		rt::println!("Quill: there is not enough memory.");
		return 1;
	}
	app.set_rows(ROWS - 2);
	open_arguments(&mut app, &mut store, &mut files);
	let mut dump = env::var("QUILL_DUMP").and_then(|p| File::open(&p, Mode::Create).ok());
	let mut n = 0;
	let target = TextCells { rows: (ROWS - 2) as i32 };
	let mut ptr = Pointer::default();
	let mut mouse = mouse::open(80, ROWS as i32, (8, 8));
	let script = script_text();
	let mut at = None;
	show::<ROWS>(&app, &store, &mut dump, &mut n, at, true);
	// A test script plays first, in place of the keyboard and the driver; the steps are the host's (`quill_host::script::play`).
	let mut hold = false;
	if let Some(text) = &script {
		let mut tick = 0;
		for step in quill_app::script::steps(text) {
			match step {
				Step::Key(k) => app.key(&mut store, &mut files, k),
				other => other.play(&mut tick, |ev, ctx| {
					ptr.handle(&mut app, &mut store, &mut files, &target, ev, ctx);
					at = Some(ptr.at());
				}),
			}
			show::<ROWS>(&app, &store, &mut dump, &mut n, at, true);
			if app.quit() {
				break;
			}
		}
		hold = env::var("QUILL_HOLD").is_some() && !app.quit();
	}
	while (script.is_none() || hold) && !app.quit() {
		let mut clicked = false;
		let waited = keys::wait_until(&mut || {
			let Some(m) = mouse.as_mut() else {
				return false;
			};
			let mut any = false;
			m.poll(|ev| {
				any = true;
				clicked |= !matches!(ev, Ev::Move { .. });
				let ctx = Ctx {
					shift: keys::shift_down(),
					tick: mouse::ticks(),
				};
				ptr.handle(&mut app, &mut store, &mut files, &target, ev, ctx);
			});
			any
		});
		at = mouse.as_ref().map(|m| m.at()).or(at);
		match waited {
			keys::Wait::Key(Some(k)) => {
				app.key(&mut store, &mut files, k);
				show::<ROWS>(&app, &store, &mut dump, &mut n, at, true);
			}
			keys::Wait::Key(None) => {}
			keys::Wait::Break => show::<ROWS>(&app, &store, &mut dump, &mut n, at, clicked),
		}
	}
	if let Some(m) = mouse {
		m.close();
	}
	app.save_settings(&mut files);
	// `save_settings` writes only the keys the app knows, so what the shell keeps goes back at the end of `[Quill]`.
	kept.put_back();
	if let Some(f) = &mut dump {
		let _ = f.write_all(b"=== end ===\n");
	}
	video::leave();
	0
}

/// Draws the whole screen into the shadow.
fn paint(
	scr: &mut gfx::Screen,
	fnt: &mut files::FntFile,
	strikes: &'static Strikes<'static>,
	app: &App<ScreenMeasure>,
	store: &HeapStore,
	window: &mut [u8; WINDOW],
) {
	let mut ui = StrikeMeasure::new(strikes);
	match scr.kind {
		gfx::Kind::Planar16 => {
			if let Some(mut sf) = Planar4::new(&mut scr.shadow, *strikes, fnt) {
				shell::paint_screen(&mut sf, app, &mut ui, store, window);
			}
		}
		gfx::Kind::Linear256 => {
			if let Some(mut sf) = Linear8::new(&mut scr.shadow, 640, 480, 640, *strikes, fnt) {
				shell::paint_screen(&mut sf, app, &mut ui, store, window);
			}
		}
	}
}

/// Runs the editor in graphics. `Err` (with the screen untouched) when the font or the mode is not there.
fn run_gfx(want: Graphics, kept: &Kept) -> Result<i32, &'static str> {
	let path = env::var("QUILL_FNT").unwrap_or_else(|| alloc::string::String::from("QUILL.FNT"));
	let (mut fnt, strikes) = files::FntFile::open(&path)?;
	let mut scr = gfx::enter(want)?;
	let mut store = HeapStore::new();
	let mut app = Box::new(App::with_measure(ScreenMeasure::Strikes(StrikeMeasure::new(strikes))));
	let mut files = files::DosFiles;
	if !start(&mut app, &mut store, &mut files) {
		gfx::leave();
		return Err("there is not enough memory");
	}
	app.set_view_height(shell::view_height());
	app.set_rows(shell::view_rows());
	open_arguments(&mut app, &mut store, &mut files);
	let mut window = Box::new([0u8; WINDOW]);
	let mut dump = env::var("QUILL_DUMP").and_then(|p| File::open(&p, Mode::Create).ok());
	let mut times = env::var("QUILL_TIMES").and_then(|p| File::open(&p, Mode::Create).ok());
	if times.is_some() {
		gfx::timer_start();
	}
	// QUILL_KEYS=text plays the characters of `text` as typed keys and then leaves, so a run that measures does not depend on the pace
	// the emulator types at (`|` is Enter, `<` is Backspace).
	let script = env::var("QUILL_KEYS");
	let mut played = 0;
	let mut n = 0u32;
	// QUILL_MOUSE names a script of keys and pointer actions (places in pixels), played in place of the keyboard and the driver.
	let steps: Option<Vec<Step>> = script_text().map(|t| quill_app::script::steps(&t).collect());
	let hold = steps.is_some() && env::var("QUILL_HOLD").is_some();
	let mut step = 0;
	let mut tick = 0;
	let mut mouse = mouse::open(shell::SCREEN_W, shell::SCREEN_H, (1, 1));
	let mut ptr = Pointer::default();
	let mut at: Option<(i32, i32)> = None;
	let mut repaint = true;
	loop {
		if repaint {
			// A pass over the laid-out lines is what the painter does several times a screen; timing one shows what layout costs.
			let layout = times.as_ref().map(|_| {
				let a = gfx::clocks();
				let mut lines = 0u32;
				app.for_each_line(&store, |_, _| lines += 1);
				(gfx::clocks().wrapping_sub(a), lines)
			});
			let t0 = gfx::clocks();
			paint(&mut scr, &mut fnt, strikes, &app, &store, &mut window);
			let t1 = gfx::clocks();
			let rows = scr.present();
			let t2 = gfx::clocks();
			n += 1;
			if let Some(f) = &mut dump {
				// The arrow is on the card and not in the shadow: it comes off for the read-back, so a frame's two sums compare.
				scr.hide_pointer();
				let line = format!(
					"frame {n} crc32 {:08x} video {:08x}\n",
					quill_paint::crc32(&scr.shadow),
					scr.read_back_crc()
				);
				let _ = f.write_all(line.as_bytes());
			}
			if let Some(f) = &mut times {
				let line = format!(
					"frame {n} paint_us {} present_us {} rows {rows} layout_pass_us {} lines {}\n",
					gfx::micros(t1.wrapping_sub(t0)),
					gfx::micros(t2.wrapping_sub(t1)),
					gfx::micros(layout.map_or(0, |l| l.0)),
					layout.map_or(0, |l| l.1)
				);
				let _ = f.write_all(line.as_bytes());
			}
			if let Some(p) = at {
				scr.show_pointer(p);
			}
			repaint = false;
		} else if let Some(p) = at {
			if scr.pointer() != Some(p) {
				scr.show_pointer(p);
			}
		}
		if app.quit() {
			break;
		}
		let key = match (&steps, &script) {
			(Some(list), _) if step < list.len() => {
				let this = list.get(step).copied();
				step += 1;
				match this {
					Some(Step::Key(k)) => Some(k),
					Some(other) => {
						other.play(&mut tick, |ev, ctx| {
							ptr.handle(&mut app, &mut store, &mut files, &shell::Pixels, ev, ctx);
							at = Some(ptr.at());
						});
						repaint = true;
						None
					}
					None => None,
				}
			}
			(Some(_), _) if !hold => break,
			(None, Some(text)) => match text.as_bytes().get(played) {
				Some(b) => {
					played += 1;
					Some(match b {
						b'|' => Key::Enter,
						b'<' => Key::Backspace,
						c => Key::Char(*c),
					})
				}
				None => break,
			},
			_ => {
				let mut moved = false;
				let waited = keys::wait_until(&mut || {
					let Some(m) = mouse.as_mut() else {
						return false;
					};
					let mut any = false;
					m.poll(|ev| {
						any = true;
						// A press, a release, the wheel, or a move that drags a selection changes the page; a plain move is the arrow's.
						let drags = ptr.dragging() || matches!(ev, Ev::Down { .. });
						moved |= !matches!(ev, Ev::Move { .. }) || drags;
						let ctx = Ctx {
							shift: keys::shift_down(),
							tick: mouse::ticks(),
						};
						ptr.handle(&mut app, &mut store, &mut files, &shell::Pixels, ev, ctx);
					});
					any
				});
				at = mouse.as_ref().map(|m| m.at()).or(at);
				repaint |= moved;
				match waited {
					keys::Wait::Key(k) => k,
					keys::Wait::Break => None,
				}
			}
		};
		if let Some(k) = key {
			app.key(&mut store, &mut files, k);
			repaint = true;
		}
	}
	if let Some(m) = mouse {
		m.close();
	}
	app.save_settings(&mut files);
	kept.put_back();
	if let Some(f) = &mut times {
		let line = format!(
			"heap_bytes {} font_pages_read {} font_bytes_resident {} shadow_bytes {}\n",
			rt::heap::HEAP.total_bytes(),
			fnt.loaded,
			fnt.resident(),
			scr.shadow.len()
		);
		let _ = f.write_all(line.as_bytes());
	}
	if let Some(f) = &mut dump {
		let _ = f.write_all(b"=== end ===\n");
	}
	gfx::leave();
	Ok(0)
}
