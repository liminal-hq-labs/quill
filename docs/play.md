# Playing with Quill by hand

Two scripts start a real Windows in a real DOSBox-X window on your desktop, with a working mouse and keyboard and Quill on `D:`: Windows for Workgroups 3.11 for the Win16 build, and Windows 95 OSR2.5 for the Win32 build. Neither uses Xvfb or any automation; the headless runs are in [testing.md](testing.md).

## Table of contents

- [Before you start](#before-you-start)
- [Windows for Workgroups 3.11](#windows-for-workgroups-311)
- [Windows 95](#windows-95)
- [The mouse and the keyboard](#the-mouse-and-the-keyboard)
- [A bigger or sharper window](#a-bigger-or-sharper-window)
- [What works and what does not](#what-works-and-what-does-not)
- [Sound](#sound)
- [Stopping, and what is cleaned up](#stopping-and-what-is-cleaned-up)
- [How it reaches your screen](#how-it-reaches-your-screen)
- [DOSBox-X on the host instead of Docker](#dosbox-x-on-the-host-instead-of-docker)

## Before you start

- Docker, with the DOSBox-X image `quill16-dosbox-x:local` (the script builds it from `win16/tools/dosbox-x.Dockerfile` on first use), `xdotool` (it reads the screen size), and `unzip`.
- A desktop session: `DISPLAY` must be set. Wayland is fine, through XWayland.
- Builds. The scripts take what exists and say what is missing:

| Program | Built by | Found at |
| --- | --- | --- |
| `QUILL.EXE` and `QUILL.HLP` (Win16) | `win16/build.sh` | `win16/out/`, or `$OUT_DIR` if you set `OUT_DIR` for the build; the help file goes on `D:` beside the program, where Help > Contents looks for it |
| `QUILL32.EXE` (Win32) | `win9x/build.sh` | `win9x/dist/QUILL32.EXE` |
| `QUILLDOS.EXE` (DOS) | `dos/build.sh` | `dos/dist/QUILLDOS.EXE` |

The documents `LETTER.RTF`, `LETTER.QLD` and `PLAIN.QLD` come from `tests/docs/` (test fixtures; the sample that ships is `samples/demo.*`, see SITE-3). All of them end up on `D:`.

## Windows for Workgroups 3.11

```sh
win16/tools/play-w31.sh                       # the S3 video driver, 800 by 600 in 64K colours; QUILL.EXE starts
win16/tools/play-w31.sh -v vga                # VGA.DRV, 640 by 480 in 16 colours (the dithering and 16-colour paths)
win16/tools/play-w31.sh -n                    # Program Manager only
win16/tools/play-w31.sh -a 'D:\LETTER.RTF'    # Quill opens a document at start
win16/tools/play-w31.sh -d ~/some/files MYAPP.EXE   # your own files on D:, and MYAPP.EXE starts instead of Quill
```

The Windows 3.11 tree is not ours and is never committed. It is unpacked once from the zip the owner supplies (`~/Downloads/win31.zip`, or `W31_ZIP`) into `~/.cache/quill16-play/w31-base` (`PLAY_HOME` moves it; `W31_TREE` points at a tree you unpacked yourself). Every session copies it to a folder of its own, so the base is never written.

| Option | Meaning |
| --- | --- |
| `-v s3\|vga` | The video, described above (default `s3`). |
| `-d DIR` | Also put the contents of this host directory on `D:`. |
| `-n` | Start nothing. |
| `-a ARGS` | Arguments for the program that starts. |
| `-s 1..4` | Window size as a multiple of the guest screen. The default is 2 on a display at least 1080 pixels tall, otherwise 1. |
| `-o opengl\|openglnb\|surface` | The video output; see [A bigger or sharper window](#a-bigger-or-sharper-window). |
| `-x` | Allow the container to reach the X server with `xhost`; see [How it reaches your screen](#how-it-reaches-your-screen). |
| `-N` | Use a DOSBox-X installed on the host; see [DOSBox-X on the host instead of Docker](#dosbox-x-on-the-host-instead-of-docker). |
| `-K` | Keep the session folder (the copied tree and `D:`). |

DOSBox-X runs `WIN /3`, 386 Enhanced mode, on an emulated Pentium with 32 MB and S3 video (plain VGA with `-v vga`). The program that starts comes from `WIN.INI`'s `run=` line, so it opens on top of Program Manager and Program Manager is still there when it closes; start anything else from File, then Run. The tree's own StartUp group (it opens Notepad and dials a modem) is left out of the session.

## Windows 95

```sh
win9x/tools/play-w95.sh                       # plain Windows 95 OSR2.5, QUILL32.EXE starts at logon
win9x/tools/play-w95.sh -b ie4                # the base with Internet Explorer 4 and the Desktop Update
win9x/tools/play-w95.sh -n                    # the desktop only
```

This wrapper hands everything to Liminal Installer's `scripts/win95/play-win95.sh`, which copies the test bed's base image to a throwaway disk, builds the `D:` disk and runs DOSBox-X; its own documentation is `docs/play.md` in that repository (branch `chore/win95-play`). The wrapper looks for the checkout in `LIMINAL_INSTALLER` and then in the usual places under `~/source/liminal-hq-labs/`, and sets `W95_HOME` to `/mnt/games/media/win95-testbed` when the base images are there. The base images are built once from the owner's CD image (`docs/win95-testbed.md` in that repository) and are never written.

## The mouse and the keyboard

DOSBox-X gives the guest a PS/2 mouse that only reports relative motion, so the host pointer has to be captured.

- **Click inside the window** to capture the pointer. It disappears from the host and moves the guest's pointer.
- **Ctrl+F10** releases it (and captures it again). The window title repeats this.
- The keyboard goes to the window while it has focus; your desktop keeps the shortcuts it grabs first (Super, and Alt+Tab on most setups).
- Fast flicks of the mouse travel further than slow ones, because the guest accelerates pointer motion.

## A bigger or sharper window

The guests run at 640 by 480 (Windows 95, and Windows 3.11 with `-v vga`) or 800 by 600 (Windows 3.11's S3 driver). `-s 2` opens a window twice that size, which is the default on a tall display; `-s 3` is three times.

`-o opengl` (the default) scales smoothly. `-o openglnb` scales with nearest-neighbour, which keeps pixels square and crisp at whole-number sizes. `-o surface` never scales: the fallback if OpenGL misbehaves in the container.

## What works and what does not

What was checked, in an Xvfb session of these scripts (the window opens on a display that needs an Xauthority cookie, as a real one does): Windows 3.11 reaches Program Manager with `QUILL.EXE` running on top, in both video modes; Windows 95 reaches the desktop with `QUILL32.EXE` running, on both bases; typing reaches the program; the pointer moves after a click and is released by Ctrl+F10; stopping with a signal or `docker rm -f` leaves no container and no working copy.

Known to fail or unfinished:

- **The Win16 Quill is not finished.** It opens documents on Windows 3.11, but it has faulted before. The vendored runtime fixed the faults in 30 of 30 scripted runs; it has not been through as many runs by hand, and it is not done. A fault writes `FAULT.LOG` (and `CRUMBS.LOG` records how far startup got) to `D:`, and both are copied out when you stop; please keep them.
- No network on either guest, and no printer.
- The OpenGL output may run in software inside the container. If the window stutters, try `-s 1` or `-o surface`.
- Closing Windows 3.11 from Program Manager returns to the DOS prompt and the script's DOSBox-X exits; Windows 95 shows "It's now safe to turn off your computer" and you close the window.

## Sound

None is expected. The Windows 3.11 tree's Sound Blaster driver needs a Sound Blaster to be present or it raises a configuration error on every start, so DOSBox-X emulates one and sends its output to SDL's dummy audio driver. Windows 95 has no sound card. Nothing touches your audio devices.

## Stopping, and what is cleaned up

Close the window, press Ctrl+C in the terminal, or run `docker rm -f quill16-play-w31` (Windows 3.11) or `docker rm -f liminal-win95-play` (Windows 95) from another terminal. The script then removes the container, copies what the guest added or changed on `D:` to an `out/` folder (its path is printed), deletes the copied tree and disks (unless `-K`), and takes back any `xhost` permission it added. If nothing was kept it removes the session folder too.

## How it reaches your screen

The container runs as your user with the host network, the X11 socket (`/tmp/.X11-unix`), `DISPLAY`, your host name and a read-only copy of your Xauthority file (`$XAUTHORITY`, which XWayland under GNOME keeps in `/run/user/<uid>/`). That is enough on a default GNOME or KDE session and needs no change to the X server's access list.

If DOSBox-X still says it cannot open the display, run with `-x`. That runs `xhost +SI:localuser:$USER` before the container starts, which lets processes of your own user connect without a cookie, and `xhost -SI:localuser:$USER` when the session ends (it does nothing if the permission was already there). Nothing else is opened to the network.

## DOSBox-X on the host instead of Docker

Arch's official repositories carry only `dosbox` 0.74.3 (`pacman -Si dosbox-x` finds nothing, `pacman -Ss dosbox` lists only `extra/dosbox`), which cannot boot these disk images. DOSBox-X is in the AUR:

```sh
paru -S dosbox-x          # the AUR package (2026.10.01 when this was written); dosbox-x-sdl2 and dosbox-x-git also exist
win16/tools/play-w31.sh -N
```

`-N` writes the same profile with host paths and runs `dosbox-x` directly, with no Docker and no X11 plumbing. That path was written but not run here, since the host has no DOSBox-X; the Docker image carries 2025.02.01. The Flatpak (`flatpak install flathub com.dosbox_x.DOSBox-X`) sandboxes file access, so it would need `--filesystem` for the session folder and a `dosbox-x` wrapper on your `PATH` that calls `flatpak run`; the container is simpler.
