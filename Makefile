# The single local gate: everything that runs without an emulator.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

.PHONY: validate fmt clippy test headers floats goldens

validate: fmt clippy test headers floats
	@echo "validate: ok"

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets --locked -- -D warnings

test:
	cargo nextest run --workspace --locked

headers:
	bash scripts/check-headers.sh

floats:
	bash scripts/check-no-floats.sh

# Rewrites the goldens from the current code. Review the diff like code.
goldens:
	QUILL_UPDATE_GOLDEN=1 cargo nextest run --workspace --locked
