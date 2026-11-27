#!/usr/bin/env python3
"""Tests for tools/stage-web.py and tools/stamp-web.py: staged names follow content and nothing else, a second run is identical,
--check fails on a changed byte, and the page's ?v= stamps are current (no network).

(c) Copyright 2026 Liminal HQ, Scott Morris
SPDX-License-Identifier: Apache-2.0 OR MIT

Run: python3 -I -m unittest discover -s site/try-reactos/guest -p 'test_stage_web.py'
"""
import hashlib
import importlib.util
import json
import os
import shutil
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
TRY = os.path.normpath(os.path.join(HERE, ".."))


def load(name):
	spec = importlib.util.spec_from_file_location(name.replace("-", "_"), os.path.join(TRY, "tools", name + ".py"))
	module = importlib.util.module_from_spec(spec)
	spec.loader.exec_module(module)
	return module


stage_web = load("stage-web")
stamp_web = load("stamp-web")


def write(path, data):
	os.makedirs(os.path.dirname(path), exist_ok=True)
	with open(path, "wb") as f:
		f.write(data)


def make_dist(root, image_sha="ab" * 32, state=b"state-bytes", part=b"part-bytes"):
	dist = os.path.join(root, "dist")
	write(os.path.join(dist, "parts", "0-8388608.img.zst"), part)
	write(os.path.join(dist, "parts", "8388608-16777216.img.zst"), b"zero")
	write(os.path.join(dist, "parts", "manifest.json"), json.dumps({"size": 16777216, "chunk": 8388608, "sha256": image_sha}).encode())
	write(os.path.join(dist, "state.l22.bin.zst"), state)
	write(os.path.join(dist, "build-manifest.json"), json.dumps({"built": "2026-01-02T03:04:05Z", "pinned": {"extra": {"quill_commit": "q" * 40, "reactos_liminal_commit": "r" * 40}}}).encode())
	return dist


def card_state(**changes):
	"""A sound card's saved registers, in the shape packages/ac97 saves: 128 codec words and three channels."""
	state = {"version": 1, "nam": [0] * 128, "globCnt": 0, "ch": [{"bdbar": 0, "civ": 0, "lvi": 0, "sr": 1, "picb": 0, "piv": 0, "cr": 0} for _ in range(3)]}
	state.update(changes)
	return state


def make_v86(root, js=b"js", wasm=b"wasm"):
	v86 = os.path.join(root, "v86")
	write(os.path.join(v86, "libv86.mjs"), js)
	write(os.path.join(v86, "v86.wasm"), wasm)
	return v86


class StageWeb(unittest.TestCase):
	def setUp(self):
		self.tmp = tempfile.mkdtemp()
		self.addCleanup(shutil.rmtree, self.tmp, True)
		self.out = os.path.join(self.tmp, "page")
		os.makedirs(os.path.join(self.out, "data"))
		self.v86 = make_v86(self.tmp)

	def stage(self, **kw):
		return stage_web.stage(make_dist(self.tmp, **kw), self.out, self.v86)

	def listing(self):
		found = {}
		for base, _dirs, names in os.walk(self.out):
			for n in names:
				p = os.path.join(base, n)
				found[os.path.relpath(p, self.out)] = stage_web.sha256_file(p)
		return found

	def test_names_follow_content(self):
		m = self.stage()
		self.assertEqual(m["parts"], "data/parts-" + "ab" * 6 + "/")
		self.assertEqual(m["state"], "data/state-" + hashlib.sha256(b"state-bytes").hexdigest()[:12] + ".l22.bin.zst")
		self.assertTrue(os.path.isfile(os.path.join(self.out, m["state"])))
		self.assertTrue(os.path.isfile(os.path.join(self.out, m["parts"], "manifest.json")))
		self.assertEqual(m["v86"]["libv86"], hashlib.sha256(b"js").hexdigest()[:8])
		self.assertEqual(m["built"], "2026-01-02T03:04:05Z")
		self.assertEqual(m["quill"], "q" * 40)

	def test_name_depends_on_content_only(self):
		first = self.stage(state=b"one")["state"]
		second = self.stage(state=b"two")["state"]
		self.assertNotEqual(first, second)
		self.assertEqual(self.stage(state=b"one")["state"], first)

	def test_second_run_is_identical(self):
		self.stage()
		before = self.listing()
		self.stage()
		self.assertEqual(self.listing(), before)

	def test_only_what_the_manifest_names_stays(self):
		old = self.stage(state=b"one", image_sha="cd" * 32)
		new = self.stage(state=b"two")
		self.assertFalse(os.path.exists(os.path.join(self.out, old["state"])))
		self.assertFalse(os.path.exists(os.path.join(self.out, old["parts"])))
		self.assertTrue(os.path.exists(os.path.join(self.out, new["state"])))

	def test_check_passes_then_fails_on_a_changed_byte(self):
		m = self.stage()
		self.assertEqual(stage_web.check(self.out, self.v86), [])
		part = os.path.join(self.out, m["parts"], "0-8388608.img.zst")
		with open(part, "r+b") as f:
			f.write(b"X")
		self.assertTrue(any("parts" in p for p in stage_web.check(self.out, self.v86)))
		write(part, b"part-bytes")
		self.assertEqual(stage_web.check(self.out, self.v86), [])
		with open(os.path.join(self.out, m["state"]), "r+b") as f:
			f.write(b"X")
		self.assertTrue(any("state" in p for p in stage_web.check(self.out, self.v86)))

	def test_check_fails_when_the_emulator_differs(self):
		self.stage()
		make_v86(self.tmp, js=b"another build")
		self.assertTrue(any("libv86" in p for p in stage_web.check(self.out, self.v86)))


class StageAudio(unittest.TestCase):
	"""The state's sound card (SITE-43): its registers ride in the manifest's audio object, and only when the state was built with the card."""

	def setUp(self):
		self.tmp = tempfile.mkdtemp()
		self.addCleanup(shutil.rmtree, self.tmp, True)
		self.out = os.path.join(self.tmp, "page")
		os.makedirs(os.path.join(self.out, "data"))
		self.v86 = make_v86(self.tmp)

	def stage(self):
		return stage_web.stage(make_dist(self.tmp), self.out, self.v86)

	def listing(self):
		found = {}
		for base, _dirs, names in os.walk(self.out):
			for n in names:
				p = os.path.join(base, n)
				found[os.path.relpath(p, self.out)] = stage_web.sha256_file(p)
		return found

	def stage_with_audio(self, audio, **kw):
		dist = make_dist(self.tmp, **kw)
		write(os.path.join(dist, "state.audio.json"), json.dumps(audio).encode())
		return stage_web.stage(dist, self.out, self.v86)

	def test_a_state_without_the_card_has_no_audio_object(self):
		self.assertNotIn("audio", self.stage())

	def test_the_registers_go_into_the_manifest(self):
		audio = card_state(globCnt=7)
		m = self.stage_with_audio(audio)
		self.assertEqual(m["audio"], audio)
		with open(os.path.join(self.out, "web-manifest.json"), encoding="utf-8") as f:
			self.assertEqual(json.load(f)["audio"], audio)
		self.assertEqual(stage_web.check(self.out, self.v86), [])

	def test_a_second_run_with_the_registers_is_identical(self):
		self.stage_with_audio(card_state())
		before = self.listing()
		self.stage_with_audio(card_state())
		self.assertEqual(self.listing(), before)

	def test_registers_of_another_shape_are_refused(self):
		for bad in (card_state(version=2), card_state(nam=[0] * 127), card_state(nam=[70000] + [0] * 127), card_state(ch=[]), card_state(globCnt="x"), []):
			with self.assertRaises(SystemExit):
				self.stage_with_audio(bad)

	def test_check_fails_on_a_manifest_whose_audio_is_broken(self):
		self.stage_with_audio(card_state())
		path = os.path.join(self.out, "web-manifest.json")
		with open(path, encoding="utf-8") as f:
			manifest = json.load(f)
		manifest["audio"]["nam"] = [1]
		with open(path, "w", encoding="utf-8") as f:
			json.dump(manifest, f)
		self.assertTrue(any("audio" in p for p in stage_web.check(self.out, self.v86)))


class StampWeb(unittest.TestCase):
	def test_the_page_versions_are_current(self):
		self.assertEqual(stamp_web.stamp_all(TRY, True), [])

	def test_a_stale_version_is_found_and_fixed(self):
		tmp = tempfile.mkdtemp()
		self.addCleanup(shutil.rmtree, tmp, True)
		write(os.path.join(tmp, "a.js"), b"export const a = 1;\n")
		write(os.path.join(tmp, "try-reactos.js"), b'import { a } from "./a.js?v=00000000";\n')
		write(os.path.join(tmp, "index.html"), b'<script src="try-reactos.js?v=00000000"></script>\n')
		self.assertEqual(len(stamp_web.stamp_all(tmp, True)), 2)
		stamp_web.stamp_all(tmp, False)
		self.assertEqual(stamp_web.stamp_all(tmp, True), [])


if __name__ == "__main__":
	unittest.main()
