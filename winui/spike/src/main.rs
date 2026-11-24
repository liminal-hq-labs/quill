// Spike S0a: a Windows Reactor window with a menu bar, a command bar with a combo box and a swap-chain panel that shows a
// pixel-exact test page.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![windows_subsystem = "windows"]

mod pattern;
mod swapchain;

use std::io::Write;
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use reactor_core::Interface as _;
use windows::core::Interface;
use windows_reactor::*;

static START: OnceLock<Instant> = OnceLock::new();

fn epoch_ms() -> u128 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|d| d.as_millis())
		.unwrap_or(0)
}

/// Appends a line to `quill-winui-spike.log` next to the exe, with milliseconds since `main` began.
pub fn log(line: &str) {
	let since = START.get().map(|s| s.elapsed().as_millis()).unwrap_or(0);
	let Ok(exe) = std::env::current_exe() else { return };
	let path = exe.with_file_name("quill-winui-spike.log");
	if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
		let _ = writeln!(f, "[{since:>6} ms] {line}");
	}
}

fn out_path(name: &str) -> Option<std::path::PathBuf> {
	std::env::current_exe().ok().map(|e| e.with_file_name(name))
}

#[derive(Clone)]
enum Msg {
	Surface(SwapChainPanelEvent),
	Menu(String),
	Combo(Option<usize>),
	Button(String),
	Attached(Result<(), String>),
}

const SIZES: [&str; 5] = ["Normal", "Heading 1", "Heading 2", "Quote", "Code"];

struct Spike {
	panel: ElementRef<SwapChainPanel>,
	_observation: ElementObservation,
	gpu: Option<swapchain::Gpu>,
	chain: Option<windows::Win32::Graphics::Dxgi::IDXGISwapChain1>,
	device: (u32, u32),
	scale: f32,
	status: String,
	presented: bool,
}

impl Component for Spike {
	type Input = ();
	type Message = Msg;

	fn create(_input: &(), context: &ComponentContext<Self>) -> Self {
		let panel = ElementRef::<SwapChainPanel>::new();
		let sender = context.sender();
		let observation = panel.observe_surface(move |event| {
			let _ = sender.send(Msg::Surface(event));
		});
		let gpu = match swapchain::Gpu::new() {
			Ok(gpu) => {
				log(&format!("d3d11 device: {}", gpu.driver));
				Some(gpu)
			}
			Err(e) => {
				log(&format!("no d3d11 device: {e}"));
				None
			}
		};
		log("component created");
		Spike {
			panel,
			_observation: observation,
			gpu,
			chain: None,
			device: (0, 0),
			scale: 1.0,
			status: String::from("Ready"),
			presented: false,
		}
	}

	fn update(&mut self, message: Msg, context: &ComponentContext<Self>) {
		match message {
			Msg::Menu(label) => {
				log(&format!("menu: {label}"));
				self.status = format!("Menu: {label}");
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
			Msg::Attached(result) => log(&format!("swap chain attached: {result:?}")),
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
				self.rebuild(scale_x, scale_y, context);
			}
		}
	}

	fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
		context.window_title("Quill WinUI spike S0a");
		context.window_visuals(WindowVisuals::new().backdrop(WindowBackdrop::Mica).client_size(1000.0, 640.0));
		let menu = context.callback(Msg::Menu);
		let styles = ComboBox::new()
			.items_source(SIZES)
			.selected_index(0usize)
			.width(Some(160.0))
			.vertical_alignment(VerticalAlignment::Center)
			.margin(Thickness::uniform(6.0))
			.grid_column(1)
			.on_selection_changed(context.callback(Msg::Combo));
		let button = |key: &'static str, label: &'static str| {
			KeyedView::new(
				key,
				AppBarButton::new()
					.label(label)
					.on_click(context.message(Msg::Button(label.to_string()))),
			)
		};
		let commands = CommandBar::new().grid_column(0).slots([SlotView::collection(
			CommandBarSlot::PrimaryCommands,
			[button("bold", "Bold"), button("italic", "Italic")],
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
					"edit",
					"Edit",
					vec![
						MenuItem::item("undo", "Undo"),
						MenuItem::item("cut", "Cut"),
						MenuItem::item("copy", "Copy"),
						MenuItem::item("paste", "Paste"),
					],
				),
				item("help", "Help", vec![MenuItem::item("about", "About")]),
			],
		)]);
		Grid::new()
			.rows([GridLength::Auto, GridLength::Auto, GridLength::STAR, GridLength::Auto])
			.children((
				menus,
				// Reactor 0.100.0 cannot put a ComboBox in a CommandBar: the native insert fails with E_NOINTERFACE, because a
				// ComboBox is not an ICommandBarElement. The box sits beside the bar in the same row instead.
				Grid::new()
					.grid_row(1)
					.columns([GridLength::STAR, GridLength::Auto])
					.children((commands, styles)),
				SwapChainPanel::new().element_ref(&self.panel).grid_row(2),
				TextBlock::new()
					.text(format!(
						"{}   |   scale {}   |   {} x {} px",
						self.status, self.scale, self.device.0, self.device.1
					))
					.margin(Thickness::uniform(6.0))
					.grid_row(3),
			))
	}
}

impl Spike {
	/// Builds a swap chain for the panel's device size, presents the page and hands the chain to the panel.
	fn rebuild(&mut self, scale_x: f32, scale_y: f32, context: &ComponentContext<Self>) {
		let Some(gpu) = self.gpu.as_ref() else { return };
		let (w, h) = self.device;
		let chain = match gpu.swap_chain(w, h, (scale_x, scale_y)) {
			Ok(chain) => chain,
			Err(e) => return log(&format!("swap chain failed: {e}")),
		};
		let pm = pattern::render(w, h, f64::from(scale_x));
		if let Err(e) = gpu.present(&chain, &pattern::bgra(&pm), w, h) {
			return log(&format!("present failed: {e}"));
		}
		let tag = format!("{}", (scale_x * 100.0).round() as u32);
		if let Some(path) = out_path(&format!("frame-{tag}-{w}x{h}.png")) {
			let _ = pm.save_png(path);
		}
		// The swap chain belongs to windows 0.62's COM types; Reactor takes windows-core 0.100's `IUnknown`.
		let raw = chain.clone().into_raw();
		let unknown = unsafe { reactor_core::IUnknown::from_raw(raw) };
		let sender = context.sender();
		let accepted = self.panel.request_set_swap_chain(unknown, move |result| {
			let _ = sender.send(Msg::Attached(result.map_err(|e| format!("{e:?}"))));
		});
		log(&format!("swap chain {w} x {h} presented, set on panel: {accepted}"));
		self.chain = Some(chain);
		if !self.presented {
			self.presented = true;
			log(&format!("first frame presented at epoch {} ms", epoch_ms()));
		}
	}
}

fn main() {
	let _ = START.set(Instant::now());
	std::panic::set_hook(Box::new(|info| {
		log(&format!("panic: {info}\n{}", std::backtrace::Backtrace::force_capture()))
	}));
	log(&format!("main at epoch {} ms", epoch_ms()));
	match App::run_component::<Spike>(()) {
		Ok(()) => log("exited"),
		Err(e) => log(&format!("App::run_component failed: {e:?}")),
	}
}
