// Spike S0b: input methods and UI Automation for a page drawn by Quill itself, in two hostings.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `--p1` shows the page in a `SwapChainPanel` and takes input through a hidden `TextBox` proxy (after probing
//! `CoreTextEditContext`); `--p2` (the default) puts a child window of Quill's own over the page area, with IMM32 input and
//! an AccessKit UI Automation tree. Both keep the S0a window: a menu bar, a command bar, a combo box, a flyout button and a
//! content dialog, so the airspace and Tab checks have something to run against.

#![windows_subsystem = "windows"]

mod child;
mod coretext;
mod doc;
mod page;
#[path = "../swapchain.rs"]
mod swapchain;

use std::cell::RefCell;
use std::io::Write;
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use reactor_core::Interface as _;
use windows::core::Interface;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows_reactor::*;

static START: OnceLock<Instant> = OnceLock::new();

fn epoch_ms() -> u128 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|d| d.as_millis())
		.unwrap_or(0)
}

/// Appends a line to `quill-winui-s0b.log` next to the exe, with milliseconds since `main` began.
pub fn log(line: &str) {
	let since = START.get().map(|s| s.elapsed().as_millis()).unwrap_or(0);
	let Ok(exe) = std::env::current_exe() else { return };
	let path = exe.with_file_name("quill-winui-s0b.log");
	if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
		let _ = writeln!(f, "[{since:>6} ms] {line}");
	}
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
	P1,
	P2,
}

#[derive(Clone)]
pub enum Msg {
	Surface(SwapChainPanelEvent),
	Menu(String),
	Combo(Option<usize>),
	Button(String),
	Attached(Result<(), String>),
	/// A status line from the child window.
	Status(String),
	/// Tab pressed in the child window: move focus into the XAML chrome.
	TabOut {
		back: bool,
	},
	/// The P1 proxy's text changed.
	Proxy(String),
	/// Take focus again on the proxy, after it was rebuilt.
	Refocus,
	DialogClosed,
	Panel,
}

thread_local! {
	static SENDER: RefCell<Option<LocalSender<Msg>>> = const { RefCell::new(None) };
}

/// Sends a message to the component from code running on the XAML thread, such as the child window's procedure.
pub fn send(msg: Msg) {
	SENDER.with(|s| {
		if let Some(sender) = s.borrow().as_ref() {
			let _ = sender.send(msg);
		}
	});
}

const SIZES: [&str; 3] = ["Normal", "Heading 1", "Heading 2"];
const STATUS_ROW: f64 = 32.0;

struct Spike {
	mode: Mode,
	panel: ElementRef<SwapChainPanel>,
	proxy: ElementRef<TextBox>,
	target: ElementRef<Button>,
	_observation: ElementObservation,
	_coretext: Option<coretext::Probe>,
	gpu: Option<swapchain::Gpu>,
	chain: Option<windows::Win32::Graphics::Dxgi::IDXGISwapChain1>,
	device: (u32, u32),
	scale: f32,
	status: String,
	top: Option<HWND>,
	child: Option<HWND>,
	dialog_open: bool,
	proxy_text: String,
	proxy_gen: u32,
	/// Where the proxy goes: the page caret in DIPs.
	caret_dip: (f64, f64),
}

impl Component for Spike {
	type Input = ();
	type Message = Msg;

	fn create(_input: &(), context: &ComponentContext<Self>) -> Self {
		let mode = if std::env::args().any(|a| a == "--p1") {
			Mode::P1
		} else {
			Mode::P2
		};
		let panel = ElementRef::<SwapChainPanel>::new();
		let sender = context.sender();
		SENDER.with(|s| *s.borrow_mut() = Some(sender.clone()));
		let observation = panel.observe_surface(move |event| {
			let _ = sender.send(Msg::Surface(event));
		});
		let gpu = (mode == Mode::P1).then(swapchain::Gpu::new).and_then(|r| match r {
			Ok(gpu) => {
				log(&format!("d3d11 device: {}", gpu.driver));
				Some(gpu)
			}
			Err(e) => {
				log(&format!("no d3d11 device: {e}"));
				None
			}
		});
		let coretext = (mode == Mode::P1).then(coretext::probe);
		log(&format!("component created, mode {mode:?}"));
		Spike {
			mode,
			panel,
			proxy: ElementRef::new(),
			target: ElementRef::new(),
			_observation: observation,
			_coretext: coretext,
			gpu,
			chain: None,
			device: (0, 0),
			scale: 1.0,
			status: String::from("Ready"),
			top: None,
			child: None,
			dialog_open: false,
			proxy_text: String::new(),
			proxy_gen: 0,
			caret_dip: (24.0, 24.0),
		}
	}

	fn update(&mut self, message: Msg, context: &ComponentContext<Self>) {
		match message {
			Msg::Menu(label) => {
				log(&format!("menu: {label}"));
				self.status = format!("Menu: {label}");
				if label == "Open dialog" {
					self.dialog_open = true;
				}
			}
			Msg::Button(label) => {
				log(&format!("button: {label}"));
				self.status = format!("Button: {label}");
			}
			Msg::Combo(index) => {
				let name = index.and_then(|i| SIZES.get(i)).copied().unwrap_or("none");
				log(&format!("combo: {name}"));
				self.status = format!("Style: {name}");
			}
			Msg::Panel => {
				log("flyout button clicked");
			}
			Msg::DialogClosed => {
				log("dialog closed");
				self.dialog_open = false;
			}
			Msg::Attached(result) => log(&format!("swap chain attached: {result:?}")),
			Msg::Status(s) => self.status = s,
			Msg::Refocus => {
				let accepted = self
					.proxy
					.request_focus_result(|r| log(&format!("proxy focus result: {r:?}")));
				log(&format!("proxy focus requested: {accepted}"));
			}
			Msg::TabOut { back } => self.tab_out(back),
			Msg::Proxy(text) => self.on_proxy(text, context),
			Msg::Surface(SwapChainPanelEvent::Rendering) => {}
			Msg::Surface(SwapChainPanelEvent::Metrics {
				width,
				height,
				scale_x,
				scale_y,
			}) => {
				let (w, h) = (
					(width * f64::from(scale_x)).round() as u32,
					(height * f64::from(scale_y)).round() as u32,
				);
				log(&format!(
					"metrics: {width} x {height} DIP, scale {scale_x} x {scale_y}, device {w} x {h}"
				));
				if w == 0 || h == 0 || ((w, h) == self.device && scale_x == self.scale) {
					return;
				}
				self.device = (w, h);
				self.scale = scale_x;
				match self.mode {
					Mode::P1 => self.rebuild(scale_x, scale_y, context),
					Mode::P2 => self.place_child(),
				}
			}
		}
	}

	fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
		context.window_title(match self.mode {
			Mode::P1 => "Quill WinUI spike S0b P1",
			Mode::P2 => "Quill WinUI spike S0b P2",
		});
		if std::env::args().any(|a| a == "--nobackdrop") {
			context.window_visuals(WindowVisuals::new().client_size(1000.0, 640.0));
		} else {
			context.window_visuals(WindowVisuals::new().backdrop(WindowBackdrop::Mica).client_size(1000.0, 640.0));
		}
		let menu = context.callback(Msg::Menu);
		let styles = ComboBox::new()
			.items_source(SIZES)
			.selected_index(0usize)
			.width(Some(160.0))
			.vertical_alignment(VerticalAlignment::Center)
			.margin(Thickness::uniform(6.0))
			.grid_column(1)
			.automation_name("Style")
			.on_selection_changed(context.callback(Msg::Combo));
		let flyout_button = Button::new()
			.element_ref(&self.target)
			.grid_column(2)
			.vertical_alignment(VerticalAlignment::Center)
			.margin(Thickness::uniform(6.0))
			.automation_name("Flyout test")
			.content(TextBlock::new().text("Flyout"))
			.flyout_with(
				Flyout::rich(TextBlock::new().text("A flyout that opens over the page")).placement(FlyoutPlacement::Bottom),
			);
		let commands = CommandBar::new().grid_column(0).slots([SlotView::collection(
			CommandBarSlot::PrimaryCommands,
			[
				KeyedView::new(
					"bold",
					AppBarButton::new()
						.label("Bold")
						.on_click(context.message(Msg::Button("Bold".to_string()))),
				),
				KeyedView::new(
					"italic",
					AppBarButton::new()
						.label("Italic")
						.on_click(context.message(Msg::Button("Italic".to_string()))),
				),
			],
		)]);
		let item = |key: &'static str, title: &'static str, entries: Vec<MenuItem>| {
			KeyedView::new(key, MenuBarItem::new().title(title).menu(Menu::new(entries, menu.clone())))
		};
		let menus = MenuBar::new().grid_row(0).slots([SlotView::collection(
			MenuBarSlot::Items,
			[
				item(
					"file",
					"File",
					vec![
						MenuItem::item("new", "New"),
						MenuItem::item("open", "Open"),
						MenuItem::separator("sep"),
						MenuItem::item("save", "Save"),
					],
				),
				item(
					"view",
					"View",
					vec![MenuItem::item("dialog", "Open dialog"), MenuItem::item("about", "About")],
				),
			],
		)]);
		let panel = SwapChainPanel::new().element_ref(&self.panel);
		let page_cell: View = match self.mode {
			Mode::P2 => Grid::new().grid_row(2).children((panel,)),
			Mode::P1 => {
				let proxy = TextBox::new()
					.element_ref(&self.proxy)
					.text(self.proxy_text.clone())
					.accepts_return(false)
					.width(Some(1200.0))
					.height(Some(32.0))
					.opacity(0.0)
					.canvas_left(self.caret_dip.0)
					.canvas_top(self.caret_dip.1)
					.automation_name("Quill page")
					.automation_id("page")
					.on_text_changed(context.callback(Msg::Proxy));
				Grid::new().grid_row(2).children((
					panel,
					Canvas::new().keyed_children([KeyedView::new(format!("proxy{}", self.proxy_gen), proxy)]),
				))
			}
		};
		let dialog = ContentDialog::new()
			.title("Dialog over the page")
			.close_button_text("Close")
			.is_open(self.dialog_open)
			.on_closed(context.callback(|_| Msg::DialogClosed))
			.content(TextBlock::new().text("Is this dialog fully visible?"));
		Grid::new()
			.rows([
				GridLength::Auto,
				GridLength::Auto,
				GridLength::STAR,
				GridLength::Pixel(STATUS_ROW),
			])
			.children((
				menus,
				Grid::new()
					.grid_row(1)
					.columns([GridLength::STAR, GridLength::Auto, GridLength::Auto])
					.children((commands, styles, flyout_button)),
				page_cell,
				TextBlock::new()
					.text(format!("{}   |   {:?}   |   scale {}", self.status, self.mode, self.scale))
					.margin(Thickness::uniform(6.0))
					.grid_row(3),
				dialog,
			))
	}
}

impl Spike {
	fn tab_out(&mut self, back: bool) {
		log(&format!("tab out of the page, back={back}"));
		if let Some(top) = self.top {
			log(&format!("children: {}", child::describe_children(top)));
			child::focus_island(top);
		}
		let accepted = self
			.target
			.request_focus_result(|r| log(&format!("flyout button focus result: {r:?}")));
		log(&format!("flyout button focus requested: {accepted}"));
	}

	/// P2: finds the top-level window, then creates or moves the child window over the page area.
	fn place_child(&mut self) {
		if self.top.is_none() {
			self.top = child::find_top_level();
			log(&format!("top-level window: {:?}", self.top));
		}
		let Some(top) = self.top else { return };
		let mut client = RECT::default();
		let _ = unsafe { GetClientRect(top, &mut client) };
		let status_px = (STATUS_ROW * f64::from(self.scale)).round() as i32;
		let (w, h) = (self.device.0 as i32, self.device.1 as i32);
		let rect = (0, client.bottom - status_px - h, w, h);
		log(&format!("client {}x{}, page rect {rect:?}", client.right, client.bottom));
		match self.child {
			Some(hwnd) => child::place(hwnd, rect),
			None => match child::create(top, rect, std::env::args().any(|a| a == "--popup")) {
				Ok(hwnd) => {
					log(&format!(
						"child window created: {:?}; children: {}",
						hwnd.0,
						child::describe_children(top)
					));
					self.child = Some(hwnd);
					child::focus(hwnd);
				}
				Err(e) => log(&format!("child window failed: {e}")),
			},
		}
	}

	/// P1: the proxy's text is the document. A character with no 1252 form reverts the box to the last accepted text.
	fn on_proxy(&mut self, text: String, context: &ComponentContext<Self>) {
		log(&format!("proxy text changed: {} chars", text.chars().count()));
		let refused = text.chars().find(|c| doc::to_1252(*c).is_none() && *c != '\r' && *c != '\n');
		let (accepted, previous) = match refused {
			Some(c) => {
				log(&format!(
					"refused via proxy: U+{:04X} has no Windows-1252 form; the box is reset",
					c as u32
				));
				self.status = format!("Refused: U+{:04X} has no Windows-1252 form (proxy)", c as u32);
				self.proxy_gen += 1;
				let sender = context.sender();
				let _ = sender.send(Msg::Refocus);
				(self.proxy_text.clone(), true)
			}
			None => {
				let old = self.proxy_text.clone();
				let fresh: String = text.chars().skip(old.chars().count().min(text.chars().count())).collect();
				if !fresh.is_empty() {
					let bytes: Vec<String> = fresh.chars().filter_map(doc::to_1252).map(|b| format!("0x{b:02X}")).collect();
					let units: Vec<String> = fresh.encode_utf16().map(|u| format!("U+{u:04X}")).collect();
					log(&format!("insert via proxy: {} -> {}", units.join(" "), bytes.join(" ")));
					self.status = format!("Inserted {} (proxy)", bytes.join(" "));
				}
				(text, false)
			}
		};
		self.proxy_text = accepted;
		let _ = previous;
		self.redraw();
	}

	/// Draws the page for the current text and updates where the proxy sits.
	fn redraw(&mut self) {
		let Some(gpu) = self.gpu.as_ref() else { return };
		let Some(chain) = self.chain.as_ref() else { return };
		let (w, h) = self.device;
		let doc = doc::Doc::from_text(&self.proxy_text);
		let scene = page::Scene {
			lines: &doc.lines,
			caret: doc.caret,
			composition: None,
			focused: true,
			scale: f64::from(self.scale),
			composition_cursor: 0,
		};
		let frame = page::render(w, h, &scene);
		if let Err(e) = gpu.present(chain, &page::bgra(&frame.pixmap), w, h) {
			log(&format!("present failed: {e}"));
		}
		// The proxy's own text is Segoe UI, so its caret sits at the width of that text; subtract an estimate so the box's
		// caret lands on the page's caret.
		let scale = f64::from(self.scale);
		let est = self.proxy_text.chars().count() as f64 * 7.4;
		self.caret_dip = (
			f64::from(frame.caret.0) / scale - est - 8.0,
			f64::from(frame.caret.1) / scale - 4.0,
		);
		log(&format!("page caret at {:.0},{:.0} px", frame.caret.0, frame.caret.1));
	}

	/// P1: builds a swap chain for the panel's device size, presents the page and hands the chain to the panel.
	fn rebuild(&mut self, scale_x: f32, scale_y: f32, context: &ComponentContext<Self>) {
		let Some(gpu) = self.gpu.as_ref() else { return };
		let (w, h) = self.device;
		let chain = match gpu.swap_chain(w, h, (scale_x, scale_y)) {
			Ok(chain) => chain,
			Err(e) => return log(&format!("swap chain failed: {e}")),
		};
		let raw = chain.clone().into_raw();
		let unknown = unsafe { reactor_core::IUnknown::from_raw(raw) };
		let sender = context.sender();
		let accepted = self.panel.request_set_swap_chain(unknown, move |result| {
			let _ = sender.send(Msg::Attached(result.map_err(|e| format!("{e:?}"))));
		});
		log(&format!("swap chain {w} x {h} set on panel: {accepted}"));
		self.chain = Some(chain);
		self.redraw();
		let _ = context.sender().send(Msg::Refocus);
	}
}

fn main() {
	let _ = START.set(Instant::now());
	std::panic::set_hook(Box::new(|info| {
		log(&format!("panic: {info}\n{}", std::backtrace::Backtrace::force_capture()))
	}));
	log(&format!(
		"main at epoch {} ms, args {:?}",
		epoch_ms(),
		std::env::args().collect::<Vec<_>>()
	));
	match App::run_component::<Spike>(()) {
		Ok(()) => log("exited"),
		Err(e) => log(&format!("App::run_component failed: {e:?}")),
	}
}
