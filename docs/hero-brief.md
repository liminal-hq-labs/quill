# Hero banner brief

A brief for `assets/hero.svg`, the README's banner. The template's rules are in the comment at the top of that file and checked by `scripts/check-banner.sh`; this brief adds Quill's scene.

## Rules it keeps

- 1200 × 320 `viewBox`, SMIL animation only, no scripts, no external references, under 60 KB.
- The `.motion` and `.static` pairs for reduced motion, `data-loop` on the root and every animation's duration dividing it (keep `data-loop="9"`, so 3 s, 4.5 s and 9 s).
- The left third stays the template's: title `Quill`, the description line, the `experimental` chip, and the small mono line.
- Colours from the template's dark background and the afterglow gradient on the title; the scene's own colours are quoted from Windows 3.1, muted so they sit on the dark background.

## Words

- **Title:** `Quill`.
- **Description:** `One word processor for DOS, Windows 3.1 and Windows 95, in Rust` (63 characters: shrink the description's font size by one step if the text-fit check asks).
- **Mono line:** `Cross-compiling to the past · Liminal HQ Labs`.
- **`<desc>`:** "Quill: one word processor for DOS, Windows 3.1 and Windows 95, written in Rust. A Liminal HQ Labs experiment and the companion to the Cross-compiling to the past series. An animated banner shows a Windows 3.1 window in which a quill nib writes lines of text, with a blue DOS text screen and a Windows 95 window behind it."

## The scene

On the right two thirds, three windows overlap like papers on a desk, back to front:

1. **Back, left:** a DOS text-mode screen, a small rectangle in VGA blue (`#0000AA`) with rows of light grey block "text", one row bright white (bold), one cyan (italic), and a grey status line at the bottom reading `Pg 1 Ln 3`. Mostly hidden; it says "DOS" by colour alone.
2. **Back, right:** a Windows 95 window, its title bar the 95 navy-to-blue gradient with the close box, a grey body. Mostly hidden.
3. **Front, centre:** a Windows 3.1 window, the hero. A navy (`#000080`) title bar with white `Quill - LETTER.QLD`, the 3.1 control-menu box on the left and the minimize and maximize arrows on the right; a white menu bar with `File Edit Insert Find Character Paragraph` in black; a grey toolbar strip with a handful of bevelled buttons (a `B`, an `I`, a `U`); a thin ruler with two indent triangles; a white page.

On the page, a **quill nib** (a pen nib, not a feather: a split steel nib with a breather hole, in the afterglow orange) writes. The lines of text are thin rounded bars in dark grey, of varied lengths, justified on the right edge except the last line of each paragraph; one bar in the first line is bold (thicker), one word in the third is orange (a text colour). A caret blinks after the last bar.

## Motion

- **Writing (9 s loop):** the nib moves left to right along the current line and the bar grows behind it (`clip-path` width animation), then the nib steps to the next line. Three lines are written in the first 6 s, the page holds for 2 s, then the written lines fade together (0.5 s) and the loop starts over at 9 s. The nib dips slightly at each word end (a 1 to 2 px `translate` wobble).
- **Caret:** blinks at 1.5 s on and off within the loop (a 3 s cycle).
- **The back windows** do not move. A barely-there glow behind the front window breathes on a 4.5 s cycle, as the template's flask glow does.

The static frame (reduced motion) shows the page with all three lines written, the nib resting at the end of the third line, and no caret.

## Checks

- `bash scripts/check-banner.sh --shots /tmp/shots`, then look at the frames: the start, mid-write and the full page.
- The title and description fit at both colour schemes; the menu bar text in the scene is drawn as text with the house font stack, or as shapes if it fails the fit check at small widths.
