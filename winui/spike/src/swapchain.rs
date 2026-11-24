// The Direct3D 11 composition swap chain the page is presented through.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use windows::core::Interface;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;

pub struct Gpu {
	pub device: ID3D11Device,
	pub context: ID3D11DeviceContext,
	pub factory: IDXGIFactory2,
	pub driver: &'static str,
}

impl Gpu {
	/// A hardware device, or WARP when the machine has no usable adapter.
	pub fn new() -> windows::core::Result<Gpu> {
		let mut last = None;
		for (kind, name) in [(D3D_DRIVER_TYPE_HARDWARE, "hardware"), (D3D_DRIVER_TYPE_WARP, "warp")] {
			match Self::with_driver(kind, name) {
				Ok(gpu) => return Ok(gpu),
				Err(e) => {
					crate::log(&format!("d3d11 {name} failed: {e}"));
					last = Some(e);
				}
			}
		}
		Err(last.expect("one attempt"))
	}

	fn with_driver(kind: D3D_DRIVER_TYPE, name: &'static str) -> windows::core::Result<Gpu> {
		let mut device = None;
		let mut context = None;
		unsafe {
			D3D11CreateDevice(
				None,
				kind,
				HMODULE::default(),
				D3D11_CREATE_DEVICE_BGRA_SUPPORT,
				None,
				D3D11_SDK_VERSION,
				Some(&mut device),
				None,
				Some(&mut context),
			)?;
		}
		let device = device.ok_or_else(windows::core::Error::empty)?;
		let context = context.ok_or_else(windows::core::Error::empty)?;
		let dxgi: IDXGIDevice = device.cast()?;
		let adapter = unsafe { dxgi.GetAdapter()? };
		let factory: IDXGIFactory2 = unsafe { adapter.GetParent()? };
		Ok(Gpu {
			device,
			context,
			factory,
			driver: name,
		})
	}

	/// A swap chain of `w` by `h` device pixels, scaled back by `1 / scale` so one buffer pixel is one screen pixel.
	pub fn swap_chain(&self, w: u32, h: u32, scale: (f32, f32)) -> windows::core::Result<IDXGISwapChain1> {
		let desc = DXGI_SWAP_CHAIN_DESC1 {
			Width: w,
			Height: h,
			Format: DXGI_FORMAT_B8G8R8A8_UNORM,
			Stereo: false.into(),
			SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
			BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
			BufferCount: 2,
			Scaling: DXGI_SCALING_STRETCH,
			SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
			AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
			Flags: 0,
		};
		let chain = unsafe { self.factory.CreateSwapChainForComposition(&self.device, &desc, None)? };
		let chain2: IDXGISwapChain2 = chain.cast()?;
		let matrix = DXGI_MATRIX_3X2_F {
			_11: 1.0 / scale.0,
			_12: 0.0,
			_21: 0.0,
			_22: 1.0 / scale.1,
			_31: 0.0,
			_32: 0.0,
		};
		unsafe { chain2.SetMatrixTransform(&matrix)? };
		Ok(chain)
	}

	/// Copies BGRA `pixels` (`w` by `h`) into the chain's back buffer and presents it.
	pub fn present(&self, chain: &IDXGISwapChain1, pixels: &[u8], w: u32, h: u32) -> windows::core::Result<()> {
		let desc = D3D11_TEXTURE2D_DESC {
			Width: w,
			Height: h,
			MipLevels: 1,
			ArraySize: 1,
			Format: DXGI_FORMAT_B8G8R8A8_UNORM,
			SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
			Usage: D3D11_USAGE_DEFAULT,
			BindFlags: 0,
			CPUAccessFlags: 0,
			MiscFlags: 0,
		};
		let init = D3D11_SUBRESOURCE_DATA {
			pSysMem: pixels.as_ptr().cast(),
			SysMemPitch: w * 4,
			SysMemSlicePitch: 0,
		};
		let mut source = None;
		unsafe {
			self.device.CreateTexture2D(&desc, Some(&init), Some(&mut source))?;
			let source = source.ok_or_else(windows::core::Error::empty)?;
			let back: ID3D11Texture2D = chain.GetBuffer(0)?;
			self.context.CopyResource(&back, &source);
			chain.Present(1, DXGI_PRESENT(0)).ok()
		}
	}
}
