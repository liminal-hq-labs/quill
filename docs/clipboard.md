# Clipboard

`quill-clip` converts between a selection and the formats a clipboard holds. It sits in the editor layer, above `quill-formats` and below `quill-app`, and it never names a platform: each shell implements the `Clipboard` trait over its own system clipboard (D177).

## Conversions

- **`copy(doc, store, range, want, clipboard)`** copies `range` of the body into a scratch `Doc` in the same store, with the format tables of `doc`, and writes it with the RTF and text writers. The clipboard gets what a file save would write for those characters. The scratch document is freed before `copy` returns and a failure frees it too. The call succeeds when at least one flavour was committed.
- **`paste_fragment(clipboard, prefer, store, out)`** reads the first flavour of `prefer` that the clipboard offers and that reads cleanly into the fragment `out`. A flavour that fails is skipped and the next is tried, so damaged RTF with plain text beside it still pastes the text. The caller chooses the order; SPEC's order is `Rich Text Format`, then `CF_DIB`, then `CF_TEXT`.
- A selection that ends inside a paragraph is written without that paragraph's end, and a fragment read from RTF or text keeps a final paragraph mark only when the source had one. A word copied from the middle of a line pastes inline, and a whole paragraph keeps its end and its paragraph format.
- A fragment is a document of its own. Its font, colour and format tables come from the clipboard; the editor merges them into the target when it inserts the fragment.
- Pictures travel inside RTF as `\pict` groups. With the `dib` feature (on by default, and it stays on: the workspace tests cover it; a shell without pictures builds with `default-features = false`) a selection that is exactly one BMP picture also goes out as a DIB, which is the bitmap file without its 14-byte header, and a DIB on the clipboard pastes as a one-picture fragment. A bitmap `quill-image` cannot decode is not offered. A PNG picture goes out inside RTF only.
- The text flavour is Windows-1252. UTF-8 and UTF-16 arrive with `quill-text`.

## Transport

```rust
pub trait Clipboard {
	type W<'a>: ClipWrite;       // `Sink` plus `commit(self) -> bool`
	type R<'a>: Source;
	fn clear(&mut self) -> bool;  // EmptyClipboard; false when it cannot be opened
	fn offers(&mut self) -> Flavours;
	fn put(&mut self, f: Flavour) -> Option<Self::W<'_>>;
	fn get(&mut self, f: Flavour) -> Option<Self::R<'_>>;
}
```

The writer and reader borrow the clipboard, so a shell can keep its clipboard handle, a locked `GMEM_DDESHARE` block or a prefetched buffer inside them. A writer dropped without `commit` abandons its data. `copy` calls `clear` first, so a new copy never leaves an old flavour behind.

| Shell | Transport |
|---|---|
| Win16 | `OpenClipboard`, `EmptyClipboard`, `SetClipboardData` with `GMEM_DDESHARE` memory: `CF_TEXT`, the registered `Rich Text Format` and `CF_DIB` (a far block, tiled with `__AHINCR` when it passes 64 KB) |
| Windows 95 | The same calls in 32 bits |
| GTK | `GdkClipboard`, which is asynchronous: the shell reads the offered flavours into buffers first and hands `paste_fragment` a `SliceSource`; a copy is served from the committed buffers on request |
| WinUI | `CF_UNICODETEXT` and RTF, converting the text between Windows-1252 and UTF-16 |
| DOS, terminal, host tools | `StoreClipboard`: the flavours held in `Store` chunks, capped at 1 MB of RTF, 1 MB of text and 4 MB of DIB, and kept until the program exits |

`StoreClipboard` owns a store of its own, so a copy outlives the document it came from and costs the document no memory. It is also the fallback for any shell whose system clipboard is not there.

## How `App` asks for a copy or a paste

`App` does not call the clipboard itself, and `key()` and `command()` keep their signatures. Copy, Cut and Paste leave a request, as the picture and file dialogs do (D86), and the shell serves it:

```rust
// in the shell, after key() or command()
if let Some(req) = app.take_clip_request() {
	match req {
		ClipRequest::Copy { cut } => app.copy_to(&mut store, &mut clipboard, cut),
		ClipRequest::Paste => app.paste_from(&mut store, &mut clipboard),
	}
}
```

`copy_to` takes the selection and calls `quill_clip::copy` with `Flavours::ALL`; on a cut it then deletes the selection as part of the same undo step. `paste_from` calls `paste_fragment` with the SPEC order, merges the fragment's tables into the document, and replaces the selection with it as one undo step. `App` keeps undo grouping and selection replacement, and the fixed 2,048-byte text buffer it holds today goes away, which is 2,050 bytes of the Win16 data segment. None of this is in `quill-app` yet.
