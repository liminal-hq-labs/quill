#!/usr/bin/env bash
# Regenerates assets/icon from the SVG masters, on the host if it has the tools and in Docker if not.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/make-icons.sh [--docker] [make_icons.py arguments]
#
# `--docker` forces the container. Without it the script uses the host when `rsvg-convert` and Python's Pillow and numpy are there.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

use_docker=0
if [ "${1:-}" = "--docker" ]; then
	use_docker=1
	shift
fi
if [ "$use_docker" = 0 ] && ! { command -v rsvg-convert > /dev/null && python3 -c 'import PIL, numpy' 2> /dev/null; }; then
	use_docker=1
fi

if [ "$use_docker" = 1 ]; then
	docker build -q -t quill-icons:local -f tools/icons.Dockerfile tools > /dev/null
	exec docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp -v "$PWD:/repo" quill-icons:local python3 -I tools/make_icons.py "$@"
fi
exec python3 -I tools/make_icons.py "$@"
