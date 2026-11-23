// Shows Quill's Windows 3.x icon: one minimised window (the class icon, as Program Manager and the desktop draw it) and one open window that draws it at 32 by 32.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The icon is resource 1 in the executable (`resources*.json` name the .ICO file). The program runs until Windows ends, so a harness
// can take screenshots, and writes ICONTEST.LOG next to itself with the display's depth.
#![no_std]
#![no_main]
use w16::*;

export_proc!(IconWndProc, icon_wndproc);

const SW_SHOWMINNOACTIVE: u32 = 7;
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
const CW_USEDEFAULT: u32 = 0x8000;
const COLOR_WINDOW: u32 = 5;
const BITSPIXEL: u32 = 12;
const PLANES: u32 = 14;
const NUMCOLORS: u32 = 24;

static mut ICON: u32 = 0;

extern "C" fn icon_wndproc(hwnd: u32, msg: u32, wparam: u32, lparam: u32) -> u32 {
	unsafe {
		match msg {
			WM_PAINT => {
				let mut ps = core::mem::zeroed::<PaintStruct>();
				let hdc = BeginPaint(hwnd, &mut ps as *mut PaintStruct as u32);
				DrawIcon(hdc, 16, 16, ICON);
				EndPaint(hwnd, &mut ps as *mut PaintStruct as u32);
				0
			}
			WM_DESTROY => {
				PostQuitMessage(0);
				0
			}
			_ => DefWindowProc(hwnd, msg, wparam, lparam),
		}
	}
}

#[unsafe(no_mangle)]
pub extern "C" fn w16_main(hinst: u32, hprev: u32, _show: u32) -> u32 {
	unsafe {
		let icon = LoadIcon(hinst, far(0, int_resource(1)));
		ICON = icon;
		if hprev == 0 {
			let class = WndClass {
				style: 0,
				wndproc: code_selector() << 16 | IconWndProc as usize as u32 & 0xFFFF,
				cls_extra: 0,
				wnd_extra: 0,
				instance: hinst as u16,
				icon: icon as u16,
				cursor: LoadCursor(0, far(0, int_resource(32512))) as u16,
				background: (COLOR_WINDOW + 1) as u16,
				menu: 0,
				class_name: far(data_selector(), cs!("QuillIconTest")),
			};
			RegisterClass(&class as *const WndClass as u32);
		}
		let open = CreateWindow(
			cs!("QuillIconTest") as u32,
			cs!("Quill icon") as u32,
			WS_OVERLAPPEDWINDOW,
			40,
			40,
			200,
			120,
			0,
			0,
			hinst,
			0,
		);
		ShowWindow(open, SW_SHOW);
		UpdateWindow(open);
		let iconic = CreateWindow(
			cs!("QuillIconTest") as u32,
			cs!("Quill") as u32,
			WS_OVERLAPPEDWINDOW,
			CW_USEDEFAULT,
			0,
			200,
			120,
			0,
			0,
			hinst,
			0,
		);
		ShowWindow(iconic, SW_SHOWMINNOACTIVE);
		let hdc = GetDC(0);
		let log = log();
		log.s("icon handle ")
			.hex(icon, 4)
			.s("; display bits per pixel ")
			.dec(GetDeviceCaps(hdc, BITSPIXEL))
			.s(", planes ")
			.dec(GetDeviceCaps(hdc, PLANES))
			.s(", colours ")
			.dec(GetDeviceCaps(hdc, NUMCOLORS))
			.nl();
		ReleaseDC(0, hdc);
		dos_mkdir(b"C:\\QUILL\0".as_ptr());
		write_file(b"C:\\QUILL\\ICONTEST.LOG\0".as_ptr(), log.bytes());
		write_file(b"D:\\ICONTEST.LOG\0".as_ptr(), log.bytes());
		let mut msg = core::mem::zeroed::<Msg>();
		while GetMessage(&mut msg as *mut Msg as u32, 0, 0, 0) != 0 {
			TranslateMessage(&mut msg as *mut Msg as u32);
			DispatchMessage(&mut msg as *mut Msg as u32);
		}
	}
	0
}
