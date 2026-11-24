#!/usr/bin/env python3
# Prints `SEGMENT:BYTES` for each segment of a segments.toml that has a budget, for ne_pack.py's --budget.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import sys
import tomllib

with open(sys.argv[1], "rb") as f:
    cfg = tomllib.load(f)
for k, v in sorted(cfg.get("segment", {}).items()):
    if "budget" in v:
        print(f"{k}:{v['budget']}")
