# Test fonts

`QuillTestSerif`, `QuillTestSans` and `QuillTestMono` (regular, bold, italic and bold italic of each) are the fonts `quill-fonts` carries so that its layout dumps and tests give the same widths on every machine. They are subsets of [Liberation](https://github.com/liberationfonts/liberation-fonts) 2.1.5 (Serif, Sans and Mono), which are metric compatible with Times New Roman, Arial and Courier New.

- **What was kept:** the 224 characters of Windows-1252 from 0x20 to 0xFF (as Unicode), `.notdef`, and the `cmap`, `glyf`, `loca`, `head`, `hhea`, `hmtx`, `maxp`, `name`, `OS/2` and `post` tables. Hinting, kerning and layout tables are dropped, since Quill measures and draws unhinted outlines.
- **How they were made:** `tools/make-test-fonts.py LIBERATION_DIR fonts` with the `ttf-liberation` 2.1.5 package's files and fontTools. The script is the whole recipe.
- **Licence:** the SIL Open Font License 1.1, in `LICENSE-LIBERATION.txt` exactly as the Liberation package ships it. The OFL lets the fonts be bundled, modified and redistributed with software as long as they are not sold by themselves and the licence and copyright notices stay with them.
- **Reserved Font Names:** the Liberation licence names them: "Liberation" (Red Hat, Inc.) and "Arimo, Tinos and Cousine" (Google Corporation). A subset is a Modified Version, and a Modified Version may not use a Reserved Font Name, so the subsets are renamed (family `Quill Test Serif`, `Quill Test Sans` and `Quill Test Mono`) and the name table keeps only the copyright, licence, designer and version entries; the trademark and description entries, which name the reserved fonts, are dropped.
- **Source files:** the SHA-256 sums of the Liberation 2.1.5 files used are in `SOURCES.txt`.

Do not add Times New Roman, Arial or Courier New here; they are not redistributable.
