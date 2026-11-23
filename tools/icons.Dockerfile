# The image tools/make-icons.sh builds the icons in when Python, Pillow, numpy or librsvg are missing on the host.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends python3 python3-pil python3-numpy librsvg2-bin && rm -rf /var/lib/apt/lists/*
WORKDIR /repo
