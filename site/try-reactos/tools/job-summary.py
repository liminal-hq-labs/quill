#!/usr/bin/env python3
# Print the demo image build manifest as a Markdown job summary: sizes, stage timings, pinned inputs, and the source and licence
# material that publishing the image would require. Usage: job-summary.py build-manifest.json
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import json, sys

m = json.load(open(sys.argv[1]))
mb = lambda n: "%.1f MB" % (n / 1e6)
pin = m["pinned"]
out = ["## ReactOS demo image", "", "### Outputs", "", "| Output | Size | SHA-256 |", "|---|---:|---|"]
for name, o in sorted(m["outputs"].items()):
	out.append("| `%s` | %s | `%s` |" % (name, mb(o["bytes"]), o["sha256"][:16] + "…"))
if m.get("parts"):
	out.append("| `parts/` (%d files, with `manifest.json`) | %s | see `parts/manifest.json` |" % (m["parts"]["files"], mb(m["parts"]["bytes"])))
out += ["", "### Stage timings", "", "| Stage | Seconds |", "|---|---:|"]
out += ["| %s | %d |" % (k, v) for k, v in m["timings_seconds"].items()]
out += ["", "Peak work-directory use: %d MB allocated." % m["peak_work_disk_mb"], "", "### Pinned inputs", ""]
out += [
	"- **ReactOS:** %s; zip SHA-256 `%s`, ISO SHA-256 `%s`" % (pin["reactos"], pin["reactos_iso_zip_sha256"], pin["reactos_iso_sha256"]),
	"- **Answers file:** `unattend.inf` SHA-256 `%s`" % pin["unattend_inf_sha256"],
	"- **v86:** %s" % pin["v86"],
	"- **Quill commit:** `%s`" % pin["quill_commit"],
]
out += ["- **Quill builds:** `%s` `%s`" % (k, v[:16] + "…") for k, v in pin["quill_builds"].items()]
out += [
	"",
	"### Before this is published anywhere",
	"",
	"The artifact is a ReactOS disk image. ReactOS is GPL-2.0-or-later, with LGPL-2.1-or-later for the Wine-derived DLLs and other free licences for bundled parts, so publishing the image means publishing, or offering in writing, the corresponding source and the licence texts. It stays a workflow artifact (7 days) until that is settled. The material to publish with it:",
	"",
	"- **ReactOS 0.4.16 source:** tag `0.4.16-release` (commit `822e8647792a2c43f033e3e00ed273d59f163c7b`) of <https://github.com/reactos/reactos>, as an archive beside the image, with `COPYING`, `COPYING.LIB` and the licence files under `media/fonts/doc/`.",
	"- **WineVDM (otvdm):** the build in `Program Files\\otvdm` (GPL-2.0): its exact release, source archive and licence from <https://github.com/otya128/winevdm>.",
	"- **v86:** the emulator is BSD-2-Clause; `bios/seabios.bin` and `bios/vgabios.bin` are SeaBIOS and SeaVGABIOS 1.16.2 (LGPL-3.0), so offer that source (see `site/try-reactos/v86/PROVENANCE.md`).",
	"- **What we changed:** the trim lists (`guest/trim-list-*.txt`), `guest/demo-settings.reg`, `guest/unattend.inf`, the wallpaper, the two desktop shortcuts, and the Quill programs in `C:\\Quill` (Apache-2.0 OR MIT).",
	"- **Fonts:** the image carries ReactOS's metric-compatible clones, not Microsoft's fonts; check a font inventory before publishing.",
]
print("\n".join(out))
