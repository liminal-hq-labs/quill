#!/usr/bin/env bash
# Fetch the pinned Afterglow source (guest/afterglow.pin) into a cache folder, check it and print the folder. Nothing is committed.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Usage: fetch-afterglow.sh [CACHE_DIR]      (default ~/.cache/reactos/afterglow, or $AFTERGLOW_CACHE)
# Downloads https://codeload.github.com/<repo>/tar.gz/<commit>, decompresses it, checks the SHA-256 of the tar against the pin and extracts it
# to CACHE_DIR/<commit>. The last line on stdout is that folder; progress goes to stderr. A folder that is already extracted is reused.
# AFTERGLOW_TARBALL=FILE uses a local .tar.gz instead of downloading it (the same check applies); AFTERGLOW_PIN=FILE reads another pin.
# Set AFTERGLOW_PRINT_SUMS=1 to print the tar's sum and not fail on a mismatch (how a new pin is made).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
PIN=${AFTERGLOW_PIN:-$HERE/afterglow.pin}
# shellcheck source=afterglow.pin
. "$PIN"
: "${AFTERGLOW_REPO:?}" "${AFTERGLOW_COMMIT:?}" "${AFTERGLOW_TAR_SHA256:?}"
case "$AFTERGLOW_COMMIT" in *[!0-9a-f]* | "") echo "the pin's commit is not a full lowercase SHA: $AFTERGLOW_COMMIT" >&2; exit 2 ;; esac
[ ${#AFTERGLOW_COMMIT} -eq 40 ] || { echo "the pin's commit is not 40 hex digits" >&2; exit 2; }
CACHE=${1:-${AFTERGLOW_CACHE:-$HOME/.cache/reactos/afterglow}}
DEST=$CACHE/$AFTERGLOW_COMMIT
if [ -f "$DEST/.verified" ] && [ "$(cat "$DEST/.verified")" = "$AFTERGLOW_TAR_SHA256" ]; then
	echo "afterglow: $DEST (already fetched and checked)" >&2
	echo "$DEST"
	exit 0
fi
mkdir -p "$CACHE"
TMP=$(mktemp -d "$CACHE/.fetch.XXXXXX")
trap 'rm -rf "$TMP"' EXIT
if [ -n "${AFTERGLOW_TARBALL:-}" ]; then
	cp "$AFTERGLOW_TARBALL" "$TMP/src.tar.gz"
else
	echo "afterglow: fetching $AFTERGLOW_REPO at $AFTERGLOW_COMMIT" >&2
	curl -fsSL --retry 3 -o "$TMP/src.tar.gz" "https://codeload.github.com/$AFTERGLOW_REPO/tar.gz/$AFTERGLOW_COMMIT"
fi
gzip -dc "$TMP/src.tar.gz" >"$TMP/src.tar"
GOT=$(sha256sum "$TMP/src.tar" | cut -d' ' -f1)
if [ "$GOT" != "$AFTERGLOW_TAR_SHA256" ]; then
	echo "afterglow: tar SHA-256 is $GOT" >&2
	if [ -z "${AFTERGLOW_PRINT_SUMS:-}" ]; then
		echo "afterglow: expected $AFTERGLOW_TAR_SHA256 (afterglow.pin); not using the download" >&2
		exit 1
	fi
fi
mkdir -p "$TMP/x"
# An archive's names are checked before anything is written: no absolute path and no ".." part.
if tar -tf "$TMP/src.tar" | grep -q -E '(^/|(^|/)\.\.(/|$))'; then echo "afterglow: the archive has an unsafe path" >&2; exit 1; fi
tar -xf "$TMP/src.tar" -C "$TMP/x" --strip-components=1 --no-same-owner --no-same-permissions
[ -f "$TMP/x/desktop/style/build.sh" ] || { echo "afterglow: the archive has no desktop/style/build.sh" >&2; exit 1; }
rm -rf "$DEST"
mv "$TMP/x" "$DEST"
echo "$GOT" >"$DEST/.verified"
echo "afterglow: $DEST" >&2
echo "$DEST"
