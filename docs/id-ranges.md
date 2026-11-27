# Decision id ranges

<!-- Generated from docs/id-ranges.toml by `python3 scripts/check-ids.py --write`. Do not edit by hand. -->

Every block of decision ids that a plan or a package has reserved, per prefix. The source of truth is [id-ranges.toml](id-ranges.toml); this page is rendered from it and CI fails when it is stale.

**Before writing a plan, reserve its block:** `scripts/next-id.sh PREFIX COUNT OWNER` finds the next free block after the highest id reserved or used for that prefix, records it here and prints it. Then name the block in the plan and in `docs/work-packages.md`. Packages take ids from inside their owner's block; `python3 scripts/check-ids.py` fails on two overlapping blocks, a decision id used twice, or a decision id outside every block. The area prefixes are in [decisions.md](decisions.md#numbering). The legacy `D` numbers are frozen history and are not recorded here.

Status: **reserved** (no rows in `decisions.md` yet), **in use** (some rows are written) or **retired** (the owner is finished and the spare ids stay unused).

## CORE

| Range | Owner | Status | Source |
|---|---|---|---|
| CORE-41 to CORE-47 | Editing review | in use | [work-packages.md](work-packages.md#decision-ids) |
| CORE-55 | Editing review | reserved | [work-packages.md](work-packages.md#decision-ids) |
| CORE-90 to CORE-99 | Optional modules (WP-MOD) | in use | [plans/modules.md](plans/modules.md) |
| CORE-100 to CORE-139 | Editor track (WP-ED) | reserved | [work-packages.md](work-packages.md#decision-ids) |
| CORE-140 to CORE-144 | Zoom review | in use | [plans/zoom-review.md](plans/zoom-review.md) |
| CORE-150 to CORE-169 | Preferences (WP-PREF) | in use | [plans/preferences.md](plans/preferences.md) |
| CORE-180 to CORE-199 | Reveal Codes (WP-CODES) | reserved | [plans/reveal-codes.md](plans/reveal-codes.md) |
| CORE-200 to CORE-209 | Status bar (WP-BAR) | reserved | [PR #182](https://github.com/liminal-hq-labs/quill/pull/182) |
| CORE-210 to CORE-229 | Unicode (WP-UNI) | reserved | [plans/unicode.md](plans/unicode.md) |

## ARCH

| Range | Owner | Status | Source |
|---|---|---|---|
| ARCH-6 to ARCH-10 | Editor track (WP-ED) | in use | [work-packages.md](work-packages.md#decision-ids) |
| ARCH-11 to ARCH-29 | Editor track (WP-ED), ARCH packages | in use | [work-packages.md](work-packages.md#decision-ids) |
| ARCH-30 to ARCH-34 | Preferences (WP-PREF) | in use | [plans/preferences.md](plans/preferences.md) |
| ARCH-35 | Reveal Codes (WP-CODES) | reserved | [plans/reveal-codes.md](plans/reveal-codes.md) |
| ARCH-40 to ARCH-42 | Optional modules (WP-MOD) | reserved | [plans/modules.md](plans/modules.md) |

## DOS

| Range | Owner | Status | Source |
|---|---|---|---|
| DOS-50 to DOS-59 | Optional modules (WP-MOD) | in use | [plans/modules.md](plans/modules.md) |
| DOS-70 to DOS-79 | Editor track (WP-ED) | in use | [work-packages.md](work-packages.md#decision-ids) |
| DOS-80 to DOS-89 | Preferences (WP-PREF) | reserved | [plans/preferences.md](plans/preferences.md) |
| DOS-90 to DOS-99 | Reveal Codes (WP-CODES) | reserved | [plans/reveal-codes.md](plans/reveal-codes.md) |
| DOS-100 to DOS-109 | Status bar (WP-BAR) | reserved | [PR #182](https://github.com/liminal-hq-labs/quill/pull/182) |
| DOS-110 to DOS-114 | Unicode (WP-UNI) | reserved | [plans/unicode.md](plans/unicode.md) |

## W16

| Range | Owner | Status | Source |
|---|---|---|---|
| W16-40 to W16-49 | Optional modules (WP-MOD) | reserved | [plans/modules.md](plans/modules.md) |
| W16-60 to W16-79 | Editor track (WP-ED) | in use | [work-packages.md](work-packages.md#decision-ids) |
| W16-80 to W16-84 | Zoom review | in use | [plans/zoom-review.md](plans/zoom-review.md) |
| W16-90 to W16-99 | Preferences (WP-PREF) | reserved | [plans/preferences.md](plans/preferences.md) |
| W16-100 to W16-104 | Reveal Codes (WP-CODES) | reserved | [plans/reveal-codes.md](plans/reveal-codes.md) |
| W16-110 to W16-114 | Status bar (WP-BAR) | reserved | [PR #182](https://github.com/liminal-hq-labs/quill/pull/182) |
| W16-115 to W16-119 | Unicode (WP-UNI) | reserved | [plans/unicode.md](plans/unicode.md) |

## W9X

| Range | Owner | Status | Source |
|---|---|---|---|
| W9X-40 to W9X-41 | Optional modules (WP-MOD) | in use | [plans/modules.md](plans/modules.md) |
| W9X-60 to W9X-79 | Editor track (WP-ED) | reserved | [work-packages.md](work-packages.md#decision-ids) |
| W9X-80 to W9X-84 | Zoom review | in use | [plans/zoom-review.md](plans/zoom-review.md) |
| W9X-90 to W9X-94 | Print to PDF (WP-WEB-3) | in use | [plans/web-demo.md](plans/web-demo.md) |
| W9X-95 to W9X-99 | Reveal Codes (WP-CODES) | reserved | [plans/reveal-codes.md](plans/reveal-codes.md) |
| W9X-100 to W9X-101 | Host integration (WP-HOST) | reserved | [plans/host-integration.md](plans/host-integration.md) |
| W9X-110 to W9X-119 | Preferences (WP-PREF) | reserved | [plans/preferences.md](plans/preferences.md) |
| W9X-120 to W9X-124 | Status bar (WP-BAR) | reserved | [PR #182](https://github.com/liminal-hq-labs/quill/pull/182) |
| W9X-125 to W9X-129 | Unicode (WP-UNI) | reserved | [plans/unicode.md](plans/unicode.md) |
| W9X-130 to W9X-139 | Optional modules on Windows 95 (WP-MOD) | reserved | [plans/modules.md](plans/modules.md) |

## LNX

| Range | Owner | Status | Source |
|---|---|---|---|
| LNX-21 to LNX-24 | UI track (WP-GTK, WP-WINUI) | in use | [ui-track.md](ui-track.md) |
| LNX-60 to LNX-64 | Preferences (WP-PREF) | reserved | [plans/preferences.md](plans/preferences.md) |
| LNX-65 to LNX-69 | Editor track (WP-ED) | reserved | [work-packages.md](work-packages.md#decision-ids) |
| LNX-70 to LNX-74 | Reveal Codes (WP-CODES) | reserved | [plans/reveal-codes.md](plans/reveal-codes.md) |
| LNX-80 to LNX-89 | GTK printing, help and dark paper (WP-GTK) | in use | [plans/gtk-print-help.md](plans/gtk-print-help.md) |
| LNX-90 to LNX-94 | Status bar (WP-BAR) | reserved | [PR #182](https://github.com/liminal-hq-labs/quill/pull/182) |
| LNX-95 to LNX-99 | Unicode (WP-UNI) | reserved | [plans/unicode.md](plans/unicode.md) |

## WUI

| Range | Owner | Status | Source |
|---|---|---|---|
| WUI-1 to WUI-2 | UI track (WP-GTK, WP-WINUI) | in use | [ui-track.md](ui-track.md) |
| WUI-60 to WUI-64 | Preferences (WP-PREF) | reserved | [plans/preferences.md](plans/preferences.md) |
| WUI-65 to WUI-69 | Editor track (WP-ED) | reserved | [work-packages.md](work-packages.md#decision-ids) |
| WUI-70 to WUI-74 | Reveal Codes (WP-CODES) | reserved | [plans/reveal-codes.md](plans/reveal-codes.md) |
| WUI-80 to WUI-84 | Status bar (WP-BAR) | reserved | [PR #182](https://github.com/liminal-hq-labs/quill/pull/182) |
| WUI-85 to WUI-94 | WinUI real window (WP-WINUI) | in use | [plans/winui-real-window.md](plans/winui-real-window.md) |
| WUI-95 to WUI-97 | Unicode (WP-UNI) | reserved | [plans/unicode.md](plans/unicode.md) |

## INS

| Range | Owner | Status | Source |
|---|---|---|---|
| INS-8 to INS-9 | Preferences (WP-PREF) | reserved | [plans/preferences.md](plans/preferences.md) |
| INS-20 to INS-22 | Optional modules (WP-MOD) | reserved | [plans/modules.md](plans/modules.md) |
| INS-23 to INS-25 | Printable media set | in use | assets/media/README.md |
| INS-26 to INS-28 | Optional modules on Windows 95 (WP-MOD) | reserved | [plans/modules.md](plans/modules.md) |

## SITE

| Range | Owner | Status | Source |
|---|---|---|---|
| SITE-14 to SITE-19 | Web demo (WP-WEB) | in use | [plans/web-demo.md](plans/web-demo.md) |
| SITE-20 to SITE-24 | Pointer (WP-PTR) | in use | [plans/pointer.md](plans/pointer.md) |
| SITE-30 to SITE-39 | Host integration (WP-HOST) | in use | [plans/host-integration.md](plans/host-integration.md) |
| SITE-40 | Web demo power state (fix/power-state) | in use | [PR #183](https://github.com/liminal-hq-labs/quill/pull/183) |
| SITE-41 to SITE-50 | First demo launch (WP-LAUNCH) | reserved | [plans/first-demo-launch.md](plans/first-demo-launch.md) |

## Grandfathered ids

Ids that `decisions.md` already used outside every block when the registry began. They stay as they are; a new id outside a block is an error.

| Prefix | Numbers |
|---|---|
| CORE | 1, 3 to 17, 20, 39 to 40, 61 to 70 |
| ARCH | 1 to 5 |
| DOS | 1 to 2, 10 to 11, 13 to 48 |
| W16 | 1 to 21 |
| W9X | 1 to 16 |
| LNX | 1 to 20, 25 to 28 |
| INS | 1 to 7 |
| SITE | 1 to 13 |
| CI | 1 |

## Known clashes

Recorded as found, not renumbered. The check accepts exactly these and fails if one stops applying.

- **Duplicate CORE-64:** The default-extension rule and the page view scrolling in pixels; docs/work-packages.md, Found while planning, says the second takes a sub-number under CORE-65 the next time it is touched.

## Open pull requests

Ids and blocks on open pull request branches, noted when the registry was last updated.

- [PR #182](https://github.com/liminal-hq-labs/quill/pull/182) (`docs/status-bar-plan`). Plans one status bar layout. Reserves the Status bar blocks in its docs/work-packages.md; adds no rows to docs/decisions.md yet.
