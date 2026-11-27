#!/usr/bin/env python3
# Tests for scripts/check-ids.py: overlap, duplicate, outside-block, stale markdown, the grandfather list and next-id.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

"""Run as `python3 scripts/test_check_ids.py` (standard library only)."""

import importlib.util
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "check-ids.py"
spec = importlib.util.spec_from_file_location("check_ids", SCRIPT)
check_ids = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check_ids)

BASE_TOML = """
[[block]]
prefix = "CORE"
first = 100
last = 109
owner = "Alpha plan"
status = "reserved"
source = "docs/plans/alpha.md"

[[block]]
prefix = "CORE"
first = 110
last = 119
owner = "Beta plan"
status = "reserved"
source = "docs/plans/beta.md"

[[block]]
prefix = "DOS"
first = 100
last = 109
owner = "Gamma plan"
status = "reserved"
source = "docs/plans/gamma.md"
"""

BASE_DECISIONS = """# Decisions

| # | Decision | Choice | Alternatives considered | Why |
|---|---|---|---|---|
| D5 | Legacy | x | y | z |
| CORE-100 | First | x | y | z |
| CORE-100.1 | Sub | x | y | z |
| CORE-110 | Second | x | y | z |
"""


class RegistryCase(unittest.TestCase):
	def make_repo(self, toml=BASE_TOML, decisions=BASE_DECISIONS, write_markdown=True):
		directory = tempfile.TemporaryDirectory()
		self.addCleanup(directory.cleanup)
		root = Path(directory.name)
		(root / "docs").mkdir()
		(root / "docs" / "id-ranges.toml").write_text(toml, encoding="utf-8")
		(root / "docs" / "decisions.md").write_text(decisions, encoding="utf-8")
		if write_markdown:
			check_ids.write_markdown(root)
		return root

	def problems(self, root):
		errors, _ = check_ids.check(root)
		return errors


class CheckTests(RegistryCase):
	def test_clean_registry_passes(self):
		self.assertEqual(self.problems(self.make_repo()), [])

	def test_overlap_names_both_owners(self):
		toml = BASE_TOML + '\n[[block]]\nprefix = "CORE"\nfirst = 105\nlast = 112\nowner = "Delta plan"\nstatus = "reserved"\nsource = "x"\n'
		errors = self.problems(self.make_repo(toml=toml))
		overlaps = [error for error in errors if error.startswith("OVERLAP")]
		self.assertEqual(len(overlaps), 2)
		self.assertTrue(any("Alpha plan" in error and "Delta plan" in error and "CORE-105 to CORE-109" in error for error in overlaps))
		self.assertTrue(any("Beta plan" in error and "Delta plan" in error for error in overlaps))

	def test_same_numbers_in_different_prefixes_do_not_overlap(self):
		self.assertEqual([error for error in self.problems(self.make_repo()) if error.startswith("OVERLAP")], [])

	def test_known_overlap_is_accepted(self):
		toml = BASE_TOML + '\n[[block]]\nprefix = "CORE"\nfirst = 105\nlast = 105\nowner = "Delta plan"\nstatus = "reserved"\nsource = "x"\n'
		toml += '\n[[known_overlap]]\nprefix = "CORE"\nowners = ["Delta plan", "Alpha plan"]\n'
		self.assertEqual(self.problems(self.make_repo(toml=toml)), [])

	def test_stale_known_overlap_fails(self):
		toml = BASE_TOML + '\n[[known_overlap]]\nprefix = "CORE"\nowners = ["Alpha plan", "Beta plan"]\n'
		errors = self.problems(self.make_repo(toml=toml))
		self.assertTrue(any(error.startswith("BAD") and "no longer overlaps" in error for error in errors))

	def test_duplicate_id_reports_both_lines(self):
		decisions = BASE_DECISIONS + "| CORE-110 | Again | x | y | z |\n"
		errors = self.problems(self.make_repo(decisions=decisions))
		self.assertEqual(len(errors), 1)
		self.assertIn("DUPLICATE  CORE-110", errors[0])
		self.assertIn("line 8", errors[0])
		self.assertIn("line 9", errors[0])

	def test_sub_id_is_not_a_duplicate_of_its_parent(self):
		self.assertEqual(self.problems(self.make_repo()), [])

	def test_known_duplicate_is_accepted_and_goes_stale(self):
		decisions = BASE_DECISIONS + "| CORE-110 | Again | x | y | z |\n"
		toml = BASE_TOML + '\n[[known_duplicate]]\nid = "CORE-110"\n'
		self.assertEqual(self.problems(self.make_repo(toml=toml, decisions=decisions)), [])
		errors = self.problems(self.make_repo(toml=toml))
		self.assertTrue(any("known_duplicate CORE-110 is no longer used twice" in error for error in errors))

	def test_id_outside_every_block_fails(self):
		decisions = BASE_DECISIONS + "| CORE-130 | Stray | x | y | z |\n"
		errors = self.problems(self.make_repo(decisions=decisions))
		self.assertEqual(len(errors), 1)
		self.assertIn("OUTSIDE  CORE-130", errors[0])
		self.assertIn("next-id.sh CORE", errors[0])

	def test_grandfathered_id_is_accepted(self):
		decisions = BASE_DECISIONS + "| CORE-7 | Old | x | y | z |\n| CORE-7.1 | Old sub | x | y | z |\n"
		toml = BASE_TOML + '\n[[grandfather]]\nprefix = "CORE"\nnumbers = [7]\n'
		self.assertEqual(self.problems(self.make_repo(toml=toml, decisions=decisions)), [])

	def test_grandfather_is_per_prefix(self):
		decisions = BASE_DECISIONS + "| DOS-7 | Old | x | y | z |\n"
		toml = BASE_TOML + '\n[[grandfather]]\nprefix = "CORE"\nnumbers = [7]\n'
		errors = self.problems(self.make_repo(toml=toml, decisions=decisions))
		self.assertTrue(any("OUTSIDE  DOS-7" in error for error in errors))

	def test_legacy_d_numbers_are_exempt(self):
		decisions = BASE_DECISIONS + "| D999 | Legacy | x | y | z |\n"
		self.assertEqual(self.problems(self.make_repo(decisions=decisions)), [])

	def test_stale_markdown_fails(self):
		root = self.make_repo()
		(root / "docs" / "id-ranges.md").write_text("old\n", encoding="utf-8")
		errors = self.problems(root)
		self.assertEqual(len(errors), 1)
		self.assertIn("STALE-MD", errors[0])

	def test_missing_markdown_fails(self):
		errors = self.problems(self.make_repo(write_markdown=False))
		self.assertTrue(any(error.startswith("STALE-MD") for error in errors))

	def test_changing_the_toml_makes_the_markdown_stale(self):
		root = self.make_repo()
		toml_path = root / "docs" / "id-ranges.toml"
		toml_path.write_text(toml_path.read_text(encoding="utf-8").replace("Beta plan", "Beta plan renamed"), encoding="utf-8")
		self.assertTrue(any(error.startswith("STALE-MD") for error in self.problems(root)))

	def test_malformed_blocks_are_reported(self):
		toml = '[[block]]\nprefix = "NOPE"\nfirst = 9\nlast = 1\nowner = ""\nstatus = "maybe"\nsource = ""\n'
		with self.assertRaises(check_ids.RegistryError) as raised:
			check_ids.load_registry(self.make_repo(toml=toml, write_markdown=False) / "docs" / "id-ranges.toml")
		message = str(raised.exception)
		for fragment in ["prefix is not one of", "first is above last", "owner is empty", "status must be one of", "source is empty"]:
			self.assertIn(fragment, message)

	def test_reserved_block_with_rows_is_a_note_not_an_error(self):
		errors, notes = check_ids.check(self.make_repo())
		self.assertEqual(errors, [])
		self.assertTrue(any("Alpha plan" in note and "in use" in note for note in notes))


class NextBlockTests(RegistryCase):
	def test_next_block_follows_the_highest_reserved_id(self):
		root = self.make_repo()
		first, last = check_ids.reserve_next_block(root, "CORE", 10, "Epsilon plan", "docs/plans/epsilon.md")
		self.assertEqual((first, last), (120, 129))
		self.assertEqual(self.problems(root), [])
		self.assertIn("Epsilon plan", (root / "docs" / "id-ranges.md").read_text(encoding="utf-8"))

	def test_next_block_follows_a_used_id_above_every_block(self):
		decisions = BASE_DECISIONS + "| CORE-300 | Far | x | y | z |\n"
		toml = BASE_TOML + '\n[[grandfather]]\nprefix = "CORE"\nnumbers = [300]\n'
		root = self.make_repo(toml=toml, decisions=decisions)
		self.assertEqual(check_ids.reserve_next_block(root, "CORE", 1, "Zeta", "x"), (301, 301))

	def test_next_block_for_a_new_prefix_starts_at_one(self):
		root = self.make_repo()
		self.assertEqual(check_ids.reserve_next_block(root, "WEB", 5, "Web plan", "x"), (1, 5))
		self.assertEqual(self.problems(root), [])

	def test_two_reservations_never_overlap(self):
		root = self.make_repo()
		first = check_ids.reserve_next_block(root, "DOS", 5, "One", "x")
		second = check_ids.reserve_next_block(root, "DOS", 5, "Two", "x")
		self.assertEqual(first, (110, 114))
		self.assertEqual(second, (115, 119))
		self.assertEqual(self.problems(root), [])

	def test_owner_with_quotes_survives(self):
		root = self.make_repo()
		check_ids.reserve_next_block(root, "LNX", 2, 'The "quoted" plan', "x")
		registry = check_ids.load_registry(root / "docs" / "id-ranges.toml")
		self.assertEqual(registry["block"][-1]["owner"], 'The "quoted" plan')

	def test_bad_arguments_are_refused(self):
		root = self.make_repo()
		for arguments in [("NOPE", 1, "x"), ("CORE", 0, "x"), ("CORE", 1, "  ")]:
			with self.assertRaises(check_ids.RegistryError):
				check_ids.reserve_next_block(root, *arguments, "x")


class RealRegistryTests(unittest.TestCase):
	def test_the_repository_registry_is_clean(self):
		errors, _ = check_ids.check(check_ids.repo_root())
		self.assertEqual(errors, [])


if __name__ == "__main__":
	unittest.main()
