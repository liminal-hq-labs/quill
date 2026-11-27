// Property tests for `fit_advances`: the word's advances end at the target and no gap moves further than its share.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::prelude::*;
use quill_core::fit::fit_advances;

proptest! {
	#[test]
	fn sum_is_the_target(adv in proptest::collection::vec(0i32..200, 0..40), target in 0i32..4000) {
		let mut got = adv.clone();
		fit_advances(&mut got, target);
		if !adv.is_empty() {
			prop_assert_eq!(got.iter().sum::<i32>(), target);
		}
		prop_assert!(got.iter().all(|a| *a >= 0));
	}

	#[test]
	fn no_residual_changes_nothing(adv in proptest::collection::vec(0i32..200, 1..40)) {
		let mut got = adv.clone();
		fit_advances(&mut got, adv.iter().sum());
		prop_assert_eq!(got, adv);
	}

	#[test]
	fn no_gap_moves_more_than_its_share(adv in proptest::collection::vec(1i32..200, 2..40), shrink in 0i32..40, grow in 0i32..400) {
		let sum: i32 = adv.iter().sum();
		let gaps = adv.len() as i32 - 1;
		let last = *adv.last().unwrap();
		// Only targets the gaps alone can reach: shrinking past that makes the last advance give way too.
		let target = sum + grow - shrink.min(gaps);
		let r = target - sum;
		let mut got = adv.clone();
		fit_advances(&mut got, target);
		let share = (r.abs() + gaps - 1) / gaps;
		for (g, a) in got.iter().zip(adv.iter()).take(gaps as usize) {
			prop_assert!((g - a).abs() <= share, "moved {} of at most {}", g - a, share);
		}
		prop_assert_eq!(*got.last().unwrap(), last);
		prop_assert_eq!(got.iter().sum::<i32>(), target);
	}

	#[test]
	fn same_input_same_result(adv in proptest::collection::vec(0i32..200, 0..40), target in 0i32..4000) {
		let (mut a, mut b) = (adv.clone(), adv);
		fit_advances(&mut a, target);
		fit_advances(&mut b, target);
		prop_assert_eq!(a, b);
	}
}

#[test]
fn one_glyph_takes_the_target() {
	let mut one = [7];
	fit_advances(&mut one, 11);
	assert_eq!(one, [11]);
}

#[test]
fn no_glyphs_is_fine() {
	let mut none: [i32; 0] = [];
	fit_advances(&mut none, 5);
}

#[test]
fn the_first_gaps_take_the_extra_pixels() {
	let mut word = [5, 5, 5, 5];
	fit_advances(&mut word, 22);
	assert_eq!(word, [6, 6, 5, 5]);
	let mut word = [5, 5, 5, 5];
	fit_advances(&mut word, 18);
	assert_eq!(word, [4, 4, 5, 5]);
}
