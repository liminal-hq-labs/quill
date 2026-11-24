# Plans

Planner write-ups kept for reference. Each was accurate when it was written, and none is updated afterwards. Where a plan disagrees with [decisions.md](../decisions.md) or a topic document ([dos-graphics.md](../dos-graphics.md), [linux.md](../linux.md), [winui.md](../winui.md), and [clipboard.md](../clipboard.md) and [dialogs.md](../dialogs.md) where they exist), the decision or the topic document wins. Decision ids in the plans are as written at the time; new decisions use per-area prefixes such as ARCH-1, as [decisions.md](../decisions.md) describes once the prefix scheme has merged.

| Plan | What it covers | Status |
| --- | --- | --- |
| [architecture-review.md](architecture-review.md) | Four layers, crate splits and the migration plan | Superseded in part: the layer work has begun and the crate names have moved on |
| [gap-analysis.md](gap-analysis.md) | What each build lacks, written against the work then in flight | Superseded in part: a snapshot of one day's branches |
| [dos-dialogs-plan.md](dos-dialogs-plan.md) | A proper DOS file dialog, and a review of every other dialog | Reference |
| [dos-flow-parity-plan.md](dos-flow-parity-plan.md) | Making the DOS dialog flows follow the Windows 3.1 and Windows 95 builds | Reference |
| [dos-graphics-plan.md](dos-graphics-plan.md) | Graphics mode beside text mode in `QUILLDOS.EXE` | Superseded in part: [dos-graphics.md](../dos-graphics.md) is the design, and `quill-strike` and `quill-paint` exist |
| [linux-client-plan.md](linux-client-plan.md) | A native GTK4 client | Superseded in part: [linux.md](../linux.md) is the design, and `linux/quill-gtk` exists |
| [winui-3-plan.md](winui-3-plan.md) | A WinUI 3 build for Windows 11 | Superseded in part: [winui.md](../winui.md) is the design |
| [installer-plan.md](installer-plan.md) | A Quill installer built with Liminal Installer | Reference |
| [web-and-layering-addendum.md](web-and-layering-addendum.md) | GTK4, WinUI 3, web (wasm32) and Desk against the layering | Later, not scheduled |
| [threading-plan.md](threading-plan.md) | One job model for single-core and threaded builds | Later, not scheduled |

The Desk page, [desk-app.md](../desk-app.md), is out of date against D130, so the plans that lean on it (the web and layering addendum in particular) inherit that.
