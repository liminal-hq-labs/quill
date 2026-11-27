// Page-side helpers for the js-dos measurement scripts (jsdos-units.ts, try-pointer.ts), injected before the page loads: find Quill's
// pointer in js-dos's own frames (ci.screenshot()) by the difference against a reference frame.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

(() => {
	// `mask`, when set, is a box of the frame (x0, y0, x1, y1) that is left out too: where the reference frame's own pointer sits.
	const spike = { ref: null, mask: null };
	// Waits for the picture to settle (the emulator shows a move some frames later), then says where the pointer is.
	spike.settled = async (windowW, windowH, top, bottom) => {
		let last = "";
		let same = 0;
		let found = null;
		for (let i = 0; i < 40 && same < 3; i++) {
			await new Promise((r) => setTimeout(r, 100));
			found = await spike.locate(windowW, windowH, top, bottom);
			const key = JSON.stringify(found && [found.x, found.y, found.n]);
			same = key === last ? same + 1 : 0;
			last = key;
		}
		return found;
	};
	window.__spike = spike;
	spike.send = (dx, dy) => window.__ci.sendMouseRelativeMotion(dx, dy);
	spike.setRef = async () => {
		spike.ref = await window.__ci.screenshot();
	};
	// Where the pointer is, as the top-left of the window of the most changed pixels against the reference frame, between the rows `top`
	// and `bottom`: the corner where the reference's pointer sits, and the menus and toolbars with their hover effects and tooltips, are left
	// out, so the pointer is measured over the page.
	spike.locate = async (windowW, windowH, top, bottom) => {
		const frame = await window.__ci.screenshot();
		const ref = spike.ref;
		const w = frame.width;
		const h = frame.height;
		const changed = new Uint8Array(w * h);
		let total = 0;
		for (let y = 0; y < h; y++) {
			for (let x = 0; x < w; x++) {
				if (y < top || y >= bottom) continue;
				if (spike.mask && x >= spike.mask.x0 && x < spike.mask.x1 && y >= spike.mask.y0 && y < spike.mask.y1) continue;
				const i = (y * w + x) * 4;
				const d = Math.abs(frame.data[i] - ref.data[i]) + Math.abs(frame.data[i + 1] - ref.data[i + 1]) + Math.abs(frame.data[i + 2] - ref.data[i + 2]);
				if (d > 60) {
					changed[y * w + x] = 1;
					total++;
				}
			}
		}
		if (!total) return null;
		// Integral image, then the window with the most changed pixels.
		const stride = w + 1;
		const integral = new Int32Array(stride * (h + 1));
		for (let y = 0; y < h; y++) {
			let row = 0;
			for (let x = 0; x < w; x++) {
				row += changed[y * w + x];
				integral[(y + 1) * stride + x + 1] = integral[y * stride + x + 1] + row;
			}
		}
		let best = -1;
		let bx = 0;
		let by = 0;
		for (let y = 0; y + windowH <= h; y++) {
			for (let x = 0; x + windowW <= w; x++) {
				const n = integral[(y + windowH) * stride + x + windowW] - integral[y * stride + x + windowW] - integral[(y + windowH) * stride + x] + integral[y * stride + x];
				if (n > best) {
					best = n;
					bx = x;
					by = y;
				}
			}
		}
		let x0 = w;
		let y0 = h;
		let x1 = -1;
		let y1 = -1;
		for (let y = by; y < by + windowH; y++) {
			for (let x = bx; x < bx + windowW; x++) {
				if (changed[y * w + x]) {
					x0 = Math.min(x0, x);
					y0 = Math.min(y0, y);
					x1 = Math.max(x1, x);
					y1 = Math.max(y1, y);
				}
			}
		}
		return { x: x0, y: y0, w: x1 - x0 + 1, h: y1 - y0 + 1, n: best, total, frameW: w, frameH: h };
	};
})();
