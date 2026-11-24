// Quill's product page: the theme toggle and the screenshot lightbox. The page works without it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

(function () {
	"use strict";

	var KEY = "quill-theme";
	var root = document.documentElement;

	function stored() {
		try {
			return localStorage.getItem(KEY);
		} catch (e) {
			return null;
		}
	}

	function store(value) {
		try {
			localStorage.setItem(KEY, value);
		} catch (e) {
			// Private windows and blocked storage: the choice lasts for this page only.
		}
	}

	function systemDark() {
		return window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches;
	}

	function current() {
		var set = root.getAttribute("data-theme");
		if (set === "light" || set === "dark") return set;
		return systemDark() ? "dark" : "light";
	}

	// The theme toggle: shows the theme it will switch to.
	var toggle = document.querySelector("[data-theme-toggle]");
	function label() {
		if (!toggle) return;
		var next = current() === "dark" ? "light" : "dark";
		toggle.setAttribute("aria-label", "Switch to the " + next + " theme");
		toggle.querySelector("[data-theme-label]").textContent = next === "dark" ? "Dark" : "Light";
		toggle.querySelector("[data-icon-moon]").hidden = next !== "dark";
		toggle.querySelector("[data-icon-sun]").hidden = next !== "light";
	}
	if (toggle) {
		toggle.hidden = false;
		label();
		toggle.addEventListener("click", function () {
			var next = current() === "dark" ? "light" : "dark";
			root.setAttribute("data-theme", next);
			store(next);
			label();
		});
		if (window.matchMedia) {
			var mq = window.matchMedia("(prefers-color-scheme: dark)");
			var onChange = function () {
				if (!stored()) label();
			};
			if (mq.addEventListener) mq.addEventListener("change", onChange);
			else if (mq.addListener) mq.addListener(onChange);
		}
	}

	// The lightbox: every screenshot link opens its full-size image in a modal dialog with the caption; arrows step through
	// the group, Escape closes, and focus returns to the screenshot that opened it. Without the dialog element the links
	// simply open the image.
	var dialog = document.querySelector("[data-lightbox]");
	if (!dialog || typeof dialog.showModal !== "function") return;

	var img = dialog.querySelector("[data-lightbox-img]");
	var caption = dialog.querySelector("[data-lightbox-caption]");
	var count = dialog.querySelector("[data-lightbox-count]");
	var prev = dialog.querySelector("[data-lightbox-prev]");
	var next = dialog.querySelector("[data-lightbox-next]");
	var close = dialog.querySelector("[data-lightbox-close]");
	var items = [];
	var index = 0;
	var opener = null;

	function show(i) {
		index = (i + items.length) % items.length;
		var link = items[index];
		var thumb = link.querySelector("img");
		var fig = link.closest("figure");
		var cap = fig ? fig.querySelector("figcaption") : null;
		img.src = link.getAttribute("href");
		img.alt = thumb ? thumb.alt : "";
		if (thumb && thumb.getAttribute("width")) {
			img.width = Number(thumb.getAttribute("width"));
			img.height = Number(thumb.getAttribute("height"));
		}
		caption.innerHTML = cap ? cap.innerHTML : "";
		count.textContent = items.length > 1 ? index + 1 + " of " + items.length : "";
		prev.hidden = next.hidden = items.length < 2;
	}

	document.addEventListener("click", function (event) {
		var link = event.target.closest("a[data-shot]");
		if (!link || event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) return;
		event.preventDefault();
		var group = link.getAttribute("data-shot");
		items = Array.prototype.slice.call(document.querySelectorAll('a[data-shot="' + group + '"]'));
		opener = link;
		show(items.indexOf(link));
		dialog.showModal();
		close.focus();
	});

	prev.addEventListener("click", function () {
		show(index - 1);
	});
	next.addEventListener("click", function () {
		show(index + 1);
	});
	close.addEventListener("click", function () {
		dialog.close();
	});
	dialog.addEventListener("keydown", function (event) {
		if (event.key === "ArrowLeft") {
			event.preventDefault();
			show(index - 1);
		} else if (event.key === "ArrowRight") {
			event.preventDefault();
			show(index + 1);
		}
	});
	// A click on the backdrop (outside the inner box) closes it.
	dialog.addEventListener("click", function (event) {
		if (event.target === dialog) dialog.close();
	});
	dialog.addEventListener("close", function () {
		img.removeAttribute("src");
		if (opener) opener.focus();
	});
})();
