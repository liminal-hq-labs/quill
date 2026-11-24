// P2: Quill's own child window over the page area, with IMM32 input and an AccessKit UI Automation tree.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The WinUI window is a normal top-level window whose XAML content lives in a child window of its own. This module finds the
//! top-level window by process id, creates a sibling child over the page area, draws the page into it with GDI, takes
//! keyboard input through window messages and the Input Method Manager (IMM32), and answers `WM_GETOBJECT` with AccessKit's
//! UI Automation provider, whose text pattern is built from the same layout the pixels come from.

use std::cell::RefCell;

use accesskit::{
	ActionHandler, ActionRequest, ActivationHandler, Node, NodeId, Rect as AkRect, Role, TextPosition, TextSelection, TreeId,
	TreeInfo, TreeUpdate,
};
use accesskit_windows::{Adapter, QueuedEvents};
use windows::core::{w, BOOL};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::Ime::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::doc::{Doc, Inserted};
use crate::page::{self, Frame, Scene};
use crate::{log, send, Msg};

const ROOT: NodeId = NodeId(1);
const RUN0: u64 = 10;

struct State {
	doc: Doc,
	composition: Option<String>,
	composition_cursor: usize,
	focused: bool,
	scale: f64,
	size: (u32, u32),
	frame: Option<Frame>,
	high_surrogate: Option<u16>,
	foreground_tries: u32,
	adapter: Adapter,
}

thread_local! {
	/// The top-level window and the page rectangle in its client coordinates, when the page is an owned pop-up window.
	static POPUP: std::cell::Cell<Option<(HWND, (i32, i32, i32, i32))>> = const { std::cell::Cell::new(None) };
	static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Runs `f` on the window's state, or returns `None` when the state is already borrowed (a nested message) or not yet made.
fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
	STATE.with(|s| s.try_borrow_mut().ok().and_then(|mut b| b.as_mut().map(f)))
}

struct Tree(Option<TreeUpdate>);

impl ActivationHandler for Tree {
	fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
		self.0.take()
	}
}

/// Requests from assistive technology: logged, so the PowerShell run shows what Narrator and the script asked for.
struct Actions;

impl ActionHandler for Actions {
	fn do_action(&mut self, request: ActionRequest) {
		log(&format!("uia action: {:?} on {:?}", request.action, request.target_node));
	}
}

impl State {
	fn render(&mut self) {
		let scene = Scene {
			lines: &self.doc.lines,
			caret: self.doc.caret,
			composition: self.composition.as_deref(),
			focused: false,
			scale: self.scale,
			composition_cursor: self.composition_cursor,
		};
		self.frame = Some(page::render(self.size.0, self.size.1, &scene));
	}

	fn tree(&mut self) -> TreeUpdate {
		if self.frame.is_none() {
			self.render();
		}
		let frame = self.frame.as_ref().expect("frame");
		let mut root = Node::new(Role::MultilineTextInput);
		root.set_author_id("page");
		root.set_label("Quill page");
		root.set_bounds(AkRect {
			x0: 0.0,
			y0: 0.0,
			x1: f64::from(self.size.0),
			y1: f64::from(self.size.1),
		});
		let mut nodes = Vec::new();
		let mut ids = Vec::new();
		let last = self.doc.lines.len().saturating_sub(1);
		for (n, line) in self.doc.lines.iter().enumerate() {
			let id = NodeId(RUN0 + n as u64);
			ids.push(id);
			let hard_break = n != last;
			let mut text = line.clone();
			if hard_break {
				text.push('\n');
			}
			let geom = &frame.lines[n.min(frame.lines.len() - 1)];
			let mut run = Node::new(Role::TextRun);
			run.set_bounds(AkRect {
				x0: f64::from(geom.left),
				y0: f64::from(geom.top),
				x1: f64::from(geom.left + geom.xs.last().copied().unwrap_or(0.0) + 1.0),
				y1: f64::from(geom.bottom),
			});
			let lengths: Vec<u8> = text.chars().map(|c| c.len_utf8() as u8).collect();
			run.set_value(text.clone());
			run.set_character_lengths(lengths);
			let mut positions: Vec<f32> = geom.xs[..geom.xs.len() - 1].to_vec();
			let mut widths: Vec<f32> = geom.xs.windows(2).map(|w| w[1] - w[0]).collect();
			if hard_break {
				positions.push(*geom.xs.last().unwrap_or(&0.0));
				widths.push(0.0);
			}
			run.set_character_positions(positions);
			run.set_character_widths(widths);
			let mut starts = Vec::new();
			let mut prev_space = true;
			for (i, c) in line.chars().enumerate() {
				if prev_space && !c.is_whitespace() {
					starts.push(i as u8);
				}
				prev_space = c.is_whitespace();
			}
			run.set_word_starts(starts);
			nodes.push((id, run));
		}
		root.set_children(ids);
		let caret_node = NodeId(RUN0 + self.doc.caret.0 as u64);
		let position = TextPosition {
			node: caret_node,
			character_index: self.doc.caret.1,
		};
		root.set_text_selection(TextSelection {
			anchor: position,
			focus: position,
		});
		nodes.push((ROOT, root));
		TreeUpdate {
			nodes,
			tree: Some(TreeInfo::new(ROOT)),
			tree_id: TreeId::ROOT,
			focus: ROOT,
		}
	}

	/// Redraws, pushes the new tree to UI Automation and returns the events to raise once no lock is held.
	fn changed(&mut self) -> Option<QueuedEvents> {
		log(&format!("doc: {:?} caret {:?}", self.doc.text(), self.doc.caret));
		self.render();
		let update = self.tree();
		self.adapter.update_if_active(|| update)
	}

	fn insert(&mut self, text: &str, via: &str) -> Option<QueuedEvents> {
		match self.doc.insert_str(text) {
			Inserted::Bytes(bytes) => {
				let hex: Vec<String> = bytes.iter().map(|b| format!("0x{b:02X}")).collect();
				let utf16: Vec<String> = text.encode_utf16().map(|u| format!("U+{u:04X}")).collect();
				log(&format!("insert via {via}: {} -> {}", utf16.join(" "), hex.join(" ")));
				send(Msg::Status(format!("Inserted {} ({via})", hex.join(" "))));
			}
			Inserted::Refused(c) => {
				log(&format!(
					"refused via {via}: U+{:04X} has no Windows-1252 form; the document is unchanged",
					c as u32
				));
				send(Msg::Status(format!(
					"Refused: U+{:04X} has no Windows-1252 form ({via})",
					c as u32
				)));
			}
		}
		self.changed()
	}
}

fn finish(hwnd: HWND, events: Option<QueuedEvents>) {
	if let Some(e) = events {
		e.raise();
	}
	let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
	position_ime(hwnd);
}

/// Puts the composition and candidate windows at the caret, in the child window's client coordinates.
fn position_ime(hwnd: HWND) {
	let Some((x, top, bottom, line_h)) = with(|s| {
		if s.frame.is_none() {
			s.render();
		}
		let f = s.frame.as_ref().expect("frame");
		(f.caret.0, f.caret.1, f.caret.2, f.line_h)
	}) else {
		return;
	};
	unsafe {
		// The system caret is what the emoji panel, the magnifier and the candidate window follow in a window with no
		// text services of its own, so it tracks the page caret.
		let _ = SetCaretPos(x.round() as i32, top.round() as i32);
		let himc = ImmGetContext(hwnd);
		if himc.is_invalid() {
			return;
		}
		let point = POINT {
			x: x.round() as i32,
			y: top.round() as i32,
		};
		let comp = COMPOSITIONFORM {
			dwStyle: CFS_POINT,
			ptCurrentPos: point,
			rcArea: RECT::default(),
		};
		let _ = ImmSetCompositionWindow(himc, &comp);
		let cand = CANDIDATEFORM {
			dwIndex: 0,
			dwStyle: CFS_EXCLUDE,
			ptCurrentPos: POINT {
				x: point.x,
				y: bottom.round() as i32,
			},
			rcArea: RECT {
				left: point.x,
				top: top.round() as i32,
				right: point.x + 2,
				bottom: bottom.round() as i32,
			},
		};
		let _ = ImmSetCandidateWindow(himc, &cand);
		let font = LOGFONTW {
			lfHeight: -(line_h * 0.8).round() as i32,
			..Default::default()
		};
		let _ = ImmSetCompositionFontW(himc, &font);
		let _ = ImmReleaseContext(hwnd, himc);
	}
}

fn read_ime_string(hwnd: HWND, kind: IME_COMPOSITION_STRING) -> Option<String> {
	unsafe {
		let himc = ImmGetContext(hwnd);
		if himc.is_invalid() {
			return None;
		}
		let bytes = ImmGetCompositionStringW(himc, kind, None, 0);
		let mut out = None;
		if bytes >= 0 {
			let mut buf = vec![0u16; bytes as usize / 2];
			ImmGetCompositionStringW(himc, kind, Some(buf.as_mut_ptr().cast()), bytes as u32);
			out = Some(String::from_utf16_lossy(&buf));
		}
		let _ = ImmReleaseContext(hwnd, himc);
		out
	}
}

fn ime_cursor(hwnd: HWND) -> usize {
	unsafe {
		let himc = ImmGetContext(hwnd);
		if himc.is_invalid() {
			return 0;
		}
		let n = ImmGetCompositionStringW(himc, GCS_CURSORPOS, None, 0);
		let _ = ImmReleaseContext(hwnd, himc);
		n.max(0) as usize
	}
}

fn vk_pressed(vk: VIRTUAL_KEY) -> bool {
	unsafe { GetKeyState(i32::from(vk.0)) < 0 }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
	match msg {
		WM_GETOBJECT => {
			let reply = with(|s| {
				let mut tree = Tree(Some(s.tree()));
				s.adapter.handle_wm_getobject(wparam, lparam, &mut tree)
			})
			.flatten();
			if let Some(reply) = reply {
				log("WM_GETOBJECT answered by AccessKit");
				return reply.into();
			}
		}
		WM_ERASEBKGND => return LRESULT(1),
		WM_TIMER => {
			if let Some((top, rel)) = POPUP.with(|p| p.get()) {
				follow(hwnd, top, rel);
			}
			// Keep asking for the foreground for the first seconds: a toast or the shell may hold it when the app starts.
			if with(|s| {
				s.foreground_tries += 1;
				s.foreground_tries < 100
			})
			.unwrap_or(false)
				&& unsafe { GetForegroundWindow() } != hwnd
			{
				let ok = force_foreground(hwnd);
				if ok {
					log("timer: took the foreground");
				}
			}
			return LRESULT(0);
		}
		WM_PAINT => {
			let mut ps = PAINTSTRUCT::default();
			let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
			log(&format!("WM_PAINT rect {:?}", ps.rcPaint));
			with(|s| {
				if s.frame.is_none() {
					s.render();
				}
				let f = s.frame.as_ref().expect("frame");
				let (w, h) = (f.pixmap.width() as i32, f.pixmap.height() as i32);
				let bits = page::bgra(&f.pixmap);
				let info = BITMAPINFO {
					bmiHeader: BITMAPINFOHEADER {
						biSize: size_of::<BITMAPINFOHEADER>() as u32,
						biWidth: w,
						biHeight: -h,
						biPlanes: 1,
						biBitCount: 32,
						biCompression: BI_RGB.0,
						..Default::default()
					},
					..Default::default()
				};
				unsafe {
					SetDIBitsToDevice(
						hdc,
						0,
						0,
						w as u32,
						h as u32,
						0,
						0,
						0,
						h as u32,
						bits.as_ptr().cast(),
						&info,
						DIB_RGB_COLORS,
					);
				}
			});
			let _ = unsafe { EndPaint(hwnd, &ps) };
			return LRESULT(0);
		}
		WM_SIZE => {
			let (w, h) = ((lparam.0 & 0xFFFF) as u32, ((lparam.0 >> 16) & 0xFFFF) as u32);
			let events = with(|s| {
				s.size = (w.max(1), h.max(1));
				s.scale = f64::from(unsafe { GetDpiForWindow(hwnd) }) / 96.0;
				s.changed()
			})
			.flatten();
			finish(hwnd, events);
			return LRESULT(0);
		}
		WM_LBUTTONDOWN => {
			let _ = unsafe { SetFocus(Some(hwnd)) };
			let (x, y) = ((lparam.0 & 0xFFFF) as i16 as f32, ((lparam.0 >> 16) & 0xFFFF) as i16 as f32);
			let events = with(|s| {
				if s.frame.is_none() {
					s.render();
				}
				let f = s.frame.as_ref().expect("frame");
				if let Some(n) = f.lines.iter().position(|l| y >= l.top && y < l.bottom) {
					let g = &f.lines[n];
					let col = (0..g.xs.len())
						.min_by(|a, b| {
							let da = (g.left + g.xs[*a] - x).abs();
							let db = (g.left + g.xs[*b] - x).abs();
							da.total_cmp(&db)
						})
						.unwrap_or(0);
					s.doc.caret = (n, col);
					log(&format!("click: caret to line {n} column {col}"));
				}
				s.changed()
			})
			.flatten();
			finish(hwnd, events);
			return LRESULT(0);
		}
		WM_SETFOCUS | WM_KILLFOCUS => {
			let focused = msg == WM_SETFOCUS;
			log(if focused { "page: focus gained" } else { "page: focus lost" });
			if focused && std::env::args().any(|a| a == "--imeon") {
				// Test aid: opens the input method in hiragana mode, since the guest has no key chord that does it from outside.
				unsafe {
					let hkl = LoadKeyboardLayoutW(w!("E0010411"), KLF_ACTIVATE);
					log(&format!(
						"LoadKeyboardLayoutW(E0010411) -> {hkl:?}; current {:?}",
						GetKeyboardLayout(0)
					));
					let himc = ImmGetContext(hwnd);
					if !himc.is_invalid() {
						let opened = ImmSetOpenStatus(himc, true).as_bool();
						let mode = ImmSetConversionStatus(himc, IME_CMODE_NATIVE | IME_CMODE_FULLSHAPE, IME_SMODE_PHRASEPREDICT)
							.as_bool();
						log(&format!("ImmSetOpenStatus {opened}, ImmSetConversionStatus {mode}"));
						let _ = ImmReleaseContext(hwnd, himc);
					}
				}
			}
			if focused {
				let lh = with(|s| s.frame.as_ref().map(|f| (f.caret.2 - f.caret.1).round() as i32))
					.flatten()
					.unwrap_or(20);
				let _ = unsafe { CreateCaret(hwnd, None, 2, lh) };
				let _ = unsafe { ShowCaret(Some(hwnd)) };
			} else {
				let _ = unsafe { DestroyCaret() };
			}
			let events = with(|s| {
				s.focused = focused;
				let e = s.adapter.update_window_focus_state(focused);
				s.render();
				e
			})
			.flatten();
			finish(hwnd, events);
			send(Msg::Status(String::from(if focused {
				"Page has focus"
			} else {
				"Page lost focus"
			})));
			return LRESULT(0);
		}
		WM_GETDLGCODE => return LRESULT((DLGC_WANTALLKEYS | DLGC_WANTCHARS) as isize),
		WM_KEYDOWN => {
			let vk = VIRTUAL_KEY(wparam.0 as u16);
			log(&format!("WM_KEYDOWN vk=0x{:02X}", vk.0));
			if vk == VK_TAB {
				let back = vk_pressed(VK_SHIFT);
				log(&format!(
					"Tab leaves the page ({})",
					if back { "backwards" } else { "forwards" }
				));
				send(Msg::TabOut { back });
				return LRESULT(0);
			}
			let events = with(|s| {
				match vk {
					VK_LEFT => s.doc.left(),
					VK_RIGHT => s.doc.right(),
					VK_UP => s.doc.up(),
					VK_DOWN => s.doc.down(),
					VK_HOME => s.doc.home(),
					VK_END => s.doc.end(),
					_ => return None,
				}
				s.changed()
			})
			.flatten();
			finish(hwnd, events);
			return LRESULT(0);
		}
		WM_CHAR | WM_SYSCHAR => {
			let unit = wparam.0 as u16;
			log(&format!(
				"{} 0x{unit:04X}",
				if msg == WM_CHAR { "WM_CHAR" } else { "WM_SYSCHAR" }
			));
			let events = with(|s| {
				let text = match unit {
					0xD800..=0xDBFF => {
						s.high_surrogate = Some(unit);
						return None;
					}
					0xDC00..=0xDFFF => {
						let hi = s.high_surrogate.take()?;
						String::from_utf16_lossy(&[hi, unit])
					}
					8 => {
						s.doc.backspace();
						return s.changed();
					}
					13 => {
						s.doc.enter();
						return s.changed();
					}
					0..=31 | 127 => return None,
					_ => String::from_utf16_lossy(&[unit]),
				};
				s.high_surrogate = None;
				s.insert(&text, "WM_CHAR")
			})
			.flatten();
			finish(hwnd, events);
			return LRESULT(0);
		}
		WM_UNICHAR => {
			if wparam.0 as u32 == UNICODE_NOCHAR {
				return LRESULT(1);
			}
			log(&format!("WM_UNICHAR U+{:X}", wparam.0));
			if let Some(c) = char::from_u32(wparam.0 as u32) {
				let events = with(|s| s.insert(&c.to_string(), "WM_UNICHAR")).flatten();
				finish(hwnd, events);
			}
			return LRESULT(1);
		}
		WM_IME_SETCONTEXT => {
			// The page draws the composition string inline, so the input method's own composition window stays hidden; its
			// candidate list is left to the system.
			let flags = lparam.0 as u32 & !ISC_SHOWUICOMPOSITIONWINDOW;
			log(&format!(
				"WM_IME_SETCONTEXT active={} lparam=0x{:08X}",
				wparam.0, lparam.0 as u32
			));
			return unsafe { DefWindowProcW(hwnd, msg, wparam, LPARAM(flags as isize)) };
		}
		WM_IME_STARTCOMPOSITION => {
			log("WM_IME_STARTCOMPOSITION");
			position_ime(hwnd);
			return LRESULT(0);
		}
		WM_IME_COMPOSITION => {
			let flags = lparam.0 as u32;
			log(&format!("WM_IME_COMPOSITION flags=0x{flags:08X}"));
			let mut events = None;
			if flags & GCS_RESULTSTR.0 != 0 {
				if let Some(result) = read_ime_string(hwnd, GCS_RESULTSTR) {
					events = with(|s| {
						s.composition = None;
						s.insert(&result, "IME result")
					})
					.flatten();
				}
			}
			if flags & GCS_COMPSTR.0 != 0 {
				let comp = read_ime_string(hwnd, GCS_COMPSTR).unwrap_or_default();
				let cursor = ime_cursor(hwnd);
				log(&format!("composition: {comp:?} cursor {cursor}"));
				send(Msg::Status(format!("Composing: {comp}")));
				events = with(|s| {
					s.composition = (!comp.is_empty()).then_some(comp);
					s.composition_cursor = cursor;
					s.changed()
				})
				.flatten()
				.or(events);
			}
			finish(hwnd, events);
			return LRESULT(0);
		}
		WM_IME_ENDCOMPOSITION => {
			log("WM_IME_ENDCOMPOSITION");
			let events = with(|s| {
				s.composition = None;
				s.changed()
			})
			.flatten();
			finish(hwnd, events);
			return LRESULT(0);
		}
		WM_IME_REQUEST => {
			if wparam.0 as u32 == IMR_QUERYCHARPOSITION {
				let info = lparam.0 as *mut IMECHARPOSITION;
				let caret = with(|s| {
					if s.frame.is_none() {
						s.render();
					}
					s.frame.as_ref().map(|f| f.caret)
				})
				.flatten();
				if let (Some((x, top, bottom)), false) = (caret, info.is_null()) {
					let mut p = POINT {
						x: x.round() as i32,
						y: top.round() as i32,
					};
					let _ = unsafe { ClientToScreen(hwnd, &mut p) };
					let mut area = RECT::default();
					let _ = unsafe { GetClientRect(hwnd, &mut area) };
					let mut tl = POINT { x: 0, y: 0 };
					let _ = unsafe { ClientToScreen(hwnd, &mut tl) };
					unsafe {
						(*info).pt = p;
						(*info).cLineHeight = (bottom - top).round() as u32;
						(*info).rcDocument = RECT {
							left: tl.x,
							top: tl.y,
							right: tl.x + area.right,
							bottom: tl.y + area.bottom,
						};
					}
					log(&format!("IMR_QUERYCHARPOSITION -> screen ({}, {})", p.x, p.y));
					return LRESULT(1);
				}
			}
		}
		WM_NCDESTROY => {
			STATE.with(|s| s.borrow_mut().take());
		}
		_ => {}
	}
	unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

struct Search {
	pid: u32,
	found: Option<HWND>,
}

unsafe extern "system" fn each_top(hwnd: HWND, lparam: LPARAM) -> BOOL {
	let search = unsafe { &mut *(lparam.0 as *mut Search) };
	let mut pid = 0;
	unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
	if pid == search.pid && unsafe { IsWindowVisible(hwnd) }.as_bool() && unsafe { GetWindow(hwnd, GW_OWNER) }.is_err_or_null() {
		let mut title = [0u16; 128];
		let n = unsafe { GetWindowTextW(hwnd, &mut title) };
		if n > 0 {
			search.found = Some(hwnd);
			return false.into();
		}
	}
	true.into()
}

trait OwnerCheck {
	fn is_err_or_null(&self) -> bool;
}

impl OwnerCheck for windows::core::Result<HWND> {
	fn is_err_or_null(&self) -> bool {
		self.as_ref().map(|h| h.0.is_null()).unwrap_or(true)
	}
}

/// The visible, titled, unowned top-level window of this process: the one Reactor made.
pub fn find_top_level() -> Option<HWND> {
	let mut search = Search {
		pid: std::process::id(),
		found: None,
	};
	let _ = unsafe { EnumWindows(Some(each_top), LPARAM(&mut search as *mut Search as isize)) };
	search.found
}

unsafe extern "system" fn each_child(hwnd: HWND, lparam: LPARAM) -> BOOL {
	let out = unsafe { &mut *(lparam.0 as *mut String) };
	let mut class = [0u16; 128];
	let n = unsafe { GetClassNameW(hwnd, &mut class) } as usize;
	let mut r = RECT::default();
	let _ = unsafe { GetWindowRect(hwnd, &mut r) };
	out.push_str(&format!(
		"{:?} class {} rect {}x{}; ",
		hwnd.0,
		String::from_utf16_lossy(&class[..n]),
		r.right - r.left,
		r.bottom - r.top
	));
	true.into()
}

/// A one-line list of the top-level window's children, for the log.
pub fn describe_children(top: HWND) -> String {
	let mut out = String::new();
	let _ = unsafe { EnumChildWindows(Some(top), Some(each_child), LPARAM(&mut out as *mut String as isize)) };
	out
}

/// Moves an owned pop-up page to the screen position of `rel` in `top`'s client area, hiding it while `top` is minimized.
fn follow(hwnd: HWND, top: HWND, rel: (i32, i32, i32, i32)) {
	unsafe {
		if IsIconic(top).as_bool() || !IsWindowVisible(top).as_bool() {
			let _ = ShowWindow(hwnd, SW_HIDE);
			return;
		}
		let mut origin = POINT { x: rel.0, y: rel.1 };
		let _ = ClientToScreen(top, &mut origin);
		let mut now = RECT::default();
		let _ = GetWindowRect(hwnd, &mut now);
		let moved = now.left != origin.x || now.top != origin.y || now.right - now.left != rel.2 || now.bottom - now.top != rel.3;
		if moved || !IsWindowVisible(hwnd).as_bool() {
			let _ = SetWindowPos(
				hwnd,
				None,
				origin.x,
				origin.y,
				rel.2,
				rel.3,
				SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
			);
		}
	}
}

/// Creates the child window at `rect` (x, y, width, height in client pixels of `top`) and its accessibility adapter.
pub fn create(top: HWND, rect: (i32, i32, i32, i32), popup: bool) -> windows::core::Result<HWND> {
	unsafe {
		let (style, x, y) = if popup {
			let mut origin = POINT { x: rect.0, y: rect.1 };
			let _ = ClientToScreen(top, &mut origin);
			(WS_POPUP | WS_VISIBLE, origin.x, origin.y)
		} else {
			(WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_TABSTOP, rect.0, rect.1)
		};
		let instance = GetModuleHandleW(None)?;
		let class = WNDCLASSEXW {
			cbSize: size_of::<WNDCLASSEXW>() as u32,
			style: CS_HREDRAW | CS_VREDRAW,
			lpfnWndProc: Some(proc),
			hInstance: instance.into(),
			hCursor: LoadCursorW(None, IDC_IBEAM)?,
			lpszClassName: w!("QuillPageChild"),
			..Default::default()
		};
		RegisterClassExW(&class);
		let hwnd = CreateWindowExW(
			WINDOW_EX_STYLE(0),
			w!("QuillPageChild"),
			w!("Quill page"),
			style,
			x,
			y,
			rect.2,
			rect.3,
			Some(top),
			None,
			Some(instance.into()),
			None,
		)?;
		let adapter = Adapter::new(hwnd, false, Actions);
		let scale = f64::from(GetDpiForWindow(hwnd)) / 96.0;
		let state = State {
			doc: Doc::new(&[
				"Dear Reader,",
				"Type here: a dead key then e, Alt+0233, or Win+. for an emoji.",
				"",
			]),
			composition: None,
			composition_cursor: 0,
			focused: false,
			scale,
			size: (rect.2.max(1) as u32, rect.3.max(1) as u32),
			frame: None,
			high_surrogate: None,
			foreground_tries: 0,
			adapter,
		};
		STATE.with(|s| *s.borrow_mut() = Some(state));
		if popup {
			POPUP.with(|p| p.set(Some((top, rect))));
			SetTimer(Some(hwnd), 1, 30, None);
		} else {
			let _ = SetWindowPos(hwnd, Some(HWND_TOP), rect.0, rect.1, rect.2, rect.3, SWP_SHOWWINDOW);
		}
		let _ = InvalidateRect(Some(hwnd), None, false);
		Ok(hwnd)
	}
}

/// Moves and sizes the child after the page area changed.
pub fn place(hwnd: HWND, rect: (i32, i32, i32, i32)) {
	if let Some((top, _)) = POPUP.with(|p| p.get()) {
		POPUP.with(|p| p.set(Some((top, rect))));
		return follow(hwnd, top, rect);
	}
	let _ = unsafe { SetWindowPos(hwnd, Some(HWND_TOP), rect.0, rect.1, rect.2, rect.3, SWP_SHOWWINDOW) };
}

/// Gives keyboard focus to the child window.
pub fn focus(hwnd: HWND) {
	let fg = force_foreground(hwnd);
	let _ = unsafe { SetFocus(Some(hwnd)) };
	log(&format!("focus: foreground {fg}, focus now {:?}", unsafe { GetFocus() }));
}

/// Makes `hwnd` the foreground window by attaching to the current foreground thread's input queue. A process started from a
/// scheduled task is not allowed to take the foreground otherwise; a real launch from the shell does not need this.
fn force_foreground(hwnd: HWND) -> bool {
	unsafe {
		// A synthetic Alt press marks the process as having seen input, which SetForegroundWindow requires.
		keybd_event(VK_MENU.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
		keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);
		let current = GetForegroundWindow();
		let theirs = if current.0.is_null() {
			0
		} else {
			GetWindowThreadProcessId(current, None)
		};
		let ours = windows::Win32::System::Threading::GetCurrentThreadId();
		let attached =
			theirs != 0 && theirs != ours && windows::Win32::System::Threading::AttachThreadInput(ours, theirs, true).as_bool();
		let ok = SetForegroundWindow(hwnd).as_bool();
		if attached {
			let _ = windows::Win32::System::Threading::AttachThreadInput(ours, theirs, false);
		}
		ok
	}
}

unsafe extern "system" fn focus_first_other(hwnd: HWND, lparam: LPARAM) -> BOOL {
	let found = unsafe { &mut *(lparam.0 as *mut Option<HWND>) };
	let mut class = [0u16; 128];
	let n = unsafe { GetClassNameW(hwnd, &mut class) } as usize;
	let name = String::from_utf16_lossy(&class[..n]);
	if name != "QuillPageChild" && found.is_none() {
		*found = Some(hwnd);
	}
	true.into()
}

/// Moves Win32 keyboard focus to the XAML content's window, found as the top-level window's first child that is not ours.
pub fn focus_island(top: HWND) {
	let mut found: Option<HWND> = None;
	let _ = unsafe {
		EnumChildWindows(
			Some(top),
			Some(focus_first_other),
			LPARAM(&mut found as *mut Option<HWND> as isize),
		)
	};
	log(&format!("focus island: {found:?}"));
	if let Some(h) = found {
		let _ = unsafe { SetFocus(Some(h)) };
	}
}
