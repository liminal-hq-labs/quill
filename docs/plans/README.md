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
| [zoom-review.md](zoom-review.md) | Root causes of the zoom, preview, paging, style box and toolbar items of the 2026-11-27 QA, and one builder brief per fix | Reference |
| [reveal-codes.md](reveal-codes.md) | Reveal Codes, a nod to WordPerfect: the code vocabulary, one view model, a pane on every build, and WP-CODES-1 to WP-CODES-10 | Planned |
| [web-demo.md](web-demo.md) | The ReactOS web demo: a clean address, Print to PDF with downloads, sound and hosting, as WP-WEB-1 to WP-WEB-8 | Planned |
| [preferences.md](preferences.md) | One settings model (`quill-settings`) and an Options window on every build, autosave first, as WP-PREF-0 to WP-PREF-13 | Planned |
| [gtk-print-help.md](gtk-print-help.md) | GTK printing, preview and Export as PDF from one `quill-pdfdoc` PDF with a text layer, the help viewer and dark paper, as WP-GTK-3a to 3c, WP-GTK-8 and WP-GTK-10 | Planned |
| [Shared libraries](https://github.com/liminal-hq-labs/liminal-installer/blob/main/docs/plans/shared-libraries.md) (in liminal-installer) | DLLs, `.so` and static sharing across the Labs retro projects. For Quill: no DLL or `.so` for `quill-pdf`, `quill-pdfdoc` or the kit crates (one Quill build per system); `QUILLPIC.DLL` on Windows 3.x after rust-win16's DLL runtime passes its go/no-go (WP-SL-2, then WP-SL-5), using the W16-40 to W16-48 ids the modules and DLLs plan reserved; the standalone `AGQUILL.SCR` stays what Quill's installers ship | Planned |
| [pointer.md](pointer.md) | One pointer module for the Try Quill and ReactOS pages: Follow by default, Capture where raw counts exist, and a direction-error measure, as WP-PTR-1 to WP-PTR-5 | Planned |

The Desk page, [desk-app.md](../desk-app.md), is out of date against D130, so the plans that lean on it (the web and layering addendum in particular) inherit that.
