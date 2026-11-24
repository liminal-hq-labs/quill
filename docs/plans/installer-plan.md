I have what I need. One finding changes the whole design: liminst only picks up payload files from inside `installer/`. Others: runtime discovery would grab Quill's own `dos/`, the Win16 pack uses 4 KiB chunks, file types are skipped by all three retro runtimes, and `decode.rs` treats any unknown op tag as a shortcut.

# Plan: a Quill installer built with Liminal Installer

**Markers.** "[V]" means I checked it in the code or docs at the path given. "[U]" means I did not, and the implementer must check it. Sizes come from reading the files in memory with Python zlib, raw deflate at level 9 in independent chunks. They approximate `li-lpk`'s packer but are not its output.

## 0. What the installer does today

- **Payload source [V]:** `li_engine::payload::DirPayload::new(&project.dir)` lists files under `installer/` only (`crates/li-engine/src/payload.rs:28`).
  - A `from = "../win16/out/QUILL.EXE"` matches nothing.
  - Symlinks are skipped, because `file_type()` is neither a directory nor a file.
  - So the Quill build must **copy** the built files into `installer/dist/<target-id>/` before `liminst build`.
  - `{target}` becomes the target id (`payload.rs:97`).
- **Runtime discovery hazard [V]:** with no `--dos-runtime`, `find_dos_dir` (`crates/li-cli/src/dos.rs:60`) walks up to the first `dos/build.sh`.
  - In the Quill repo that is **Quill's own `dos/`**. It runs that script, then fails reading `dos/dist/SETUP.EXE`.
  - With no `--win16-runtime`, the Windows 3.1 build carries no `SETUP.EXE` at all. It only issues a "warning" artefact.
  - So Quill must always pass all three `--*-runtime` flags.
- **Default folders [V]:** `lib.rs:309`. DOS and Win16 use `C:\` plus the last segment of the product id (upper case, at most 8 characters), so `hq.liminal.quill` gives `C:\QUILL`. Win32 uses `%PROGRAMFILES%\Quill`, from the product name.
- **Shortcuts [V]:** the Win32 runtime writes a `.lnk` in `Programs\Quill\` through `IShellLink`.
  - It also writes the `HKLM\…\Uninstall\<product id>` key and keeps `UNINST.EXE` and `UNINST.DAT` (`docs/retro-win32.md`, D65, D66).
  - **Windows 95 shortcuts and Add/Remove Programs are already done.**
  - Win16: `ShortcutKind::GroupItem`. `add_group_item` returns `Ok(false)`, so the install writes a `.BAT` launcher (`retro/win16/src/bin/lisetup/plat.rs:214`).
  - DOS: `.BAT` launcher only.
- **File types [V]:** all three retro flavours drop `file_types`, `url_schemes` and `path` with a warning (`li-retro-build/src/payload.rs:174`).
  - `Op` has only `Mkdir`, `WriteIni` and `Shortcut` (`li-retro/src/model.rs:293`).
  - `WriteIni` writes only below the install folder (`install.rs:414`). On rollback a file that already existed is left as the new copy, so it **must not** be used for `WIN.INI`.
- **Decoder trap [V]:** `li-retro/src/decode.rs:290` decodes `_ => Op::Shortcut`. Any new `Op` tag would be misread as a shortcut. `model::FORMAT` is 2.
- **Shell trait [V]:** `add_group_item(group, name, command, work_dir) -> Result<bool>` and `remove_group_item(group, name)` (`li-retro/src/platform.rs`).
  - The receipt line is `G group|name`. The host sim records `groups` (`sim.rs:52`).
  - The Win16 Shell has no uninstall list. `SETUP /U` runs from the disk.
- **Floppy [V]:** `li-package/src/dos.rs` writes `CAPACITY` = 2847 × 512 = **1,457,664 bytes**.
  - The root holds only `SETUP.EXE`, `SETUP.LPK` and a generic `README.TXT`.
  - The volume label is the slug (`QUILL`).
  - When the files do not fit, the image is "not built" with a note, and the exit code may still be 0 [U].
- **Win16 pack [V]:** 4 KiB chunks (`WIN16_CHUNK_LOG2`), so it compresses worse than the 32 KiB DOS and Win32 packs. `SETUP.EXE` is 206,179 bytes.
- **Not done in any retro runtime [V]:** repair, update, modify and launch.
- **No liminst releases [V]:** no tags, and `release.yml` does not publish the retro runtimes. Both repos pin Rust 1.98.0.

## 1. Where the project lives, and how liminst is obtained

- **Location:** `installer/` in the Quill repo. Quill's D28, `AGENTS.md` layout and SPEC line 108 all say so, and so does the installer README ("next to your code"). Do not make it an example in the installer repo: it would drift, and that repo's CI cannot build Quill.
  - To cover the installer repo's side, extend `examples/planboard` instead: give its `app16` a `file_types` entry, and use it in the host goldens.
- **How liminst is pinned:** `installer/liminst.rev` holds one installer commit SHA.
  - `installer/tools/liminst.sh` (new) resolves the checkout. It uses `$LIMINAL_INSTALLER` if set (the convention `win9x/tools/play-w95.sh` already uses), warning if its HEAD differs from the pin. Otherwise it clones or fetches into `${LIMINST_HOME:-~/.cache/quill16-liminst}/<sha>`.
  - It builds and caches four things:
    - `li-cli` (`cargo build --release -p li-cli`)
    - `retro/win16/out/SETUP.EXE` (`retro/win16/build.sh`)
    - `win32/dist/SETUP.EXE` (`win32/build.sh`, needs `lld`)
    - `dos/dist/SETUP.EXE` (`dos/tools/fetch-tools.sh`, then `dos/build.sh`)
  - It prints their paths.
  - CI uses `actions/checkout` of `liminal-hq-labs/liminal-installer` at the pinned SHA, plus a cache keyed by the SHA.
  - Move to a pinned release once the installer publishes the runtimes (optional PR L7).

## 2. The project files

```text
installer/
  installer.toml  components.toml  LICENCE.TXT (Quill's MIT + Apache text, ASCII)  liminst.rev
  flows/install|uninstall|update|modify|launch.flow.json   (copied from examples/neutral; retro compiles install and uninstall only)
  layouts/welcome|licence|folder|components|ready|copying|done|confirm-uninstall.layout.json
  themes/quill-dos/{theme.toml,layouts/*.layout.json,assets/}    renderers = ["tui-retro"]  (structure of lt-fizz fizz-dos)
  themes/quill-classic/theme.toml (+layouts)                     renderers = ["win16","win32-classic"]  (copy of neutral's installshield-classic)
  tools/liminst.sh  tools/stage.sh  tools/build-disks.sh  tools/e2e-dos.sh  tools/e2e-w31.sh  tools/e2e-w95.sh
  dist/   (gitignored: /installer/dist/ and /installer/out/)
```

**`installer.toml`**

- `[product]`: id `hq.liminal.quill`, name `Quill`, publisher `Liminal HQ`, `licence = "LICENCE.TXT"`.
- No `version`: `--version` is always passed, from `[workspace.package]` in Quill's `Cargo.toml` [U: confirm that key exists].
- `[defaults]`: `scope = "user"`, `theme = "quill-classic"`.
- Three targets:

| Target id | Renderer | Theme | Outputs |
|---|---|---|---|
| `dos-x86` | `tui-retro` | `quill-dos` | `floppy-1440`, `zip` |
| `win16-x86` | `win16` | `quill-classic` | `floppy-1440`, `zip` |
| `windows-x86` | `win32-classic` | `quill-classic` | `floppy-1440`, `zip` |

- No `[signing]` or feed. I did not verify that `check` accepts a project without them [U]; if it refuses, copy `feed/` and `keys/` from neutral.

**`components.toml`**

- Install names must be upper-case 8.3 on DOS and Win16. They are checked at compile time (`dos_rel`).
- There is no "unticked by default" component [V]: every optional component starts ticked.

| id | targets | files (from `dist/…`) | shortcut / extra |
|---|---|---|---|
| `appdos` (required) | dos-x86 | `dos-x86/QUILLDOS.EXE` | "Quill" → `QUILLDOS.BAT` launcher |
| `app16` (required) | win16-x86 | `win16-x86/QUILL.EXE` | "Quill"; later `file_types=[{extension=".qld",name="Quill document",open_with="{install}/QUILL.EXE"}]` |
| `app32` (required) | windows-x86 | `windows-x86/QUILL32.EXE` | "Quill"; later the same `.qld` rule with `QUILL32.EXE` |
| `help` | win16-x86, windows-x86 | `help/QUILL.HLP` | "Quill Help" → `{install}/QUILL.HLP` |
| `readme` (required) | all | `common/README.TXT`, `common/LICENCE.TXT` | "Read Me" → `{install}/README.TXT` (Windows targets only, because a DOS `.BAT` that runs a `.TXT` is useless) |
| `dicts` | all | `dict/EN_CA.DIC`, `EN_US.DIC`, `EN_GB.DIC` | to `{install}/` (Quill looks beside the program, SPEC line 246) |
| `samples` | all | `samples/LETTER.QLD`, `LETTER.RTF`, `PLAIN.QLD`, `PICTURES.RTF` | to `{install}/SAMPLES/` |

- **Help item [U]:** a Program Manager item or `.lnk` whose command is a `.HLP` file relies on the `hlp=` association. Check it on 3.11. If it fails, add `args` to `ShortcutRule` and target `WINHELP.EXE`, which needs a placeholder for the Windows folder.
- **Icon:** embedded in each executable. `feat/win16-gui` puts it back into `QUILL.EXE` (commit c26c8ee). `QUILL32.EXE` gets `QUILL.ICO` through `res_pack.py` (`win9x/build.sh:13`). No loose `.ICO` is installed.
  - The `.QLD` default icon on Windows 95 is `QUILL32.EXE,0` until a document icon exists.

**Folders on disk**:

| OS | Folder | Holds |
|---|---|---|
| DOS | `C:\QUILL\` | `QUILLDOS.EXE`, `QUILLDOS.BAT`, `EN_*.DIC`, `README.TXT`, `LICENCE.TXT`, `SAMPLES\`, `INSTALL.LIR` |
| Windows 3.x | `C:\QUILL\` | `QUILL.EXE`, `QUILL.HLP`, the same shared files, `INSTALL.LIR` |
| Windows 95 | `C:\Program Files\Quill\` | `QUILL32.EXE`, `QUILL.HLP`, the shared files, `UNINST.EXE`, `UNINST.DAT`, `INSTALL.LIR` |

- On DOS, `QUILL.INI` sits beside the program. On Windows it lives in the Windows folder (SPEC line 476).
- Uninstall removes only what the receipt lists and leaves the folder if anything else is in it [V, `install.rs`]. So it leaves `QUILL.INI`, `QUILL.UDC` and `CUSTOM.DIC`, including the recent-files list that `fix/win9x-editing` adds.
- The SPEC has both `QUILL.UDC` (line 246) and `CUSTOM.DIC` (line 454) for the user dictionary. Fix that in Quill separately.

**Sizes against the floppy (1,457,664 bytes, about 1,423 KiB)**:

| Disk | Runtime | Quill program, packed | Help | 3 dictionaries (3 × 300 KB raw) | Samples | Total | Margin |
|---|---|---|---|---|---|---|---|
| W31 | 206 KB [V] | `QUILL.EXE` from `feat/win16-gui`, 310,448 → **148 KB at 4 KiB chunks** (136 KB at 32 KiB) | 6.5 KB → 2 KB today; budget 190 KB | 600–900 KB [U] | about 25 KB | about 0.98–1.28 MB | 140–440 KB free |
| W95 | about 135 KB [V, doc] | `QUILL32.EXE`, 329,728 → **197 KB** at 32 KiB | as above | 600–900 KB [U] | about 25 KB | about 0.96–1.26 MB | fits |
| DOS | about 153 KB [V at the time of D51; current size U] | `QUILLDOS.EXE`, 196,096 → 113 KB | none | 600–900 KB [U] | about 25 KB | about 0.89–1.19 MB | fits; `QUILL.FNT` and `QUILL.HLX` are not built yet |

- How well the dictionaries compress is unknown. They are a trie at about 2.9 bytes a word.
- `QUILL32.EXE` already beats the roadmap's 130 KB packed budget. Correct the roadmap.
- **All-in-one disk: do not make one.** liminst writes one `SETUP.EXE` per target into the image root, so one image cannot hold three installers.
- `build-disks.sh` fails if any image is missing, and warns below 64 KB free.

**Flows**:

- Install: `welcome → licence → folder → components → ready (on_enter builtin:check-disk-space) → copying (commit) → done`. No `check-feed` or `launch-app`, because the retro runtimes skip them.
- Uninstall: `confirm-uninstall → copying → done`.
- Repair: no retro runtime does it. Running `SETUP` again over the folder overwrites the files, and on Win16 `ReplaceItem` stops a duplicate item. A real repair from the receipt's CRCs is installer follow-up L8. Document this in Quill's `README.TXT`.

**Themes**:

- `quill-dos`: a `backdrop` with title "Quill Setup", credit "Liminal Installer", and a side column with pixel `art` traced from `assets/icon/quill-16.png` as rects.
- `quill-classic`: the Studio's Windows 95 look, for Win16 and Win32.
  - Be honest in the docs: the real Win16 runtime draws system controls in the system font.
  - The Win32 runtime draws a fixed teal banner [V].
  - Neither draws theme art.

**Associations and WIN.INI / registry (after installer PRs L4 to L6)**:

- Windows 3.x: `WIN.INI` `[Extensions] qld=C:\QUILL\QUILL.EXE ^.qld`, then broadcast `WM_WININICHANGE` with "Extensions". `REG.DAT` is not written (decision 6).
- Windows 95, under `HKCR`:
  - `.qld` = `hq.liminal.quill.qld`
  - `hq.liminal.quill.qld` = "Quill document"
  - `…\DefaultIcon` = `C:\Program Files\Quill\QUILL32.EXE,0`
  - `…\shell\open\command` = `"C:\Program Files\Quill\QUILL32.EXE" "%1"`
  - then `SHChangeNotify(SHCNE_ASSOCCHANGED)`.
- `.RTF` and `.TXT` are not taken over by default (decision 5). Doing that needs an unticked component (L9) and must restore the previous owner. The previous value is recorded either way.
- Quill opens a file passed on its command line [V, play.md `-a`].

## 3. Program Manager group (Windows 3.x)

**Choice: raw `WM_DDE_*` messages, not DDEML.**

- `DDEML.DLL` does ship with 3.1, but it needs a callback through `MakeProcInstance`, string handles, and a fourth imported module in `ne_pack.py` that nothing has tested yet.
- Raw DDE needs only a few new KERNEL and USER ordinals and a window procedure, which `export_proc!` already handles.
- Quill's own probes found that `SendMessage` to Program Manager leaves the high register halves alone (`docs/progress.md:84`).
- The 32-bit stack alias already handles Windows calling back in during a `SendMessage`.

**The message sequence** (Win16 `lParam` packing):

1. Create a hidden window of class `LiminalDde` with its own `export_proc!(DdeProc, dde_proc)`. It is separate from the wizard's `DefDlgProc` window.
2. `a = GlobalAddAtom("PROGMAN")`, then `SendMessage(0xFFFF, WM_DDE_INITIATE 0x3E0, hwnd, MAKELONG(a, a))`.
   - Keep the first `WM_DDE_ACK` (0x3E4) as the server.
   - Delete the atoms the ACK carries, and send `WM_DDE_TERMINATE` to any extra servers.
   - Delete our own atom when `SendMessage` returns.
   - No server (for example Norton Desktop as the shell) means `Ok(Unavailable)`, and the `.BAT` fallback stays.
3. Execute: `h = GlobalAlloc(GMEM_MOVEABLE|GMEM_DDESHARE 0x2002, len+1)`, copy the string, unlock, then `PostMessage(server, WM_DDE_EXECUTE 0x3E8, hwnd, MAKELONG(0, h))`.
   - Pump `PeekMessage`/`DispatchMessage`/`Yield` until the ACK arrives, or 5 s by `GetTickCount`.
   - The ACK's `LOWORD` bit 0x8000 is fAck. `GlobalFree(HIWORD)`.
4. Request the group list: `PostMessage(server, WM_DDE_REQUEST 0x3E6, hwnd, MAKELONG(CF_TEXT, atom "PROGMAN"))`.
   - The reply is `WM_DDE_DATA` (0x3E5): `MAKELONG(hData, aItem)`. `hData` points to a `DDEDATA` (flags word with fRelease 0x2000 and fAckReq 0x8000, then `cfFormat`, then CRLF-separated group names).
   - Free it if fRelease is set; send an ACK if fAckReq is set.
5. `PostMessage(server, WM_DDE_TERMINATE 0x3E1, hwnd, 0)`, wait for the server's own terminate, then destroy the window.

**Commands.** A pure builder in `crates/li-retro/src/progman.rs`, tested on the host:

- It quotes any argument that holds `,`, `(`, `)`, `[` or `]` in `"…"`, and refuses `"`.
- IconPath is the command itself when the command ends in `.EXE`, `.COM`, `.BAT` or `.PIF`, else it is left out.

| When | Commands |
|---|---|
| Install | Request the group list and note whether "Quill" exists. `[CreateGroup(Quill,QUILL.GRP)]` (activates an existing group), then `[ShowGroup(Quill,1)]`. For each item: `[ReplaceItem(Quill)]` in its own execute (a NACK is fine), then `[AddItem(C:\QUILL\QUILL.EXE,Quill,C:\QUILL\QUILL.EXE,0,-1,-1,C:\QUILL)]`. If that is NACKed, retry with the 4-argument form, which leaves the working folder blank so Program Manager uses the program's folder [U]. |
| Uninstall | `[ShowGroup(Quill,1)]`, `[DeleteItem(name)]` for each `G` record, then `[DeleteGroup(Quill)]` only when the receipt has `P Quill`. |

- Program Manager writes `QUILL.GRP` in the Windows folder and adds `GroupN=` under `[Groups]` in `PROGMAN.INI`. Whether it flushes before the test harness kills DOSBox-X is [U].

**Trait and receipt changes**:

- `add_group_item(group, name, command, work_dir) -> Result<GroupAdd>`, where `GroupAdd = Unavailable | Added { made_group: bool }`.
- `remove_group(group, items: &[&str], made_group: bool)` replaces `remove_group_item`, so there is one conversation per uninstall.
- New receipt record `P group` (the group was made by this install). Older readers ignore unknown letters [V, SPEC §9].

**New Win16 imports** (`retro/win16/tools/imports.tbl`):

- KERNEL: `GlobalAddAtom` 70, `GlobalDeleteAtom` 71, `GetProfileString` 58, `WriteProfileString` 59.
- USER: `IsWindow` 47.
- These ordinals are from memory and **[U]**. The docs say the atom ordinals were never checked. Verify them with `python3 retro/win16/tools/ne_dump.py $W31_TREE/WINDOWS/SYSTEM/KRNL386.EXE` (and `USER.EXE`) against Wine's `krnl386.exe16.spec` and `user.exe16.spec`, then regenerate with `gen_imports.py`.
- Segment budgets have room (segment 4 is at 11,769 of 40,000 bytes [V]). The data-segment heap peak is 33.7 of 40 KiB, so keep the DDE state small and static.

**Testing**:

- **Host:**
  - Builder goldens.
  - The sim shell records the command strings.
  - The receipt has `G` and `P`.
  - Uninstall deletes items, and deletes the group only with `P`.
  - Cancelling part-way removes the items.
- **Wine:** `e2e-wine.sh`. Whether Wine's shell32 PROGMAN DDE server answers a 16-bit client and writes `Start Menu\Programs\Quill\*.lnk` is [U]. The test asserts the result `SETUP` logs (`progman: added` or `unavailable`) and passes on either, with a note.
- **Windows 3.11 in DOSBox-X:** `retro/win16/tools/e2e-w31.sh`.
  - After `AddItem`, `SETUP` logs the Request reply to `/LOG`. In-guest proof does not depend on Program Manager flushing.
  - The host also checks `C:\WINDOWS\QUILL.GRP` and `PROGMAN.INI`.
  - With `SHOTS`, take an extra `import -window root` 3 s after `SETUP` exits: Program Manager with the Quill group open.
  - After `/U`: the group is gone from the Request log and `QUILL.GRP` is gone.
  - Quill's `win16/tools/play-w31.sh -d out/w31 SETUP.EXE` (from #5) is the manual check.

## 4. Windows 95

- **Already done [V]:**
  - Start menu folder `Programs\Quill\` with "Quill", "Quill Help" and "Read Me" `.lnk` files.
  - The `HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\hq.liminal.quill` key (`DisplayName`, `DisplayVersion`, `Publisher`, `InstallLocation`, `UninstallString` = `"<dir>\UNINST.EXE" /U "<dir>"`).
  - The kept uninstaller, which copies itself to `%TEMP%` and leaves the copy there on 9x.
- **Missing:** associations, which the compiler drops. Plan L5:
  - `Shell::register_file_type` writes the `HKCR` keys in section 2 using the existing `RegCreateKeyExA`, `RegSetValueExA`, `RegQueryValueExA` and `RegDeleteKeyA` imports.
  - Add `shell32.dll!SHChangeNotify` to `win32/tools/imports.allow`, checked by hand per that file's rule.
  - Delete keys deepest first, because NT's `RegDeleteKey` does not recurse.
  - On uninstall, restore `.qld` only if it still names our ProgID.
- **Proof:** `regedit /e D:\QLD.REG HKEY_CLASSES_ROOT\.qld` in the guest, read back from `D:` [U: that 95's `regedit /e` takes a key argument].

## 5. Packaging

**`.LPK` and the `.QPK` flip.**

- liminst always writes `SETUP.LPK`, and Quill never reads a pack. No `quill-pack` crate exists [V: `crates/` list].
- So the flip is **docs only**:
  - Supersede the `.QPK` part of D57 and all of D71 with a new decision: release disks use `.LPK` through liminst.
  - Rewrite `docs/architecture.md` "Packs" and the roadmap's "Release disks" paragraph and T5.
  - Drop the `qpk_read` fuzz target from `docs/testing.md`.
  - Fix SPEC line 108: there is one `SETUP.EXE` per Windows disk, and DOS uses `SETUP.EXE`, not `INSTALL.EXE`.
- If the flip is not accepted, nothing in the build changes. The docs simply stay wrong, so this PR (Q0) is cheap and goes first.

**The chain** (`installer/tools/build-disks.sh`, the "quill-disk-image" plan):

1. Build Quill:
   - `win16/build.sh` → `QUILL.EXE` and `QUILL.HLP`. The help comes out of `win16/build.sh` only on `feat/win16-gui`; on `main`, use `hlpc assets/help/quill.qhm` from #7 [U: exact CLI].
   - `win9x/build.sh` → `QUILL32.EXE`.
   - `dos/build.sh` → `QUILLDOS.EXE`.
   - `cargo run -p quill-dict -- $OUT/dic`.
   - Or take prebuilt paths through `QUILL16_EXE` and similar variables.
2. `installer/tools/stage.sh` copies the files into `installer/dist/…`, upper-casing the sample names. `--stand-ins` writes tiny placeholders for check-only CI.
3. `liminst.sh` gives the `liminst` binary and the three runtimes.
4. `liminst build -C . -t <id> --version $V -o installer/out/<id> --<flavour>-runtime <path>` for `dos-x86`, `win16-x86` and `windows-x86`.
5. Fail if `installer/out/<id>/*-1440.img` is missing [U: the exact file-name slug].
6. With mtools: `mlabel` to `QUILL DOS`, `QUILL W31` and `QUILL W95`, and `mcopy -o` a Quill-specific disk `README.TXT`. Then `minfo` for free space.
7. Copy to `installer/out/QUILL-DOS.IMG`, `QUILL-W31.IMG` and `QUILL-W95.IMG`, plus the liminst zips.
8. Write `SIZES.txt` (each file raw and packed from `liminst lpk list`, free bytes) and `SHA256SUMS`.

**CI.** New workflow `.github/workflows/disks.yml`.

- **`installer-check`** (pull requests touching `installer/**`): pinned liminst, stand-ins, `liminst check -C .`, and `liminst preview -C . -t <id>` for all three targets.
- **`disks`** (manual dispatch and `v*` tags):
  - Installs `rust-src`, `lld`, `llvm`, `python3`, `mtools`, `dosfstools`, `curl` and `xz`.
  - Builds everything, gates the sizes, and uploads the three `.IMG` files, the zips, `SIZES.txt` and `SHA256SUMS`.
  - On a tag, attaches them to the GitHub release.
- Use the shared `ci-rust` image pinned by digest, per `AGENTS.md`.
- The emulator end-to-end runs stay local, because the Windows trees cannot be redistributed (D21).

## 6. Testing

- **Host (Quill):**
  - `check`, `preview` and `build` with stand-ins.
  - `liminst lpk verify` on each `SETUP.LPK`.
  - Unpack each pack and compare its files byte for byte with the staged ones.
  - `mdir` shows exactly `SETUP.EXE`, `SETUP.LPK` and `README.TXT` with the right label.
  - The size gate.
- **Host (installer):** goldens in `li-retro-build/tests/win16.rs` and `win32.rs` for the group commands, the `P`, `G` and `A` records, and the file-type registry and `WIN.INI` effects on the sim, using `examples/planboard`.
- **Wine (installer):** `retro/win16/tools/e2e-wine.sh` and `win32/tools/wine-e2e.sh`, extended with association checks (`wine reg query 'HKCR\.planboard'`).
- **DOSBox Staging (`installer/tools/e2e-dos.sh`):**
  - Run Quill's `dos/tools/dosrun.sh` with the image's files on `A:` and `DOS_AUTOTYPE` keys through the pages, as the installer's `dos/tools/e2e.sh` does for Lt. Fizz. `LI_DUMP` writes the screens.
  - Check the files against `INSTALL.LIR`.
  - Run `C:\QUILL\QUILLDOS.BAT` with `QUILL_DUMP` and the exit keys, compare the first frame with Quill's golden, then `SETUP /U C:\QUILL`. The folder must be gone.
- **Windows 3.11 in DOSBox-X (`installer/tools/e2e-w31.sh`):** modelled on the installer's `retro/win16/tools/e2e-w31.sh`, using the tree from `~/Downloads/win31.zip`. Three sessions:
  1. `SETUP.EXE /PLAY /DUMP /LOG /PAUSE=2500`, with a screenshot of each page.
  2. Program Manager with the Quill group (screenshot). Press Return on the selected item to start Quill, and take a screenshot of Quill open on a sample.
  3. `D:\SETUP.EXE /U C:\QUILL`, then a screenshot of Program Manager without the group.
  - Host checks: the receipt sizes, `C:\QUILL` gone, `QUILL.GRP` gone, `[Extensions] qld` removed, `C:\WINDOWS\QUILL.INI` left alone.
  - Commit the screenshots to `docs/media/install-w31-*.png`.
- **Windows 95 OSR2.5 (`installer/tools/e2e-w95.sh`):**
  - Sources the installer's `scripts/win95/lib.sh` and drives `run-win95.sh -d installer/out/windows-x86/win32 -w 90 SETUP.EXE /PLAY D:\PLAY.TXT /DUMP D:\DUMP.TXT`.
  - The mouse script opens Start, then Programs, then Quill (screenshot) and clicks Quill (screenshot).
  - The Run box: `regedit /e D:\QLD.REG HKEY_CLASSES_ROOT\.qld`. Then `control appwiz.cpl` (screenshot of the entry).
  - Uninstall in the **same** boot, because every run starts from the base image: run `"C:\Program Files\Quill\UNINST.EXE" /U "C:\Program Files\Quill"` with keys, then take a Start menu screenshot without Quill.
  - Read `D:` back. Also try `-b ie4`.
  - Commit `docs/media/install-w95-*.png`.

## 7. Pull requests, in order (each sized for one Sonnet implementer)

**Installer repo (`liminal-installer`)**

| PR | Branch | Content | Acceptance |
|---|---|---|---|
| L1 | `fix/runtime-discovery` | `find_dos_dir` requires `<flavour>/setup/Cargo.toml`; a Win16 target with `floppy-1440` and no runtime fails | A CLI test where a temporary directory holds a fake `dos/build.sh`: the script is not run, and the error names `--dos-runtime` |
| L2 | `feat/progman-model` | `li-retro::progman` builder; the `GroupAdd` and `remove_group` trait change; the `P` record; sim and goldens; `.HLP`/`.TXT` items get no IconPath | `cargo nextest -p li-retro -p li-retro-build` green; new goldens for the command strings and the receipt |
| L3 | `feat/win16-progman-dde` (on L2) | `dde.rs` in `lisetup`; the verified ordinals in `imports.tbl`; `/LOG` lines; harness shot after exit and `.GRP` checks | `retro/win16/check.sh` passes; Wine result logged; 20 of 20 on 3.11 for Lt. Fizz and Planboard with the group made and removed; a screenshot in `docs/images/`; `retro-win16.md` and D73 updated |
| L4 | `feat/file-types-model` (on L2) | `Op::FileType{ext, prog_id, name, target}`; **fix `decode.rs` to explicit tags and an error for unknown ones**; `FORMAT` 3; `Shell::register_file_type` and `unregister_file_type` returning the previous value; the `A ext\|previous` record; the compiler emits the op for Win16 and Win32 and keeps the warning for DOS; Planboard `app16` gains `.planboard` | Host tests: install, uninstall restoring the previous value, rollback; the decoder refuses tag 3 under format 2 |
| L5 | `feat/win32-file-types` (on L4) | Registry writes, `SHChangeNotify`, deepest-first delete; `wine-e2e.sh` checks | `win32/check.sh` passes; Wine: key present after install and gone after uninstall |
| L6 | `feat/win16-file-types` (on L3, L4) | `WriteProfileString` and `GetProfileString` for `[Extensions]`; `WM_WININICHANGE` broadcast | Wine and 3.11: `WIN.INI` line added and restored |
| L7 (optional) | `ci/publish-runtimes` | Release assets: the three `SETUP.EXE` runtimes and `liminst` | A dry-run dispatch lists the artefacts |
| L8, L9, L10 (later) | | Repair from the receipt (L8); unticked components with a `default = false` field (L9); a kept Win16 uninstaller plus an "Uninstall Quill" item, copying to `%TEMP%` and starting it with `WinExec` (L10) | |

**Quill repo**:

| PR | Branch | Content | Acceptance |
|---|---|---|---|
| Q0 | `docs/release-disks-lpk` | The `.QPK` to `.LPK` decision; roadmap sizes corrected; `SETUP.EXE` names; no all-in-one disk; SPEC line 108 | Markdown lint; the decision numbered after whatever has merged (D89 is already used twice across branches) |
| Q1 | `feat/installer-project` | `installer/` files, `liminst.sh`, `stage.sh --stand-ins`, `.gitignore`, header-checked TOML | `liminst check` and preview for 3 targets green on stand-ins; `scripts/check-headers.sh` passes |
| Q2 | `build/disk-images` (on Q1) | `build-disks.sh`, mtools post-processing, `SIZES.txt`, host verification test | Three images built from `main`'s binaries, each with ≥ 64 KB free, and pack contents byte-equal to the staged files |
| Q3 | `test/install-dos` (on Q2) | `e2e-dos.sh` | Install, run, uninstall pass in DOSBox Staging |
| Q4 | `test/install-w31` (on Q2; pin bump to L3, later L6) | `e2e-w31.sh`; screenshots | Group visible, Quill starts from it, uninstall removes the group and the folder; evidence in `docs/media` |
| Q5 | `test/install-w95` (on Q2; pin bump to L5) | `e2e-w95.sh`; screenshots; `.qld` association in the components | Start menu item, Add/Remove entry, `QLD.REG` content, clean uninstall |
| Q6 | `ci/disks` (on Q2) | `disks.yml` | `actionlint` passes; the check job green on a pull request; a manual dispatch uploads the three images |

**Order against the open Quill pull requests and agents**:

- Q0 and Q1 start from `main` now.
  - Q0 touches `docs/decisions.md`, `roadmap.md` and `progress.md`, which #6, #7, #8 and #10 also touch. Merge Q0 after them or rebase it.
- Q2 needs the help compiler and the real Win16 executable: base it on `main` after #7 and #8 merge.
- Q4's evidence should wait for `feat/win16-gui`. It puts the icon back into `QUILL.EXE` and builds `QUILL.HLP` beside it.
- Q4 and Q5 reuse the play scripts from #5 for manual checks.
- The installer never writes `QUILL.INI`, so `fix/win9x-editing` (recent files in `QUILL.INI`) does not conflict. Q4 and Q5 assert that `QUILL.INI` survives uninstall.
- The association PR adds a "Windows" section to `docs/file-types.md` from #10.
- #4 (site) can link the release images after Q6.
- On the installer side, L1, L2 and L4 can run in parallel. L3 is the long pole for the Program Manager group.

## 8. Decisions for you (my recommended default in bold)

1. **Where the project lives:** **Quill repo `installer/`**, or an example in the installer repo.
2. **How liminst is obtained:** **a pinned commit SHA**, a local `LIMINAL_INSTALLER` checkout, or a CI checkout. Switch to releases after L7.
3. **DOS installer name:** **keep `SETUP.EXE`**, or rename it to `INSTALL.EXE` as the SPEC says (that would need a runtime change).
4. **Disks:** **separate W31 and W95 disks, and no all-in-one disk**. This drops the SPEC's "one Windows `SETUP` picks the build".
5. **Associations:** **`.QLD` only**. `.RTF` and `.TXT` come later as an unticked option that restores the previous owner.
6. **Windows 3.x registration:** **`WIN.INI` `[Extensions]` only**, or `REG.DAT` as well through `SHELL.DLL`.
7. **Windows 3.x uninstall:** **from the disk (`SETUP /U`) in v1**, or a kept uninstaller with a group item (L10).
8. **Dictionaries:** **one optional component with all three, ticked**. The fallback if a disk overflows is to drop `EN_GB` and `EN_US` from that disk.
9. **Naming:** **group, folder and product name "Quill"; product id `hq.liminal.quill`; version from Quill's Cargo workspace.**
10. **Disk labels and README:** **patch with mtools in Quill's script**, or add `label` and `readme` keys to liminst targets.
11. **Settings:** **leave `QUILL.INI` and the user dictionary on uninstall.**
12. **DDE:** **raw `WM_DDE_*`**, or DDEML.

## Risks

- **Disk space:**
  - How well the dictionaries compress is unmeasured.
  - The Win16 pack's 4 KiB chunks cost about 9 % on `QUILL.EXE`.
  - `QUILL.HLP` could grow to its 190 KB budget. Worst case, the W31 disk is about 1.47 MB, over the limit. The size gate catches it.
- **Program Manager behaviour [U]:**
  - `AddItem` with `-1,-1` positions and a working folder.
  - Items whose command is a document.
  - Whether `PROGMAN.INI` and the `.GRP` file are flushed before the harness kills DOSBox-X. The in-guest Request log covers this.
- **Ordinals:** the KERNEL and USER ordinals are unverified until checked with `ne_dump`.
- **Wine:** whether a 16-bit client reaches Wine's PROGMAN DDE server is unknown.
- **Decoder:** the `decode.rs` fall-through. If L4 skips the explicit-tag fix, an older runtime would misread a new pack.
- **`WIN.INI` safety:** associations must not go through `WriteIni`. Its rollback would leave a changed `WIN.INI` in place.
- **Runtime discovery:** until L1 lands, every Quill-side build must pass all three `--*-runtime` flags.
- **Win16 data segment:** the heap peak is already 33.7 of 40 KiB, so DDE buffers must stay small.
- **CI cost and fragility:** a long build chain (`-Zbuild-std`, `lld-link`, the CWSDPMI download, three platform builds). Cache by SHA.
- **Proof:** emulators only. There is no real-hardware or ME run unless the owner adds one.

### Critical files for implementation

- /home/scott/source/liminal-hq-labs/liminal-installer/retro/win16/src/bin/lisetup/plat.rs (Win16 `Shell`; DDE goes beside it, with `ui.rs` as the pattern for `export_proc!` and the message pump)
- /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-retro/src/install.rs and /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-retro/src/platform.rs (`shell_shortcut`, receipt records, the `Shell` trait)
- /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-retro-build/src/payload.rs and /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-retro/src/decode.rs (file-type ops, the `_ => Shortcut` decoder trap, `FORMAT`)
- /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-cli/src/dos.rs (runtime discovery, floppy output) and /home/scott/source/liminal-hq-labs/liminal-installer/retro/win16/tools/imports.tbl
- /home/scott/source/experiments/quill16/docs/decisions.md and /home/scott/source/experiments/quill16/docs/roadmap.md (D57, D71 and the release-disk budgets to supersede); the new /home/scott/source/experiments/quill16/installer/ goes beside them
