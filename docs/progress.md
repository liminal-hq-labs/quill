# Progress

What is built, how it was shown to work, what is blocked and what was assumed. Updated with every merge. Behaviour is in [SPEC.md](../SPEC.md), the order in [roadmap.md](roadmap.md), the choices in [decisions.md](decisions.md).

## Table of contents

- [Status](#status)
- [Evidence](#evidence)
- [Blocked](#blocked)
- [Assumptions](#assumptions)

## Status

| Step | State | Notes |
|---|---|---|
| M0 Foundations | Done on the host, with two items open | The workspace, `Store` and `FarVec`, the piece table with undo, the document, layout, the `.QLD` reader and writer, `quill-render`. The fuzz smoke job is not set up (needs nightly and `cargo-fuzz`); the property tests include damaged-file and random-byte inputs in its place |

## Evidence

### M0

- `make validate` passes: format, clippy with the portable-crate lint set, 16 tests under `cargo nextest`, headers, float ban.
- Property tests against a `Vec` and a `String` model run with 8-byte and 96-byte chunks, so every few operations cross a chunk edge: after any edit sequence the text and both format layers match the model, undoing every step reaches each earlier state, redoing reaches each later one, and an out-of-memory failure leaves the text as it was.
- `tests/docs/plain.qld` is built by `quill-host`'s `make-docs`, read back, laid out with `GridMeasure` and compared with `tests/golden/layout/plain.grid.txt`; read then write gives the same bytes.
- `tests/golden/pages/plain.grid.png` is the page map (see D61).

## Blocked

Nothing yet.

## Assumptions

D58 to D62 in [decisions.md](decisions.md) were made while implementing M0.
