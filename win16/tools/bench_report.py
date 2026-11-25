#!/usr/bin/env python3
# Turns the BENCH.LOG of QUILLB.EXE (the paint benchmark, win16/examples/quill/bench.rs) into a table, and checks it against a budget.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: bench_report.py [--budget win16/bench-budget.json] [--imports win16/tools/imports.tbl] [--top N] [--compare OTHER.LOG] BENCH.LOG
#
# The log has one `K` line per key (the phase, the calls the key made, the calls its paint made, the window-manager calls of each,
# the milliseconds, the time since the start, then the shell counters) and `F` lines holding, for each phase and for the key (0) and
# its paint (1), how many times each import was called, by its row in tools/imports.tbl (the program holds no names). Per phase this
# prints the median, 95th percentile and maximum of each number and the calls by function. A budget names, for each phase, the most
# a number may be; call counts, window-manager calls and the work counters are the gate and times are only reported, because the
# counts are the same on every machine. Exits 1 when a limit is passed.
#
# `--compare OTHER.LOG` checks the call counts of the two logs are exactly equal (the same run on another host, such as ReactOS under
# WineVDM): any difference is printed and exits 1.
import argparse
import importlib.util
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SH_BASE = 224
SH_NAMES = ["inv_px", "paint_px", "blit_px", "fill_px", "lines", "runs", "chars", "toolbar", "ruler", "status", "caches", "metrics", "walks", "paints", "fills"]
# The K line's columns after the `K index`: the phase, then these.
COLS = ["phase", "calls_key", "calls_paint", "wm_key", "wm_paint", "ms", "at_ms"] + SH_NAMES


def table(path):
	spec = importlib.util.spec_from_file_location("gen_imports", os.path.join(HERE, "gen_imports.py"))
	mod = importlib.util.module_from_spec(spec)
	spec.loader.exec_module(mod)
	return [r[2] for r in mod.parse_table(path)]


def parse(path):
	log = {"phases": [], "keys": [], "func": {}, "info": {}, "back": ""}
	for line in open(path, encoding="ascii", errors="replace"):
		t = line.split()
		if not t:
			continue
		if t[0] == "I":
			log["info"] = {t[i]: t[i + 1] for i in range(1, len(t) - 1, 2)}
		elif t[0] == "B":
			log["back"] = " ".join(t[1:])
		elif t[0] == "P":
			log["phases"].append(t[2] if len(t) > 2 else t[1])
		elif t[0] == "K":
			v = [int(x) for x in t[2:]]
			log["keys"].append(dict(zip(COLS, v)))
		elif t[0] == "F":
			p, kind = int(t[1]), int(t[2])
			d = log["func"].setdefault((p, kind), {})
			for kv in t[3:]:
				i, c = kv.split("=")
				d[int(i)] = int(c)
	return log


def pct(xs, q):
	xs = sorted(xs)
	if not xs:
		return 0
	return xs[min(len(xs) - 1, (len(xs) * q + 99) // 100 - 1 if q < 100 else len(xs) - 1)]


def median(xs):
	xs = sorted(xs)
	n = len(xs)
	if n == 0:
		return 0
	return xs[n // 2] if n % 2 else (xs[n // 2 - 1] + xs[n // 2]) // 2


def metric(k, name):
	if name == "calls":
		return k["calls_key"] + k["calls_paint"]
	if name == "wm":
		return k["wm_key"] + k["wm_paint"]
	return k[name]


def build(log, imports):
	rows = []
	for p, name in enumerate(log["phases"]):
		keys = [k for k in log["keys"] if k["phase"] == p]
		if keys:
			rows.append((p, name, keys))
	return rows


def render(log, imports, top):
	out = []
	info = log["info"]
	out.append(f"client {info.get('client')}  bits {info.get('bits')} x planes {info.get('planes')}  page_w {info.get('page_w')}  hmax {info.get('hmax')}")
	out.append(f"back buffer: {log['back']}")
	out.append("")
	cols = ["calls", "wm", "inv_px", "paint_px", "lines", "caches", "walks", "ms"]
	head = f"{'phase':8} {'keys':>4}  " + "  ".join(f"{c + ' med/p95/max':>22}" for c in cols[:2]) + "  " + "  ".join(f"{c + ' med':>12}" for c in cols[2:7]) + f"  {'ms mean/p95/max':>16}"
	out.append(head)
	for p, name, keys in build(log, imports):
		cells = []
		for c in cols[:2]:
			xs = [metric(k, c) for k in keys]
			cells.append(f"{median(xs):>6}/{pct(xs, 95):>6}/{max(xs):>6}")
		med = [str(median([metric(k, c) for k in keys])) for c in cols[2:7]]
		ms = [k["ms"] for k in keys]
		out.append(f"{name:8} {len(keys):>4}  " + "  ".join(f"{c:>22}" for c in cells) + "  " + "  ".join(f"{m:>12}" for m in med) + f"  {sum(ms) / len(ms):>6.1f}/{pct(ms, 95):>4}/{max(ms):>4}")
	out.append("")
	for p, name, keys in build(log, imports):
		for kind, label in ((0, "key"), (1, "paint")):
			f = log["func"].get((p, kind), {})
			calls = {i: c for i, c in f.items() if i < len(imports)}
			total = sum(calls.values())
			if total == 0:
				continue
			best = sorted(calls.items(), key=lambda x: -x[1])[:top]
			per = len(keys)
			out.append(f"{name} {label}: {total / per:.1f} calls per key: " + ", ".join(f"{imports[i]} {c / per:.1f}" for i, c in best))
	return "\n".join(out)


def check(log, budget):
	bad = []
	for name, limits in budget.get("phases", {}).items():
		if name not in log["phases"]:
			continue
		p = log["phases"].index(name)
		keys = [k for k in log["keys"] if k["phase"] == p]
		if not keys:
			continue
		for key, limit in limits.items():
			m = re.fullmatch(r"(\w+?)_(median|max)", key)
			if not m:
				bad.append(f"{name}: budget key {key!r} is not NAME_median or NAME_max")
				continue
			xs = [metric(k, m.group(1)) for k in keys]
			got = median(xs) if m.group(2) == "median" else max(xs)
			if got > limit:
				bad.append(f"{name}: {key} is {got}, the limit is {limit}")
	return bad


def counts(log):
	return [{k: v for k, v in key.items() if k not in ("ms", "at_ms")} for key in log["keys"]], log["func"]


def main():
	ap = argparse.ArgumentParser()
	ap.add_argument("log")
	ap.add_argument("--budget")
	ap.add_argument("--imports", default=os.path.join(HERE, "imports.tbl"))
	ap.add_argument("--top", type=int, default=8)
	ap.add_argument("--compare")
	a = ap.parse_args()
	imports = table(a.imports)
	log = parse(a.log)
	if not log["keys"]:
		sys.exit("bench_report: no keys in " + a.log)
	if int(log["info"].get("imports", len(imports))) != len(imports):
		sys.exit(f"bench_report: the log counts {log['info'].get('imports')} imports and {a.imports} has {len(imports)}")
	print(render(log, imports, a.top))
	rc = 0
	if a.compare:
		other = parse(a.compare)
		if counts(log) != counts(other):
			print("\nCOMPARE: the counts differ from " + a.compare)
			for i, (x, y) in enumerate(zip(log["keys"], other["keys"])):
				for k in COLS:
					if k not in ("ms", "at_ms") and x[k] != y[k]:
						print(f"  key {i} {k}: {x[k]} against {y[k]}")
			rc = 1
		else:
			print("\nCOMPARE: the counts equal " + a.compare)
	if a.budget and os.path.exists(a.budget):
		bad = check(log, json.load(open(a.budget)))
		print()
		if bad:
			print("BUDGET: FAIL")
			for b in bad:
				print("  " + b)
			rc = 1
		else:
			print("BUDGET: ok")
	sys.exit(rc)


main()
