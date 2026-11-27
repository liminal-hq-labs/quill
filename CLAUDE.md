# CLAUDE.md

A summary of [AGENTS.md](AGENTS.md), which is canonical. Keep the two in sync.

## What this is

Quill is a Liminal HQ Labs experiment: one word processor for DOS, Windows 3.1 and Windows 95, written in Rust, and the companion to the *Cross-compiling to the past* blog series. Experimental, so anything may change. Behaviour lives in `SPEC.md`, design in `docs/architecture.md`, order in `docs/roadmap.md`, decisions in `docs/decisions.md`.

## Rules to remember

- **Canadian English.** Real em dashes. Sentence case. No hard-wrapped markdown prose. Ship the result, not the journey: no "this PR" or reviewer names in code or docs.
- **Conventional Commits.** Body in markdown with bold labels and no headings, hard-wrapped, written to a file and committed with `git commit -F`. End with a `Co-Authored-By` trailer.
- **Git.** Topic branches, merge commits only, branches deleted on merge. Never push, tag, publish or dispatch workflows unless asked. Never `git add -A` or `git commit -a`. `PLAN.md` stays out of git.
- **Removing files.** Name every path; never `rm` with globs or `find -delete` with patterns.
- **Architecture.** Portable crates never name a platform, use no `alloc`, no floats and no panics; only `quill-formats` reads and writes documents; Win32 and DOS lead, Win16 follows.
- **Formatting.** Tabs everywhere except YAML. Rust uses rustfmt with hard tabs (`rustfmt.toml`).
- **Headers.** SPDX header on every source, script, YAML, TOML and SVG file: summary line, blank, "(c) Copyright 2026 Liminal HQ, Scott Morris", `SPDX-License-Identifier: Apache-2.0 OR MIT`.
- **CI.** One job per check. Test results reach the PR through the pr-results publisher (`publish-test-results.yml`, pinned by SHA) from the `rust-test-results` and `event-file` artefacts. Shared images come from `liminal-hq/.github` (`ci-rust`, `ci-dos`), pinned by digest; the DOS tools come from the image's `DOS_TOOLCHAIN`, not a download (CI-1). Dependabot is off unless the project opts in.
- **Docs.** Update `SPEC.md` and `docs/decisions.md` in the same change as the behaviour.
- **Decision IDs.** New decisions in `docs/decisions.md` take an area prefix (`CORE`, `ARCH`, `DOS`, `W16`, `W9X`, `LNX`, `WUI`, `WEB`, `DSK`, `INS`, `SITE`, `CI`) and a number, such as `LNX-1`, with sub-decisions as `LNX-1.1`. Legacy `D1` to `D2xx` numbers are frozen and stay valid as references; nothing new takes one. **Reserve a block with `scripts/next-id.sh PREFIX COUNT OWNER` before writing a plan** (it records the block in `docs/id-ranges.toml` and regenerates `docs/id-ranges.md`); a package takes ids from inside its owner's block, and `python3 scripts/check-ids.py` fails on overlapping blocks, a duplicate id or an id outside every block. Never renumber to fix a clash: record it.

## Commands

```sh
make validate                    # fmt, clippy, nextest, headers, float ban
bash scripts/check-headers.sh
make goldens                     # rewrite goldens; review the diff
bash scripts/check-banner.sh     # add --shots /tmp/shots for review frames
python3 -I scripts/check-site.py # links and assets in site/, the GitHub Pages product page
bash scripts/build-try.sh --build        # the Try Quill disk bundle, site/try/quill.<hash>.jsdos and try-manifest.json (FreeDOS 1.3 + QUILLDOS.EXE for js-dos; needs mtools)
bash scripts/check-try.sh        # the Try Quill cache busting: hashed bundle, manifest, page versions (needs mtools)
python3 -I tools/test_icons.py   # the committed icons; bash tools/make-icons.sh rebuilds them
python3 -I installer/tools/test_size_gate.py  # make sizes; make disks runs the size gate on real disks against installer/budgets.toml
python3 -I scripts/check-ids.py   # decision-id registry; scripts/next-id.sh PREFIX COUNT OWNER reserves a block
docker run --rm -v "$PWD:/workdir" davidanson/markdownlint-cli2:v0.17.2 "**/*.md"
```
