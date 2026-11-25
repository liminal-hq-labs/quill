// The pointer on the graphics screen: menus, dialogs, the toolbars, the ruler and the scroll bars, and the page under them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`Gfx`] takes the events a mouse makes (`quill_app::mouse::Ev`, in pixels of the 640 by 480 screen) and turns each into what the
//! keyboard would have done, using the rectangles the painter draws with: [`menu::title_rects`] and [`menu::geom_of`], the
//! dialog's [`dialog::hit`], [`toolbar::button_at`] and [`toolbar::box_at`], [`drop::item_at`], `quill_app::view::ruler_hit_with` and
//! [`scroll::parts`], [`scroll::hit`] and [`scroll::pos_at`]. None of them is a second copy of a layout, so a click lands on what is
//! on the screen. The page keeps the app's own [`Pointer`]: a press in the page area goes to it, and so does everything until the
//! button is up.
//!
//! The rules are Windows': a menu opens on the press and stays open until a click, a press and release over an item runs it, a
//! press on a title and a drag through to an item and a release there runs it too, and hovering another title switches menus. A
//! button runs when the button is let go over it and not when it is pressed (it shows sunk while it is held, and raised again when
//! the pointer leaves it). A drop-down list works like a menu. A dialog's buttons, check boxes and radio buttons act on release; a
//! field or a list entry takes the press at once, and a second press on a list entry within a double click's time accepts it.
//!
//! What the painter needs to show a gesture in progress is [`View`], read with [`Gfx::view`]; the rest is the app's.

use quill_app::mouse::{Ctx, Ev, Pointer, DOUBLE_TICKS};
use quill_app::view::{snap, text_extent, Drag, Hit, RulerState, RULER_H, TOOLBAR, TWIPS_PX};
use quill_app::{menu_items_for, menu_title, App, Files, Key, Overlay, Slot, Wk, MENU_COUNT};
use quill_core::layout::Measure;
use quill_core::store::{ReadStore, Store};

use crate::drop::{self, Pop};
use crate::layout::Layout;
use crate::menu::{geom_of, title_rects, MenuGeom, Row};
use crate::scroll::{self, Part, Parts};
use crate::shell::{self, Pixels};
use crate::tip::{Target, TIP_TICKS};
use crate::toolbar::{box_at, button_at};
use crate::ui::ui_font;
use crate::{dialog, Rect};
use quill_core::layout::GLYPHS;

/// What the painter shows of a gesture in progress.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct View {
	/// The toolbar button held down, as an index into `TOOLBAR.buttons`, while the pointer is over it.
	pub button: Option<usize>,
	/// The dialog button held down, as an index into the dialog's widgets, while the pointer is over it.
	pub widget: Option<usize>,
	/// The open drop-down list and its lit entry.
	pub drop: Option<(Pop, usize)>,
	/// The tip showing.
	pub tip: Option<Target>,
	/// The ruler as a drag has it, ahead of the app.
	pub ruler: Option<RulerState>,
}

#[derive(Copy, Clone, Debug)]
#[allow(clippy::large_enum_variant)] // one value lives in the pointer; boxing would need an allocator
enum Cap {
	/// No button is down.
	None,
	/// A press that was used up (it closed a menu or a list, or hit nothing): nothing follows from it.
	Spent,
	/// A press in the page; the app's pointer has the rest.
	Page,
	/// A menu is being tracked; `opened` says this press opened it (or switched it) from a title.
	Menu { opened: bool },
	/// A toolbar button is held.
	Button { i: usize, over: bool },
	/// A dialog's button, check box or radio button is held.
	Widget { idx: usize, over: bool },
	/// A drop-down list is being tracked.
	Pop { opened: bool },
	/// The thumb of the vertical scroll bar is being dragged.
	Thumb { grab: i32, parts: Parts, range: i32 },
	/// An indent marker or a tab stop on the ruler is being dragged.
	Ruler { drag: Drag, at: usize },
}

/// The pointer's state on the graphics screen.
pub struct Gfx {
	ptr: Pointer,
	cap: Cap,
	at: (i32, i32),
	now: u32,
	/// The widths of characters 0x20 to 0xFF in the interface font, for the menu's and the bar's geometry.
	adv: [u16; GLYPHS],
	drop: Option<(Pop, usize)>,
	/// What the pointer rests on, since when, and whether the tip for it has been used up by a click.
	rest: Option<(Target, u32, bool)>,
	tip: Option<Target>,
	/// The last press on a dialog's list entry, for a double click.
	last: Option<(u32, usize)>,
}

fn inside(r: Rect, x: i32, y: i32) -> bool {
	x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

fn adv_of(w: &[u16; GLYPHS], c: u8) -> i32 {
	c.checked_sub(0x20)
		.and_then(|i| w.get(usize::from(i)))
		.map_or(0, |v| i32::from(*v))
}

/// True when the item at `row` of `menu` is greyed: choosing it does nothing.
fn greyed<M: Measure + Clone, S: ReadStore + ?Sized>(
	app: &App<M>,
	s: &S,
	menu: u8,
	row: &Row,
) -> bool {
	match row.slot {
		Slot::Item(k) => menu_items_for(menu, true)
			.get(k)
			.is_some_and(|it| it.sub.is_empty() && app.menu_mark(s, it.command).disabled),
		Slot::Recent(_) => false,
	}
}

impl Gfx {
	/// A pointer for a screen whose interface font `m` measures.
	pub fn new<Me: Measure>(m: &mut Me) -> Gfx {
		let mut adv = [0u16; GLYPHS];
		m.widths(&ui_font(), &mut adv);
		Gfx {
			ptr: Pointer::default(),
			cap: Cap::None,
			at: (0, 0),
			now: 0,
			adv,
			drop: None,
			rest: None,
			tip: None,
			last: None,
		}
	}

	/// Where the pointer was last.
	pub fn at(&self) -> (i32, i32) {
		self.at
	}

	/// True while a button is down and something has hold of it: a drag of the page, a held button, a thumb, a menu being tracked.
	pub fn holding(&self) -> bool {
		!matches!(self.cap, Cap::None | Cap::Spent)
	}

	/// What the painter shows of the gesture.
	pub fn view(&self) -> View {
		View {
			button: match self.cap {
				Cap::Button { i, over: true } => Some(i),
				_ => None,
			},
			widget: match self.cap {
				Cap::Widget { idx, over: true } => Some(idx),
				_ => None,
			},
			drop: self.drop,
			tip: self.tip,
			ruler: match self.cap {
				Cap::Ruler { drag, .. } => Some(drag.state),
				_ => None,
			},
		}
	}

	fn geom<M: Measure + Clone>(&self, app: &App<M>, menu: u8) -> Option<MenuGeom> {
		geom_of(app, menu, |c| adv_of(&self.adv, c))
	}

	fn title_at(&self, x: i32, y: i32) -> Option<u8> {
		title_rects(|c| adv_of(&self.adv, c))
			.iter()
			.position(|r| inside(*r, x, y))
			.map(|m| m as u8)
			.filter(|m| *m < MENU_COUNT && !menu_title(*m).is_empty())
	}

	fn target_at<M: Measure + Clone>(app: &App<M>, x: i32, y: i32) -> Option<Target> {
		let lay = Layout::of(app);
		button_at(&lay, x, y)
			.map(Target::Button)
			.or_else(|| box_at(&lay, x, y).map(Target::Box))
	}

	/// Takes one event. True when the screen has to be drawn again: the event was a press, a release or the wheel, or it moved
	/// something that shows (a lit menu item, a held button, a drag, a tip).
	pub fn handle<M: Measure + Clone, S: Store + ?Sized, F: Files>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		f: &mut F,
		ev: Ev,
		ctx: Ctx,
	) -> bool {
		self.now = ctx.tick;
		let before = (self.view(), app.menu_open());
		let mut sure = !matches!(ev, Ev::Move { .. } | Ev::Tick);
		match ev {
			Ev::Move { x, y } => {
				self.at = (x, y);
				sure |= self.moved(app, s, f, x, y, ctx);
			}
			Ev::Down { x, y } => {
				self.at = (x, y);
				self.down(app, s, f, x, y, ctx);
			}
			Ev::Up { x, y } => {
				self.at = (x, y);
				self.up(app, s, f, x, y, ctx);
			}
			Ev::Wheel(n) => self.wheel(app, s, f, n, ctx),
			Ev::Tick => self.rest_tip(),
		}
		if !matches!(ev, Ev::Move { .. } | Ev::Tick) {
			shell::sync_view(app);
		}
		sure || before != (self.view(), app.menu_open())
	}

	/// A key is about to go to the app. An open list takes the keys that move in it (Up, Down, Enter and Esc) and closes on any
	/// other; a tip goes. True when the list used the key.
	pub fn key<M: Measure + Clone, S: Store + ?Sized>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		k: Key,
	) -> bool {
		self.tip = None;
		self.rest = self.rest.map(|(t, at, _)| (t, at, true));
		let Some((pop, hot)) = self.drop else {
			return false;
		};
		match k {
			Key::Up => self.drop = Some((pop, hot.saturating_sub(1))),
			Key::Down => self.drop = Some((pop, (hot + 1).min(pop.count().saturating_sub(1)))),
			Key::Enter => {
				self.drop = None;
				drop::choose(app, s, pop, hot);
			}
			Key::Esc => self.drop = None,
			_ => {
				self.drop = None;
				return false;
			}
		}
		true
	}

	// ---- the tip -------------------------------------------------------------------------------------------------------------

	fn rest_on(&mut self, t: Option<Target>) {
		match (t, self.rest) {
			(None, _) => {
				self.rest = None;
				self.tip = None;
			}
			(Some(t), Some((r, _, _))) if r == t => {}
			(Some(t), _) => {
				self.rest = Some((t, self.now, false));
				self.tip = None;
			}
		}
	}

	fn rest_tip(&mut self) {
		if let Some((t, since, spent)) = self.rest {
			if !spent && self.tip.is_none() && self.now.wrapping_sub(since) >= TIP_TICKS {
				self.tip = Some(t);
			}
		}
	}

	// ---- moves ---------------------------------------------------------------------------------------------------------------

	fn moved<M: Measure + Clone, S: Store + ?Sized, F: Files>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		f: &mut F,
		x: i32,
		y: i32,
		ctx: Ctx,
	) -> bool {
		match self.cap {
			Cap::Page => {
				let space = Pixels::of(app);
				self.ptr.handle(app, s, f, &space, Ev::Move { x, y }, ctx);
				return true;
			}
			Cap::Spent => {}
			Cap::Menu { .. } => self.menu_over(app, x, y),
			Cap::Button { i, .. } => {
				let over = button_at(&Layout::of(app), x, y) == Some(i);
				self.cap = Cap::Button { i, over };
			}
			Cap::Widget { idx, .. } => {
				let mut o = Overlay::default();
				let over = app.overlay(s, &mut o) && dialog::hit(&o, x, y) == Some(idx);
				self.cap = Cap::Widget { idx, over };
			}
			Cap::Pop { .. } => self.drop_over(app, x, y),
			Cap::Thumb { grab, parts, range } => {
				let pos = scroll::pos_at(&parts, y - grab, range);
				app.scroll_to(s, pos.max(0) as u32);
				return true;
			}
			Cap::Ruler { mut drag, at } => {
				let lay = Layout::of(app);
				let (left, w) = ruler_extent(app, &lay);
				drag_to(&mut drag, at, x, left, w);
				self.cap = Cap::Ruler { drag, at };
			}
			Cap::None => {
				if app.menu_open().is_some() {
					self.menu_over(app, x, y);
				} else if self.drop.is_some() {
					self.drop_over(app, x, y);
				} else if app.editing() {
					let t = Self::target_at(app, x, y);
					self.rest_on(t);
				}
			}
		}
		false
	}

	/// The pointer is over the open menu's bar or items, with or without the button: another title opens its menu, an item lights.
	fn menu_over<M: Measure + Clone>(&mut self, app: &mut App<M>, x: i32, y: i32) {
		let Some((menu, _)) = app.menu_open() else {
			return;
		};
		if let Some(t) = self.title_at(x, y) {
			if t != menu {
				app.menu_pull(t);
			}
			return;
		}
		if let Some(g) = self.geom(app, menu) {
			if let Some(i) = g.item_at(x, y) {
				app.menu_light(i as u8);
			}
		}
	}

	fn drop_over<M: Measure + Clone>(&mut self, app: &App<M>, x: i32, y: i32) {
		let Some((pop, _)) = self.drop else { return };
		if let Some(r) = drop::list_rect(&Layout::of(app), pop) {
			if let Some(i) = drop::item_at(r, pop.count(), x, y) {
				self.drop = Some((pop, i));
			}
		}
	}

	// ---- presses -------------------------------------------------------------------------------------------------------------

	fn down<M: Measure + Clone, S: Store + ?Sized, F: Files>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		f: &mut F,
		x: i32,
		y: i32,
		ctx: Ctx,
	) {
		self.tip = None;
		self.rest = Self::target_at(app, x, y).map(|t| (t, ctx.tick, true));
		self.cap = Cap::Spent;
		if let Some((menu, _)) = app.menu_open() {
			self.menu_down(app, menu, x, y);
		} else if !app.editing() {
			self.dialog_down(app, s, f, x, y, ctx);
		} else if let Some((pop, _)) = self.drop {
			self.pop_down(app, pop, x, y);
		} else {
			self.chrome_down(app, s, f, x, y, ctx);
		}
	}

	fn menu_down<M: Measure + Clone>(&mut self, app: &mut App<M>, menu: u8, x: i32, y: i32) {
		if let Some(t) = self.title_at(x, y) {
			if t == menu {
				app.menu_shut();
			} else {
				app.menu_pull(t);
				self.cap = Cap::Menu { opened: true };
			}
			return;
		}
		if let Some(g) = self.geom(app, menu) {
			if let Some(i) = g.item_at(x, y) {
				app.menu_light(i as u8);
				self.cap = Cap::Menu { opened: false };
				return;
			}
			if g.contains(x, y) {
				// The frame or a rule: nothing happens, and the menu stays.
				return;
			}
		}
		app.menu_shut();
	}

	fn pop_down<M: Measure + Clone>(&mut self, app: &App<M>, pop: Pop, x: i32, y: i32) {
		let lay = Layout::of(app);
		if let Some(r) = drop::list_rect(&lay, pop) {
			if let Some(i) = drop::item_at(r, pop.count(), x, y) {
				self.drop = Some((pop, i));
				self.cap = Cap::Pop { opened: false };
				return;
			}
			if inside(r, x, y) {
				return;
			}
		}
		// A press anywhere else, the box that opened the list included, shuts it.
		self.drop = None;
	}

	fn dialog_down<M: Measure + Clone, S: Store + ?Sized, F: Files>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		f: &mut F,
		x: i32,
		y: i32,
		ctx: Ctx,
	) {
		let mut o = Overlay::default();
		if !app.overlay(s, &mut o) {
			return;
		}
		// The title bar, the border and the space between controls do nothing; the dialog is not dragged.
		let Some(idx) = dialog::hit(&o, x, y) else {
			return;
		};
		match o.widgets().get(idx).map(|w| w.kind) {
			Some(Wk::Button | Wk::Check | Wk::Radio) => self.cap = Cap::Widget { idx, over: true },
			Some(Wk::ListItem) => {
				let again = self
					.last
					.is_some_and(|(t, i)| i == idx && ctx.tick.wrapping_sub(t) <= DOUBLE_TICKS);
				self.last = Some((ctx.tick, idx));
				app.press_widget(s, f, idx, again);
				if again {
					self.last = None;
				}
			}
			Some(_) => {
				app.press_widget(s, f, idx, false);
			}
			None => {}
		}
	}

	fn chrome_down<M: Measure + Clone, S: Store + ?Sized, F: Files>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		f: &mut F,
		x: i32,
		y: i32,
		ctx: Ctx,
	) {
		let lay = Layout::of(app);
		if inside(lay.menu, x, y) {
			if let Some(t) = self.title_at(x, y) {
				if app.menu_pull(t) {
					self.cap = Cap::Menu { opened: true };
				}
			}
			return;
		}
		if let Some(i) = button_at(&lay, x, y) {
			self.cap = Cap::Button { i, over: true };
			return;
		}
		if let Some(b) = box_at(&lay, x, y) {
			let pop = Pop::of_box(b);
			self.drop = Some((pop, drop::current(app, s, pop)));
			self.cap = Cap::Pop { opened: true };
			return;
		}
		if lay.ruler.is_some_and(|r| inside(r, x, y)) {
			self.ruler_down(app, s, &lay, x, y);
			return;
		}
		if inside(lay.vbar, x, y) {
			self.bar_down(app, s, &lay, x, y);
			return;
		}
		// The horizontal bar has nothing to scroll: the page is always shown with its text column at the left edge.
		if inside(lay.page, x, y) {
			self.cap = Cap::Page;
			let space = Pixels::of(app);
			self.ptr.handle(app, s, f, &space, Ev::Down { x, y }, ctx);
		}
	}

	fn bar_down<M: Measure + Clone, S: Store + ?Sized>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		lay: &Layout,
		x: i32,
		y: i32,
	) {
		let total = app.line_total(s) as i32;
		let view = (app.page_rows() as i32).min(total);
		let range = (total - view).max(0);
		let top = app.top_line() as i32;
		let p = scroll::parts(lay.vbar, true, top, total, view);
		let page = (view - 1).max(1);
		let to = match scroll::hit(&p, x, y) {
			Some(Part::LineUp) => top - 1,
			Some(Part::LineDown) => top + 1,
			Some(Part::PageUp) => top - page,
			Some(Part::PageDown) => top + page,
			Some(Part::Thumb) => {
				self.cap = Cap::Thumb {
					grab: y - p.thumb.top,
					parts: p,
					range,
				};
				return;
			}
			None => return,
		};
		app.scroll_to(s, to.clamp(0, range) as u32);
	}

	fn ruler_down<M: Measure + Clone, S: Store + ?Sized>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		lay: &Layout,
		x: i32,
		y: i32,
	) {
		let Some(r) = lay.ruler else { return };
		let (left, w) = ruler_extent(app, lay);
		let st = RulerState::of(app, s);
		// No button for the kind of tab is drawn here, so the whole ruler is the scale and the markers: kind 0, a left tab.
		match quill_app::view::ruler_hit_with(x, y, r.top, left, w, &st, 0) {
			Some(Hit::Scale(t)) => app.add_tab(s, snap(t), 0),
			Some(Hit::Kind) | None => {}
			Some(hit) => {
				let at = match hit {
					Hit::Tab(p) => st.stops().iter().position(|t| t.0 == p).unwrap_or(0),
					_ => 0,
				};
				self.cap = Cap::Ruler {
					drag: Drag {
						hit,
						state: st,
						start: st,
					},
					at,
				};
			}
		}
	}

	// ---- releases ------------------------------------------------------------------------------------------------------------

	fn up<M: Measure + Clone, S: Store + ?Sized, F: Files>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		f: &mut F,
		x: i32,
		y: i32,
		ctx: Ctx,
	) {
		let cap = core::mem::replace(&mut self.cap, Cap::None);
		match cap {
			Cap::None | Cap::Spent | Cap::Thumb { .. } => {}
			Cap::Page => {
				let space = Pixels::of(app);
				self.ptr.handle(app, s, f, &space, Ev::Up { x, y }, ctx);
			}
			Cap::Menu { opened } => self.menu_up(app, s, f, opened, x, y),
			Cap::Button { i, .. } => {
				let lay = Layout::of(app);
				if button_at(&lay, x, y) == Some(i) {
					self.run_button(app, s, f, i);
				}
			}
			Cap::Widget { idx, .. } => {
				let mut o = Overlay::default();
				if app.overlay(s, &mut o) && dialog::hit(&o, x, y) == Some(idx) {
					app.press_widget(s, f, idx, false);
				}
			}
			Cap::Pop { opened } => self.pop_up(app, s, opened, x, y),
			Cap::Ruler { mut drag, at } => {
				let lay = Layout::of(app);
				let (left, w) = ruler_extent(app, &lay);
				drag_to(&mut drag, at, x, left, w);
				self.ruler_up(app, s, &lay, drag, at, y);
			}
		}
		// What a gesture left behind is not a tip's business until the pointer moves on.
		if let Some((t, at, _)) = self.rest {
			self.rest = Some((t, at, true));
		}
	}

	fn menu_up<M: Measure + Clone, S: Store + ?Sized, F: Files>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		f: &mut F,
		opened: bool,
		x: i32,
		y: i32,
	) {
		let Some((menu, _)) = app.menu_open() else {
			return;
		};
		if let Some(g) = self.geom(app, menu) {
			if let Some(i) = g.item_at(x, y) {
				let off = g.rows().get(i).is_some_and(|r| greyed(app, s, menu, r));
				if !off {
					app.menu_light(i as u8);
					app.key(s, f, Key::Enter);
				}
				return;
			}
			if g.contains(x, y) {
				return;
			}
		}
		// Let go over a title keeps the menu open (a click on the bar); anywhere else after a drag from the bar closes it.
		if opened && self.title_at(x, y).is_none() {
			app.menu_shut();
		}
	}

	fn run_button<M: Measure + Clone, S: Store + ?Sized, F: Files>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		f: &mut F,
		i: usize,
	) {
		use quill_app::view::Act;
		match TOOLBAR.buttons.get(i).map(|b| b.act) {
			Some(Act::Cmd(c)) => app.command(s, f, c),
			Some(Act::Colour) => {
				self.drop = Some((Pop::Colour, drop::current(app, s, Pop::Colour)));
			}
			None => {}
		}
	}

	fn pop_up<M: Measure + Clone, S: Store + ?Sized>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		opened: bool,
		x: i32,
		y: i32,
	) {
		let Some((pop, _)) = self.drop else { return };
		let lay = Layout::of(app);
		if let Some(r) = drop::list_rect(&lay, pop) {
			if let Some(i) = drop::item_at(r, pop.count(), x, y) {
				self.drop = None;
				drop::choose(app, s, pop, i);
				return;
			}
		}
		// Let go over the box that was pressed leaves the list open, as a click on it does; anywhere else after a drag shuts it.
		let on_anchor = drop::anchor(&lay, pop).is_some_and(|a| inside(a, x, y));
		if opened && !on_anchor {
			self.drop = None;
		}
	}

	fn ruler_up<M: Measure + Clone, S: Store + ?Sized>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		lay: &Layout,
		d: Drag,
		at: usize,
		y: i32,
	) {
		match d.hit {
			Hit::First | Hit::Left | Hit::Both | Hit::Right => {
				app.set_indents(s, d.state.left, d.state.first - d.state.left, d.state.right);
			}
			Hit::Tab(_) => {
				let Some(r) = lay.ruler else { return };
				let (Some(was), Some(now)) = (d.start.stops().get(at), d.state.stops().get(at))
				else {
					return;
				};
				if y < r.top - RULER_H || y >= r.bottom + RULER_H {
					// Dragged off the ruler: the tab stop is cleared.
					app.remove_tab(s, was.0);
				} else if now.0 != was.0 {
					app.remove_tab(s, was.0);
					app.add_tab(s, now.0, now.1);
				}
			}
			_ => {}
		}
	}

	// ---- the wheel -----------------------------------------------------------------------------------------------------------

	fn wheel<M: Measure + Clone, S: Store + ?Sized, F: Files>(
		&mut self,
		app: &mut App<M>,
		s: &mut S,
		f: &mut F,
		n: i32,
		ctx: Ctx,
	) {
		self.tip = None;
		if app.menu_open().is_some() {
			return;
		}
		if let Some((pop, hot)) = self.drop {
			let last = pop.count().saturating_sub(1) as i32;
			self.drop = Some((pop, (hot as i32 + n).clamp(0, last) as usize));
			return;
		}
		if app.editing() {
			let space = Pixels::of(app);
			self.ptr.handle(app, s, f, &space, Ev::Wheel(n), ctx);
			return;
		}
		// A list box in a dialog: the wheel moves the selection a place a notch.
		let mut o = Overlay::default();
		if app.overlay(s, &mut o) && o.widgets().iter().any(|w| w.kind == Wk::ListItem) {
			for _ in 0..n.unsigned_abs().min(8) {
				app.key(s, f, if n > 0 { Key::Down } else { Key::Up });
			}
		}
	}
}

/// The text column's left edge and width in the ruler, as the painter draws them.
fn ruler_extent<M: Measure + Clone>(app: &App<M>, lay: &Layout) -> (i32, i32) {
	let w = lay.page.width();
	text_extent(app, w, shell::hscroll(app, w))
}

/// Applies the pointer position `x` to a drag: the indent markers as the shared logic moves them, and a tab stop (entry `at` of the
/// ruler's stops) to where the pointer is, snapped to a sixteenth of an inch.
fn drag_to(d: &mut Drag, at: usize, x: i32, left: i32, w: i32) {
	if let Hit::Tab(_) = d.hit {
		let to = snap((x - left) * TWIPS_PX).clamp(0, w * TWIPS_PX);
		if let Some(slot) = d.state.tabs.get_mut(at) {
			slot.0 = to;
		}
	} else {
		quill_app::view::drag_to(d, x, left, w);
	}
}
