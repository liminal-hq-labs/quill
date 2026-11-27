// A picture module's shape in a .DMD file: the info and the table survive the round trip, relocate with the image at any address
// and any link base, and the build-id check refuses a module from another build or another major.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_module::{load_into, Builder, FormatError, LoadError, Module, KIND_MODULE, LINK_BASE};
use quill_modabi::*;

// The image's layout, in bytes: code stand-ins first (read-only), then the writable data with the table, the info and an import slot.
const CODE_END: u32 = 0x40;
const START: u32 = 0x10;
const FN_INFO: u32 = 0x20;
const FN_OPEN: u32 = 0x24;
const FN_ROWS: u32 = 0x28;
const FN_CLOSE: u32 = 0x2C;
const TABLE: u32 = 0x40;
const INFO: u32 = 0x60;
const SLOT: u32 = 0x80;
const FILE_SIZE: u32 = 0x84;
const MEM_SIZE: u32 = 0x400;

const BUILD: u32 = 0x1A2B_3C4D;

fn put16(image: &mut [u8], at: u32, value: u16) {
	image[at as usize..at as usize + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(image: &mut [u8], at: u32, value: u32) {
	image[at as usize..at as usize + 4].copy_from_slice(&value.to_le_bytes());
}

fn get32(image: &[u8], at: u32) -> u32 {
	u32::from_le_bytes(image[at as usize..at as usize + 4].try_into().unwrap())
}

fn get16(image: &[u8], at: u32) -> u16 {
	u16::from_le_bytes(image[at as usize..at as usize + 2].try_into().unwrap())
}

/// The picture module as a linker would leave it for `link_base`: the table's four entries point at the stand-in functions, which
/// is what makes them relocated words; the info carries `info`'s fields; one import slot is zero.
fn picture_module(link_base: u32, info: &ModInfo) -> Builder {
	let mut image = vec![0x90u8; FILE_SIZE as usize];
	put32(&mut image, TABLE, 20);
	for (slot, function) in [FN_INFO, FN_OPEN, FN_ROWS, FN_CLOSE].into_iter().enumerate() {
		put32(&mut image, TABLE + 4 + 4 * slot as u32, link_base + function);
	}
	put16(&mut image, INFO, info.size);
	put16(&mut image, INFO + 2, info.abi_major);
	put16(&mut image, INFO + 4, info.abi_minor);
	put16(&mut image, INFO + 6, info.kind);
	put32(&mut image, INFO + 8, info.build_id);
	put32(&mut image, INFO + 12, info.arena);
	put32(&mut image, INFO + 16, info.limit);
	put32(&mut image, SLOT, 0);
	Builder {
		sdk: 1,
		kind: KIND_MODULE,
		align: 16,
		link_base,
		image,
		mem_size: MEM_SIZE,
		ro_size: CODE_END,
		start: START,
		event: 0,
		stop: 0,
		relocs: (0..4).map(|slot| TABLE + 4 + 4 * slot).collect(),
		imports: vec![("desk_log".to_string(), SLOT)],
	}
}

fn own_info(build: u32) -> ModInfo {
	ModInfo {
		size: core::mem::size_of::<ModInfo>() as u16,
		abi_major: ABI_MAJOR,
		abi_minor: ABI_MINOR,
		kind: KIND_PIC,
		build_id: build,
		arena: 0,
		limit: 8192,
	}
}

const LOG: u32 = 0x0001_0000;

fn resolve(name: &str) -> Option<u32> {
	(name == "desk_log").then_some(LOG)
}

/// Packs `module`, parses the bytes and loads them at `addr`.
fn pack_and_load(module: &Builder, addr: u32) -> (Vec<u8>, desk_module::Placed) {
	let bytes = module.write().unwrap();
	let parsed = Module::parse(&bytes, 1).unwrap();
	let mut dest = vec![0xAAu8; MEM_SIZE as usize];
	let placed = load_into(&parsed, KIND_MODULE, &mut dest, addr, &resolve).unwrap();
	(dest, placed)
}

/// Reads the info the module's data holds, as the host would after `info` filled it.
fn read_info(image: &[u8]) -> ModInfo {
	ModInfo {
		size: get16(image, INFO),
		abi_major: get16(image, INFO + 2),
		abi_minor: get16(image, INFO + 4),
		kind: get16(image, INFO + 6),
		build_id: get32(image, INFO + 8),
		arena: get32(image, INFO + 12),
		limit: get32(image, INFO + 16),
	}
}

#[test]
fn a_picture_module_round_trips_through_the_format() {
	let module = picture_module(LINK_BASE, &own_info(BUILD));
	let bytes = module.write().unwrap();
	let parsed = Module::parse(&bytes, 1).unwrap();
	assert_eq!(parsed.header().kind, KIND_MODULE);
	assert_eq!(parsed.image(), &module.image[..]);
	assert_eq!(parsed.relocations().count(), 4, "the four table entries");
	let imports: Vec<_> = parsed.imports().collect();
	assert_eq!(imports.len(), 1);
}

#[test]
fn the_table_and_info_relocate_at_three_other_addresses() {
	for addr in [0x0080_0000u32, 0x0100_0000, 0x0234_5600] {
		let (dest, placed) = pack_and_load(&picture_module(LINK_BASE, &own_info(BUILD)), addr);
		assert_eq!(placed.base, addr);
		assert_eq!(placed.start, addr + START);
		assert_eq!(get32(&dest, TABLE), 20, "the size word is not an address");
		for (slot, function) in [FN_INFO, FN_OPEN, FN_ROWS, FN_CLOSE].into_iter().enumerate() {
			assert_eq!(
				get32(&dest, TABLE + 4 + 4 * slot as u32),
				addr + function,
				"entry {slot} at {addr:#x}"
			);
		}
		assert_eq!(read_info(&dest), own_info(BUILD), "the info is data and does not move");
		assert_eq!(get32(&dest, SLOT), LOG);
		assert!(dest[FILE_SIZE as usize..].iter().all(|&b| b == 0), ".bss is zeroed");
	}
}

/// The same module linked at the standard base and at three others loads to the same bytes at one address: the table's words are
/// the only ones that depend on the base, and relocation removes the difference. (`dmdpack cross` proves the same of a real link, in
/// `pack/tests/pack.rs`.)
#[test]
fn the_same_module_linked_at_four_bases_loads_to_the_same_bytes() {
	let addr = 0x0600_0000;
	let (reference, _) = pack_and_load(&picture_module(LINK_BASE, &own_info(BUILD)), addr);
	for base in [0x0080_0000u32, 0x0100_0000, 0x0234_5000] {
		let (dest, _) = pack_and_load(&picture_module(base, &own_info(BUILD)), addr);
		assert_eq!(dest, reference, "linked at {base:#x}");
	}
}

#[test]
fn a_link_base_off_a_page_boundary_is_refused() {
	for base in [0x0080_0004u32, 0x0234_5600] {
		let bytes = picture_module(base, &own_info(BUILD)).write().unwrap_or_default();
		assert!(bytes.is_empty() || Module::parse(&bytes, 1).is_err(), "{base:#x}");
	}
}

#[test]
fn an_entry_that_points_outside_the_image_is_refused() {
	let mut module = picture_module(LINK_BASE, &own_info(BUILD));
	put32(&mut module.image, TABLE + 4, LINK_BASE + MEM_SIZE + 4);
	let bytes = module.write().unwrap();
	let parsed = Module::parse(&bytes, 1).unwrap();
	let mut dest = vec![0u8; MEM_SIZE as usize];
	let refusal = load_into(&parsed, KIND_MODULE, &mut dest, 0x0080_0000, &resolve).unwrap_err();
	assert!(matches!(refusal, LoadError::RelocTargetOutOfRange { .. }), "{refusal:?}");
	assert!(
		dest.iter().all(|&b| b == 0),
		"a refused module leaves the destination untouched"
	);
}

#[test]
fn a_damaged_file_is_refused_before_anything_loads() {
	let mut bytes = picture_module(LINK_BASE, &own_info(BUILD)).write().unwrap();
	bytes[100] ^= 0x01;
	assert!(matches!(Module::parse(&bytes, 1), Err(FormatError::BadChecksum { .. })));
	let good = picture_module(LINK_BASE, &own_info(BUILD)).write().unwrap();
	assert!(Module::parse(&good[..good.len() - 1], 1).is_err());
	assert!(
		Module::parse(&good, 0).is_err(),
		"a module needing SDK 1 is refused by a core that offers none"
	);
}

#[test]
fn the_loaded_info_is_checked_against_the_programs_build() {
	let addr = 0x0080_0000;
	let (own, _) = pack_and_load(&picture_module(LINK_BASE, &own_info(BUILD)), addr);
	assert!(check_info(&read_info(&own), KIND_PIC, BUILD, Policy::Release).is_ok());

	// A module from another build: refused in a release build, accepted and flagged in a development build.
	let (other, _) = pack_and_load(&picture_module(LINK_BASE, &own_info(BUILD + 1)), addr);
	assert_eq!(
		check_info(&read_info(&other), KIND_PIC, BUILD, Policy::Release),
		Err(QS_VERSION)
	);
	assert!(
		check_info(&read_info(&other), KIND_PIC, BUILD, Policy::Development)
			.unwrap()
			.other_build
	);

	// A module from a dirty tree is another build too.
	let (dirty, _) = pack_and_load(&picture_module(LINK_BASE, &own_info(build_id(BUILD, true))), addr);
	assert_eq!(
		check_info(&read_info(&dirty), KIND_PIC, BUILD, Policy::Release),
		Err(QS_VERSION)
	);

	// Another major is refused in every policy; a higher minor is accepted.
	let mut wrong = own_info(BUILD);
	wrong.abi_major = ABI_MAJOR + 1;
	let (major, _) = pack_and_load(&picture_module(LINK_BASE, &wrong), addr);
	assert_eq!(
		check_info(&read_info(&major), KIND_PIC, BUILD, Policy::Development),
		Err(QS_VERSION)
	);
	let mut newer = own_info(BUILD);
	newer.abi_minor = ABI_MINOR + 1;
	let (minor, _) = pack_and_load(&picture_module(LINK_BASE, &newer), addr);
	assert!(
		check_info(&read_info(&minor), KIND_PIC, BUILD, Policy::Release)
			.unwrap()
			.newer_minor
	);

	// A module of another kind is not the one the program asked for.
	let mut dialog = own_info(BUILD);
	dialog.kind = KIND_DLG;
	let (dlg, _) = pack_and_load(&picture_module(LINK_BASE, &dialog), addr);
	assert_eq!(
		check_info(&read_info(&dlg), KIND_PIC, BUILD, Policy::Release),
		Err(QS_UNSUPPORTED)
	);
}

/// Flip every bit of the packed file in turn: each damaged file is refused by the format (its CRC) or, if the flip hit the CRC
/// itself, by the CRC. None panics, and none of the refusals reaches the build-id check.
#[test]
fn every_single_bit_flip_is_refused_without_a_panic() {
	let bytes = picture_module(LINK_BASE, &own_info(BUILD)).write().unwrap();
	for index in 0..bytes.len() {
		for bit in 0..8 {
			let mut damaged = bytes.clone();
			damaged[index] ^= 1 << bit;
			assert!(Module::parse(&damaged, 1).is_err(), "byte {index} bit {bit} was accepted");
		}
	}
}
