# F-002 — A real reader and annotator

Status: in progress
Components: C-reader-001, 002, 005, 007, 008, 018, 019, 020, 021, 022, 023, 025, 032, 033, 043, 046, 048, 049, 051, 057,
C-annotate-001, 002, 003, 004, 006, 007, 010, 022, 025, 026, 030, C-output-001, C-files-001/002/004/011
Grade before: reader avg 0.7, annotate 0 (Sheaf had no reader: pages only inside Redact/Sign/Edit)
Why: Nassim (20/10): "Sheaf must become a reader": open any PDF to read and study, outline, thumbnails,
continuous scroll, highlights, notes, night/sepia — at PDF Expert's level.

## Target
Opening a PDF (double-click, drop, Open, Recents) shows it in its own document tab, like PDF Expert: toolbar with
the side panel, annotation tools and colours, page field, zoom, page theme, search, undo/redo/save; a side panel with
page thumbnails, the outline and the list of annotations; pages in a continuous scroll. Text can be selected and
copied, highlighted, underlined or struck through from a floating bar or with the tool active; notes, pen strokes and
text boxes are added by clicking or drawing; annotations are listed, selected, deleted, undone. ⌘S writes them into
the file as an incremental update; closing with unsaved annotations asks Save / Don't Save / Cancel.

## Closing criteria
- [ ] Function: open from Home, drop, Recents, open-with; same file twice brings its tab forward — proof: GUI run.
- [ ] Function: continuous scroll, ←/→, PgUp/PgDn/space, Home/End, page field (numbers and labels), ⌘[ ⌘] — proof: unit tests + GUI.
- [ ] Function: zoom 25–400 %, ⌘+/⌘−, ⌘0 fit width, ⌘9 fit page, ⌘-wheel — proof: unit test + GUI.
- [ ] Function: outline tree (expand/collapse, current section, click to jump) — proof: `reader::tests::outline_rows_*` + GUI.
- [ ] Function: search with count, next/previous (Enter, ⇧Enter, ⌘G), hits highlighted, streamed on big files — proof: GUI on 1,982 pages.
- [ ] Function: select text (drag, double-click word, ⌘A), copy ⌘C — proof: `reader::tests::a_drag_selects_*` + GUI.
- [ ] Function: highlight / underline / strike (5 colours), note, pen, text box; delete; undo/redo; list — proof: GUI + saved file read back by qpdf/pdfium.
- [ ] Function: ⌘S incremental save; Save As; unsaved prompt on tab close and window close — proof: GUI + file check.
- [ ] Function: night and sepia page themes, remembered — proof: `themes_map_white_*` + captures.
- [ ] Function: links followed (internal pages, web links in the browser) — proof: GUI.
- [ ] Aesthetics: side by side with PDF Expert captures (02, 03, 04a, 05, 09, 11, 11b) — proof: dev/re/evidence/F-002/.
- [ ] Performance: open the 1,982-page, 70 MB book in < 2 s (release), scroll without blank pages, an annotation shows
      in < 150 ms, RSS < 400 MB — proof: numbers in the journal.

## From their code
- main (pdfexpert): MainMenu — View ▸ Theme Day/Night/Sepia, ⌘[ ⌘] Go Back/Forward, five highlight colours, the
  selection context menu (Copy, Highlight, Underline, Strikeout, Note), the left panel tabs (Thumbnails, Outline,
  Bookmarks, Annotations) — evidence README 02–11b.
- secondary (preview): Highlights and Notes sidebar with the marked text; ⇧⌘A markup toolbar.
- secondary (acrobat): reading colours (white on black), Go To Page ⇧⌘N, undo label.
- engine (PdfCraft, Apache-2.0): annotations with appearance streams (`AddAnnotation`), undo/redo, incremental save,
  outline, text layer (`PageText`: nearest glyph, word, line rects).

## Out of scope (next features)
Two-page spread, bookmarks of one's own, split view, reading aloud, measure, comment replies, forms.

## Journal
- 2026-10-20: reader core (`reader.rs`, tests), document tabs (`reader_ui.rs`, `screens/reader.slint`), opening on a
  worker, revisions in the render cache (an edit re-renders only its page, old render stays until then), links and
  history, unsaved prompt. First GUI run on Linux/Xvfb: the 1,982-page book opens with its outline.
- 24/10 17:00, flaky smoke check fixed: "an edited paragraph shows on the page before ⌘↩" read the screen a fixed
  1.5 s after typing and failed once on a busy machine (passed on the next run); it now waits for the preview
  (and for its removal after Escape) up to 6 s. It failed again: the cause was that it looked for "quick" on the
  whole screen, and when tesseract read it in the open thumbnails the click went there, no editor opened and
  " zebra" was typed as tool shortcuts. It now looks on the page only.
- 24/10 18:30, flaky "⌘ + wheel zooms around the pointer" (961 → 889): after a zoom the view may cut the new
  horizontal scroll to the old content width; the wanted scroll was set again when the width came, but only for
  300 ms, and on a busy machine the width came later. It is now kept until the view shows it (2 s at most).
