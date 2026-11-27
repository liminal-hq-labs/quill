// Host tests for the module interface in a .DMD file: nothing here ships.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! This crate has no code of its own. Its tests (`tests/`) build a module image the way a picture module's linker output looks,
//! with a [`quill_modabi::ModInfo`] and a [`quill_modabi::PicTableV1`] in the data and the table's entries as relocated words, pack
//! it with `desk-module`, unpack and load it at several addresses and link bases, and run the interface's build-id check on what
//! came out.
