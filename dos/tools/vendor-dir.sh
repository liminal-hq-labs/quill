#!/bin/bash
# Prints the folder that holds the DOS tools (dosbox/ and cwsdpmi/): the one place every DOS script looks.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/vendor-dir.sh <default-folder>
#
# In order: $DOS_VENDOR when set; $DOS_TOOLCHAIN when it holds CWSDPMI's stub (the Liminal HQ ci-dos image installs DJGPP, DOSBox
# Staging and CWSDPMI under /opt/dos-toolchain with the same dosbox/ and cwsdpmi/ layout, so a job in that image fetches nothing);
# otherwise the default folder (dos/vendor, which dos/tools/fetch-tools.sh fills for a developer's machine).
set -euo pipefail
if [ -n "${DOS_VENDOR:-}" ]; then
	echo "$DOS_VENDOR"
elif [ -n "${DOS_TOOLCHAIN:-}" ] && [ -f "$DOS_TOOLCHAIN/cwsdpmi/bin/CWSDSTUB.EXE" ]; then
	echo "$DOS_TOOLCHAIN"
else
	echo "$1"
fi
