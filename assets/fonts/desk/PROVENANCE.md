# Desk fonts

Desk Serif, Sans and Mono, each in Regular, Bold, Italic and Bold Italic, are the outline faces `quill-fntgen` renders into `QUILL.FNT`. They are copied unchanged from [Desk](https://github.com/liminal-hq/desk) `fonts/generated/` at commit `4a021a7`, the commit `dos/PROVENANCE.md` names.

- **Licence:** SIL Open Font License 1.1, copyright 2026 Liminal HQ, Scott Morris, in `OFL.txt` exactly as Desk ships it. The copyright statement names no Reserved Font Name, so a rendering of the faces may keep the names "Desk Serif", "Desk Sans" and "Desk Mono". The licence text must still travel with any bundled copy or rendering, so `OFL.TXT` goes on the DOS disk beside `QUILL.FNT` and the About box names it.
- **Checksums:** `SHA256SUMS` lists the SHA-256 of every file here (`sha256sum -c SHA256SUMS`).
- **Hinting:** the TrueType files carry no hinting instructions (no `fpgm`, `prep` or `cvt`), so skrifa's automatic hinter does the hinting.
- **Refreshing:** copy the twelve `desk-{serif,sans,mono}[-bold][-italic].ttf` files and `OFL.txt` from Desk, run `sha256sum *.ttf OFL.txt > SHA256SUMS`, and regenerate the proof sheets and the report (`QUILL_UPDATE_GOLDEN=1 cargo nextest run -p quill-fntgen`).
