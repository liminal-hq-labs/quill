# Provenance

The Rust-on-DOS foundation in this folder comes from [Desk](https://github.com/liminal-hq/desk)'s `dos/`, by way of the Liminal Installer's trimmed copy in `dos/`, which dropped the hardware-proof modules. House rules allow copies with provenance, so the files are kept close to the originals and every change is listed here.

- **Source:** `liminal-hq/desk`, commit `4a021a7` ("Record the layered layout as merged in the Manual and the progress report (#100)"), through `liminal-hq-labs/liminal-installer` `dos/` as of November 2026.
- **Licence:** Apache-2.0 OR MIT, the same as this repository; every copied file keeps its header.

## What was copied

| Here | From | Changes |
|---|---|---|
| `dos-elf.json`, `link.ld`, `.cargo/config.toml`, `rustfmt.toml` | The installer's `dos/` | None. The i486 ELF target, its linker script, the `-Zbuild-std` settings and the wider rustfmt width |
| `rt/` (`start`, `dpmi`, `dos`, `heap`, `env`, `fpu`, `io`, `panic`, `cleanup`) | The installer's `dos/rt/` | Quill adds folder, drive, rename and INT 24h calls (below). Entry, DPMI client, INT 21h, heap, command tail and environment, x87, `println!`, panic and clean-up |
| `tools/elf2go32.py`, `tools/dosrun.sh`, `tools/fetch-tools.sh`, `tools/frames.py` | The installer's `dos/tools/` | `fetch-tools.sh` takes `--stub-only`, which skips DOSBox Staging (the browser demo's build needs only the stub). The others unchanged. Packs the ELF behind a go32 stub; runs a command in headless DOSBox Staging with typed keys and a scratch drive; fetches DOSBox Staging and CWSDPMI with checksums |
| `quill-dos/src/video.rs` | The installer's `dos/setup/src/video.rs` | Draws a `quill_grid::Grid` instead of the installer's `Cells`; adds `read_back` and `wait_key`; `enter` takes the rows and sets 80 × 50 with INT 10h AX=1112h when the BIOS accepts it |
| `hal/hal` (`desk-hal`) | Desk's `hal/hal`, whole | None to the sources. Its `Cargo.toml` is unchanged. The traits and types the drivers are written against: port I/O, MMIO regions, the error type, video modes and frame buffers, `Bios`, input events, IRQ and ring types |
| `hal/drivers/vga` (`desk-vga`) | Desk's `hal/drivers/vga` | None. Mode 12h and 13h register tables, the DAC, the planar and linear canvases, VBE 4F00h to 4F05h, the software cursor, the text-mode return |
| `hal/drivers/int33` (`desk-int33`) | Desk's `hal/drivers/int33` | None. The INT 33h mouse interface over `Bios`, and the DOS input source |
| `hal/drivers/kbd`, `hal/drivers/i8042` | Desk's `hal/drivers` | None. `desk-int33` depends on `desk-kbd`, and its tests use `desk-i8042`'s keyboard interrupt service |
| `hal/platform/mock` (`desk-hal-mock`) | Desk's `hal/platform/mock` | None. The register-file model, the scripted BIOS and the INT 33h model that the host tests run against |
| `hal/Cargo.toml`, `hal/clippy.toml` | Desk's `hal/` | Drops the `serial`, `pit` and `pic` members, which Quill does not use. `clippy.toml` keeps the ban on compare-and-swap (DOSBox with `cputype = 486` rejects `CMPXCHG`) and gains a header |
| `hal/rustfmt.toml`, `hal/.cargo/config.toml` | Quill's | Hard tabs at 130 columns. The config sets the host target and clears `-Zbuild-std`, so `cd dos/hal && cargo test` runs on the host although `dos/.cargo/config.toml` builds for DOS |
| `rt/src/bios.rs` | `dos/rt/src/hal.rs` (`DosBios`, `buffer_segment`) | Only `DosBios`: `DosDisplay` and the mode restore stay out until `quill-gfx` needs them. A new file header |
| `build.sh`, `check.sh` | The installer's `dos/` | Build `quill-dos` and pack it as `QUILLDOS.EXE` |

Every copied file has its Desk header, which names the same copyright holder and licence as this repository's, so none needed changing. The files were reformatted with hard tabs (`cargo fmt` in `dos/hal`), and nothing else in them was touched.

## What is Quill's own

`quill-dos/` (the program), `tools/e2e.sh` (the golden comparison) and the screens in `crates/quill-grid`.

`rt/src/mem.rs` is written here, not copied: `memcpy` and `memset` with `rep movsd` and `rep stosd`. Desk's `dos/rt/src/mem.rs` is a byte-wise version for its Windows-flavoured COFF build and is not part of its ELF runtime (its roadmap lists the string-move version as planned), so there was nothing to copy. `hal-probe/` and `tools/hal-probe.sh` are also Quill's: a probe that draws and reads back a picture in mode 12h and in VBE 640 x 480 x 8. Desk's `spikes/a-vga-vbe` is the model.

Added to the copied `rt/` (the end of `rt/src/dos.rs`, and `free_memory` in `rt/src/dpmi.rs`): searching a folder (`find_first`, `find_next`, `find_each`, `DirEntry`), drives (`current_drive`, `set_current_drive`, `current_dir`, `drive_attributes`, `drive_valid`, `logical_drive_map`, `floppy_count`, `drive_mask`), `rename`, `set_attributes`, `dos_version` and the INT 24h stub (`install_critical_handler`, `remove_critical_handler`). Refreshing from Desk keeps them. `rt-test/` and `tools/rt-test.sh` check them under headless DOSBox.

## Still to add (roadmap T4 and M1 to M2)

The installer's copy dropped Desk's keyboard, timer and VGA modules; the VGA and INT 33h drivers are back as `hal/`; `quill-dos` uses `desk-hal` and `desk-vga` (mode 12h registers, the DAC, VBE) for its graphics screen, and the INT 33h client is not wired in. Quill reads the keyboard through BIOS INT 16h (copied in M1 from the installer's `keys.rs`) and writes B8000h directly, which needs none of them. The mouse client is `desk-int33`, not yet wired in.

## Refreshing

1. Check out Desk at the commit to take, and diff each file in the table against its original, ignoring whitespace (`diff -w`).
2. Copy the changed files over, then run `cargo fmt --all` in `dos/` to put the hard tabs back.
3. Re-apply the changes in the table.
4. Run `dos/check.sh` with the tools fetched, and update the commit hash above.
