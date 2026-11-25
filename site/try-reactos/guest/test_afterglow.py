#!/usr/bin/env python3
"""Tests for the Afterglow pin, the fetch script and the Quillify overlay (no network: the fetch runs on a local tarball).

(c) Copyright 2026 Liminal HQ, Scott Morris
SPDX-License-Identifier: Apache-2.0 OR MIT

Run: python3 -I -m unittest discover -s site/try-reactos/guest -p 'test_afterglow.py'
The render test needs rsvg-convert and ImageMagick 7 and is skipped without them.
"""
import hashlib
import gzip
import io
import os
import shutil
import subprocess
import tarfile
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
import quillify  # noqa: E402  (the guest folder is the script's own directory)


def read_pin(path):
	out = {}
	with open(path, encoding='utf-8') as f:
		for line in f:
			line = line.strip()
			if line and not line.startswith('#'):
				k, _, v = line.partition('=')
				out[k] = v
	return out


def make_tar(members, bad_name=None):
	"""A gzip'd tar of {name: bytes} under one top folder, as GitHub serves a commit. Returns (gz bytes, sha256 of the tar)."""
	raw = io.BytesIO()
	with tarfile.open(fileobj=raw, mode='w') as t:
		for name, data in members.items():
			info = tarfile.TarInfo('top-abc/' + name)
			info.size = len(data)
			t.addfile(info, io.BytesIO(data))
		if bad_name:
			info = tarfile.TarInfo(bad_name)
			info.size = 1
			t.addfile(info, io.BytesIO(b'x'))
	tar = raw.getvalue()
	return gzip.compress(tar, mtime=0), hashlib.sha256(tar).hexdigest()


class PinTest(unittest.TestCase):
	def test_pin_is_complete_and_well_formed(self):
		pin = read_pin(os.path.join(HERE, 'afterglow.pin'))
		self.assertEqual(pin['AFTERGLOW_REPO'], 'liminal-hq/afterglow-themes')
		self.assertRegex(pin['AFTERGLOW_COMMIT'], r'^[0-9a-f]{40}$')
		self.assertRegex(pin['AFTERGLOW_TAR_SHA256'], r'^[0-9a-f]{64}$')
		self.assertRegex(pin['AFTERGLOW_MSSTYLES_SHA256'], r'^[0-9a-f]{64}$')
		self.assertTrue(pin['AFTERGLOW_WALLPAPER'].endswith('.svg'))
		self.assertTrue(pin['AFTERGLOW_APPLY_REG'].endswith('.reg'))


class FetchTest(unittest.TestCase):
	def setUp(self):
		self.tmp = tempfile.mkdtemp()
		self.addCleanup(shutil.rmtree, self.tmp)

	def fetch(self, gz, tar_sha):
		pin = os.path.join(self.tmp, 'test.pin')
		with open(pin, 'w') as f:
			f.write('AFTERGLOW_REPO=x/y\nAFTERGLOW_COMMIT=%s\nAFTERGLOW_TAR_SHA256=%s\n' % ('a' * 40, tar_sha))
		tarball = os.path.join(self.tmp, 'src.tar.gz')
		with open(tarball, 'wb') as f:
			f.write(gz)
		env = dict(os.environ, AFTERGLOW_PIN=pin, AFTERGLOW_TARBALL=tarball)
		cache = os.path.join(self.tmp, 'cache')
		r = subprocess.run(['bash', os.path.join(HERE, 'fetch-afterglow.sh'), cache], env=env, capture_output=True, text=True)
		return r, cache

	def test_good_tarball_is_extracted(self):
		gz, sha = make_tar({'desktop/style/build.sh': b'#!/bin/sh\n', 'README.md': b'hi'})
		r, cache = self.fetch(gz, sha)
		self.assertEqual(r.returncode, 0, r.stderr)
		dest = r.stdout.strip().splitlines()[-1]
		self.assertEqual(dest, os.path.join(cache, 'a' * 40))
		self.assertTrue(os.path.isfile(os.path.join(dest, 'desktop/style/build.sh')))

	def test_changed_tarball_is_refused(self):
		gz, sha = make_tar({'desktop/style/build.sh': b'x'})
		other, _ = make_tar({'desktop/style/build.sh': b'y'})
		r, cache = self.fetch(other, sha)
		self.assertNotEqual(r.returncode, 0)
		self.assertIn('expected', r.stderr)
		self.assertFalse(os.path.exists(os.path.join(cache, 'a' * 40)))

	def test_unsafe_member_is_refused(self):
		gz, sha = make_tar({'desktop/style/build.sh': b'x'}, bad_name='top-abc/../evil')
		r, _ = self.fetch(gz, sha)
		self.assertNotEqual(r.returncode, 0)
		self.assertIn('unsafe', r.stderr)

	def test_recompressed_archive_still_matches(self):
		"""The sum is of the tar, so gzip's level or header does not matter."""
		_, sha = make_tar({'desktop/style/build.sh': b'x'})
		raw = gzip.decompress(make_tar({'desktop/style/build.sh': b'x'})[0])
		r, _ = self.fetch(gzip.compress(raw, 1, mtime=12345), sha)
		self.assertEqual(r.returncode, 0, r.stderr)


class QuillifyTest(unittest.TestCase):
	def test_crop_and_sun_geometry(self):
		self.assertEqual((quillify.CROP_X, quillify.CROP_WIDTH), (480, 2880))
		x, y, r = quillify.sun_in_bmp()
		self.assertAlmostEqual(x, 2400 * 1024 / 2880)
		self.assertAlmostEqual(y, 1846 * 768 / 2160)
		self.assertAlmostEqual(r, 104 * 1024 / 2880)

	def test_find_sun_takes_the_gold_disc(self):
		svg = '<circle cx="2880" cy="1846" r="228.8" fill="#824934"/><circle cx="2880" cy="1846" r="104" fill="#e7ad27"/><circle cx="2380" cy="150" r="3.5"/>'
		self.assertEqual(quillify.find_sun(svg), (2880.0, 1846.0, 104.0))
		self.assertIsNone(quillify.find_sun('<circle cx="10" cy="10" r="5"/>'))

	def test_emblem_is_centred_on_the_sun(self):
		with open(os.path.join(HERE, 'quill-emblem.svg'), encoding='utf-8') as f:
			text = f.read()
		self.assertIn('translate(%d %d)' % quillify.SUN, text)

	@unittest.skipUnless(shutil.which('rsvg-convert') and shutil.which('magick'), 'needs rsvg-convert and ImageMagick')
	def test_render_puts_the_ring_round_the_sun(self):
		tmp = tempfile.mkdtemp()
		self.addCleanup(shutil.rmtree, tmp)
		wall = os.path.join(tmp, 'w.svg')
		with open(wall, 'w') as f:
			f.write('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 3840 2160" width="3840" height="2160">'
				'<rect width="3840" height="2160" fill="#000000"/><circle cx="2880" cy="1846" r="104" fill="#0000ff"/></svg>')
		out = os.path.join(tmp, 'o.bmp')
		quillify.quillify(wall, os.path.join(HERE, 'quill-emblem.svg'), out)
		with open(out, 'rb') as f:
			head = f.read(54)
		self.assertEqual(head[:2], b'BM')
		self.assertEqual(int.from_bytes(head[18:22], 'little'), 1024)
		self.assertEqual(int.from_bytes(head[22:26], 'little'), 768)
		self.assertEqual(int.from_bytes(head[28:30], 'little'), 24)

		def px(x, y):
			r = subprocess.run(['magick', out, '-format', '%%[fx:int(255*p{%d,%d}.r)],%%[fx:int(255*p{%d,%d}.b)]' % (x, y, x, y), 'info:'],
				capture_output=True, text=True, check=True)
			return tuple(int(v) for v in r.stdout.split(','))
		sx, sy, _ = quillify.sun_in_bmp()
		red, blue = px(round(sx), round(sy) - 10)
		self.assertEqual((red, blue), (0, 255), 'the sun stays inside the ring')
		red, blue = px(round(sx) - 55, round(sy))
		self.assertGreater(red, 150, 'the gilt ring is on the sun')
		self.assertLess(blue, 120)
		self.assertEqual(px(20, 20), (0, 0))


@unittest.skipUnless(shutil.which('mcopy') and shutil.which('mformat') and shutil.which('mmd'), 'needs mtools')
class PointersTest(unittest.TestCase):
	"""prepare-pointers.sh on a tiny FAT disk and a fake Afterglow tree whose patch tool just writes known bytes."""

	PATCHED = b'patched-user32'

	def setUp(self):
		self.tmp = tempfile.mkdtemp()
		self.addCleanup(shutil.rmtree, self.tmp)
		self.env = dict(os.environ, MTOOLS_SKIP_CHECK='1')
		self.img = os.path.join(self.tmp, 'd.img')
		with open(self.img, 'wb') as f:
			f.truncate(8 * 1024 * 1024)
		d = self.img + '@@1048576'
		subprocess.run(['mformat', '-i', d, '-T', '14336', '-h', '1', '-s', '1', '::'], check=True, env=self.env, capture_output=True)
		subprocess.run(['mmd', '-i', d, '::/ReactOS', '::/ReactOS/system32'], check=True, env=self.env)
		dll = os.path.join(self.tmp, 'user32.dll')
		with open(dll, 'wb') as f:
			f.write(b'original-user32')
		subprocess.run(['mcopy', '-i', d, dll, '::/ReactOS/system32/user32.dll'], check=True, env=self.env)
		self.src = os.path.join(self.tmp, 'src')
		os.makedirs(os.path.join(self.src, 'desktop/pointers'))
		with open(os.path.join(self.src, 'desktop/pointers/patch-user32.py'), 'w') as f:
			f.write('import sys\nassert open(sys.argv[1], "rb").read() == b"original-user32"\nopen(sys.argv[3], "wb").write(%r)\n' % self.PATCHED)
		self.out = os.path.join(self.tmp, 'out')
		os.makedirs(os.path.join(self.out, 'cursors'))

	def sums(self, sha):
		with open(os.path.join(self.src, 'desktop/pointers/SHA256SUMS.user32'), 'w') as f:
			f.write('%s  user32-afterglow.dll\n' % sha)

	def run_script(self):
		env = dict(self.env, AFTERGLOW_SRC=self.src)
		return subprocess.run(['bash', os.path.join(HERE, 'prepare-pointers.sh'), self.img, self.out], env=env, capture_output=True, text=True)

	def test_patched_copy_is_written_when_the_sum_matches(self):
		self.sums(hashlib.sha256(self.PATCHED).hexdigest())
		r = self.run_script()
		self.assertEqual(r.returncode, 0, r.stderr)
		with open(os.path.join(self.out, 'user32-afterglow.dll'), 'rb') as f:
			self.assertEqual(f.read(), self.PATCHED)

	def test_wrong_sum_is_refused_and_writes_nothing(self):
		self.sums('0' * 64)
		r = self.run_script()
		self.assertNotEqual(r.returncode, 0)
		self.assertIn('expected', r.stderr)
		self.assertFalse(os.path.exists(os.path.join(self.out, 'user32-afterglow.dll')))

	def test_missing_cursors_is_a_usage_error(self):
		os.rmdir(os.path.join(self.out, 'cursors'))
		self.sums('0' * 64)
		r = self.run_script()
		self.assertEqual(r.returncode, 2)

	def test_the_disk_is_not_changed(self):
		self.sums(hashlib.sha256(self.PATCHED).hexdigest())
		before = hashlib.sha256(open(self.img, 'rb').read()).hexdigest()
		self.run_script()
		self.assertEqual(before, hashlib.sha256(open(self.img, 'rb').read()).hexdigest())


if __name__ == '__main__':
	unittest.main()
