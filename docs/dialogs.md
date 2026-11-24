# Dialogs

`assets/dialogs/dialogs.json` describes Quill's five resource dialogs: About, Find and Replace, Fonts, Tabs and Spelling. The Win16 packer (`ne_pack.py`) and the Win32 packer (`res_pack.py`) read it to build `RT_DIALOG` resources, and `quill-dialogs` carries the same description as Rust constants (ARCH-5).

## The file

Each dialog has an `id`, a `name`, a `caption`, a position and size in dialog units, and `items`. Each item has a `class` (`static`, `button`, `edit`, `listbox`, `combobox`, `scrollbar`), an `id`, `text` with `&` before the mnemonic letter, its geometry, an optional `icon` resource ID and `style` names. `name` is optional on a control; the packers ignore it, and a control with no name gets no constant. Names are lower case with underscores and unique within a dialog (`ID` and `DIALOG` are reserved).

The style names are the ones in `quill_dialogs::style`: `pushbutton`, `defpushbutton`, `autocheckbox`, `autoradiobutton`, `groupbox`, `icon`, `tabstop`, `group`, `border`, `vscroll`, `autohscroll`, `notify`, `dropdown`, `dropdownlist`. A new class or style is added to the generator in `crates/quill-host/src/dialoggen.rs` and to `quill-dialogs` together; the generator refuses a name it does not know.

## The generated crate

```sh
cargo run -p quill-host --bin emit-resources -- --dialogs          # rewrites crates/quill-dialogs/src/generated.rs
cargo run -p quill-host --bin emit-resources -- --dialogs --check  # fails when it is out of date
QUILL_UPDATE_GOLDEN=1 cargo nextest run -p quill-dialogs           # the same rewrite, from the test
```

`quill_dialogs::find` holds `ID`, a constant for each named control (`WHAT`, `WITH`, `MATCH_CASE`, `FIND_NEXT`) and `DIALOG`, a `Dialog` with the caption, geometry, default button and an `Item` for every control. `quill_dialogs::ALL` lists the dialogs in file order and `by_id` finds one by resource ID. `Item::mnemonic` and `Dialog::item_for_mnemonic` read the `&` labels, so the text-cell dialogs and a shell's Alt handling share one answer.

## What the tests hold in place

- `generated.rs` is exactly what `dialogs.json` generates.
- The constants carry every caption, ID, label, icon and rectangle of the JSON.
- Every dialog has one default button, unique control IDs and unique mnemonics, apart from two pairs in Tabs that exist today (`Tab &stops:` and `&Set`, `&Left` and `C&lear`); a new duplicate fails.
- The `DLG_*` constants in `win16/examples/quill/dialogs.rs` and `win9x/quill-win9x/src/dialogs.rs`, and the `5:NNNN` list in `win16/build.sh`'s resource check, equal the generated dialog IDs.
- Every control ID a shell writes as a number in a dialog procedure (the argument after `dlg`, a match arm, an `id ==` test, the radio-group ranges) belongs to some dialog, and the controls the shells rely on keep the IDs they use.

The shells still write control IDs as numbers. They adopt the constants in their own changes, after which the last two checks become compile-time facts.
