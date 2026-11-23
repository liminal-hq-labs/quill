#!/usr/bin/env python3
# Picks one screen out of an LI_DUMP file, the last frame that contains a marker.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/frames.py <dump file> <marker>
#
# The runtime writes each screen it shows as `=== screen N ===` and the 25 text lines. This prints the last frame whose text
# contains the marker, exactly as the runtime wrote it (UTF-8, `\n` line ends), and exits 1 when there is none.
import re
import sys


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    with open(sys.argv[1], "rb") as f:
        text = f.read().decode("utf-8").replace("\r\n", "\n")
    frames = re.split(r"^=== screen \d+ ===\n", text, flags=re.M)[1:]
    for frame in reversed(frames):
        if sys.argv[2] in frame:
            sys.stdout.write(frame)
            return
    sys.exit("frames: no screen contains " + repr(sys.argv[2]))


if __name__ == "__main__":
    main()
