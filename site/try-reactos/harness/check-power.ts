// Checks the power cycle (SITE-40): Start, Shut Down from the guest's Start menu, Turn on, Stop, Start, Reset, and the diagnostics
// panel's cold boot. After each step the buttons, their labels, the status line and the stage's power attribute are read back and must
// agree with the machine (`is_running()`), and the guest's own pixels and a screenshot of the page are saved for each state.
// Needs `tools/stage-web.py` to have staged data/ and web-manifest.json, and the server (bun serve.ts) running.
// Input goes through the page's emulator API and pixels come from the canvas, never the host desktop. Exits non-zero on a failed check.
// Usage: BASE=http://127.0.0.1:8161 bun check-power.ts OUTDIR
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { KEY, PROFILES, grab, launch, pageUrl, save, sleep, waitFor } from "./lib.ts";

const outDir = process.argv[2];
if (!outDir) throw new Error("usage: check-power.ts OUTDIR");

const failures: string[] = [];
const expect = (ok: boolean, message: string) => {
	if (!ok) {
		failures.push(message);
		console.error("FAIL " + message);
	}
};

const session = await launch("", PROFILES.unthrottled, false);
const page = session.page;
const desktop = "window.__m.taskbarScore() > 0.3";

// What the visitor sees, and what the machine is.
const ui = () =>
	page.evaluate(() => {
		const b = (s: string) => document.querySelector(s) as HTMLButtonElement;
		const e = (window as any).tryReactos.emulator;
		return {
			start: b("[data-start]").textContent,
			startOff: b("[data-start]").disabled,
			resetOff: b("[data-reset]").disabled,
			stopOff: b("[data-stop]").disabled,
			fullOff: b("[data-fullscreen]").disabled,
			coldOff: b("[data-cold-boot]").disabled,
			status: document.querySelector("[data-status]")!.textContent ?? "",
			power: (document.querySelector("[data-stage]") as HTMLElement).dataset.power,
			api: (window as any).tryReactos.powerState(),
			link: !!(window as any).tryReactos.hostLink,
			clipboard: !!(window as any).tryReactos.clipboard,
			bar: !(document.querySelector("[data-clip-bar]") as HTMLElement).hidden,
			chip: document.querySelector("[data-clip-chip]")!.textContent ?? "",
			clipRow: document.querySelector("[data-clip-readout] dd")!.textContent ?? "",
			machine: e ? (e.is_running() ? "running" : "stopped") : "none",
			dim: getComputedStyle(document.querySelector(".screen-canvas")!).filter,
		};
	});
type Ui = Awaited<ReturnType<typeof ui>>;

const shoot = async (name: string) => {
	const frame = await grab(page);
	if (frame) save(outDir, `${name}-canvas.png`, frame.png);
	await page.evaluate(() => ((document.querySelector("[data-diag]") as HTMLDetailsElement).open = true));
	save(outDir, `${name}-page.png`, await page.screenshot({ fullPage: true }));
};

const key = (codes: number[]) =>
	page.evaluate((c) => {
		const i = (window as any).tryReactos.input;
		i.key(c);
		i.key(c.map((v: number) => v | 0x80));
	}, codes);

// The buttons must be exactly what the state says, whatever the state is.
const matches = (state: string, u: Ui, why: string) => {
	const want: Record<string, Partial<Ui>> = {
		off: { start: "Start", startOff: false, stopOff: true, resetOff: true, fullOff: true, machine: "none" },
		on: { start: "Start", startOff: true, stopOff: false, resetOff: false, fullOff: false, machine: "running" },
		shutdown: { start: "Turn on", startOff: false, stopOff: false, resetOff: false, fullOff: true, machine: "stopped" },
	};
	expect(u.power === state && u.api === state, `${why}: the power state is ${u.power}/${u.api}, wanted ${state}`);
	// The host link attaches with the machine and detaches when it shuts down or stops (the seams' attach and detach across the cycle).
	expect(u.link === (state === "on"), `${why}: the host link is ${u.link ? "attached" : "detached"} in the ${state} state`);
	// The clipboard follows the link: its state machine and the chip exist while the machine is on, and neither is left behind otherwise.
	expect(u.clipboard === (state === "on"), `${why}: the clipboard is ${u.clipboard ? "attached" : "detached"} in the ${state} state`);
	expect(u.bar === (state === "on"), `${why}: the clipboard chip is ${u.bar ? "shown" : "hidden"} in the ${state} state`);
	if (state !== "on") expect(u.clipRow === "off", `${why}: the Clipboard row says "${u.clipRow}" in the ${state} state`);
	for (const [field, value] of Object.entries(want[state])) expect((u as any)[field] === value, `${why}: ${field} is ${(u as any)[field]}, wanted ${value} (${state})`);
	expect(state === "shutdown" ? /brightness/.test(u.dim) : u.dim === "none", `${why}: the screen's filter is ${u.dim}`);
};

// POWER_PARAMS (a JSON object of address fields, for example {"disk":"/images/hl-e2e.img","state":"/images/hl-state.bin","boot":"hd"}) runs the cycle on
// a state with the link helper in it, and then each reattach must also reach a connected helper with its hello answered at once.
const params = process.env.POWER_PARAMS ? (JSON.parse(process.env.POWER_PARAMS) as Record<string, string>) : null;
const helperUp = async (why: string) => {
	if (!params) return;
	const ready = await waitFor(page, "window.tryReactos.hostLink && window.tryReactos.hostLink.stats().state === 'ready'", 60000, 20);
	const helloMs = ready === null ? null : await page.evaluate("window.tryReactos.hostLink.stats().helloMs");
	expect(ready !== null && (helloMs as number) < 1000, `${why}: the helper's hello was answered in ${helloMs} ms`);
	const chip = await page.textContent("[data-clip-chip]");
	expect(/^Clipboard: connected/.test(chip ?? ""), `${why}: the chip says "${chip}"`);
	(report.helloMs as Record<string, unknown>)[why] = helloMs;
};

const settled = () => page.waitForFunction("document.querySelector('[data-build]').textContent !== 'Build: loading'", null, { timeout: 15000 });
const toDesktop = async (why: string) => {
	expect((await waitFor(page, desktop, 240000)) !== null, `${why}: no themed taskbar`);
	await sleep(3000);
};
// Shut Down from the guest's Start menu: the Windows key, Up (wraps to the last item), Enter (the Shut Down ReactOS dialog, with Shut Down
// selected) and Enter again.
const shutDown = async (why: string) => {
	if (params) {
		// The helper test state's Start menu is not the product's, so with POWER_PARAMS the guest is powered off the way the page sees any stop (step 8).
		await page.evaluate("window.tryReactos.emulator.stop()");
		const stopped = await waitFor(page, "window.tryReactos.powerState() === 'shutdown'", 10000, 100);
		expect(stopped !== null, `${why}: the page never noticed the stop`);
		return stopped;
	}
	await key(KEY.win);
	await sleep(1500);
	await key(KEY.up);
	await sleep(800);
	await key(KEY.enter);
	await sleep(2500);
	await key(KEY.enter);
	const t = await waitFor(page, "window.tryReactos.powerState() === 'shutdown'", 60000, 250);
	expect(t !== null, `${why}: the page never noticed the shut down`);
	return t;
};

const report: Record<string, unknown> = { helloMs: {} };
try {
	await page.goto(params ? pageUrl("", params) : pageUrl("", {}, { plain: true }));
	await settled();
	matches("off", await ui(), "before Start");
	await shoot("1-off");

	// 1. Start.
	await page.click("[data-start]");
	expect((await ui()).power === "starting", "Start did not show the starting state");
	expect((await waitFor(page, "window.tryReactos.powerState() === 'on'", 60000)) !== null, "Start never reached the on state");
	matches("on", await ui(), "Start");
	await toDesktop("Start");
	matches("on", await ui(), "Start, desktop");
	await helperUp("Start");
	await shoot("2-running");

	// 2. Shut Down in the guest.
	await page.evaluate("window.__first = window.tryReactos.emulator");
	report.shutdownDetectedMs = await shutDown("Shut Down");
	await sleep(1500);
	const down = await ui();
	matches("shutdown", down, "after Shut Down");
	expect(/^ReactOS has shut down/.test(down.status), `the status after Shut Down is "${down.status}"`);
	expect(!down.coldOff, "the cold boot button is disabled after Shut Down");
	report.statusAfterShutdown = down.status;
	await shoot("3-shut-down");
	await sleep(3000);
	matches("shutdown", await ui(), "the shut down state, three seconds on");

	// 3. Turn on: the saved desktop, through the normal path, on a new machine.
	await page.click("[data-start]");
	await toDesktop("Turn on");
	const on = await ui();
	matches("on", on, "Turn on");
	await helperUp("Turn on");
	expect(await page.evaluate("window.tryReactos.emulator !== window.__first && !window.__first.is_running()"), "Turn on did not replace the stopped machine");
	expect(/^Running\./.test(on.status), `the status after Turn on is "${on.status}"`);
	await shoot("4-turned-on");

	// 4. Stop.
	await page.click("[data-stop]");
	const stopped = await ui();
	matches("off", stopped, "Stop");
	expect(/^Stopped\./.test(stopped.status), `the status after Stop is "${stopped.status}"`);
	await shoot("5-stopped");

	// 5. Start again.
	await page.click("[data-start]");
	await toDesktop("Start after Stop");
	matches("on", await ui(), "Start after Stop");
	await helperUp("Start after Stop");

	// 6. Reset.
	await page.evaluate("window.__first = window.tryReactos.emulator");
	await page.click("[data-reset]");
	expect((await ui()).power === "starting", "Reset did not show the starting state");
	await toDesktop("Reset");
	matches("on", await ui(), "Reset");
	await helperUp("Reset");
	expect(await page.evaluate("window.tryReactos.emulator !== window.__first"), "Reset kept the same machine");
	await shoot("6-reset");

	// 7. Shut Down once more, then Reset from the shut down state, then Stop from it.
	await shutDown("second Shut Down");
	matches("shutdown", await ui(), "second Shut Down");
	await page.click("[data-reset]");
	await toDesktop("Reset after Shut Down");
	matches("on", await ui(), "Reset after Shut Down");
	await helperUp("Reset after Shut Down");
	await shutDown("third Shut Down");
	await page.click("[data-stop]");
	matches("off", await ui(), "Stop after Shut Down");
	await page.click("[data-start]");
	await toDesktop("Start after Stop after Shut Down");
	matches("on", await ui(), "Start after Stop after Shut Down");

	// 8. A stop from outside (anything that stops the emulator) is seen the same way.
	await page.evaluate("window.tryReactos.emulator.stop()");
	expect((await waitFor(page, "window.tryReactos.powerState() === 'shutdown'", 5000, 100)) !== null, "an outside stop was not noticed");
	matches("shutdown", await ui(), "after an outside stop");

	// 9. The cold boot option: no saved desktop.
	await page.click("[data-cold-boot]");
	const cold = await ui();
	expect(/^Starting ReactOS\. This takes a while/.test(cold.status) || cold.power === "starting", `the cold boot status is "${cold.status}"`);
	expect((await waitFor(page, "window.tryReactos.powerState() === 'on'", 60000)) !== null, "the cold boot never reached the on state");
	matches("on", await ui(), "cold boot");
	expect((await page.evaluate("window.tryReactos.config.state")) === "", "the cold boot kept a saved state");
	await page.click("[data-stop]");
	matches("off", await ui(), "Stop after the cold boot");

	console.log(JSON.stringify({ ...report, failures }, null, 1));
} finally {
	await session.close();
}
if (failures.length > 0) {
	console.error(failures.join("\n"));
	process.exit(1);
}
