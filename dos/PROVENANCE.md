# Provenance

The Rust-on-DOS foundation in this folder comes from [Desk](https://github.com/liminal-hq/desk)'s `dos/`, by way of the Liminal Installer's trimmed copy in `dos/`, which dropped the hardware-proof modules. House rules allow copies with provenance, so the files are kept close to the originals and every change is listed here.

- **Source:** `liminal-hq/desk`, commit `4a021a7` ("Record the layered layout as merged in the Manual and the progress report (#100)"), through `liminal-hq-labs/liminal-installer` `dos/` as of November 2026.
- **Licence:** Apache-2.0 OR MIT, the same as this repository; every copied file keeps its header.

## What was copied

| Here | From | Changes |
|---|---|---|
| `dos-elf.json`, `link.ld`, `.cargo/config.toml`, `rustfmt.toml` | The installer's `dos/` | None. The i486 ELF target, its linker script, the `-Zbuild-std` settings and the wider rustfmt width |
| `rt/` (`start`, `dpmi`, `dos`, `heap`, `env`, `fpu`, `io`, `panic`, `cleanup`) | The installer's `dos/rt/` | Quill adds folder, drive, rename and INT 24h calls (below). Entry, DPMI client, INT 21h, heap, command tail and environment, x87, `println!`, panic and clean-up |
| `tools/elf2go32.py`, `tools/dosrun.sh`, `tools/fetch-tools.sh`, `tools/frames.py` | The installer's `dos/tools/` | None. Packs the ELF behind a go32 stub; runs a command in headless DOSBox Staging with typed keys and a scratch drive; fetches DOSBox Staging and CWSDPMI with checksums |
| `quill-dos/src/video.rs` | The installer's `dos/setup/src/video.rs` | Draws a `quill_grid::Grid` instead of the installer's `Cells`; adds `read_back` and `wait_key` |
| `build.sh`, `check.sh` | The installer's `dos/` | Build `quill-dos` and pack it as `QUILLDOS.EXE` |

## What is Quill's own

`quill-dos/` (the program), `tools/e2e.sh` (the golden comparison) and the screens in `crates/quill-grid`.

Added to the copied `rt/` (the end of `rt/src/dos.rs`, and `free_memory` in `rt/src/dpmi.rs`): searching a folder (`find_first`, `find_next`, `find_each`, `DirEntry`), drives (`current_drive`, `set_current_drive`, `current_dir`, `drive_attributes`, `drive_valid`, `logical_drive_map`, `floppy_count`, `drive_mask`), `rename`, `set_attributes`, `dos_version` and the INT 24h stub (`install_critical_handler`, `remove_critical_handler`). Refreshing from Desk keeps them. `rt-test/` and `tools/rt-test.sh` check them under headless DOSBox.

## Still to add (roadmap T4 and M1 to M2)

The installer's copy dropped Desk's keyboard, timer and VGA modules. Quill reads the keyboard through BIOS INT 16h (copied in M1 from the installer's `keys.rs`) and writes B8000h directly, which needs none of them. An INT 33h mouse client is new work for M2.

## Refreshing

1. Check out Desk at the commit to take, and diff each file in the table against its original, ignoring whitespace (`diff -w`).
2. Copy the changed files over, then run `cargo fmt --all` in `dos/` to put the hard tabs back.
3. Re-apply the changes in the table.
4. Run `dos/check.sh` with the tools fetched, and update the commit hash above.
