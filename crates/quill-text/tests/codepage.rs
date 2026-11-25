// The code page tables against Python's codecs (a golden committed as data), the Win16 resource against the tables, and the page lookups.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use quill_text::codepage::{resource_offset, to_unicode, CodePage, RESOURCE_LEN};
use quill_text::cp1252;

const GOLDEN: &str = include_str!("data/codepages.tsv");
const RESOURCE: &[u8] = include_bytes!("../../../assets/codepages/CPAGES.BIN");

fn page(n: u32) -> CodePage {
	CodePage::from_number(i64::from(n)).unwrap()
}

#[test]
fn every_entry_of_every_page_equals_pythons_codec() {
	let mut seen = 0;
	for line in GOLDEN.lines().filter(|l| !l.starts_with('#')) {
		let mut f = line.split('\t');
		let cp = page(f.next().unwrap().parse().unwrap());
		let byte = u8::from_str_radix(f.next().unwrap(), 16).unwrap();
		let unit = u32::from_str_radix(f.next().unwrap(), 16).unwrap();
		assert_eq!(
			u32::from(to_unicode(cp, byte)),
			unit,
			"{} byte {byte:#04x}",
			cp.number()
		);
		seen += 1;
	}
	assert_eq!(seen, 6 * 128);
}

#[test]
fn ascii_is_ascii_in_every_page() {
	for cp in CodePage::ALL {
		for b in 0..0x80u8 {
			assert_eq!(to_unicode(cp, b), char::from(b));
		}
	}
}

#[test]
fn page_1252_equals_the_cp1252_module_for_every_byte() {
	for b in 0..=255u8 {
		assert_eq!(
			to_unicode(CodePage::Cp1252, b),
			cp1252::to_char(b),
			"{b:#04x}"
		);
	}
}

#[test]
fn the_win16_resource_holds_the_tables_in_the_order_resource_offset_says() {
	assert_eq!(RESOURCE.len(), RESOURCE_LEN);
	for cp in CodePage::ALL {
		for b in 0x80..=0xFFu8 {
			let at = resource_offset(cp, b);
			let unit = u16::from_le_bytes([RESOURCE[at], RESOURCE[at + 1]]);
			assert_eq!(u32::from(unit), u32::from(to_unicode(cp, b)));
		}
	}
}

#[test]
fn each_page_has_its_own_letters() {
	assert_eq!(to_unicode(CodePage::Cp1250, 0xB9), 'ą');
	assert_eq!(to_unicode(CodePage::Cp1251, 0xE0), 'а');
	assert_eq!(to_unicode(CodePage::Cp1253, 0xE1), 'α');
	assert_eq!(to_unicode(CodePage::Cp1254, 0xFE), 'ş');
	assert_eq!(to_unicode(CodePage::Cp1257, 0xE8), 'č');
	assert_eq!(to_unicode(CodePage::Cp1252, 0x9A), 'š');
}

#[test]
fn undefined_bytes_follow_the_policy() {
	// Below 0xA0 the C1 control of the same number; above it the replacement character.
	assert_eq!(to_unicode(CodePage::Cp1250, 0x81), '\u{81}');
	assert_eq!(to_unicode(CodePage::Cp1251, 0x98), '\u{98}');
	assert_eq!(to_unicode(CodePage::Cp1253, 0xAA), '\u{FFFD}');
	assert_eq!(to_unicode(CodePage::Cp1257, 0xA1), '\u{FFFD}');
	for b in [0x81, 0x8D, 0x8F, 0x90, 0x9D] {
		assert_eq!(to_unicode(CodePage::Cp1252, b), char::from(b));
	}
}

#[test]
fn numbers_and_charsets_name_the_pages() {
	for cp in CodePage::ALL {
		assert_eq!(CodePage::from_number(i64::from(cp.number())), Some(cp));
	}
	assert_eq!(CodePage::from_number(437), None);
	assert_eq!(CodePage::from_number(-1), None);
	let cs = [
		(0, 1252),
		(238, 1250),
		(204, 1251),
		(161, 1253),
		(162, 1254),
		(186, 1257),
	];
	for (c, n) in cs {
		assert_eq!(CodePage::from_charset(c), Some(page(n)));
	}
	assert_eq!(CodePage::from_charset(128), None);
	assert_eq!(CodePage::from_charset(1), None);
}
