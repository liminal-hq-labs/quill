# Quill's site

The product page published at <https://liminal-hq-labs.github.io/quill/>. Plain HTML, CSS and a little JavaScript, with no framework and no build step: the folder is published as it is.

## Layout

| Path | What it is |
|---|---|
| `index.html` | The page |
| `404.html` | The page GitHub Pages shows for a missing path |
| `css/site.css` | Styles, the light and dark themes (Quill's palette, D70) |
| `js/site.js` | The theme toggle and the screenshot lightbox; the page works without it |
| `img/` | The icon (from `assets/icon/`), the link preview `social-card.png` (written by `tools/make_social_card.py`), and `shots/` |
| `favicon.ico` | The favicon: 16, 32 and 48 px from `assets/icon/` |

Every URL in `index.html` is relative, so the page works from `https://liminal-hq-labs.github.io/quill/`, from a fork's Pages address, or from a local folder. `404.html` is the exception: GitHub serves it for a missing path at any depth, so its links start with `/quill/`.

## Screenshots

The files in `img/shots/` are real captures of Quill and its test programs, copied from `docs/media/` (where `docs/media/README.md` says where each was taken), reduced to 256 colours and run through `optipng`. Each caption on the page says where its screenshot was taken. Add a screenshot only if it shows Quill running, and check that it shows no product key and no personal name, such as a registered owner on a Windows system screen.

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

`.github/workflows/pages.yml` checks the links (fetching the outside ones), uploads `site/` and deploys it when a change under `site/` (or the workflow itself) reaches `main`, or when it is run by hand from the **Actions** tab. Pages has to be turned on once before the first deployment:

1. Open the repository's **Settings**, then **Pages**.
2. Under **Build and deployment**, set **Source** to **GitHub Actions**.
3. Merge a change under `site/` to `main`, or run the **Pages** workflow from the **Actions** tab. The `github-pages` environment the workflow uses is created on the first run.
