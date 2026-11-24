// Real-mode BIOS calls through the DPMI host, as the HAL's `Bios` trait the VGA and mouse drivers are written against.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `DosBios` runs a real-mode interrupt with DPMI 0300h. A buffer for the call is a block of conventional memory allocated once
// (DPMI 0100h), which is also where `read_low` finds the VBE strings and mode list the BIOS leaves in it. Calls that return a
// real-mode segment (the ROM font pointer, VBE far pointers) must come through here: a direct `int` cannot hand one back.
use crate::dpmi::{self, RealRegs};
use core::sync::atomic::{AtomicU32, Ordering};
use desk_hal::{Bios, BiosRegs, HalError};

/// Paragraphs of the conventional-memory buffer: 512 bytes, a VBE information block.
const BUFFER_PARAGRAPHS: u16 = 32;
static BUFFER_SEGMENT: AtomicU32 = AtomicU32::new(0);

fn buffer_segment() -> Result<u16, HalError> {
	let seg = BUFFER_SEGMENT.load(Ordering::Relaxed);
	if seg != 0 {
		return Ok(seg as u16);
	}
	let (segment, _selector) = dpmi::alloc_dos_memory(BUFFER_PARAGRAPHS).map_err(|_| HalError::OutOfMemory)?;
	BUFFER_SEGMENT.store(segment.into(), Ordering::Relaxed);
	Ok(segment)
}

/// Real-mode software interrupts through the DPMI host.
#[derive(Clone, Copy, Debug, Default)]
pub struct DosBios;

impl Bios for DosBios {
	fn int(&self, vector: u8, regs: &mut BiosRegs, buffer: Option<&mut [u8]>) -> Result<(), HalError> {
		let mut r = RealRegs {
			eax: regs.eax,
			ebx: regs.ebx,
			ecx: regs.ecx,
			edx: regs.edx,
			esi: regs.esi,
			edi: regs.edi,
			ebp: regs.ebp,
			es: regs.es,
			ds: regs.ds,
			..RealRegs::default()
		};
		let mut block = None;
		if let Some(b) = buffer {
			if b.len() > desk_hal::bios::BIOS_BUFFER_MAX {
				return Err(HalError::InvalidArgument);
			}
			let seg = buffer_segment()?;
			let p = dpmi::linear_to_ptr::<u8>(u32::from(seg) << 4);
			// SAFETY: the block is 512 bytes of conventional memory (mapped at linear 0 by the host) and `b.len()` is at most 512.
			unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), p, b.len()) };
			r.es = seg;
			r.edi = 0;
			block = Some((b, p));
		}
		dpmi::simulate_int(vector, &mut r).map_err(|e| HalError::Platform(e.0))?;
		if let Some((b, p)) = block {
			// SAFETY: as above.
			unsafe { core::ptr::copy_nonoverlapping(p, b.as_mut_ptr(), b.len()) };
		}
		regs.eax = r.eax;
		regs.ebx = r.ebx;
		regs.ecx = r.ecx;
		regs.edx = r.edx;
		regs.esi = r.esi;
		regs.edi = r.edi;
		regs.ebp = r.ebp;
		regs.es = r.es;
		regs.ds = r.ds;
		regs.flags = r.flags;
		Ok(())
	}

	fn read_low(&self, segment: u16, offset: u16, out: &mut [u8]) -> Result<(), HalError> {
		let start = (u32::from(segment) << 4) + u32::from(offset);
		if u64::from(start) + out.len() as u64 > 0x10_0000 {
			return Err(HalError::InvalidArgument);
		}
		let p = dpmi::linear_to_ptr::<u8>(start);
		for (i, o) in out.iter_mut().enumerate() {
			// SAFETY: inside the first megabyte, which the host maps at linear 0, reached through the 4 GiB data segment.
			*o = unsafe { p.add(i).read_volatile() };
		}
		Ok(())
	}
}
