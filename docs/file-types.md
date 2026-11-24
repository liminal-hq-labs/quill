# File types

How Linux and other Unix-like systems recognise Quill's files. Nothing here changes how Quill reads files: it reads by content (see `SPEC.md`).

## What a `.QLD` file looks like

A `.QLD` file is a RIFF file with form type `QLD1`. The first chunk is `HEAD`, which holds the format version as two little-endian 16-bit numbers, major then minor. So the bytes start `RIFF`, a length, `QLD1`, `HEAD`, a length of 8, then the version. The format is described in `docs/decisions.md` (D12) and written by `crates/quill-formats/src/qld.rs`.

## `file` and libmagic

`assets/magic/quill.magic` teaches `file` to report a Quill document with its format version and the MIME type `application/x-quill-document`:

```sh
file -m assets/magic/quill.magic tests/docs/letter.qld
# tests/docs/letter.qld: Quill document, version 1.0
```

To use it everywhere, copy it to `~/.magic` (a per-user file) or add its contents to `/etc/magic`. To compile it for libmagic, run `file -C -m assets/magic/quill.magic`, which writes `quill.magic.mgc`. `scripts/check-magic.sh` checks it against the sample documents and runs as part of `make validate`.

## File managers

`assets/mime/quill.xml` is a shared-mime-info package. It names the type "Quill document" and matches on the same bytes, and on the `.qld` extension:

```sh
cp assets/mime/quill.xml ~/.local/share/mime/packages/
update-mime-database ~/.local/share/mime
```

RTF and plain text need nothing: the system already knows them.
