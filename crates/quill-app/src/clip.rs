// Copy, Cut and Paste through `quill-clip`: the request a shell serves, and the clipboard the app keeps when none does (CORE-101).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub use quill_clip::{ClipError, Clipboard, Flavour, Flavours};

use quill_clip::{copy, empty_doc, insert_fragment, paste_fragment, Held, PasteStyle};
use quill_core::doc::Doc;
use quill_core::layout::Measure;
use quill_core::store::Store;

use crate::{label, App, Command};

/// What the app keeps when no shell serves the clipboard: RTF and text, up to 1 MB each, in chunks of the document's store (no DIB:
/// a picture travels inside the RTF).
pub(crate) type HeldClipboard = Held<128, 0>;

/// The size of one chunk of the held clipboard: 128 of them hold 1 MB.
pub(crate) const CHUNK: usize = 8192;

/// The order a paste reads the clipboard in: Rich Text Format, a lone picture, then text.
const PREFER: [Flavour; 3] = [Flavour::Rtf, Flavour::Dib, Flavour::Text1252];

/// What Copy, Cut or Paste asks of a shell that serves the clipboard.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ClipRequest {
	/// Put the selection on the system clipboard ([`App::copy_to`]); `cut` also deletes it.
	Copy {
		/// True for Cut.
		cut: bool,
	},
	/// Take what is on the system clipboard ([`App::paste_from`]).
	Paste,
}

/// Whether a shell serves the clipboard, what it asked for last and what its clipboard offers.
#[derive(Copy, Clone)]
pub(crate) struct Serve {
	served: bool,
	req: Option<ClipRequest>,
	offers: Flavours,
}

impl Serve {
	pub(crate) const fn new() -> Self {
		Serve {
			served: false,
			req: None,
			offers: Flavours::NONE,
		}
	}
}

impl<M: Measure + Clone> App<M> {
	/// Says that the shell serves the clipboard: Copy, Cut and Paste then leave a [`ClipRequest`] for [`App::take_clip_request`]
	/// instead of using the app's own clipboard. A shell that never calls this keeps working, with formatting, inside Quill.
	pub fn serve_clipboard(&mut self, on: bool) {
		self.clip_serve.served = on;
		self.clip_serve.req = None;
	}

	/// Tells the app what the system clipboard offers now, so Paste is greyed when it offers nothing Quill reads. Call it when the
	/// clipboard changes or a menu opens. Only a shell that serves the clipboard needs it.
	pub fn set_clip_offers(&mut self, offers: Flavours) {
		self.clip_serve.offers = offers;
	}

	/// The Copy, Cut or Paste the last key or command asked for, once.
	pub fn take_clip_request(&mut self) -> Option<ClipRequest> {
		self.clip_serve.req.take()
	}

	/// True when Paste has something to take.
	pub(crate) fn clip_can_paste(&self) -> bool {
		if self.clip_serve.served {
			!self.clip_serve.offers.is_empty()
		} else {
			!self.clip.is_empty()
		}
	}

	#[inline(never)]
	pub(crate) fn clip_command<S: Store + ?Sized>(&mut self, s: &mut S, c: Command) {
		let serve = self.clip_serve.served;
		match c {
			Command::Copy | Command::Cut => {
				let cut = c == Command::Cut;
				if serve {
					self.clip_serve.req = Some(ClipRequest::Copy { cut });
				} else if let Some((lo, hi)) = self.selection() {
					let mut r = self
						.clip
						.copy_from(&self.doc, s, lo..hi, Flavours::TEXT_AND_RTF);
					// Text alone would lose the formatting a Cut deletes, so a selection whose RTF does not fit is refused.
					if r.is_ok() && !self.clip.ready().has(Flavour::Rtf) {
						self.clip.clear(s);
						r = Err(ClipError::TooLarge);
					}
					self.copy_done(s, r, cut);
				}
			}
			Command::Paste => {
				if serve {
					self.clip_serve.req = Some(ClipRequest::Paste);
				} else {
					let mut frag = empty_doc();
					let got = self.clip.paste_to(&PREFER, s, &mut frag);
					self.paste_done(s, &frag, got);
					frag.reset(s);
				}
			}
			_ => {}
		}
	}

	/// Copies the selection to `cb` in every flavour (Rich Text Format, text and a lone picture); with `cut` it then deletes the
	/// selection as one undo step. A selection `cb` cannot hold is refused with the "too large" message and nothing changes, so a
	/// Cut never deletes text it did not keep (CORE-44). True when the clipboard has the selection.
	#[inline(never)]
	pub fn copy_to<S: Store + ?Sized, C: Clipboard>(
		&mut self,
		s: &mut S,
		cb: &mut C,
		cut: bool,
	) -> bool {
		let Some((lo, hi)) = self.selection() else {
			return false;
		};
		let r = copy(&self.doc, s, lo..hi, Flavours::ALL, cb);
		self.copy_done(s, r, cut)
	}

	/// Replaces the selection with what `cb` offers, preferring Rich Text Format, then a picture, then text, as one undo step.
	/// Text takes the format at the caret; the other flavours bring their own. True when something was inserted.
	#[inline(never)]
	pub fn paste_from<S: Store + ?Sized, C: Clipboard>(&mut self, s: &mut S, cb: &mut C) -> bool {
		let mut frag = empty_doc();
		let got = paste_fragment(cb, &PREFER, s, &mut frag);
		let done = self.paste_done(s, &frag, got);
		frag.reset(s);
		done
	}

	fn clip_failed(&mut self, e: ClipError) {
		match e {
			ClipError::NoMemory => self
				.msg
				.set("There is not enough memory to complete this operation."),
			ClipError::TooLarge | ClipError::Refused => {
				self.msg.set("The document is too large for Quill.")
			}
			ClipError::Empty | ClipError::BadData => {}
		}
	}

	#[inline(never)]
	fn copy_done<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		r: Result<(), ClipError>,
		cut: bool,
	) -> bool {
		if let Err(e) = r {
			self.clip_failed(e);
			return false;
		}
		if cut {
			self.end_typing();
			self.delete_selection(s);
		}
		true
	}

	#[inline(never)]
	fn paste_done<S: Store + ?Sized>(
		&mut self,
		s: &mut S,
		frag: &Doc,
		got: Result<Flavour, ClipError>,
	) -> bool {
		let flavour = match got {
			Ok(f) => f,
			Err(e) => {
				self.clip_failed(e);
				return false;
			}
		};
		self.end_typing();
		self.doc.body.begin_group(label::TYPING);
		self.typing = true;
		if self.selection().is_some() {
			self.delete_selection(s);
		}
		self.anchor = None;
		let at = self.caret;
		let pfmt = self
			.para_mark(s, at)
			.and_then(|m| self.doc.body.pfmt_at(s, m))
			.unwrap_or(0);
		let style = if flavour == Flavour::Text1252 {
			PasteStyle::Plain {
				cfmt: self
					.typing_fmt
					.unwrap_or_else(|| self.typing_cfmt_at(s, at)),
				pfmt,
			}
		} else {
			let starts = at == 0 || self.doc.body.read_byte(s, at - 1) == Some(0x0D);
			PasteStyle::Rich {
				first_pfmt: (!starts).then_some(pfmt),
			}
		};
		let mut done = 0;
		let r = insert_fragment(&mut self.doc, s, frag, at, style, label::TYPING, &mut done);
		if done > 0 {
			self.spell_note_edit(s, at, 0, done);
			self.caret = at + done;
			self.modified = true;
			self.want_x = -1;
			self.typing_fmt = None;
		}
		self.end_typing();
		if let Err(e) = r {
			self.clip_failed(e);
		}
		done > 0
	}
}
