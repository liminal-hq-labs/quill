# AGENTS.md

## Table of contents

- [Project status](#project-status)
- [Spelling and writing](#spelling-and-writing)
- [Markdown formatting](#markdown-formatting)
- [Authoring voice](#authoring-voice)
- [Commit messages](#commit-messages)
- [Pull requests](#pull-requests)
- [Git workflow](#git-workflow)
- [Local tooling](#local-tooling)
- [Licence headers](#licence-headers)
- [Architecture rules](#architecture-rules)
- [Testing](#testing)
- [Documentation](#documentation)
- [Repository layout](#repository-layout)

## Project status

Quill is a [Liminal HQ Labs](https://github.com/liminal-hq-labs) experiment: one word processor for DOS, Windows 3.1 and Windows 95, written in Rust, and the companion application to the *Cross-compiling to the past* blog series. It is **experimental**, so interfaces, file formats and behaviour may change between any two commits. `SPEC.md` is the source of truth for behaviour, and `docs/decisions.md` records why. The [Labs charter](https://github.com/liminal-hq-labs/.github/blob/main/docs/labs-charter.md) covers what that status means and how a project graduates.

## Spelling and writing

**REQUIREMENT:** Use **Canadian English** in UI strings, comments, commit messages, pull request text and documentation: `colour`, `centre`, `behaviour`, `cancelled`, `licence` (noun) and `license` (verb), and `-ize` endings such as `customize`. Keep American spellings where an API, crate, CSS property or protocol requires them (`color`, `background-color`).

- Use a real em dash (`—`) in prose, never `--`. A command-line flag such as `--check` is not prose.
- Sentence case in the UI and in headings. Title Case for window titles.
- Write tight. No padding, no restating the heading.

## Markdown formatting

**REQUIREMENT:** Do not hard-wrap markdown prose. Write each paragraph or bullet as one line and let the renderer reflow it. This applies to `README.md`, `SPEC.md`, `docs/` and pull request descriptions. Commit message bodies are the one exception: hard-wrap them at roughly 72 characters. Code comments wrap normally.

## Authoring voice

**REQUIREMENT:** Ship the result, not how it was arrived at. Write comments, identifiers, docs and pull request text as the author of the artifact, for the person who meets it later.

- No "this PR", reviewer names or commit hashes in comments or docs. State the fact or the reasoning directly.
- When a comment has been edited more than once, rewrite it as one clean explanation.
- Commit messages and a pull request's `## Test plan` are the exceptions: they are the right place to record why a change happened and how it was checked.

## Commit messages

**Format:** Conventional Commits (`feat`, `fix`, `docs`, `test`, `ci`, `build`, `chore`, `refactor`, `perf`, `style`), with an optional scope.

- The body explains what and why, not how. Use markdown with **bold labels** and no headings. Hard-wrap.
- Backtick every code-level reference (identifiers, paths, config keys, flags, crate names). User-facing strings use quotes.
- End with a `Co-Authored-By:` trailer for the model that wrote the change.
- Write the message to a file with a single-quoted heredoc and commit with `git commit -F <file>`, so backticks and `$()` are not expanded. Check the result with `git log -1 --pretty=fuller`.

## Pull requests

- **Title:** capitalized, imperative, about 70 characters, no Conventional Commit prefix, outcome-focused.
- **Body:** `## Summary` with flat bullets that open with a **bold lead-in**, then `## Test plan` with `- [x]` or `- [ ]` items naming the commands run. Finish with what was not verified.
- **Labels:** one of `enhancement`, `bug`, `documentation`, `testing`, `ci`, `build`, `chore`, plus scope labels where useful.
- Fix a bug with a test that fails before the fix, and state exactly what was and was not run.

## Git workflow

- `main` is the default branch. Work on topic branches (`feat/`, `fix/issue-<n>-…`, `docs/`, `chore/`, `ci/`, `build/`). Never commit to `main`.
- **Merge commits only.** No squash and no rebase merges. Branches are deleted on merge.
- **Never** push, tag, publish or dispatch workflows unless asked.
- **Never** run `git add -A` or `git commit -a`, because checkouts may be shared. Stage explicit paths.
- **Never** remove files with wildcard patterns (no `rm` with globs, no `find -delete` with patterns). Name each path explicitly; patterns trip the owner's approval dialog.
- Prefer the `gh` CLI for GitHub work.
- Planning files such as `PLAN.md` stay out of git.

## Local tooling

- **Editor settings:** `.editorconfig` is authoritative: tabs everywhere except YAML, which uses spaces.
- **Rust**, if the project uses it, is formatted with rustfmt using hard tabs (`rustfmt.toml`). Strictness comes from CI: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and the test suite.
- **Shared CI images** come from [`liminal-hq/.github`](https://github.com/liminal-hq/.github) (`ci-rust`, `ci-web`, `tauri-ci-desktop` and others). Pick the leanest tier that fits and pin it by digest. A project image belongs here only when no shared tier covers it.
- **Dependabot** is off. Add `.github/dependabot.yml` only when the project opts in (see `docs/new-project.md`).

- **Toolchain:** Rust 1.98.0 (`rust-toolchain.toml`) for the host workspace. `make validate` is the single local gate: format check, clippy, `cargo nextest`, headers and the float ban. Set `CARGO_TARGET_DIR` outside the repository for big builds.
- **Goldens:** `QUILL_UPDATE_GOLDEN=1 cargo nextest run --workspace` (or `make goldens`) rewrites them; review the diff like code.

## Licence headers

Every source, script, YAML, TOML and SVG file starts with a one-line summary, a blank comment line and the copyright block:

```rust
// One-line summary of the file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
```

Use `#` comments for shell, YAML, TOML and Dockerfiles, and an XML comment for SVG and `.ui` files. Markdown, JSON and lockfiles don't need one. `scripts/check-headers.sh` enforces it. The project is licensed `Apache-2.0 OR MIT`.

## Architecture rules

- **Core first.** The portable crates (`quill-core`, `quill-formats`, `quill-image`, `quill-inflate`, `quill-spell`, `quill-app`, `quill-grid`, `quill-dlg`, `quill-strike`, `quill-paint`) never name a platform. Anything Windows or DOS comes in through a trait.
- **No `alloc`, no floats, no panics in portable crates.** Document-sized data goes through `Store`; layout is integer; fallible work returns `Result`. Clippy enforces it.
- **One file format.** Only `quill-formats` reads or writes documents, and every build saves the same bytes for the same document.
- **Win32 and DOS lead, Win16 follows.** Windows UI lands on Win32 first and editing behaviour on DOS text mode first; Win16 ports a working design (D34, D37).
- **SPEC first.** A change to behaviour, interfaces, settings or defaults updates `SPEC.md` in the same change.
- **Decision IDs.** New decisions in `docs/decisions.md` take an area prefix (`CORE`, `ARCH`, `DOS`, `W16`, `W9X`, `LNX`, `WUI`, `WEB`, `DSK`, `INS`, `SITE`) and a number, such as `LNX-1`, with sub-decisions as `LNX-1.1`. Legacy `D1` to `D2xx` numbers are frozen and stay valid as references; nothing new takes one.

## Testing

- `bash scripts/check-headers.sh` — SPDX headers
- `python3 -I tools/test_icons.py` — structure, depths, palettes and masks of the committed icons (needs Pillow and numpy); `bash tools/make-icons.sh` regenerates them, in Docker if the host lacks `rsvg-convert`, Pillow or numpy
- `bash scripts/check-banner.sh` — hero banner rules and text fit (needs `xmllint`, `rsvg-convert`, `python3` and Chromium; `--shots DIR` writes frames to look at)
- `python3 -I scripts/check-site.py` — every link, image, script and stylesheet in the product site (`site/`) resolves, with alt text, titles and heading order; `--external` fetches the outside links too
- `docker run --rm -v "$PWD:/workdir" davidanson/markdownlint-cli2:v0.17.2 "**/*.md"` — markdown
- `docker run --rm -v "$PWD:/mnt" -w /mnt koalaman/shellcheck:stable scripts/*.sh` — shell scripts
- `docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint` — workflows

State exactly what was and was not run when reporting results.

## Documentation

- Update `SPEC.md` when behaviour changes, and add or amend a `docs/decisions.md` entry when a decision is made. A change that alters behaviour without touching the doc that describes it is incomplete.
- `README.md` is for people who want to use or understand the project: short, scannable, no padding. Keep architecture and history out of it, and keep the `> **Status:**` line honest.
- `CLAUDE.md` is a summary of this file. Keep them in sync.

## Repository layout

```text
README.md SPEC.md AGENTS.md CLAUDE.md LICENSE-APACHE LICENSE-MIT
.editorconfig .markdownlint.jsonc .gitignore rustfmt.toml
assets/        hero banner, the app icon (`assets/icon`: SVG masters, PNGs, `QUILL.ICO`, the Windows 3.x icons)
docs/          architecture, decisions, roadmap, toolchain needs, testing, documentation plan, hero brief
scripts/       header check, banner check and its Chromium text-fit harness, site link check
tools/         the icon generator (`make_icons.py`, `make-icons.sh` for Docker) and its checks, and the site's link-preview card (`make_social_card.py`)
installer/     Quill's own installer, built with Liminal Installer (`tools/liminst.sh` finds the pinned `liminst`, `tools/stage.sh` fills `dist/`)
site/          the product page published to GitHub Pages (see `site/README.md`)
.github/       CI and Pages workflows, issue and pull request templates, release notes configuration
```

`crates/` holds the portable crates and host tools, and `tests/` the test documents and goldens. Planned, as the roadmap adds them: `win16/`, `win9x/` and `dos/` (one workspace per platform), `help/`, `dict/`, `fuzz/` and `installer/`. See [docs/architecture.md](docs/architecture.md#crates-and-workspaces).
