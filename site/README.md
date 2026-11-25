# Quill's site

The product page published at <http://labs.liminalhq.ca/quill/>. The organization's Pages sit on a custom domain, so `https://liminal-hq-labs.github.io/quill/` redirects there. Plain HTML, CSS and a little JavaScript, with no framework and no build step: the folder is published as it is.

## Layout

| Path | What it is |
|---|---|
| `index.html` | The page |
| `404.html` | The page GitHub Pages shows for a missing path |
| `css/site.css` | Styles, the light and dark themes (Quill's palette, D70) |
| `try/` | The Try Quill page (`index.html`, `try.js`, `licences.html`), the vendored js-dos player in `try/js-dos/` (see its `PROVENANCE.md`) and, built and not committed, `quill.<hash>.jsdos` and `try-manifest.json` |
| `css/try.css` | The Try Quill page's styles |
| `js/site.js` | The theme toggle and the screenshot lightbox; the page works without it |
| `img/` | The icon (from `assets/icon/`), the link preview `social-card.png` (written by `tools/make_social_card.py`), and `shots/` |
| `favicon.ico` | The favicon: 16, 32 and 48 px from `assets/icon/` |

Every URL in `index.html` is relative, so the page works from `http://labs.liminalhq.ca/quill/`, from the root of any server, from a fork's Pages address and from a local folder. `scripts/check-site.py` resolves each reference the way a browser does below `/quill/` and at the root, and fails if the two reach different files. `404.html` is the exception: GitHub serves it for a missing path at any depth, so its links start with `/quill/`. The Open Graph and canonical URLs have to be absolute, and name the published address.

## The Try Quill page

`try/` runs `QUILLDOS.EXE` on FreeDOS 1.3 in the visitor's browser with js-dos (SITE-1). The disk bundle `try/quill.<hash>.jsdos` and the manifest `try/try-manifest.json` that names it are build outputs, ignored by git (SITE-4: the bundle is named by its content, and the page asks the manifest for the current name): `bash scripts/build-try.sh --build` makes it (the Pages workflow runs the same command), and without `--build` it packs the programs named by `QUILLDOS_EXE`, `QUILL_FNT` and `QUILL_DICT`. It needs mtools and, with `--build`, the pinned Rust toolchain. `bash scripts/stamp-try.py` writes the `?v=` versions of the page's own files into `try/index.html` (a build does too), which is committed; `bash scripts/check-try.sh` tests the scheme. Without the bundle the page loads and Start reports that the disk could not be fetched.

The player is loaded only when Start is selected, and makes no request outside the site. After updating `try/js-dos/`, open the page with the network panel open and start it. `scripts/check-site.py` does not scan the vendored player.

## Screenshots

The files in `img/shots/` are real captures of Quill, copied from `docs/media/` (where `docs/media/README.md` says where each was taken), reduced to 256 colours and run through `optipng`. The Windows 3.11 captures come from the play harness (`win16/tools`, `docs/play.md`) and the Windows 95 ones from Liminal Installer's test bed (`scripts/win95`), each on a private display. Each caption on the page says where its screenshot was taken. Add a screenshot only if it shows Quill running with nothing else of anyone's in frame (no other program's icons, no product key, no personal name such as a registered owner on a Windows system screen), and retake it when the program it shows has changed.

## Checks

```sh
python3 -I scripts/check-site.py   # every link, image, script and stylesheet in site/ resolves
bash scripts/check-headers.sh
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint
```

To look at it locally the way Pages serves it, put the folder under a `quill` path and start a static server:

```sh
mkdir -p /tmp/pages && ln -sfn "$PWD/site" /tmp/pages/quill
python3 -m http.server 8000 --directory /tmp/pages   # then open http://localhost:8000/quill/
```

## Publishing

`.github/workflows/pages.yml` checks the links (fetching the outside ones), uploads `site/` and deploys it when a change under `site/` (or the workflow itself) reaches `main`, or when it is run by hand from the **Actions** tab. The repository's Pages setting is **Source: GitHub Actions** (build type `workflow`), and the `github-pages` environment the workflow uses is created on the first run. The address comes from the organization's custom domain; nothing in this folder names it except the canonical and Open Graph URLs in `index.html`.
