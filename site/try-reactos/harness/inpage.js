// Page-side helpers for the measurement scripts, injected before the page loads. Input reaches the guest only through the
// page's own emulator API; pixels are read from the emulator's canvas.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

(() => {
	const m = {};
	window.__m = m;

	const ctx = () => {
		const c = window.tryReactos && window.tryReactos.canvas();
		if (!c || !c.width) return null;
		return { c, g: c.getContext("2d") };
	};

	// The fraction of the bottom 26 rows that is ReactOS's cream face colour (212, 208, 200), or the dark taskbar panel colours of a themed desktop
	// (#0c0c10 and #14161d): a taskbar, not a boot screen.
	m.taskbarScore = () => {
		const k = ctx();
		if (!k || k.c.width < 640) return 0;
		const { c, g } = k;
		const d = g.getImageData(0, c.height - 26, c.width, 26).data;
		let hit = 0;
		for (let i = 0; i < d.length; i += 4) {
			if (Math.abs(d[i] - 212) < 10 && Math.abs(d[i + 1] - 208) < 10 && Math.abs(d[i + 2] - 200) < 10) hit++;
			else if (Math.abs(d[i] - 12) < 3 && Math.abs(d[i + 1] - 12) < 3 && Math.abs(d[i + 2] - 16) < 3) hit++;
			else if (Math.abs(d[i] - 20) < 3 && Math.abs(d[i + 1] - 22) < 3 && Math.abs(d[i + 2] - 29) < 3) hit++;
		}
		return hit / (d.length / 4);
	};

	// Fraction of the whole screen that is blue-screen blue (0, 0, 170 in VGA text, or ReactOS's 0, 0, 128 / 0, 0, 170).
	m.blueScore = () => {
		const k = ctx();
		if (!k) return 0;
		const { c, g } = k;
		const d = g.getImageData(0, 0, c.width, c.height).data;
		let hit = 0;
		for (let i = 0; i < d.length; i += 16) {
			if (d[i] < 20 && d[i + 1] < 20 && d[i + 2] > 110 && d[i + 2] < 190) hit++;
		}
		return hit / (d.length / 16);
	};

	const snap = (skipBottom) => {
		const k = ctx();
		if (!k) return null;
		const { c, g } = k;
		return g.getImageData(0, 0, c.width, c.height - skipBottom).data;
	};

	const diff = (a, b) => {
		let n = 0;
		for (let i = 0; i < a.length; i += 4) {
			if (a[i] !== b[i] || a[i + 1] !== b[i + 1] || a[i + 2] !== b[i + 2]) n++;
		}
		return n;
	};

	const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

	// Poll as fast as the event loop allows, without waiting for the next frame.
	const nextTick = () =>
		new Promise((r) => {
			const ch = new MessageChannel();
			ch.port1.onmessage = () => r();
			ch.port2.postMessage(0);
		});

	// How many pixels change by themselves in a quiet second: a blinking caret, a clock.
	m.noise = async (ms) => {
		const base = snap(30);
		if (!base) return 0;
		let max = 0;
		const t0 = performance.now();
		while (performance.now() - t0 < ms) {
			await sleep(16);
			const s = snap(30);
			max = Math.max(max, diff(base, s));
		}
		return max;
	};

	const down = { a: [0x1e], w: [0x11], m: [0x32] };
	m.scan = down;

	// Send one key (make then break) and report the milliseconds until at least `threshold` pixels differ from before the key.
	m.keyLatency = async (make, threshold, timeoutMs) => {
		const base = snap(30);
		if (!base) return null;
		const t0 = performance.now();
		window.tryReactos.input.key(make);
		window.tryReactos.input.key(make.map((c) => c | 0x80));
		while (performance.now() - t0 < timeoutMs) {
			await nextTick();
			const s = snap(30);
			if (diff(base, s) >= threshold) return performance.now() - t0;
		}
		return null;
	};

	// Count the frames that differ from the previous one while `driver` runs, and return frames per second.
	m.frameRate = async (ms, driverKeys, hz) => {
		let last = snap(0);
		let changed = 0;
		let polls = 0;
		const t0 = performance.now();
		let nextKey = t0;
		let n = 0;
		while (performance.now() - t0 < ms) {
			const now = performance.now();
			if (driverKeys.length && now >= nextKey) {
				const k = driverKeys[n++ % driverKeys.length];
				window.tryReactos.input.key(k);
				window.tryReactos.input.key(k.map((c) => c | 0x80));
				nextKey += 1000 / hz;
			}
			await new Promise((r) => requestAnimationFrame(r));
			const s = snap(0);
			polls++;
			if (diff(last, s) > 0) changed++;
			last = s;
		}
		const secs = (performance.now() - t0) / 1000;
		return { changedFrames: changed, polls, seconds: secs, fps: changed / secs };
	};

	m.heap = () => {
		const p = performance.memory;
		return p ? { used: p.usedJSHeapSize, total: p.totalJSHeapSize, limit: p.jsHeapSizeLimit } : null;
	};
})();
