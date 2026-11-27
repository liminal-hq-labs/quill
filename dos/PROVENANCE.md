# Provenance

The Rust-on-DOS foundation in this folder comes from [Desk](https://github.com/liminal-hq/desk)'s `dos/`, by way of the Liminal Installer's trimmed copy in `dos/`, which dropped the hardware-proof modules. House rules allow copies with provenance, so the files are kept close to the originals and every change is listed here.

- **Source:** `liminal-hq/desk`, commit `4a021a7` ("Record the layered layout as merged in the Manual and the progress report (#100)"), through `liminal-hq-labs/liminal-installer` `dos/` as of November 2026.
- **Licence:** Apache-2.0 OR MIT, the same as this repository; every copied file keeps its header.

## What was copied

| Here | From | Changes |
|---|---|---|
| `dos-elf.json`, `link.ld`, `rustfmt.toml` | The installer's `dos/` | None. The i486 ELF target, its linker script, the `-Zbuild-std` settings and the wider rustfmt width |
| `rt/` (`start`, `dpmi`, `dos`, `heap`, `env`, `fpu`, `io`, `panic`, `cleanup`) | The installer's `dos/rt/` | Quill adds folder, drive, rename and INT 24h calls (below), and `File::finish`, which commits and closes a written file and reports the close's error. Entry, DPMI client, INT 21h, heap, command tail and environment, x87, `println!`, panic and clean-up |
| `tools/elf2go32.py`, `tools/dosrun.sh`, `tools/fetch-tools.sh`, `tools/frames.py` | The installer's `dos/tools/` | `fetch-tools.sh` takes `--stub-only`, which skips DOSBox Staging (the browser demo's build needs only the stub). The others unchanged. Packs the ELF behind a go32 stub; runs a command in headless DOSBox Staging with typed keys and a scratch drive; fetches DOSBox Staging and CWSDPMI with checksums |
| `quill-dos/src/video.rs` | The installer's `dos/setup/src/video.rs` | Draws a `quill_grid::Grid` instead of the installer's `Cells`; adds `read_back` and `wait_key`; `enter` takes the rows and sets 80 × 50 with INT 10h AX=1112h when the BIOS accepts it |
| `hal/hal` (`desk-hal`) | Desk's `hal/hal`, whole | None to the sources. Its `Cargo.toml` is unchanged. The traits and types the drivers are written against: port I/O, MMIO regions, the error type, video modes and frame buffers, `Bios`, input events, IRQ and ring types |
| `hal/drivers/vga` (`desk-vga`) | Desk's `hal/drivers/vga` | None. Mode 12h and 13h register tables, the DAC, the planar and linear canvases, VBE 4F00h to 4F05h, the software cursor, the text-mode return |
| `hal/drivers/int33` (`desk-int33`) | Desk's `hal/drivers/int33` | None. The INT 33h mouse interface over `Bios`, and the DOS input source |
| `hal/drivers/pic` (`desk-pic`), `rt/src/irq.rs` | Desk's `hal/drivers/pic` and `dos/rt/src/irq.rs` at 4a021a7 | None beyond the hard tabs. The 8259 driver and the DOS interrupt registry (a protected-mode stub for each IRQ line through DPMI 0204h and 0205h, locked memory, the end-of-interrupt); `quill-dos` uses them for IRQ 12 when no INT 33h driver is loaded (DOS-27) |
| `hal/drivers/i8042` (`start_mouse` and `stop_mouse`), `hal/drivers/int33` (the wheel's `CX` bit), `hal/platform/mock/src/int33.rs` | Desk PR #125 (`feat/hal-i8042-mouse-only`) and PR #124 (`fix/int33-wheel-capability`), both open | The files of those two branches, on the 4a021a7 base (nothing else in these crates changed upstream since): pending Desk PR #124 and #125. Do not edit them here without the Desk change |
| `hal/drivers/kbd`, `hal/drivers/i8042` | Desk's `hal/drivers` | Their `Cargo.toml` (and `pic`'s) gain the `[package.metadata.quill]` table the layer check wants, as `desk-vga`'s has. `desk-int33` depends on `desk-kbd`, and its tests use `desk-i8042`'s keyboard interrupt service |
| `hal/platform/mock` (`desk-hal-mock`) | Desk's `hal/platform/mock` | None. The register-file model, the scripted BIOS and the INT 33h model that the host tests run against |
| `hal/Cargo.toml`, `hal/clippy.toml` | Desk's `hal/` | Drops the `serial` and `pit` members, which Quill does not use (`pic` came back for the IRQ 12 handler). `clippy.toml` keeps the ban on compare-and-swap (DOSBox with `cputype = 486` rejects `CMPXCHG`) and gains a header |
| `hal/rustfmt.toml`, `hal/.cargo/config.toml` | Quill's | Hard tabs at 130 columns. The config sets the host target and clears `-Zbuild-std`, so `cd dos/hal && cargo test` runs on the host although `dos/.cargo/config.toml` builds for DOS |
| `rt/src/bios.rs` | `dos/rt/src/hal.rs` (`DosBios`, `buffer_segment`) | Only `DosBios`: `DosDisplay` and the mode restore stay out until `quill-gfx` needs them. A new file header |
| `.cargo/config.toml` | The installer's `dos/` | Adds `-C llvm-args=-inline-threshold=20`, which holds `QUILLDOS.EXE` under its budget (DOS-28) |
| `build.sh`, `check.sh` | The installer's `dos/` | Build `quill-dos` and pack it as `QUILLDOS.EXE` |
| `module/format` (`desk-module`), `module/pack` (`dmdpack`), `module/guest` (`desk-guest`) | Desk's `module/` at `862ec02` ("Refresh the Manual after the R139 code tasks (#127)") | The sources only reformatted with hard tabs. Three `Cargo.toml` files gain the `[package.metadata.quill]` table the layer check wants (`desk-module` and `desk-guest` base, `no_std`; `dmdpack` tool). One test path changed, below. The `.DMD` and `.DKX` format (parse, validate, write, `load_into`), the host packer and its cross check, and the module side's import slots, entry points, logging and panic handler. The format is specified in Desk's `docs/LOADER.md` |
| `module/pack/tests/real.rs` | Desk's `module/pack/tests/real.rs` | Builds its test module for `dos/dos-elf.json` instead of Desk's kernel target, and passes its link flags in `CARGO_ENCODED_RUSTFLAGS`, because the build now sits under `dos/`, whose `.cargo/config.toml` would otherwise add `QUILLDOS.EXE`'s linker script. No assertion changed |
| `module/Cargo.toml`, `module/Cargo.lock`, `module/clippy.toml` | Desk's `module/` | The workspace names Quill's four members (Desk's `mods` and `dos` excludes are gone) and carries a header. The lock file is Desk's, with the one member added. `clippy.toml` keeps the ban on compare-and-swap and gains a header |
| `module/rustfmt.toml`, `module/.cargo/config.toml` | Quill's | Hard tabs at 130 columns. The config sets the host target and clears `-Zbuild-std`, as `hal/.cargo/config.toml` does, so `cd dos/module && cargo test` runs on the host |
| `mods/module.ld`, `mods/link.rs` | Desk's `module/mods/` | None. The linker script that gives a module its read-only, read-only data and writable segments from the link base (`.desk.imp` in a section of its own, which the packer needs), and the link arguments each module crate's `build.rs` includes |

Every copied file has its Desk header, which names the same copyright holder and licence as this repository's, so none needed changing. The files were reformatted with hard tabs (`cargo fmt` in `dos/hal`), and nothing else in them was touched.

## What is Quill's own

`quill-dos/` (the program), `tools/e2e.sh` (the golden comparison) and the screens in `crates/quill-grid`. `module/quill` (`quill-modtest`) holds the host tests that put `quill-modabi`'s picture interface into a `.DMD` file; nothing in it ships. The interface itself, `quill-modabi`, is in `crates/` (CORE-90).

`rt/src/mem.rs` is written here, not copied: `memcpy` and `memset` with `rep movsd` and `rep stosd`. Desk's `dos/rt/src/mem.rs` is a byte-wise version for its Windows-flavoured COFF build and is not part of its ELF runtime (its roadmap lists the string-move version as planned), so there was nothing to copy. `hal-probe/` and `tools/hal-probe.sh` are also Quill's: a probe that draws and reads back a picture in mode 12h and in VBE 640 x 480 x 8. Desk's `spikes/a-vga-vbe` is the model.

Added to the copied `rt/` (the end of `rt/src/dos.rs`, and `free_memory` in `rt/src/dpmi.rs`): searching a folder (`find_first`, `find_next`, `find_each`, `DirEntry`), drives (`current_drive`, `set_current_drive`, `current_dir`, `drive_attributes`, `drive_valid`, `logical_drive_map`, `floppy_count`, `drive_mask`, `drive_removable`, `drive_is_cdrom`, `extended_error`), `rename`, `set_attributes`, `dos_version`, `File::finish` and the INT 24h stub (`install_critical_handler`, `remove_critical_handler`). Refreshing from Desk keeps them. `rt-test/` and `tools/rt-test.sh` check them under headless DOSBox.

## Still to add (roadmap T4 and M1 to M2)

The installer's copy dropped Desk's keyboard, timer and VGA modules; the VGA and INT 33h drivers are back as `hal/`; `quill-dos` uses `desk-hal`, `desk-vga`, `desk-i8042` and `desk-pic` (mode 12h registers, the DAC, VBE) for its graphics screen, and the mouse is Quill's `quill-mouse` over INT 33h, with `desk-i8042` and `rt::irq` as the PS/2 fallback. Quill reads the keyboard through BIOS INT 16h (copied in M1 from the installer's `keys.rs`) and writes B8000h directly, which needs none of them. `desk-int33` itself is not used.

## Refreshing

The module crates (`module/`, `mods/`) refresh from Desk's `module/` the same way, keeping the changes the table lists; their tests are `cd module && cargo test`, which `check.sh` runs.

1. Check out Desk at the commit to take, and diff each file in the table against its original, ignoring whitespace (`diff -w`).
2. Copy the changed files over, then run `cargo fmt --all` in `dos/` to put the hard tabs back.
3. Re-apply the changes in the table.
4. Run `dos/check.sh` with the tools fetched, and update the commit hash above.
