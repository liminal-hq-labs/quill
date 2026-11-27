// disks.ts and index.ts over a fake command interface: what the page writes to D: and how the bridge reports it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from "bun:test";
import { attach, describe as describeState, type BridgeState, type FsInterface } from "../src/index.ts";
import { dateBatch, readC, writeD } from "../src/disks.ts";
import { listDir, openVolume, readFile } from "../src/fat.ts";
import { bridgeDisks, golden } from "./golden.ts";

const text = (b: Uint8Array): string => Array.from(b, (c) => String.fromCharCode(c)).join("");

function fakeFs(images: Record<string, Uint8Array>): FsInterface & { writes: string[]; images: Record<string, Uint8Array> } {
	return {
		images,
		writes: [],
		async fsReadFile(path) {
			const image = images[path];
			if (!image) throw new Error(`no ${path}`);
			return image.slice();
		},
		async fsWriteFile(path, contents) {
			this.writes.push(path);
			images[path] = contents;
		},
	};
}

describe("the date file", () => {
	test("is the visitor's local day in FreeDOS's US format", () => {
		expect(text(dateBatch(new Date(2026, 11, 25, 23, 59)))).toBe("@DATE 12-25-2026\r\n");
		expect(text(dateBatch(new Date(2027, 0, 3)))).toBe("@DATE 01-03-2027\r\n");
		expect(text(dateBatch(new Date(2026, 1, 28, 0, 0, 1)))).toBe("@DATE 02-28-2026\r\n");
	});
});

describe("writeD and readC", () => {
	test("write D: whole from its own image, and read C: as it is", async () => {
		const fs = fakeFs({ "d.img": golden("fat16.img.gz"), "c.img": golden("fat12.img.gz") });
		const report = await writeD(fs, [{ name: "SETDATE.BAT", bytes: dateBatch(new Date(2026, 11, 25)), time: new Date(2026, 11, 25) }]);
		expect(report.files).toBe(1);
		expect(fs.writes).toEqual(["d.img"]);
		expect(text(readFile(openVolume(fs.images["d.img"]!), "SETDATE.BAT"))).toBe("@DATE 12-25-2026\r\n");
		expect(listDir(await readC(fs)).some((e) => e.name === "FRAG.BIN")).toBe(true);
		// The page never writes C:.
		expect(fs.writes).not.toContain("c.img");
	});
});

describe("attach", () => {
	const day = new Date(2026, 11, 25, 9, 30);

	test("writes the date onto D:, once, and says so", async () => {
		const disks = bridgeDisks();
		const fs = fakeFs({ "d.img": disks ? disks.d : golden("fat16.img.gz"), "c.img": disks ? disks.c : golden("fat12.img.gz") });
		const seen: BridgeState[] = [];
		const bridge = attach(fs, { now: () => day, onChange: (s) => seen.push(s) });
		expect(bridge.state().status).toBe("starting");
		const state = await bridge.ready;
		expect(state.status).toBe("on");
		expect(state.date).toBe("12-25-2026");
		expect(fs.writes).toEqual(["d.img"]);
		expect(seen.map((s) => s.status)).toEqual(["on"]);
		expect(describeState(state)).toMatch(/^bridge: on, D: written \d+ ms, date 12-25-2026$/);
		expect(text(readFile(openVolume(fs.images["d.img"]!), "SETDATE.BAT"))).toBe("@DATE 12-25-2026\r\n");
	});

	test("a second write keeps one date file and adds the files given", async () => {
		const fs = fakeFs({ "d.img": golden("fat16.img.gz") });
		const bridge = attach(fs, { now: () => day });
		await bridge.ready;
		await bridge.write([
			{ name: "SETDATE.BAT", bytes: new Uint8Array(1), time: day },
			{ name: "HELLO.TXT", bytes: Uint8Array.from("hi", (c) => c.charCodeAt(0)), time: day },
		]);
		const v = openVolume(fs.images["d.img"]!);
		expect(listDir(v).map((e) => e.name)).toEqual(["SETDATE.BAT", "HELLO.TXT"]);
		expect(text(readFile(v, "SETDATE.BAT"))).toBe("@DATE 12-25-2026\r\n");
	});

	test("a failed write is reported and never throws", async () => {
		const fs = fakeFs({});
		const bridge = attach(fs, { now: () => day });
		const state = await bridge.ready;
		expect(state.status).toBe("failed");
		expect(state.error).toContain("no d.img");
		expect(describeState(state)).toBe("bridge: failed (no d.img)");
	});

	test("detach stops later writes and is safe twice", async () => {
		const fs = fakeFs({ "d.img": golden("fat16.img.gz") });
		const bridge = attach(fs, { now: () => day });
		await bridge.ready;
		bridge.detach();
		bridge.detach();
		const before = fs.writes.length;
		await bridge.write([]);
		expect(fs.writes.length).toBe(before);
		expect(bridge.state().status).toBe("detached");
		expect(describeState(bridge.state())).toBe("bridge: off");
	});

	test("detaching while the first write is under way leaves the image unwritten and the state detached", async () => {
		const fs = fakeFs({ "d.img": golden("fat16.img.gz") });
		const bridge = attach(fs, { now: () => day });
		bridge.detach();
		const state = await bridge.ready;
		expect(state.status).toBe("detached");
		expect(fs.writes).toEqual([]);
	});
});
