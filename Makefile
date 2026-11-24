# The single local gate: everything that runs without an emulator.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

.PHONY: dos validate fmt clippy test headers floats layers goldens

validate: fmt clippy test headers floats layers magic
	@echo "validate: ok"

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets --locked -- -D warnings

test:
	cargo nextest run --workspace --locked
	cargo nextest run -p quill-host --features long-names --test names --locked

headers:
	bash scripts/check-headers.sh

floats:
	bash scripts/check-no-floats.sh

layers:
	python3 -I scripts/check-layers.py

magic:
	bash scripts/check-magic.sh

# Rewrites the goldens from the current code. Review the diff like code.
goldens:
	QUILL_UPDATE_GOLDEN=1 cargo nextest run --workspace --locked

# The DOS runtime: format, lints, build and the headless DOSBox golden comparison (fetches the tools first).
dos:
	bash dos/tools/fetch-tools.sh
	bash dos/check.sh
