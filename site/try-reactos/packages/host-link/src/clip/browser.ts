// The only file that touches `navigator.clipboard`, paste events and the canvas. The state machine reaches the browser through `ClipboardPort`;
// the glue calls the readers below, and only from a paste event or a click on Paste into ReactOS. Nothing here makes a network request.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { dibToPixels, MAX_SIDE, pixelsToDib } from "./dib.ts";
import type { ClipboardPort, HostContent } from "./state.ts";

function canvasOf(width: number, height: number): { canvas: OffscreenCanvas | HTMLCanvasElement; context: OffscreenCanvasRenderingContext2D | CanvasRenderingContext2D } {
	const canvas = typeof OffscreenCanvas === "function" ? new OffscreenCanvas(width, height) : Object.assign(document.createElement("canvas"), { width, height });
	const context = canvas.getContext("2d", { willReadFrequently: true }) as OffscreenCanvasRenderingContext2D | CanvasRenderingContext2D | null;
	if (!context) throw new Error("no canvas");
	return { canvas, context };
}

async function blobOf(canvas: OffscreenCanvas | HTMLCanvasElement): Promise<Blob> {
	if ("convertToBlob" in canvas) return canvas.convertToBlob({ type: "image/png" });
	return new Promise((resolve, reject) => (canvas as HTMLCanvasElement).toBlob((b) => (b ? resolve(b) : reject(new Error("no PNG"))), "image/png"));
}

/** A packed DIB as a PNG. */
export async function dibToPng(dib: Uint8Array): Promise<Blob> {
	const { width, height, rgba } = dibToPixels(dib);
	const { canvas, context } = canvasOf(width, height);
	context.putImageData(new ImageData(rgba as Uint8ClampedArray<ArrayBuffer>, width, height), 0, 0);
	return blobOf(canvas);
}

/** A picture the browser can decode as a 24-bit packed DIB (transparency over white). Throws a `RangeError` over the pixel limit. */
export async function imageToDib(image: Blob): Promise<Uint8Array> {
	const bitmap = await createImageBitmap(image);
	try {
		if (bitmap.width > MAX_SIDE || bitmap.height > MAX_SIDE) throw new RangeError(`the picture is ${bitmap.width} by ${bitmap.height} pixels; the limit is ${MAX_SIDE} on a side`);
		const { context } = canvasOf(bitmap.width, bitmap.height);
		context.fillStyle = "#fff";
		context.fillRect(0, 0, bitmap.width, bitmap.height);
		context.drawImage(bitmap, 0, 0);
		const data = context.getImageData(0, 0, bitmap.width, bitmap.height);
		return pixelsToDib(bitmap.width, bitmap.height, data.data);
	} finally {
		bitmap.close();
	}
}

/** Last resort for a page with no async clipboard (an insecure address): a `copy` listener sets what the selection would copy. */
function legacyCopy(text: string, html: string): void {
	const box = document.createElement("textarea");
	box.value = text || " ";
	box.setAttribute("aria-hidden", "true");
	box.style.cssText = "position:fixed;left:-9999px;top:0;opacity:0";
	document.body.appendChild(box);
	const listener = (event: ClipboardEvent): void => {
		event.preventDefault();
		if (text) event.clipboardData?.setData("text/plain", text);
		if (html) event.clipboardData?.setData("text/html", html);
	};
	document.addEventListener("copy", listener, { once: true });
	try {
		box.select();
		if (!document.execCommand("copy")) throw new Error("the browser refused the copy");
	} finally {
		document.removeEventListener("copy", listener);
		box.remove();
	}
}

/**
 * The browser's clipboard as a port. A `ClipboardItem` takes promises for its formats, so the write can be made at the start of the visitor's
 * gesture while the guest's data is still arriving. `text/rtf` cannot be written by a web page (only `text/plain`, `text/html` and `image/png`
 * are allowed), so the guest's rich text goes out as `text/html` made from it, next to the plain text.
 */
export function createBrowserPort(): ClipboardPort {
	return {
		async write(item) {
			if (typeof ClipboardItem === "function" && navigator.clipboard && typeof navigator.clipboard.write === "function") {
				const entries: Record<string, Promise<Blob>> = {};
				if (item.text) entries["text/plain"] = item.text.then((t) => new Blob([t], { type: "text/plain" }));
				if (item.html) entries["text/html"] = item.html.then((h) => new Blob([h], { type: "text/html" }));
				if (item.png) entries["image/png"] = item.png;
				await navigator.clipboard.write([new ClipboardItem(entries)]);
				return;
			}
			if (item.png && !item.text && !item.html) throw new Error("this browser cannot copy a picture here");
			legacyCopy((await item.text) ?? "", (await item.html) ?? "");
		},
		dibToPng,
		imageToDib,
	};
}

/** What a `paste` event carries: plain text, HTML, RTF where the browser offers it, and the first picture. Reads nothing else. */
export function contentFromPaste(event: ClipboardEvent): HostContent {
	const data = event.clipboardData;
	const content: HostContent = {};
	if (!data) return content;
	const text = data.getData("text/plain");
	if (text) content.text = text;
	const html = data.getData("text/html");
	if (html) content.html = html;
	const rtf = data.getData("text/rtf");
	if (rtf) content.rtf = rtf;
	for (const file of Array.from(data.files ?? [])) {
		if (file.type.startsWith("image/")) {
			content.image = file;
			break;
		}
	}
	return content;
}

/** The async clipboard read behind the Paste into ReactOS button: the browser may ask the visitor. Throws when it is missing or refused. */
export async function contentFromRead(): Promise<HostContent> {
	if (!navigator.clipboard || typeof navigator.clipboard.read !== "function") throw new Error("this browser cannot read the clipboard from a button");
	const content: HostContent = {};
	for (const item of await navigator.clipboard.read()) {
		if (item.types.includes("text/plain") && content.text === undefined) content.text = await (await item.getType("text/plain")).text();
		if (item.types.includes("text/html") && content.html === undefined) content.html = await (await item.getType("text/html")).text();
		if (item.types.includes("text/rtf") && content.rtf === undefined) content.rtf = await (await item.getType("text/rtf")).text();
		const picture = item.types.find((t) => t.startsWith("image/"));
		if (picture && !content.image) content.image = await item.getType(picture);
	}
	return content;
}
