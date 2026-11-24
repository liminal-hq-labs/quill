// The `.qhm` help source: a small text format that `hlpc` compiles.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// ```text
// title: Quill Help
// copyright: (c) 2026 Liminal HQ
//
// = start | Quill Help | 1      a topic: its context name, its title and, if programs ask for it by number, that number
// # A heading
// A paragraph, with **bold**, a [jump](other) and a [pop-up](!term).
// - A bulleted item
// ```
//
// Blank lines end a paragraph; lines of one paragraph are joined with spaces. `#` starts a heading, `- ` a bulleted item.

use crate::write::{Help, Para, Run, Style, Topic};
use crate::HelpError;

/// Compiles source text into a [`Help`].
pub fn parse(src: &str) -> Result<Help, HelpError> {
	let mut h = Help::default();
	let mut para: Vec<(usize, String)> = Vec::new();
	let mut style = Style::Body;
	fn finish(
		h: &mut Help,
		para: &mut Vec<(usize, String)>,
		style: &mut Style,
	) -> Result<(), HelpError> {
		if para.is_empty() {
			return Ok(());
		}
		let line = para[0].0;
		let text = para
			.iter()
			.map(|(_, t)| t.as_str())
			.collect::<Vec<_>>()
			.join(" ");
		para.clear();
		let runs = runs(&text).map_err(|m| HelpError::Source(line, m))?;
		let Some(t) = h.topics.last_mut() else {
			return Err(HelpError::Source(
				line,
				"text before the first topic (= context | title)".into(),
			));
		};
		t.paras.push(Para {
			style: *style,
			runs,
		});
		*style = Style::Body;
		Ok(())
	}
	for (i, raw) in src.lines().enumerate() {
		let n = i + 1;
		let line = raw.trim_end();
		if let Some(v) = line.strip_prefix("title:") {
			h.title = v.trim().to_string();
		} else if let Some(v) = line.strip_prefix("copyright:") {
			h.copyright = v.trim().to_string();
		} else if let Some(v) = line.strip_prefix("= ") {
			finish(&mut h, &mut para, &mut style)?;
			let mut parts = v.split('|');
			let (Some(ctx), Some(title)) = (parts.next(), parts.next()) else {
				return Err(HelpError::Source(
					n,
					"a topic line is `= context | title` or `= context | title | number`".into(),
				));
			};
			let map = match parts.next() {
				Some(m) => Some(m.trim().parse::<u32>().map_err(|_| {
					HelpError::Source(n, "the topic number is not a number".into())
				})?),
				None => None,
			};
			h.topics.push(Topic {
				context: ctx.trim().to_string(),
				title: title.trim().to_string(),
				map,
				paras: Vec::new(),
			});
		} else if line.is_empty() {
			finish(&mut h, &mut para, &mut style)?;
		} else if let Some(v) = line.strip_prefix("# ") {
			finish(&mut h, &mut para, &mut style)?;
			style = Style::Heading;
			para.push((n, v.to_string()));
			finish(&mut h, &mut para, &mut style)?;
		} else if let Some(v) = line.strip_prefix("- ") {
			finish(&mut h, &mut para, &mut style)?;
			style = Style::Item;
			para.push((n, v.to_string()));
			finish(&mut h, &mut para, &mut style)?;
		} else {
			para.push((n, line.trim_start().to_string()));
		}
	}
	finish(&mut h, &mut para, &mut style)?;
	Ok(h)
}

fn runs(text: &str) -> Result<Vec<Run>, String> {
	let mut out = Vec::new();
	let mut plain = String::new();
	let b: Vec<char> = text.chars().collect();
	let mut i = 0;
	let flush = |plain: &mut String, out: &mut Vec<Run>| {
		if !plain.is_empty() {
			out.push(Run::Text(std::mem::take(plain)));
		}
	};
	while i < b.len() {
		if b[i] == '*' && b.get(i + 1) == Some(&'*') {
			let end = (i + 2..b.len().saturating_sub(1))
				.find(|&j| b[j] == '*' && b[j + 1] == '*')
				.ok_or("`**` is not closed")?;
			flush(&mut plain, &mut out);
			out.push(Run::Bold(b[i + 2..end].iter().collect()));
			i = end + 2;
		} else if b[i] == '[' {
			let close = (i + 1..b.len())
				.find(|&j| b[j] == ']')
				.ok_or("`[` is not closed")?;
			if b.get(close + 1) != Some(&'(') {
				return Err("a link is `[words](context)`".into());
			}
			let end = (close + 2..b.len())
				.find(|&j| b[j] == ')')
				.ok_or("`(` is not closed")?;
			let words: String = b[i + 1..close].iter().collect();
			let target: String = b[close + 2..end].iter().collect();
			flush(&mut plain, &mut out);
			if let Some(t) = target.strip_prefix('!') {
				out.push(Run::Popup(words, t.to_string()));
			} else {
				out.push(Run::Jump(words, target));
			}
			i = end + 1;
		} else {
			plain.push(b[i]);
			i += 1;
		}
	}
	flush(&mut plain, &mut out);
	Ok(out)
}
