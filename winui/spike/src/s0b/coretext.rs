// P1, first attempt: CoreTextEditContext, which Microsoft documents for custom text controls and reports broken on WinUI 3.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `CoreTextServicesManager::GetForCurrentView` is the only way into the edit context from a non-UWP app, and
//! microsoft-ui-xaml issue 4239 reports that it fails on WinUI 3 desktop. This probe makes the call on the XAML thread,
//! logs the exact result and, if it succeeds, subscribes to the edit context's events so a later run shows whether text ever
//! reaches it.

use windows::Foundation::TypedEventHandler;
use windows::UI::Text::Core::{CoreTextEditContext, CoreTextServicesManager};

use crate::log;

/// Keeps the edit context alive for the life of the process when the call succeeds.
pub struct Probe {
	_context: Option<CoreTextEditContext>,
}

pub fn probe() -> Probe {
	let manager = match CoreTextServicesManager::GetForCurrentView() {
		Ok(m) => m,
		Err(e) => {
			log(&format!(
				"coretext: CoreTextServicesManager::GetForCurrentView failed: {} (0x{:08X})",
				e.message(),
				e.code().0 as u32
			));
			return Probe { _context: None };
		}
	};
	log("coretext: CoreTextServicesManager::GetForCurrentView succeeded");
	let context = match manager.CreateEditContext() {
		Ok(c) => c,
		Err(e) => {
			log(&format!(
				"coretext: CreateEditContext failed: {} (0x{:08X})",
				e.message(),
				e.code().0 as u32
			));
			return Probe { _context: None };
		}
	};
	let _ = context.TextRequested(&TypedEventHandler::new(|_, _| {
		log("coretext: TextRequested");
		Ok(())
	}));
	let _ = context.SelectionRequested(&TypedEventHandler::new(|_, _| {
		log("coretext: SelectionRequested");
		Ok(())
	}));
	let _ = context.CompositionStarted(&TypedEventHandler::new(|_, _| {
		log("coretext: CompositionStarted");
		Ok(())
	}));
	let _ = context.TextUpdating(&TypedEventHandler::new(
		|_, args: windows::core::Ref<'_, windows::UI::Text::Core::CoreTextTextUpdatingEventArgs>| {
			if let Some(a) = args.as_ref() {
				log(&format!("coretext: TextUpdating {:?}", a.Text().map(|t| t.to_string())));
			}
			Ok(())
		},
	));
	match context.NotifyFocusEnter() {
		Ok(()) => log("coretext: NotifyFocusEnter ok"),
		Err(e) => log(&format!(
			"coretext: NotifyFocusEnter failed: {} (0x{:08X})",
			e.message(),
			e.code().0 as u32
		)),
	}
	Probe { _context: Some(context) }
}
