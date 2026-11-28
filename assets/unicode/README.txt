Quill's Unicode tables

UNITAB.BIN holds every table of crates/quill-text's cluster, breaks, word, fold, compose, width,
script and blocks modules in one blob (its layout is in crates/quill-text/src/unitab.rs), for a
build that serves the tables from a resource instead of from compiled-in data (Windows 3.x,
where const data is the one data segment). No build links the tables or the blob yet.

Everything here is generated from the Unicode Character Database 16.0.0 by scripts/gen-unicode.py.
Do not edit UNITAB.BIN or the generated sections of the Rust files by hand.

The files the generator reads go in ~/.cache/quill-ucd/16.0.0/ (or the folder in QUILL_UCD), all
in that one folder, whatever their place in the UCD. ucd-16.0.0.sha256 lists their SHA-256 and the
generator refuses a file that does not match. Download them with:

  base=https://www.unicode.org/Public/16.0.0/ucd
  mkdir -p ~/.cache/quill-ucd/16.0.0 && cd ~/.cache/quill-ucd/16.0.0
  for f in Blocks.txt CaseFolding.txt EastAsianWidth.txt LineBreak.txt Scripts.txt UnicodeData.txt \
      auxiliary/GraphemeBreakProperty.txt auxiliary/GraphemeBreakTest.txt auxiliary/LineBreakTest.txt \
      emoji/emoji-data.txt extracted/DerivedGeneralCategory.txt; do curl -fsSLO "$base/$f"; done

Then:

  python3 -I scripts/gen-unicode.py            rewrite the Rust tables, UNITAB.BIN, the goldens and the trimmed test files
  python3 -I scripts/gen-unicode.py --check    fail when any of them is out of date (make unicode)
  python3 -I scripts/gen-unicode.py --sizes    print the bytes of each table

Without the files, --check checks only that UNITAB.BIN holds what the committed Rust tables hold,
which needs no download and is what CI runs.

Unicode data files are used under the Unicode License v3 (https://www.unicode.org/terms_of_use.html).
The trimmed copies of GraphemeBreakTest.txt and LineBreakTest.txt in crates/quill-text/tests/ucd/ keep
their copyright and licence header.
