// Fits the advances of a word to the width the layout gave it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A zoomed font has its own whole-pixel advances, and a word drawn with them ends away from where the layout ends it. [`fit_advances`]
//! spreads the difference over the gaps inside the word, so the word starts and ends where the layout says whatever the system does
//! to the font. Nothing here depends on how a platform scales a font.

/// Spreads `target - sum(adv)` over the gaps between the glyphs of a word, so the word's advances sum to `target`: the first
/// `|r| % gaps` gaps (`r` the difference) take one more pixel than the rest, in order, and the last glyph's advance takes nothing (it has no gap after
/// it). One glyph: its advance becomes `target`. No advance goes below 0; when the target is so small that the gaps alone cannot
/// shrink the word enough, the last advances give up the rest.
pub fn fit_advances(adv: &mut [i32], target: i32) {
	let n = adv.len();
	let target = target.max(0);
	let sum: i64 = adv.iter().map(|a| i64::from(*a)).sum();
	let r = i64::from(target) - sum;
	if n == 1 {
		if let Some(a) = adv.first_mut() {
			*a = target;
		}
		return;
	}
	if n == 0 || r == 0 {
		return;
	}
	let gaps = (n - 1) as i64;
	let each = r / gaps;
	let extra = (r % gaps).abs();
	let step = r.signum();
	let mut total = 0i64;
	for (i, a) in adv.iter_mut().enumerate().take(n - 1) {
		let more = if (i as i64) < extra { step } else { 0 };
		*a = (i64::from(*a) + each + more).clamp(0, i64::from(i32::MAX)) as i32;
		total += i64::from(*a);
	}
	if let Some(last) = adv.last() {
		total += i64::from(*last);
	}
	// A word shrunk past what its gaps hold: the last advances take the remainder, back to front, none below 0.
	for a in adv.iter_mut().rev() {
		let delta = i64::from(target) - total;
		if delta == 0 {
			break;
		}
		let now = (i64::from(*a) + delta).clamp(0, i64::from(i32::MAX));
		total += now - i64::from(*a);
		*a = now as i32;
	}
}
