// The module interface's layout, values and compatibility check: a change to any of them fails here, in one place.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::mem::{align_of, offset_of, size_of};

use quill_modabi::*;

/// Size, alignment and every field's offset, written out here rather than derived so a moved field fails.
#[test]
fn the_structs_have_the_layout_both_loaders_assume() {
	assert_eq!((size_of::<ModInfo>(), align_of::<ModInfo>()), (20, 4));
	assert_eq!(offset_of!(ModInfo, size), 0);
	assert_eq!(offset_of!(ModInfo, abi_major), 2);
	assert_eq!(offset_of!(ModInfo, abi_minor), 4);
	assert_eq!(offset_of!(ModInfo, kind), 6);
	assert_eq!(offset_of!(ModInfo, build_id), 8);
	assert_eq!(offset_of!(ModInfo, arena), 12);
	assert_eq!(offset_of!(ModInfo, limit), 16);

	assert_eq!((size_of::<ModStart>(), align_of::<ModStart>()), (20, 4));
	assert_eq!(offset_of!(ModStart, size), 0);
	assert_eq!(offset_of!(ModStart, abi_major), 2);
	assert_eq!(offset_of!(ModStart, abi_minor), 4);
	assert_eq!(offset_of!(ModStart, kind), 6);
	assert_eq!(offset_of!(ModStart, build_id), 8);
	assert_eq!(offset_of!(ModStart, table), 12);
	assert_eq!(offset_of!(ModStart, table_len), 16);

	assert_eq!((size_of::<Chunk>(), align_of::<Chunk>()), (8, 4));
	assert_eq!(offset_of!(Chunk, addr), 0);
	assert_eq!(offset_of!(Chunk, len), 4);

	assert_eq!((size_of::<ChunkList>(), align_of::<ChunkList>()), (8, 4));
	assert_eq!(offset_of!(ChunkList, count), 0);
	assert_eq!(offset_of!(ChunkList, pad), 2);
	assert_eq!(offset_of!(ChunkList, chunks), 4);

	assert_eq!((size_of::<PicOpen>(), align_of::<PicOpen>()), (16, 4));
	assert_eq!(offset_of!(PicOpen, kind), 0);
	assert_eq!(offset_of!(PicOpen, flags), 2);
	assert_eq!(offset_of!(PicOpen, width), 4);
	assert_eq!(offset_of!(PicOpen, height), 8);
	assert_eq!(offset_of!(PicOpen, orientation), 12);
	assert_eq!(offset_of!(PicOpen, depth), 14);

	assert_eq!((size_of::<PicRows>(), align_of::<PicRows>()), (20, 4));
	assert_eq!(offset_of!(PicRows, dst), 0);
	assert_eq!(offset_of!(PicRows, stride), 4);
	assert_eq!(offset_of!(PicRows, shown_width), 8);
	assert_eq!(offset_of!(PicRows, rows), 10);
	assert_eq!(offset_of!(PicRows, done), 12);
	assert_eq!(offset_of!(PicRows, pad), 14);
	assert_eq!(offset_of!(PicRows, palette), 16);
}

/// The table holds a `u32` size and four function pointers. A pointer is four bytes on DOS (the crate's own const assertion checks
/// that on a 32-bit build) and wider on the machines the tests run on, so the offsets are those of a pointer's width.
#[test]
fn the_dos_table_is_a_size_and_four_entries() {
	let pointer = size_of::<usize>();
	assert_eq!(size_of::<Option<PicInfoFn>>(), pointer);
	assert_eq!(offset_of!(PicTableV1, size), 0);
	assert_eq!(offset_of!(PicTableV1, info), pointer);
	assert_eq!(offset_of!(PicTableV1, open), 2 * pointer);
	assert_eq!(offset_of!(PicTableV1, rows), 3 * pointer);
	assert_eq!(offset_of!(PicTableV1, close), 4 * pointer);
	assert_eq!(size_of::<PicTableV1>(), 5 * pointer);
	assert_eq!(PIC_TABLE_V1_NAMES.len(), 4);
	let empty = PicTableV1::default();
	assert!(empty.info.is_none() && empty.close.is_none());
}

#[test]
fn the_versions_kinds_and_statuses_are_the_published_values() {
	assert_eq!((ABI_MAJOR, ABI_MINOR), (1, 0));
	assert_eq!((KIND_PIC, KIND_DLG, KIND_HLP, KIND_PDF), (1, 2, 3, 4));
	let statuses = [
		QS_OK,
		QS_MORE,
		QS_BUSY,
		QS_UNSUPPORTED,
		QS_CORRUPT,
		QS_TOO_LARGE,
		QS_NOALIAS,
		QS_INTERNAL,
		QS_MISSING,
		QS_VERSION,
		QS_NOMEMORY,
	];
	assert_eq!(statuses, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
	for (i, a) in statuses.iter().enumerate() {
		assert_ne!(status_name(*a), "?");
		for b in &statuses[i + 1..] {
			assert_ne!(a, b);
			assert_ne!(status_name(*a), status_name(*b));
		}
	}
	assert_eq!(status_name(QS_NOMEMORY), "NOMEMORY");
	assert_eq!(status_name(11), "?");
	assert_eq!(status_name(u16::MAX), "?");
	assert_eq!(size_of::<Status>(), 2);
}

#[test]
fn the_win16_ordinals_and_the_export_table_agree() {
	assert_eq!(
		(PIC_WEP, PIC_INFO, PIC_OPEN, PIC_ROWS, PIC_CLOSE),
		(1, 2, 3, 4, 5)
	);
	assert_eq!(PIC_EXPORTS.len(), 5);
	for (index, export) in PIC_EXPORTS.iter().enumerate() {
		assert_eq!(export.ordinal as usize, index + 1);
	}
	assert_eq!(PIC_EXPORTS[0].name, "WEP");
	// The DOS table's entries are the exports after `WEP`, in order.
	for (entry, export) in PIC_TABLE_V1_NAMES.iter().zip(&PIC_EXPORTS[1..]) {
		assert_eq!(*entry, export.name);
	}
}

#[test]
fn file_names_carry_no_version_digit() {
	for kind in [KIND_PIC, KIND_DLG, KIND_HLP, KIND_PDF] {
		let name = name_of(kind).unwrap();
		assert_eq!(name.len(), 8, "{name} fits an 8.3 name");
		assert!(
			name.bytes().all(|b| b.is_ascii_uppercase()),
			"{name} has no digit"
		);
	}
	assert_eq!(name_of(KIND_PIC), Some("QUILLPIC"));
	assert_eq!((DOS_EXTENSION, WIN16_EXTENSION), (".DMD", ".DLL"));
	// Windows 95 and later keep the stems and the extension of Windows 3.x, in a folder of their own where both share one (INS-27).
	assert_eq!(WIN32_EXTENSION, ".DLL");
}

#[test]
fn the_win32_exports_are_two_names_found_by_name() {
	assert_eq!(
		(WIN32_INFO_EXPORT, WIN32_START_EXPORT),
		("QuillModuleInfo", "QuillModuleStart")
	);
	for name in [WIN32_INFO_EXPORT, WIN32_START_EXPORT] {
		// Undecorated: a cdecl name carries no `@n`, and nothing in it needs quoting for `GetProcAddress`.
		assert!(name.bytes().all(|b| b.is_ascii_alphabetic()));
	}
	// `NOMEMORY` is a loader's answer, so a module never raises it and the major stays 1.
	assert_eq!((QS_NOMEMORY, ABI_MAJOR), (10, 1));
}

#[test]
fn a_kind_the_interface_does_not_know_has_no_name() {
	assert_eq!(name_of(0), None);
	assert_eq!(name_of(5), None);
}

#[test]
fn the_build_id_carries_a_dirty_flag_in_the_top_bit() {
	assert_eq!(build_id(0x1234_5678, false), 0x1234_5678);
	assert_eq!(build_id(0x1234_5678, true), 0x9234_5678);
	// The top bit of a commit's digits is dropped, so a clean id is never mistaken for a dirty one.
	assert_eq!(build_id(0xF234_5678, false), 0x7234_5678);
	assert!(!is_dirty(build_id(0xFFFF_FFFF, false)));
	assert!(is_dirty(build_id(0, true)));
	assert_ne!(build_id(0x1234_5678, true), build_id(0x1234_5678, false));
}

fn info(build: u32) -> ModInfo {
	ModInfo {
		size: size_of::<ModInfo>() as u16,
		abi_major: ABI_MAJOR,
		abi_minor: ABI_MINOR,
		kind: KIND_PIC,
		build_id: build,
		arena: 0,
		limit: 4096,
	}
}

#[test]
fn a_release_build_accepts_only_its_own_build() {
	let host = build_id(0xAB12_CD34, false);
	let accepted = check_info(&info(host), KIND_PIC, host, Policy::Release).unwrap();
	assert_eq!(
		accepted,
		Accepted {
			other_build: false,
			newer_minor: false
		}
	);
	// Another commit, and the same commit from a dirty tree, are both refused.
	assert_eq!(
		check_info(&info(0xAB12_CD35), KIND_PIC, host, Policy::Release),
		Err(QS_VERSION)
	);
	assert_eq!(
		check_info(&info(host | BUILD_DIRTY), KIND_PIC, host, Policy::Release),
		Err(QS_VERSION)
	);
}

#[test]
fn a_development_build_accepts_another_build_and_says_so() {
	let host = build_id(0xAB12_CD34, true);
	let accepted = check_info(&info(0x0000_0001), KIND_PIC, host, Policy::Development).unwrap();
	assert!(accepted.other_build);
	assert!(
		!check_info(&info(host), KIND_PIC, host, Policy::Development)
			.unwrap()
			.other_build
	);
}

#[test]
fn another_major_is_refused_in_both_policies_and_a_higher_minor_is_accepted() {
	let host = 7;
	for policy in [Policy::Release, Policy::Development] {
		let mut other = info(host);
		other.abi_major = ABI_MAJOR + 1;
		assert_eq!(check_info(&other, KIND_PIC, host, policy), Err(QS_VERSION));
		other.abi_major = 0;
		assert_eq!(check_info(&other, KIND_PIC, host, policy), Err(QS_VERSION));
		let mut newer = info(host);
		newer.abi_minor = ABI_MINOR + 3;
		assert!(
			check_info(&newer, KIND_PIC, host, policy)
				.unwrap()
				.newer_minor
		);
	}
}

#[test]
fn a_short_or_mistaken_info_is_corrupt_or_unsupported() {
	let host = 7;
	let mut short = info(host);
	short.size = size_of::<ModInfo>() as u16 - 1;
	assert_eq!(
		check_info(&short, KIND_PIC, host, Policy::Development),
		Err(QS_CORRUPT)
	);
	short.size = 0;
	assert_eq!(
		check_info(&short, KIND_PIC, host, Policy::Release),
		Err(QS_CORRUPT)
	);
	// A newer module with a longer struct is fine.
	let mut longer = info(host);
	longer.size += 8;
	assert!(check_info(&longer, KIND_PIC, host, Policy::Release).is_ok());
	// The wrong kind is refused before the version is looked at.
	let mut wrong = info(host);
	wrong.kind = KIND_DLG;
	wrong.abi_major = 9;
	assert_eq!(
		check_info(&wrong, KIND_PIC, host, Policy::Release),
		Err(QS_UNSUPPORTED)
	);
}

#[test]
fn the_module_side_check_mirrors_the_host_side() {
	let start = ModStart {
		size: size_of::<ModStart>() as u16,
		abi_major: ABI_MAJOR,
		abi_minor: ABI_MINOR,
		kind: KIND_PIC,
		build_id: 0x42,
		table: 0,
		table_len: 0,
	};
	assert!(check_start(&start, KIND_PIC, 0x42, Policy::Release).is_ok());
	assert_eq!(
		check_start(&start, KIND_PIC, 0x43, Policy::Release),
		Err(QS_VERSION)
	);
	assert!(
		check_start(&start, KIND_PIC, 0x43, Policy::Development)
			.unwrap()
			.other_build
	);
	assert_eq!(
		check_start(&start, KIND_DLG, 0x42, Policy::Release),
		Err(QS_UNSUPPORTED)
	);
	let mut other = start;
	other.abi_major = 2;
	assert_eq!(
		check_start(&other, KIND_PIC, 0x42, Policy::Development),
		Err(QS_VERSION)
	);
	other = start;
	other.size = 4;
	assert_eq!(
		check_start(&other, KIND_PIC, 0x42, Policy::Development),
		Err(QS_CORRUPT)
	);
}

/// Garbage in every field, from a fixed-seed generator: the checks answer and never panic (no overflow, no index, no unwrap).
#[test]
fn the_checks_never_panic_whatever_the_fields_hold() {
	let mut state: u32 = 0x2545_F491;
	let mut next = move || {
		state ^= state << 13;
		state ^= state >> 17;
		state ^= state << 5;
		state
	};
	for _ in 0..20_000 {
		let word = |value: u32| {
			if value.is_multiple_of(5) {
				u32::MAX
			} else {
				value
			}
		};
		let a = ModInfo {
			size: word(next()) as u16,
			abi_major: word(next()) as u16,
			abi_minor: word(next()) as u16,
			kind: word(next()) as u16,
			build_id: word(next()),
			arena: next(),
			limit: next(),
		};
		let b = ModStart {
			size: a.size,
			abi_major: a.abi_major,
			abi_minor: a.abi_minor,
			kind: a.kind,
			build_id: a.build_id,
			table: next(),
			table_len: next(),
		};
		for policy in [Policy::Release, Policy::Development] {
			let host = word(next());
			for result in [
				check_info(&a, a.kind, host, policy).map(|_| ()),
				check_start(&b, b.kind, host, policy).map(|_| ()),
			] {
				if let Err(status) = result {
					assert_ne!(status_name(status), "?");
				}
			}
		}
		let _ = (
			name_of(a.kind),
			status_name(a.size),
			is_dirty(a.build_id),
			build_id(a.build_id, a.size.is_multiple_of(2)),
		);
	}
}
