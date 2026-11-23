# Dictionaries

`scowl/` holds the SCOWL (Spell Checker Oriented Word Lists) word lists that `quill-dict` turns into `EN_CA.DIC`, `EN_US.DIC` and `EN_GB.DIC`, with SCOWL's own `COPYRIGHT` file, which every built dictionary also carries inside itself. SCOWL is Copyright 2000 to 2018 Kevin Atkinson and others and may be copied and used for any purpose provided its copyright notice and permission notice appear in all copies.

- **Source.** SCOWL 2020.12.07, the `final` word lists at sizes 10, 20, 35, 40 and 50 of `english`, `american`, `british` and `canadian`: words, capitalised words, proper names, contractions and abbreviations.
- **Build.** `cargo run -p quill-dict -- dist/dic` writes the three files. Canadian takes the English, Canadian and American lists, American the English and American, British the English and British. At size 50 each file is under 300 KB.
- **`test-words.txt`** is the few words of the spelling scenarios; `tests/docs/en_ca.dic` is built from it by `make-docs`.
