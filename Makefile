# The single local gate: everything that runs without an emulator.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

.PHONY: installer disks installer-dos installer-w31 dos validate fmt clippy test headers floats layers goldens wasm

validate: fmt clippy test headers floats layers magic wasm
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

# The pixel surface must build for the browser: no_std plus alloc, no operating system.
wasm:
	rustup target add wasm32-unknown-unknown
	cargo check --target wasm32-unknown-unknown -p quill-paint --features pixel --locked

layers:
	python3 -I scripts/check-layers.py

magic:
	bash scripts/check-magic.sh

# Rewrites the goldens from the current code. Review the diff like code.
goldens:
	QUILL_UPDATE_GOLDEN=1 cargo nextest run --workspace --locked

# The DOS runtime: format, lints, build and the headless DOSBox golden comparison (fetches the tools first).
installer:
	bash installer/tools/check.sh

disks:
	bash installer/tools/build-disks.sh --stage
	bash installer/tools/verify-disks.sh

installer-dos:
	bash dos/tools/fetch-tools.sh
	bash installer/tools/e2e-dos.sh

# Needs a Windows 3.11 tree: W31_TREE, or the play harness's base.
installer-w31:
	bash installer/tools/e2e-w31.sh

dos:
	bash dos/tools/fetch-tools.sh
	bash dos/check.sh
