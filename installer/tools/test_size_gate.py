#!/usr/bin/env python3
# Tests for size-gate.py: the free-space and NE readers, the budget judgement and the budgets file itself.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I installer/tools/test_size_gate.py
import importlib.util
import os
import struct
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("size_gate", os.path.join(HERE, "size-gate.py"))
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)

BUDGETS = gate.load_budgets()


def fake_ne(data_length, heap, stack):
	"""A header-only NE program with one segment (the data segment)."""
	ne = 0x40
	header = bytearray(0x100)
	struct.pack_into("<I", header, 0x3C, ne)
	header[ne : ne + 2] = b"NE"
	struct.pack_into("<H", header, ne + 0x0E, 1)
	struct.pack_into("<HH", header, ne + 0x10, heap, stack)
	struct.pack_into("<H", header, ne + 0x1C, 1)
	struct.pack_into("<H", header, ne + 0x22, 0x40)
	struct.pack_into("<HHHH", header, ne + 0x40, 0, data_length, 0, data_length)
	return bytes(header)


class Readers(unittest.TestCase):
	def test_free_bytes_with_spaces(self):
		self.assertEqual(gate.parse_free(" Volume in drive : is QUILL DOS\n424 448 bytes free\n"), 424448)
		self.assertEqual(gate.parse_free("   0 files   1 457 664 bytes\n     65 536 bytes free\n"), 65536)

	def test_no_free_line(self):
		with self.assertRaises(ValueError):
			gate.parse_free("nothing here")

	def test_data_segment_free(self):
		self.assertEqual(gate.win16_data_free(fake_ne(29836, 4096, 29696)), 1908)
		self.assertEqual(gate.win16_data_free(fake_ne(0, 0, 0)), 0x10000)

	def test_not_an_ne_program(self):
		with self.assertRaises(ValueError):
			gate.win16_data_free(bytes(0x100))

	def test_program_bytes(self):
		packed = {"QUILLDOS.EXE": 100, "QUILLPIC.DMD": 20, "EN_CA.DIC": 999}
		self.assertEqual(gate.program_bytes(packed, "QUILLDOS.EXE", []), 100)
		self.assertEqual(gate.program_bytes(packed, "QUILLDOS.EXE", ["QUILLPIC.DMD"]), 120)


class Judging(unittest.TestCase):
	def facts(self, **over):
		base = {
			"executable": {"QUILLDOS": 438272},
			"win16_data_free": 1908,
			"modules": {},
			"free": {"dos": 424448, "w31": 342016, "w95": 434176},
		}
		base.update(over)
		return base

	def test_today_passes(self):
		self.assertEqual(gate.judge(BUDGETS, self.facts()), [])

	def test_exe_over_budget(self):
		failures = gate.judge(BUDGETS, self.facts(executable={"QUILLDOS": 454657}))
		self.assertEqual(len(failures), 1)
		self.assertIn("1 over", failures[0].replace(",", ""))

	def test_exe_at_budget_passes(self):
		self.assertEqual(gate.judge(BUDGETS, self.facts(executable={"QUILLDOS": 454656})), [])

	def test_exe_past_stop(self):
		failures = gate.judge(BUDGETS, self.facts(executable={"QUILLDOS": 491521}))
		self.assertTrue(any("stop" in f for f in failures))

	def test_data_segment_floor(self):
		self.assertEqual(gate.judge(BUDGETS, self.facts(win16_data_free=1024)), [])
		self.assertEqual(len(gate.judge(BUDGETS, self.facts(win16_data_free=1023))), 1)

	def test_dos_floppy_floor(self):
		self.assertEqual(gate.judge(BUDGETS, self.facts(free={"dos": 131072})), [])
		failures = gate.judge(BUDGETS, self.facts(free={"dos": 131071}))
		self.assertEqual(len(failures), 1)
		self.assertIn("QUILL-DOS.IMG", failures[0])

	def test_module_budget_and_missing_row(self):
		self.assertEqual(gate.judge(BUDGETS, self.facts(modules={"QUILLPIC.DMD": 16384})), [])
		self.assertEqual(len(gate.judge(BUDGETS, self.facts(modules={"QUILLPIC.DMD": 16385}))), 1)
		failures = gate.judge(BUDGETS, self.facts(modules={"MYSTERY.DMD": 1}))
		self.assertIn("no [module", failures[0])

	def test_unmeasured_is_not_checked(self):
		self.assertEqual(gate.judge(BUDGETS, {}), [])


class BudgetsFile(unittest.TestCase):
	def test_figures_the_plan_names(self):
		self.assertEqual(BUDGETS["executable"]["QUILLDOS"]["budget"], 454656)
		self.assertEqual(BUDGETS["executable"]["QUILLDOS"]["stop"], 491520)
		self.assertEqual(BUDGETS["win16"]["data_segment"]["min_free"], 1024)
		self.assertEqual(BUDGETS["disk"]["dos"]["min_free"], 131072)
		self.assertEqual(BUDGETS["module"]["QUILLPIC.DMD"]["budget"], 16384)
		self.assertEqual(BUDGETS["module"]["QUILLDLG.DMD"]["budget"], 65536)

	def test_every_disk_is_complete(self):
		for row in BUDGETS["disk"].values():
			self.assertEqual(set(row), {"image", "target", "program", "min_free"})
			self.assertIn(row["target"], gate.PACK_TARGET)
			self.assertLess(row["min_free"], BUDGETS["floppy"]["capacity"])

	def test_lookup(self):
		self.assertEqual(gate.lookup(BUDGETS, "disk.dos.min_free"), 131072)
		self.assertEqual(gate.lookup(BUDGETS, 'module."QUILLPIC.DMD".budget'), 16384)


if __name__ == "__main__":
	unittest.main()
