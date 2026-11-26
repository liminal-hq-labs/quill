#!/usr/bin/env python3
# Replaces today's date, and the time that follows it, in a screen dump with the host's fixed clock.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: normalise-clock.py FRAMES [GOLDEN] > FRAMES.NORMAL
#
# DOS gives a file or folder a run makes the real date and time, where the host gives it the clock of its script (2026-11-25 14:32).
# The file dialog shows both (a date in the list, a date and time in the preview), so the e2e run replaces today's date (month, day and
# two-digit year, as the dialog writes it) by 11/25/26 and the time after it by 2:32 PM, keeping every line as long as it was. Dates that
# are not today's, which a scenario sets with `@stamp`, are left alone. With GOLDEN, a line that differs from the golden's only in a time
# (a box over the preview has hidden the date that says the time is today's) is taken from the golden.
import re
import sys
import time

HOST_DATE = "11/25/26"
HOST_TIME = "2:32 PM"


def main(path, golden_path=None):
    today = time.strftime("%m/%d/%y")
    # A date and time as the preview writes them, which a box over the pane may have cut at the left (`1/25/26  11:36 PM`).
    both = re.compile(r"((?:\d{0,2}/)?\d\d/\d\d)  (\d{1,2}:\d\d [AP]M)")
    # A date alone, as the list's Modified column writes it.
    alone = re.compile(r"(?<![\d/])(\d\d/\d\d/\d\d)(?![\d/])")
    out = []
    in_text = True
    for line in open(path, encoding="utf-8").read().split("\n"):
        if line.startswith("=== screen"):
            in_text = True
        elif line.startswith("--- attributes"):
            in_text = False
        if in_text:

            def swap_both(m):
                if not today.endswith(m.group(1)):
                    return m.group(0)
                date = HOST_DATE[len(HOST_DATE) - len(m.group(1)) :]
                pad = " " * (len(m.group(2)) - len(HOST_TIME))
                return date + "  " + HOST_TIME + pad

            def swap_alone(m):
                return HOST_DATE if m.group(1) == today else m.group(0)

            line = both.sub(swap_both, line)
            line = alone.sub(swap_alone, line)
        out.append(line)
    if golden_path:
        golden = open(golden_path, encoding="utf-8").read().split("\n")
        clock = re.compile(r"\d{1,2}:\d\d [AP]M ?")
        if len(golden) == len(out):
            for i, (mine, theirs) in enumerate(zip(out, golden)):
                if mine != theirs and clock.sub("T", mine) == clock.sub("T", theirs):
                    out[i] = theirs
    sys.stdout.write("\n".join(out))


main(*sys.argv[1:3])
