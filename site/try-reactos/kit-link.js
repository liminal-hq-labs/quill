// The page's Host Link Kit glue: what the PC's helper says about its sound, system and power (Diagnostics) and the buttons that act on it. host-link.js hands over
// the link and the emulator; the sound card is read through window.tryReactos.soundHandle when it is there. Nothing is asked or kept and no timer runs while
// the panel is closed, and nothing leaves the tab (SITE-124, SITE-125, SITE-127, SITE-128). The rules are in kit-rules.js.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// The ?v= is the first 8 hex of the module's SHA-256; tools/stamp-web.py keeps it current (SITE-14).
import { hasChannel, hasFeature, hostFields, logLevelFields, parseCaps, parseDevices, parseLogLine, parseMixer, parsePlayed, parsePowerEnding, parseSysInfo, parseWatch, playFields, powerRequest, setFields, statsFields, SYS_GET } from "./vendor/v86-host-link/host-link.js?v=0d03ac0f";
import { T, badgeVisible, codecWrite, createLogBuffer, createPoller, createStep, describePc, fill, hostOutput, mixerMessage, playTracker, sliderMoved, speakerVolume, switchBoard, switchedOff, switchedOn, systemLines, unmuteRequest, GROUPS } from "./kit-rules.js?v=3b6928a0";

const KIT_ISO = new URL("./vendor/v86-host-link/kit/hostlink-kit.iso", import.meta.url);
const STATS_MS = 500;
const $ = (selector) => document.querySelector(selector);
const el = (tag, props = {}, ...children) => {
	const node = Object.assign(document.createElement(tag), props);
	node.append(...children);
	return node;
};

let k = null; // the live state of the glue, null while no machine is attached

const fresh = (host, emulator) => ({
	host,
	emulator,
	board: null,
	step: createStep(),
	tracker: playTracker(),
	pc: { devices: null, mixer: null },
	sys: null,
	log: createLogBuffer(200),
	level: "info",
	poll: null,
	stepTimer: null,
	watch: { open: false, at: 0, last: 0, timer: null },
	test: null,
	lastHost: null,
	lastCodec: null,
	off: false,
	pending: null,
	ui: {},
	undo: [],
});

const card = () => (window.tryReactos && window.tryReactos.soundHandle) || null;
const slider = () => $("[data-volume]");
const diagOpen = () => Boolean($("[data-diag]") && $("[data-diag]").open);

// Whether both sides list the channel (and the helper the feature).
function peer(channel, feature) {
	const s = k.host.stats();
	if (s.state !== "ready" || !s.peer) return false;
	if (!hasChannel(parseCaps(s.peer.caps), channel) || !parseCaps(k.host.link.caps).channels.has(channel)) return false;
	return !feature || hasFeature(parseCaps(s.peer.caps), feature);
}
const send = (channel, op, fields, data) => (k.host.stats().state === "ready" ? k.host.send(channel, op, fields, data) : null);

// ---- messages from the PC ----

function onMessage(m) {
	if (!k) return;
	if (m.channel === "audio") {
		if (m.op === "devices") k.pc.devices = parseDevices(m);
		else if (m.op === "mixer") {
			const mixer = parseMixer(m);
			if (!mixer) return;
			k.pc.mixer = mixer;
			applyStep(mixerMessage(k.step, mixer));
		} else if (m.op === "watch") {
			const state = parseWatch(m);
			if (state === null) return;
			k.watch.open = state === "open";
			k.lastHost = null;
			if (k.watch.open) {
				k.watch.at = performance.now();
				k.watch.last = 0;
				if (k.watch.timer === null) k.watch.timer = setInterval(watchTick, STATS_MS);
			}
		} else if (m.op === "played") {
			const played = parsePlayed(m);
			if (played && k.test && played.sound === "test") {
				k.test.played = played.ok;
				if (!played.ok) endTest(T.testFailed);
			}
		}
	} else if (m.channel === "sys") {
		const info = parseSysInfo(m);
		if (info && diagOpen()) k.sys = info;
	} else if (m.channel === "log") {
		const line = parseLogLine(m);
		if (line) k.log.push(line);
	} else if (m.channel === "power") {
		const ending = parsePowerEnding(m);
		if (ending) setNote("power", T.ending[ending.kind]);
	} else if (m.channel === "ctl" && m.op === "error" && k.pending && Number(m.fields.id) === k.pending.id) setNote("power", T.shutdownRefused);
	draw();
}

// ---- the sound: Keep in step, the badge, the test sound, audio host ----

function applyStep(result) {
	k.step = result.state;
	if (result.slider !== undefined && slider()) {
		slider().value = String(result.slider);
		k.mute = true;
		slider().dispatchEvent(new Event("input", { bubbles: true }));
		k.mute = false;
		holdSpeaker();
	}
	if (result.set) send("audio", "set", setFields(result.set));
}

// While the PC follows the slider the speaker's gain stays at 1 (SITE-123); the page's own slider handler has run first.
function holdSpeaker() {
	setTimeout(() => card() && slider() && card().setVolume(speakerVolume(k.step.on, Number(slider().value))), 0);
}

function codecMaster() {
	const codec = card() && card().codec && card().codec();
	return codec ? codec.master.raw : null;
}

// The guest's own volume control, read from the card's registers while Keep in step is on.
function stepTick() {
	const word = codecMaster();
	if (word === null || word === k.lastCodec) return;
	k.lastCodec = word;
	applyStep(codecWrite(k.step, word, performance.now()));
}

function onSlider() {
	if (!k || k.mute) return;
	applyStep(sliderMoved(k.step, Number(slider().value), performance.now()));
	if (k.step.on) holdSpeaker();
	k.lastHost = null;
	draw();
}

function setStep(on) {
	applyStep(on ? switchedOn(k.step, Number(slider().value), performance.now()) : switchedOff(k.step));
	if (on && k.stepTimer === null) k.stepTimer = setInterval(stepTick, 250);
	if (!on && k.stepTimer !== null) (clearInterval(k.stepTimer), (k.stepTimer = null));
	holdSpeaker();
	draw();
}

function startTest() {
	if (!card() || !peer("audio", "audio-play")) return;
	const before = card().stats().counters.dmaFrames;
	k.test = { before, last: before, stable: performance.now(), started: performance.now(), played: null, timer: setInterval(watchTest, 250) };
	setNote("test", T.testing);
	send("audio", "play", playFields("test"));
}

function endTest(text) {
	if (!k.test) return;
	clearInterval(k.test.timer);
	k.test = null;
	setNote("test", text);
}

// The tone's frames at the card, until the count has stood still for 1.5 s.
function watchTest() {
	const test = k && k.test;
	if (!test || !card()) return;
	const now = performance.now();
	const frames = card().stats().counters.dmaFrames;
	if (frames !== test.last) ((test.last = frames), (test.stable = now));
	if ((test.played && now - test.stable > 1500) || now - test.started > 40000) {
		const total = frames - test.before;
		endTest(total > 0 ? fill(T.testCounted, { n: total.toLocaleString("en-CA") }) : T.testSilent);
	}
}

// `audio host`: the page's output, microphone and volume, sent when they change.
function sendHost() {
	if (!peer("audio") || !card()) return;
	const s = card().stats();
	const host = { output: hostOutput(card().outputState(), s.muted), mic: s.mic === "on" ? "on" : "unavailable", volume: Math.round(slider() ? Number(slider().value) : s.volume * 100) };
	const text = JSON.stringify(host);
	if (text !== k.lastHost) {
		k.lastHost = text;
		send("audio", "host", hostFields(host));
	}
}

// `audio stats`, twice a second at most, only while the guest's sound card window is open (15 s after its last `open`).
function watchTick() {
	const w = k.watch;
	const now = performance.now();
	if (now - w.at > 15000 || !peer("audio")) {
		w.open = false;
		clearInterval(w.timer);
		w.timer = null;
		return;
	}
	sendHost();
	const s = card() && card().stats();
	if (s && s.counters && now - w.last >= STATS_MS) {
		w.last = now;
		card().sinkStats().then((sink) => send("audio", "stats", statsFields({ frames: s.counters.dmaFrames, underruns: sink ? sink.underrunEvents : 0, rate: s.guestRate })), () => {});
	}
}

// ---- the system and the log: asked only while the panel is open ----

function onToggle() {
	if (!k) return;
	const open = diagOpen();
	if (open && peer("sys")) {
		k.log.open();
		k.poll.open();
	} else {
		k.poll.close();
		k.log.close();
		k.sys = null;
	}
	draw();
}

const askSys = () => k && diagOpen() && peer("sys") && k.host.send(...SYS_GET);

// ---- power ----

// ReactOS in v86 accepts ExitWindowsEx, then waits for a byte on COM1: a harmless `ctl resume` goes every 2 s, up to 8 times. power.js reports the end.
function nudge(count) {
	if (count >= 8 || !k) return;
	k.undo.push(
		setTimeout(() => {
			if (!k || k.off) return;
			k.host.send("ctl", "resume", [["channel", "file"]]);
			nudge(count + 1);
		}, 2000),
	);
}

function shutDown() {
	askConfirm(false);
	if (!peer("power", "power-shutdown")) return;
	k.pending = { id: send(...powerRequest("shutdown")) };
	setNote("power", T.shutdownAsking);
	nudge(0);
}

function askConfirm(show) {
	k.ui.confirm.hidden = !show;
	k.ui.shutdown.hidden = show;
	if (show) k.ui.yes.focus();
}

// ---- the kit CD ----

async function insertCd() {
	setNote("cd", T.inserting);
	try {
		const response = await fetch(KIT_ISO);
		if (!response.ok) throw new Error(response.status + " " + response.statusText);
		await k.emulator.set_cdrom({ buffer: await response.arrayBuffer() });
		setNote("cd", T.inserted);
	} catch (error) {
		setNote("cd", fill(T.insertFailed, { why: error.message }));
	}
}

function ejectCd() {
	k.emulator.eject_cdrom();
	setNote("cd", T.ejected);
}

// ---- drawing ----

const setNote = (key, text) => k && (k.ui.note[key].textContent = text);

function draw() {
	if (!k) return;
	const { ui } = k;
	const now = performance.now();
	const codec = card() && card().codec && card().codec();
	if (card()) k.tracker.observe(card().stats(), now);
	// The badge is a button when the helper can set the volume. The page's own badge is used when it has one.
	const set = peer("audio", "audio-set");
	const badge = $("[data-sound-badge]") || ui.badge;
	if (badge === ui.badge) badge.hidden = !badgeVisible(codec, k.tracker.recent(now));
	badge.classList.toggle("kit-badge--button", set);
	badge.setAttribute("role", set ? "button" : "status");
	badge.tabIndex = set ? 0 : -1;
	ui.shutdown.disabled = k.off || !peer("power", "power-shutdown");
	ui.shutdown.title = ui.shutdown.disabled && !k.off ? T.shutdownNeeds : "";
	ui.insert.disabled = ui.eject.disabled = k.off;
	if (!diagOpen()) return;
	ui.pc.textContent = describePc(k.pc.devices, k.pc.mixer);
	const usable = Boolean(card() && slider() && set && k.pc.mixer && k.pc.mixer.lines > 0);
	ui.step.disabled = !usable;
	if (!usable && k.step.on) ((ui.step.checked = false), setStep(false));
	ui.note.step.textContent = usable ? (k.step.on ? T.stepOn : "") : !set ? T.stepNoHelper : k.pc.mixer ? T.stepNoMixer : "";
	ui.test.disabled = !(card() && peer("audio", "audio-play"));
	if (!k.test && ui.test.disabled) ui.note.test.textContent = T.testNeeds;
	ui.system.textContent = k.sys ? systemLines(k.sys).join("; ") : peer("sys") || !k.host.stats().peer ? T.waiting : T.noSys;
	ui.log.textContent = k.log.text(k.level) || T.noLog;
	ui.log.scrollTop = ui.log.scrollHeight;
	for (const group of GROUPS) ui.boxes[group.key].checked = k.board.switches[group.key];
}

function build() {
	const ui = k.ui;
	const btn = (text, cls = "button button--alt") => el("button", { type: "button", className: cls, textContent: text });
	const note = (props = {}) => el("span", { className: "kit-note", role: "status", ...props });
	ui.note = { power: note(), cd: note(), step: note({ role: null }), test: note() };
	ui.badge = el("button", { type: "button", className: "kit-badge", textContent: T.badge, title: T.badgeTitle, hidden: true });
	ui.shutdown = btn(T.shutdown);
	ui.yes = btn(T.shutdownYes, "button");
	const no = btn(T.shutdownNo);
	ui.confirm = el("span", { className: "kit-confirm", hidden: true }, T.shutdownConfirm + " ", ui.yes, no);
	ui.insert = btn(T.insert);
	ui.eject = btn(T.eject);
	const bar = $("[data-kit-bar]");
	bar.replaceChildren(ui.badge, ui.shutdown, ui.confirm, ui.note.power, ui.insert, ui.eject, ui.note.cd);
	bar.hidden = false;
	ui.pc = el("dd");
	ui.system = el("dd");
	ui.log = el("pre", { className: "kit-log", tabIndex: 0 });
	const level = el("select", { ariaLabel: T.level });
	for (const [key, text] of Object.entries(T.levels)) level.add(new Option(text, key));
	const copy = btn(T.copy);
	const copied = note();
	ui.step = el("input", { type: "checkbox", disabled: true });
	ui.test = btn(T.test);
	ui.test.disabled = true;
	ui.boxes = {};
	const switches = el("p", { className: "kit-switches" }, el("strong", { textContent: T.switches + ": " }));
	for (const group of GROUPS) {
		const box = (ui.boxes[group.key] = el("input", { type: "checkbox", checked: true }));
		switches.append(el("label", {}, box, " " + T.groups[group.key] + " "));
		box.addEventListener("change", () => {
			k.host.setCaps(k.board.set(group.key, box.checked));
			if (group.key === "sys") onToggle();
			draw();
		});
	}
	const rows = el("dl", { className: "diag-readout" }, el("dt", { textContent: T.pcSound }), ui.pc, el("dt", { textContent: T.system }), ui.system, el("dt", { textContent: T.log }), el("dd", {}, ui.log, level, copy, copied));
	ui.diag = $("[data-kit-diag]");
	ui.diag.replaceChildren(rows, switches, el("p", {}, el("label", {}, ui.step, " " + T.step + " "), ui.note.step), el("p", {}, ui.test, ui.note.test));
	ui.diag.hidden = false;
	ui.yes.addEventListener("click", shutDown);
	no.addEventListener("click", () => askConfirm(false));
	ui.shutdown.addEventListener("click", () => askConfirm(true));
	ui.insert.addEventListener("click", insertCd);
	ui.eject.addEventListener("click", ejectCd);
	ui.step.addEventListener("change", () => setStep(ui.step.checked));
	ui.test.addEventListener("click", startTest);
	level.addEventListener("change", () => {
		k.level = level.value;
		send("log", "level", logLevelFields(level.value));
		draw();
	});
	copy.addEventListener("click", async () => {
		const text = k.log.text(k.level);
		try {
			await navigator.clipboard.writeText(text);
			copied.textContent = fill(T.copied, { n: text ? text.split("\n").length : 0 });
		} catch {
			copied.textContent = T.copyFailed;
		}
	});
	const onBadge = () => peer("audio", "audio-set") && send("audio", "set", setFields(unmuteRequest(card() && card().codec())));
	for (const node of [ui.badge, $("[data-sound-badge]")]) {
		if (!node) continue;
		node.addEventListener("click", onBadge);
		node.addEventListener("keydown", (e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), onBadge()));
	}
}

// ---- attach and detach ----

/** Joins the glue to a machine: `host` is the link's handle. A second call replaces the first. */
export function attach(host, emulator) {
	detach();
	k = fresh(host, emulator);
	build();
	k.poll = createPoller(askSys, { setInterval: (...a) => setInterval(...a), clearInterval: (...a) => clearInterval(...a) });
	const diag = $("[data-diag]");
	diag.addEventListener("toggle", onToggle);
	if (slider()) slider().addEventListener("input", onSlider);
	host.on("message", onMessage);
	emulator.add_listener("emulator-stopped", () => k && k.host === host && ((k.off = true), setNote("power", T.off), draw()));
	k.undo.push(() => diag.removeEventListener("toggle", onToggle));
	k.undo.push(() => slider() && slider().removeEventListener("input", onSlider));
	// The card is attached just after the link, so the page's list is made a tick later.
	queueMicrotask(() => {
		if (!k || k.host !== host) return;
		const base = host.link.caps.split(",").filter(Boolean);
		if (card() && !base.includes("audio")) base.push("audio", "audio-set", "audio-play");
		k.board = switchBoard(base);
		host.setCaps(k.board.tokens());
		onToggle();
	});
	if (window.tryReactos) window.tryReactos.kitLink = { state: () => k };
}

export function detach() {
	if (!k) return;
	for (const item of k.undo) typeof item === "function" ? item() : clearTimeout(item);
	if (k.poll) k.poll.close();
	if (k.stepTimer !== null) clearInterval(k.stepTimer);
	if (k.watch.timer !== null) clearInterval(k.watch.timer);
	if (k.test) clearInterval(k.test.timer);
	const bar = $("[data-kit-bar]");
	const diag = $("[data-kit-diag]");
	if (bar) ((bar.hidden = true), bar.replaceChildren());
	if (diag) ((diag.hidden = true), diag.replaceChildren());
	k = null;
	if (window.tryReactos) window.tryReactos.kitLink = null;
}
