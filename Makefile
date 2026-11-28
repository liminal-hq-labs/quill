# The single local gate: everything that runs without an emulator.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

.PHONY: installer disks sizes installer-dos installer-w31 installer-w95 dos gtk validate fmt clippy test headers floats layers ids unicode-docs goldens wasm bare

validate: fmt clippy test headers floats layers ids unicode-docs sizes magic wasm bare
	@echo "validate: ok"

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets --locked -- -D warnings

test:
	cargo nextest run --workspace --locked
	cargo nextest run -p quill-host --features long-names --test names --test autosave_modes --locked

headers:
	bash scripts/check-headers.sh

floats:
	bash scripts/check-no-floats.sh

# The pixel surface must build for the browser: no_std plus alloc, no operating system.
wasm:
	rustup target add wasm32-unknown-unknown
	cargo check --target wasm32-unknown-unknown -p quill-paint --features pixel --locked

# The job crate must build with no operating system and no allocator: `x86_64-unknown-none` has neither.
bare:
	rustup target add x86_64-unknown-none
	cargo build --release --target x86_64-unknown-none -p quill-job --locked

layers:
	python3 -I scripts/check-layers.py

ids:
	python3 -I scripts/test_check_ids.py
	python3 -I scripts/check-ids.py

magic:
	bash scripts/check-magic.sh

# The Unicode test documents are generated, and the frozen .QLD 1.0 reader is never edited.
unicode-docs:
	python3 -I scripts/gen-unicode-docs.py --check
	bash scripts/check-frozen-reader.sh

# Rewrites the goldens from the current code. Review the diff like code.
goldens:
	QUILL_UPDATE_GOLDEN=1 cargo nextest run --workspace --locked

# The DOS runtime: format, lints, build and the headless DOSBox golden comparison (fetches the tools first).
installer:
	bash installer/tools/check.sh

disks:
	bash installer/tools/build-disks.sh --stage
	bash installer/tools/verify-disks.sh

# The size gate's own tests (installer/budgets.toml is the one file of budgets); `make disks` runs the gate on real disks.
sizes:
	python3 -I installer/tools/test_size_gate.py

installer-dos:
	bash dos/tools/fetch-tools.sh
	bash installer/tools/e2e-dos.sh

# Needs a Windows 3.11 tree: W31_TREE, or the play harness's base.
installer-w31:
	bash installer/tools/e2e-w31.sh

# Needs the Windows 95 base image of Liminal Installer's test bed (W95_BASE_DIR).
installer-w95:
	bash installer/tools/e2e-w95.sh

# The GTK4 client has its own workspace and needs the GTK4 development files, so it is not part of `validate`.
gtk:
	cd linux/quill-gtk && cargo fmt --check && cargo clippy --all-targets --locked -- -D warnings && cargo test --locked

dos:
	bash dos/tools/fetch-tools.sh
	bash dos/check.sh
