// A small RTF to HTML reader and an HTML to RTF writer, for the clipboard. Both rebuild their output from what they understood (bold,
// italic, underline, paragraphs, line breaks, tabs, characters): nothing is passed through, so no script, style sheet or link survives.
// No DOM, so the same code runs in the tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The Windows-1252 characters for bytes 0x80 to 0x9f. */
const CP1252_HIGH = "€\u0081‚ƒ„…†‡ˆ‰Š‹Œ\u008dŽ\u008f\u0090‘’“”•–—˜™š›œ\u009džŸ";

function cp1252(byte: number): string {
	return byte >= 0x80 && byte < 0xa0 ? CP1252_HIGH[byte - 0x80]! : String.fromCharCode(byte);
}

/** Groups whose contents are not text for the reader. */
const SKIPPED = new Set(["fonttbl", "colortbl", "stylesheet", "info", "pict", "header", "footer", "headerl", "headerr", "footerl", "footerr", "object", "listtable", "listoverridetable", "rsidtbl", "generator", "xmlnstbl", "themedata", "colorschememapping", "datastore", "latentstyles", "fldinst", "private", "mmathPr"]);

export function escapeHtml(text: string): string {
	return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

interface Frame {
	skip: boolean;
	bold: boolean;
	italic: boolean;
	underline: boolean;
	unicodeSkip: number;
}

/** Reads RTF (bytes or text) into a small HTML fragment: `<p>` paragraphs holding `<b>`, `<i>` and `<u>` runs. */
export function rtfToHtml(rtf: Uint8Array | string): string {
	const source = typeof rtf === "string" ? rtf : Array.from(rtf, (b) => String.fromCharCode(b)).join("");
	if (!/^\s*\{\\rtf/.test(source)) return "";
	const paragraphs: string[] = [];
	let current = "";
	let frame: Frame = { skip: false, bold: false, italic: false, underline: false, unicodeSkip: 1 };
	const stack: Frame[] = [];
	let pendingSkip = 0;
	// Adjacent characters of one look make one run (`<b>bold</b>`, not a tag for each letter).
	let run = "";
	let runLook = "";
	const flush = (): void => {
		if (run === "") return;
		let html = escapeHtml(run);
		if (runLook[2] === "u") html = "<u>" + html + "</u>";
		if (runLook[1] === "i") html = "<i>" + html + "</i>";
		if (runLook[0] === "b") html = "<b>" + html + "</b>";
		current += html;
		run = "";
	};
	const text = (chars: string): void => {
		if (frame.skip || chars === "") return;
		const look = (frame.bold ? "b" : "-") + (frame.italic ? "i" : "-") + (frame.underline ? "u" : "-");
		if (look !== runLook) flush();
		runLook = look;
		run += chars;
	};
	const endParagraph = (): void => {
		if (frame.skip) return;
		flush();
		paragraphs.push(current);
		current = "";
	};
	let at = 0;
	const n = source.length;
	while (at < n) {
		const ch = source[at]!;
		if (ch === "{") {
			stack.push({ ...frame });
			at++;
		} else if (ch === "}") {
			frame = stack.pop() ?? frame;
			at++;
		} else if (ch === "\\") {
			const next = source[at + 1] ?? "";
			if (next === "\\" || next === "{" || next === "}") {
				if (pendingSkip > 0) pendingSkip--;
				else text(next);
				at += 2;
			} else if (next === "'") {
				const byte = parseInt(source.slice(at + 2, at + 4), 16);
				if (pendingSkip > 0) pendingSkip--;
				else if (!Number.isNaN(byte)) text(cp1252(byte));
				at += 4;
			} else if (next === "~") {
				text(" ");
				at += 2;
			} else if (next === "-") at += 2;
			else if (next === "_") {
				text("‑");
				at += 2;
			} else if (next === "*") {
				// An optional destination: skipped unless a later control word names a group the reader knows (none is), so skip the group.
				frame.skip = true;
				at += 2;
			} else if (/[a-zA-Z]/.test(next)) {
				let end = at + 1;
				while (end < n && /[a-zA-Z]/.test(source[end]!)) end++;
				const word = source.slice(at + 1, end);
				let sign = 1;
				if (source[end] === "-") {
					sign = -1;
					end++;
				}
				let digits = "";
				while (end < n && /[0-9]/.test(source[end]!)) digits += source[end++];
				const param = digits === "" ? undefined : sign * Number(digits);
				if (source[end] === " ") end++;
				at = end;
				switch (word) {
					case "par":
					case "sect":
					case "page":
						endParagraph();
						break;
					case "line":
						if (!frame.skip) {
							flush();
							current += "<br>";
						}
						break;
					case "tab":
						text("\t");
						break;
					case "emdash":
						text("—");
						break;
					case "endash":
						text("–");
						break;
					case "bullet":
						text("•");
						break;
					case "lquote":
						text("‘");
						break;
					case "rquote":
						text("’");
						break;
					case "ldblquote":
						text("“");
						break;
					case "rdblquote":
						text("”");
						break;
					case "b":
						frame.bold = param !== 0;
						break;
					case "i":
						frame.italic = param !== 0;
						break;
					case "ul":
						frame.underline = param !== 0;
						break;
					case "ulnone":
						frame.underline = false;
						break;
					case "uc":
						frame.unicodeSkip = param ?? 1;
						break;
					case "u": {
						if (param === undefined) break;
						if (pendingSkip === 0) text(String.fromCharCode(param < 0 ? param + 65536 : param));
						pendingSkip = frame.unicodeSkip;
						break;
					}
					default:
						if (SKIPPED.has(word)) frame.skip = true;
				}
			} else at += 2;
		} else if (ch === "\r" || ch === "\n") at++;
		else {
			if (pendingSkip > 0) pendingSkip--;
			else text(ch);
			at++;
		}
	}
	flush();
	if (current !== "") paragraphs.push(current);
	return paragraphs.map((p) => "<p>" + p + "</p>").join("");
}

const NAMED: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " ", copy: "©", reg: "®", ndash: "–", mdash: "—", hellip: "…", lsquo: "‘", rsquo: "’", ldquo: "“", rdquo: "”", euro: "€", bull: "•" };

function decodeEntities(text: string): string {
	return text.replace(/&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);/g, (whole, name: string) => {
		if (name[0] === "#") {
			const code = name[1] === "x" || name[1] === "X" ? parseInt(name.slice(2), 16) : parseInt(name.slice(1), 10);
			return code > 0 && code <= 0x10ffff ? String.fromCodePoint(code) : "";
		}
		return NAMED[name] ?? whole;
	});
}

function escapeRtf(text: string): string {
	let out = "";
	for (const ch of text) {
		const code = ch.codePointAt(0)!;
		if (ch === "\\" || ch === "{" || ch === "}") out += "\\" + ch;
		else if (ch === "\t") out += "\\tab ";
		else if (code === 10 || code === 13) continue;
		else if (code < 0x20) continue;
		else if (code < 0x80) out += ch;
		else if (code > 0xffff) {
			const hi = 0xd800 + ((code - 0x10000) >> 10);
			const lo = 0xdc00 + ((code - 0x10000) & 0x3ff);
			out += "\\u" + (hi - 65536) + "?\\u" + (lo - 65536) + "?";
		} else out += "\\u" + (code > 32767 ? code - 65536 : code) + "?";
	}
	return out;
}

const BLOCK = new Set(["p", "div", "li", "tr", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "pre", "ul", "ol", "table", "section", "article", "header", "footer"]);
const DROPPED = new Set(["script", "style", "head", "title", "template", "noscript", "svg", "iframe", "object", "embed"]);

/** Writes the text, bold, italic, underline, line breaks and paragraphs of an HTML fragment as RTF. Images, links and styles are dropped. */
export function htmlToRtf(html: string): string {
	const body = /<body[^>]*>([\s\S]*)<\/body>/i.exec(html)?.[1] ?? html;
	let out = "{\\rtf1\\ansi\\ansicpg1252\\deff0{\\fonttbl{\\f0\\fswiss Arial;}}\\f0\\fs22 ";
	const open: string[] = [];
	const dropDepth: string[] = [];
	const tag = /<(\/?)([a-zA-Z][a-zA-Z0-9]*)\b[^>]*?(\/?)>|<!--[\s\S]*?-->|([^<]+|<)/g;
	let pendingBreak = false;
	let wrote = false;
	for (let m = tag.exec(body); m !== null; m = tag.exec(body)) {
		if (m[4] !== undefined) {
			if (dropDepth.length > 0) continue;
			const raw = decodeEntities(m[4]).replace(/\s+/g, " ");
			if (raw === "" || (raw === " " && (!wrote || pendingBreak))) continue;
			const chunk = pendingBreak || !wrote ? raw.replace(/^ /, "") : raw;
			if (chunk === "") continue;
			if (pendingBreak) {
				out += "\\par ";
				pendingBreak = false;
			}
			out += escapeRtf(chunk);
			wrote = true;
			continue;
		}
		if (m[2] === undefined) continue;
		const name = m[2].toLowerCase();
		const closing = m[1] === "/";
		const selfClosing = m[3] === "/" || name === "br" || name === "img" || name === "hr" || name === "input" || name === "meta" || name === "link";
		if (DROPPED.has(name)) {
			if (closing) {
				const at = dropDepth.lastIndexOf(name);
				if (at >= 0) dropDepth.length = at;
			} else if (!selfClosing) dropDepth.push(name);
			continue;
		}
		if (dropDepth.length > 0) continue;
		if (name === "br") {
			out += "\\line ";
			wrote = true;
			continue;
		}
		if (BLOCK.has(name)) {
			if (wrote) pendingBreak = true;
			if (name === "li" && !closing) {
				if (pendingBreak) {
					out += "\\par ";
					pendingBreak = false;
				}
				out += "\\bullet\\tab ";
				wrote = true;
			}
			continue;
		}
		const style = name === "b" || name === "strong" ? "\\b " : name === "i" || name === "em" ? "\\i " : name === "u" ? "\\ul " : "";
		if (!style) continue;
		if (closing) {
			const at = open.lastIndexOf(name);
			if (at >= 0) {
				open.length = at;
				out += "}";
			}
		} else if (!selfClosing) {
			if (pendingBreak) {
				out += "\\par ";
				pendingBreak = false;
			}
			open.push(name);
			out += "{" + style;
		}
	}
	out += "}".repeat(open.length) + "}";
	return out;
}
