// Measures the resampler's cost in the browser: milliseconds of main-thread time for each second of 48 kHz stereo output (playback: the guest's rate to 48 kHz,
// two channels) and each second of the guest's record rate (capture: the microphone's 48 kHz to the guest's rate, one channel), best and median of 15 runs.
// Needs the server (bun serve.ts) running. Usage: BASE=http://127.0.0.1:8161 [BROWSER=firefox] bun measure-resample.ts   (DISPLAY set, WAYLAND_DISPLAY unset)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { chromium, firefox } from "playwright-core";
import { BASE, CHROMIUM } from "./lib.ts";

const browser = process.env.BROWSER === "firefox" ? await firefox.launch({ headless: true }) : await chromium.launch({ executablePath: CHROMIUM, headless: true, args: ["--no-first-run"] });
try {
	const page = await browser.newPage();
	await page.goto(`${BASE}/try-reactos/`);
	const result = await page.evaluate(async () => {
		const { Resampler } = (await import("/try-reactos/lib/ac97.js")) as any;
		const median = (a: number[]) => [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)] as number;
		const rows: any[] = [];
		const run = (inRate: number, outRate: number, channels: number) => {
			const resamplers = Array.from({ length: channels }, () => new Resampler(inRate, outRate));
			const input = new Int16Array(inRate * 2);
			for (let i = 0; i < input.length; i++) input[i] = Math.round(12000 * Math.sin(i * 0.05));
			const out = new Float32Array(outRate + 64);
			const times: number[] = [];
			for (let r = 0; r < 15; r++) {
				const t0 = performance.now();
				// One second of the faster rate's worth of work, in 10 ms blocks as the page does it.
				const outBlock = Math.round(outRate / 100);
				for (let b = 0; b < 100; b++) {
					for (let c = 0; c < channels; c++) {
						const rs = resamplers[c];
						const need = rs.inputNeeded(outBlock);
						rs.write(input, (b * 7 * channels) % 5000 + c, need, channels, 1 / 32768);
						rs.read(out, outBlock);
					}
				}
				times.push(performance.now() - t0);
			}
			return { inRate, outRate, channels, bestMs: Math.round(Math.min(...times) * 100) / 100, medianMs: Math.round(median(times) * 100) / 100 };
		};
		for (const rate of [8000, 11025, 16000, 22050, 32000, 44100]) rows.push({ what: "playback", ...run(rate, 48000, 2) });
		rows.push({ what: "playback", ...run(48000, 48000, 2) });
		for (const rate of [8000, 11025, 22050, 44100]) rows.push({ what: "capture", ...run(48000, rate, 1) });
		return rows;
	});
	for (const row of result) console.log(JSON.stringify(row));
} finally {
	await browser.close();
}
