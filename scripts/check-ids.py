#!/usr/bin/env python3
# Checks the decision-id range registry against docs/decisions.md, regenerates docs/id-ranges.md and reserves new blocks.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

"""Decision-id range registry.

    python3 scripts/check-ids.py                  check (what CI runs)
    python3 scripts/check-ids.py --write          regenerate docs/id-ranges.md from docs/id-ranges.toml
    python3 scripts/check-ids.py next PREFIX COUNT OWNER [--source PATH]
                                                  reserve the next free block (scripts/next-id.sh wraps this)

The registry is docs/id-ranges.toml: one `[[block]]` per reserved range of ids (prefix, first, last, owner, status, source).
The check fails when

  OVERLAP    two blocks of one prefix share an id (the message names both owners), unless the pair is a `known_overlap`
  DUPLICATE  a decision id is a first-column row of docs/decisions.md twice, unless it is a `known_duplicate`
  OUTSIDE    a decision id sits in no block and is not in the `grandfather` list (legacy `D` numbers are exempt)
  STALE-MD   docs/id-ranges.md is not what the toml renders to
  BAD        the toml is malformed (unknown prefix or status, first above last, a known entry that no longer applies)

Standard library only (Python 3.11 or later, for tomllib).
"""

import argparse
import json
import re
import sys
import tomllib
from pathlib import Path

PREFIXES = ["CORE", "ARCH", "DOS", "W16", "W9X", "LNX", "WUI", "WEB", "DSK", "INS", "SITE", "CI"]
STATUSES = ["reserved", "in use", "retired"]

DECISION_ROW = re.compile(r"^\|\s*(" + "|".join(PREFIXES) + r")-(\d+)((?:\.\d+)*)\s*\|")

MD_HEADER = """\
# Decision id ranges

<!-- Generated from docs/id-ranges.toml by `python3 scripts/check-ids.py --write`. Do not edit by hand. -->

Every block of decision ids that a plan or a package has reserved, per prefix. The source of truth is [id-ranges.toml](id-ranges.toml); this page is rendered from it and CI fails when it is stale.

**Before writing a plan, reserve its block:** `scripts/next-id.sh PREFIX COUNT OWNER` finds the next free block after the highest id reserved or used for that prefix, records it here and prints it. Then name the block in the plan and in `docs/work-packages.md`. Packages take ids from inside their owner's block; `python3 scripts/check-ids.py` fails on two overlapping blocks, a decision id used twice, or a decision id outside every block. The area prefixes are in [decisions.md](decisions.md#numbering). The legacy `D` numbers are frozen history and are not recorded here.

Status: **reserved** (no rows in `decisions.md` yet), **in use** (some rows are written) or **retired** (the owner is finished and the spare ids stay unused).
"""


class RegistryError(Exception):
	"""A problem in the registry or its inputs."""


def repo_root():
	return Path(__file__).resolve().parent.parent


def load_registry(toml_path):
	"""Read the toml and return its tables as plain dicts, checking their shape."""
	try:
		with open(toml_path, "rb") as handle:
			data = tomllib.load(handle)
	except (OSError, tomllib.TOMLDecodeError) as error:
		raise RegistryError(f"BAD  {toml_path}: {error}")
	problems = []
	blocks = data.get("block", [])
	for index, block in enumerate(blocks, 1):
		label = f"block {index} ({block.get('prefix')}-{block.get('first')} to {block.get('last')})"
		if block.get("prefix") not in PREFIXES:
			problems.append(f"BAD  {label}: prefix is not one of {', '.join(PREFIXES)}")
		if not (isinstance(block.get("first"), int) and isinstance(block.get("last"), int)):
			problems.append(f"BAD  {label}: first and last must be integers")
		elif block["first"] > block["last"]:
			problems.append(f"BAD  {label}: first is above last")
		if not str(block.get("owner", "")).strip():
			problems.append(f"BAD  {label}: owner is empty")
		if block.get("status") not in STATUSES:
			problems.append(f"BAD  {label}: status must be one of {', '.join(STATUSES)}")
		if not str(block.get("source", "")).strip():
			problems.append(f"BAD  {label}: source is empty")
	if problems:
		raise RegistryError("\n".join(problems))
	return data


def read_decision_rows(decisions_path):
	"""Return [(prefix, number, full_id, line_number)] for every decision row in the table's first column."""
	rows = []
	with open(decisions_path, encoding="utf-8") as handle:
		for line_number, line in enumerate(handle, 1):
			match = DECISION_ROW.match(line)
			if match:
				prefix, number, sub_ids = match.group(1), int(match.group(2)), match.group(3)
				rows.append((prefix, number, f"{prefix}-{number}{sub_ids}", line_number))
	return rows


def describe(block):
	return f"{block['prefix']}-{block['first']} to {block['prefix']}-{block['last']} ({block['owner']})"


def find_overlaps(blocks, known_overlaps):
	"""Return (errors, used_known) for blocks of one prefix that share an id."""
	errors = []
	used_known = set()
	for position, first_block in enumerate(blocks):
		for second_block in blocks[position + 1:]:
			if first_block["prefix"] != second_block["prefix"]:
				continue
			low = max(first_block["first"], second_block["first"])
			high = min(first_block["last"], second_block["last"])
			if low > high:
				continue
			pair = {first_block["owner"], second_block["owner"]}
			accepted = [
				index for index, known in enumerate(known_overlaps)
				if known.get("prefix") == first_block["prefix"] and set(known.get("owners", [])) == pair
			]
			if accepted:
				used_known.add(accepted[0])
				continue
			errors.append(
				f"OVERLAP  {first_block['prefix']}-{low} to {first_block['prefix']}-{high} is reserved twice: "
				f"{describe(first_block)} and {describe(second_block)}"
			)
	return errors, used_known


def find_duplicates(rows, known_duplicates):
	"""Return (errors, used_known) for decision ids that appear as a row more than once."""
	errors = []
	used_known = set()
	lines_by_id = {}
	for _, _, full_id, line_number in rows:
		lines_by_id.setdefault(full_id, []).append(line_number)
	accepted_ids = {known["id"] for known in known_duplicates}
	for full_id, lines in lines_by_id.items():
		if len(lines) < 2:
			continue
		if full_id in accepted_ids:
			used_known.add(full_id)
			continue
		where = ", ".join(f"line {number}" for number in lines)
		errors.append(f"DUPLICATE  {full_id} is a row of docs/decisions.md more than once ({where})")
	return errors, used_known


def find_outside(rows, blocks, grandfather):
	"""Return errors for decision rows whose number is in no block and not grandfathered."""
	allowed = {(entry["prefix"], number) for entry in grandfather for number in entry.get("numbers", [])}
	errors = []
	reported = set()
	for prefix, number, full_id, line_number in rows:
		if (prefix, number) in allowed or (prefix, number) in reported:
			continue
		if any(block["prefix"] == prefix and block["first"] <= number <= block["last"] for block in blocks):
			continue
		reported.add((prefix, number))
		errors.append(
			f"OUTSIDE  {full_id} (docs/decisions.md line {line_number}) is in no reserved block; "
			f"reserve one with scripts/next-id.sh {prefix} COUNT OWNER"
		)
	return errors


def source_link(source):
	"""Markdown for a block's source: a link into docs/, a URL, or the text as written."""
	if source.startswith("http://") or source.startswith("https://"):
		label = "PR #" + source.rsplit("/", 1)[-1] if "/pull/" in source else source
		return f"[{label}]({source})"
	if source.startswith("docs/"):
		path = source[len("docs/"):]
		return f"[{path.split('#')[0]}]({path})"
	return source


def render_markdown(registry):
	"""Render the registry as docs/id-ranges.md."""
	blocks = registry.get("block", [])
	lines = [MD_HEADER]
	for prefix in PREFIXES:
		prefix_blocks = sorted((block for block in blocks if block["prefix"] == prefix), key=lambda b: (b["first"], b["last"]))
		if not prefix_blocks:
			continue
		lines.append(f"## {prefix}\n")
		lines.append("| Range | Owner | Status | Source |")
		lines.append("|---|---|---|---|")
		for block in prefix_blocks:
			if block["first"] == block["last"]:
				span = f"{prefix}-{block['first']}"
			else:
				span = f"{prefix}-{block['first']} to {prefix}-{block['last']}"
			lines.append(f"| {span} | {block['owner']} | {block['status']} | {source_link(block['source'])} |")
		lines.append("")
	grandfather = registry.get("grandfather", [])
	if grandfather:
		lines.append("## Grandfathered ids\n")
		lines.append("Ids that `decisions.md` already used outside every block when the registry began. They stay as they are; a new id outside a block is an error.\n")
		lines.append("| Prefix | Numbers |")
		lines.append("|---|---|")
		for entry in sorted(grandfather, key=lambda e: PREFIXES.index(e["prefix"])):
			lines.append(f"| {entry['prefix']} | {format_numbers(entry['numbers'])} |")
		lines.append("")
	overlaps = registry.get("known_overlap", [])
	duplicates = registry.get("known_duplicate", [])
	if overlaps or duplicates:
		lines.append("## Known clashes\n")
		lines.append("Recorded as found, not renumbered. The check accepts exactly these and fails if one stops applying.\n")
		for known in overlaps:
			lines.append(f"- **Overlap in {known['prefix']}:** {' and '.join(known['owners'])}. {known.get('note', '')}".rstrip())
		for known in duplicates:
			lines.append(f"- **Duplicate {known['id']}:** {known.get('note', '')}".rstrip())
		lines.append("")
	pull_requests = registry.get("open_pr", [])
	if pull_requests:
		lines.append("## Open pull requests\n")
		lines.append("Ids and blocks on open pull request branches, noted when the registry was last updated.\n")
		for pull_request in pull_requests:
			url = f"https://github.com/liminal-hq-labs/quill/pull/{pull_request['number']}"
			lines.append(f"- [PR #{pull_request['number']}]({url}) (`{pull_request['branch']}`). {pull_request.get('note', '')}".rstrip())
		lines.append("")
	return "\n".join(lines).rstrip("\n") + "\n"


def format_numbers(numbers):
	"""Write sorted numbers as runs: 1 to 5, 9, 12 to 14."""
	numbers = sorted(set(numbers))
	runs = []
	start = previous = None
	for number in numbers:
		if start is None:
			start = previous = number
		elif number == previous + 1:
			previous = number
		else:
			runs.append((start, previous))
			start = previous = number
	if start is not None:
		runs.append((start, previous))
	return ", ".join(str(a) if a == b else f"{a} to {b}" for a, b in runs)


def check(root):
	"""Run every check. Returns (errors, notes)."""
	toml_path = root / "docs" / "id-ranges.toml"
	markdown_path = root / "docs" / "id-ranges.md"
	registry = load_registry(toml_path)
	blocks = registry.get("block", [])
	rows = read_decision_rows(root / "docs" / "decisions.md")
	errors = []

	overlap_errors, used_overlaps = find_overlaps(blocks, registry.get("known_overlap", []))
	errors += overlap_errors
	for index, known in enumerate(registry.get("known_overlap", [])):
		if index not in used_overlaps:
			errors.append(f"BAD  known_overlap for {known.get('prefix')} ({' and '.join(known.get('owners', []))}) no longer overlaps; remove it from docs/id-ranges.toml")

	duplicate_errors, used_duplicates = find_duplicates(rows, registry.get("known_duplicate", []))
	errors += duplicate_errors
	for known in registry.get("known_duplicate", []):
		if known["id"] not in used_duplicates:
			errors.append(f"BAD  known_duplicate {known['id']} is no longer used twice; remove it from docs/id-ranges.toml")

	errors += find_outside(rows, blocks, registry.get("grandfather", []))

	expected = render_markdown(registry)
	try:
		current = markdown_path.read_text(encoding="utf-8")
	except OSError:
		current = None
	if current != expected:
		errors.append("STALE-MD  docs/id-ranges.md is not what docs/id-ranges.toml renders to; run `python3 scripts/check-ids.py --write`")

	notes = []
	for block in blocks:
		if block["status"] == "reserved" and any(
			prefix == block["prefix"] and block["first"] <= number <= block["last"] for prefix, number, _, _ in rows
		):
			notes.append(f"note: {describe(block)} is marked reserved but has rows in docs/decisions.md; set it to in use")
	return errors, notes


def reserve_next_block(root, prefix, count, owner, source):
	"""Append the next free block of `count` ids for `prefix` to the toml and regenerate the markdown."""
	if prefix not in PREFIXES:
		raise RegistryError(f"{prefix} is not a prefix; use one of {', '.join(PREFIXES)}")
	if count < 1:
		raise RegistryError("COUNT must be at least 1")
	if not owner.strip():
		raise RegistryError("OWNER must not be empty")
	toml_path = root / "docs" / "id-ranges.toml"
	registry = load_registry(toml_path)
	highest = max([block["last"] for block in registry.get("block", []) if block["prefix"] == prefix], default=0)
	for row_prefix, number, _, _ in read_decision_rows(root / "docs" / "decisions.md"):
		if row_prefix == prefix:
			highest = max(highest, number)
	first = highest + 1
	last = first + count - 1
	entry = (
		"\n[[block]]\n"
		f'prefix = "{prefix}"\n'
		f"first = {first}\n"
		f"last = {last}\n"
		f"owner = {json.dumps(owner, ensure_ascii=False)}\n"
		'status = "reserved"\n'
		f"source = {json.dumps(source, ensure_ascii=False)}\n"
	)
	text = toml_path.read_text(encoding="utf-8")
	if not text.endswith("\n"):
		text += "\n"
	toml_path.write_text(text + entry, encoding="utf-8")
	write_markdown(root)
	return first, last


def write_markdown(root):
	registry = load_registry(root / "docs" / "id-ranges.toml")
	(root / "docs" / "id-ranges.md").write_text(render_markdown(registry), encoding="utf-8")


def main(argv=None):
	parser = argparse.ArgumentParser(description="Decision-id range registry")
	parser.add_argument("--root", type=Path, default=repo_root(), help="repository root (for tests)")
	parser.add_argument("--write", action="store_true", help="regenerate docs/id-ranges.md")
	subcommands = parser.add_subparsers(dest="command")
	next_parser = subcommands.add_parser("next", help="reserve the next free block")
	next_parser.add_argument("prefix")
	next_parser.add_argument("count", type=int)
	next_parser.add_argument("owner")
	next_parser.add_argument("--source", default="docs/id-ranges.toml", help="where the plan lives (default: the registry itself)")
	args = parser.parse_args(argv)

	try:
		if args.command == "next":
			first, last = reserve_next_block(args.root, args.prefix, args.count, args.owner, args.source)
			span = f"{args.prefix}-{first}" if first == last else f"{args.prefix}-{first} to {args.prefix}-{last}"
			print(f"Reserved {span} for {args.owner}")
			return 0
		if args.write:
			write_markdown(args.root)
			print("Wrote docs/id-ranges.md")
			return 0
		errors, notes = check(args.root)
	except RegistryError as error:
		print(error, file=sys.stderr)
		return 1
	for note in notes:
		print(note)
	if errors:
		print("\n".join(errors), file=sys.stderr)
		print(f"\n{len(errors)} problem(s) in the decision-id registry (docs/id-ranges.toml)", file=sys.stderr)
		return 1
	print("Decision ids: no overlaps, no duplicates, nothing outside a block, id-ranges.md is current")
	return 0


if __name__ == "__main__":
	sys.exit(main())
