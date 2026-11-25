// Key tokens: the names the test scenarios use for keys, as DOSBox's AUTOTYPE spells them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::Key;

/// Turns one token into a key: a letter or digit, or a name such as `enter`, `f10`, `down`, `space`.
pub fn parse_token(t: &str) -> Option<Key> {
	Some(match t {
		"enter" => Key::Enter,
		"bspace" => Key::Backspace,
		"delete" => Key::Delete,
		"esc" => Key::Esc,
		"tab" => Key::Tab,
		"left" => Key::Left,
		"right" => Key::Right,
		"up" => Key::Up,
		"down" => Key::Down,
		"home" => Key::Home,
		"end" => Key::End,
		"pageup" => Key::PageUp,
		"pagedown" => Key::PageDown,
		"space" => Key::Char(b' '),
		"period" => Key::Char(b'.'),
		"minus" => Key::Char(b'-'),
		"insert" => Key::Insert,
		_ => {
			let b = t.as_bytes();
			if b.len() == 1 && b.first().is_some_and(u8::is_ascii_alphanumeric) {
				return b.first().map(|c| Key::Char(*c));
			}
			let n = t.strip_prefix('f')?.parse::<u8>().ok()?;
			Key::F(n)
		}
	})
}

/// One step of a scenario that has a pointer: a key, or a pointer action at a place. The place is a cell on the text screen and a
/// pixel in graphics. `@click x y`, `@shiftclick x y`, `@dclick x y`, `@tclick x y`, `@drag x1 y1 x2 y2` and `@wheel n` are the
/// directives that spell them; `@move x y`, `@down x y` and `@up x y` are the parts of a click on their own (to hold a button down
/// for a frame, or to hover), and `@wait n` lets `n` ticks pass with the pointer where it is (a tip shows after about nine).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Step {
	/// A typed key.
	Key(Key),
	/// A click, with Shift held when the flag is set.
	Click { x: i32, y: i32, shift: bool },
	/// Two clicks in a row.
	DClick { x: i32, y: i32 },
	/// Three clicks in a row.
	TClick { x: i32, y: i32 },
	/// Press at the first place, move to the second, release.
	Drag { x1: i32, y1: i32, x2: i32, y2: i32 },
	/// Turn the wheel; positive is towards the user.
	Wheel(i32),
	/// Move the pointer with no button down.
	Move { x: i32, y: i32 },
	/// Move to the place and press the button, and keep it down.
	Down { x: i32, y: i32 },
	/// Move to the place and let the button go.
	Up { x: i32, y: i32 },
	/// Let this many ticks pass.
	Wait(i32),
}

/// Reads the pointer directive `name` and its numbers from `rest`. `None` when `name` is not one; a missing or bad number is `Err`.
pub fn parse_pointer<'a>(
	name: &str,
	rest: &mut impl Iterator<Item = &'a str>,
) -> Option<Result<Step, &'static str>> {
	let mut num = || {
		rest.next()
			.and_then(|t| t.parse::<i32>().ok())
			.ok_or("a number is missing")
	};
	let two = |n: &mut dyn FnMut() -> Result<i32, &'static str>| Ok((n()?, n()?));
	Some((|| {
		Ok(match name {
			"@click" | "@shiftclick" => {
				let (x, y) = two(&mut num)?;
				Step::Click {
					x,
					y,
					shift: name == "@shiftclick",
				}
			}
			"@dclick" => {
				let (x, y) = two(&mut num)?;
				Step::DClick { x, y }
			}
			"@tclick" => {
				let (x, y) = two(&mut num)?;
				Step::TClick { x, y }
			}
			"@drag" => {
				let (x1, y1) = two(&mut num)?;
				let (x2, y2) = two(&mut num)?;
				Step::Drag { x1, y1, x2, y2 }
			}
			"@wheel" => Step::Wheel(num()?),
			"@wait" => Step::Wait(num()?),
			"@move" | "@down" | "@up" => {
				let (x, y) = two(&mut num)?;
				match name {
					"@move" => Step::Move { x, y },
					"@down" => Step::Down { x, y },
					_ => Step::Up { x, y },
				}
			}
			_ => return Err(""),
		})
	})())
	.filter(|r| !matches!(r, Err("")))
}

/// The steps of a script text, in order: pointer directives and key tokens, `#` starting a comment. Directives that are not the
/// pointer's (`@copy FILE`, `@open FILE`, `@rows N`, `@mono`, `@live`) and tokens that are not keys are skipped, which is how the
/// DOS shell reads the `QUILL_MOUSE` file the test harness writes.
pub fn steps(text: &str) -> impl Iterator<Item = Step> + '_ {
	let mut tokens = text
		.lines()
		.map(|l| l.split('#').next().unwrap_or(""))
		.flat_map(str::split_whitespace);
	core::iter::from_fn(move || loop {
		let t = tokens.next()?;
		if t.starts_with('@') {
			match parse_pointer(t, &mut tokens) {
				Some(Ok(s)) => return Some(s),
				Some(Err(_)) => return None,
				None if matches!(t, "@rows" | "@open" | "@copy") => {
					tokens.next();
				}
				None => {}
			}
		} else if let Some(k) = parse_token(t) {
			return Some(Step::Key(k));
		}
	})
}

impl Step {
	/// Plays the step through `f` as the pointer events and the context each is made in. `tick` is the clock the clicks are stamped
	/// with; it moves on past the step, so two steps are never one double click.
	pub fn play(&self, tick: &mut u32, mut f: impl FnMut(quill_mouse::Ev, crate::mouse::Ctx)) {
		use quill_mouse::Ev;
		let at = *tick;
		let click =
			|x: i32, y: i32, shift: bool, n: u32, f: &mut dyn FnMut(Ev, crate::mouse::Ctx)| {
				let ctx = crate::mouse::Ctx {
					shift,
					tick: at + n,
				};
				f(Ev::Move { x, y }, ctx);
				f(Ev::Down { x, y }, ctx);
				f(Ev::Up { x, y }, ctx);
			};
		match *self {
			Step::Key(_) => {}
			Step::Click { x, y, shift } => click(x, y, shift, 0, &mut f),
			Step::DClick { x, y } => {
				click(x, y, false, 0, &mut f);
				click(x, y, false, 2, &mut f);
			}
			Step::TClick { x, y } => {
				click(x, y, false, 0, &mut f);
				click(x, y, false, 2, &mut f);
				click(x, y, false, 4, &mut f);
			}
			Step::Drag { x1, y1, x2, y2 } => {
				let ctx = crate::mouse::Ctx {
					shift: false,
					tick: at,
				};
				f(Ev::Move { x: x1, y: y1 }, ctx);
				f(Ev::Down { x: x1, y: y1 }, ctx);
				f(
					Ev::Move {
						x: (x1 + x2) / 2,
						y: (y1 + y2) / 2,
					},
					ctx,
				);
				f(Ev::Move { x: x2, y: y2 }, ctx);
				f(Ev::Up { x: x2, y: y2 }, ctx);
			}
			Step::Wheel(n) => f(
				Ev::Wheel(n),
				crate::mouse::Ctx {
					shift: false,
					tick: at,
				},
			),
			Step::Move { x, y } | Step::Down { x, y } | Step::Up { x, y } => {
				let ctx = crate::mouse::Ctx {
					shift: false,
					tick: at,
				};
				f(Ev::Move { x, y }, ctx);
				match *self {
					Step::Down { .. } => f(Ev::Down { x, y }, ctx),
					Step::Up { .. } => f(Ev::Up { x, y }, ctx),
					_ => {}
				}
				// These are parts of one gesture: the next is a tick later, not a double click's distance away.
				*tick = at.wrapping_add(1);
				return;
			}
			Step::Wait(n) => {
				let now = at.wrapping_add(n.max(0) as u32);
				f(
					Ev::Tick,
					crate::mouse::Ctx {
						shift: false,
						tick: now,
					},
				);
				*tick = now;
				return;
			}
		}
		*tick = at.wrapping_add(100);
	}
}
