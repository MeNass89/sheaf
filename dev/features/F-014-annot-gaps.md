# F-014 · Annotation and bookmark gaps from Sol's re-review (25/10)

Split with the parallel session (nassim-3e): this journal covers C-annotate-001, -007, -009, -019, -022 and
C-reader-026; Sol's gaps are in `dev/board/reviews/sol-rereview-2026-10-25.md`.

- **C-annotate-001** At 900 px tools fell off the bar. Under 1,180 px the edit group (Edit text, Add image, Link,
  Redact…, Sign) folds into one button with its menu, and Two pages / Fit leave the bar for the More menu and the
  zoom list (⌘8, ⌘9 still work); the search field narrows. 900 px: every control in the bar, save included
  (screenshot by hand).
- **C-annotate-007** A text box was typed in a floating card and appeared after Done: a new text box or added
  text is typed on the page where it goes, in its font, size and colour (Acrobat, PDF Expert); a click elsewhere,
  Escape or ⌘↩ finishes it (`ReaderNote::inplace`, `pointer::pressed` commits). Proof: gui-smoke 2c ("Boxtest"
  read at the click before it is committed).
- **C-annotate-009** Changing a box's size or alignment replaced its font by the row's: only a family, bold or
  italic click changes the font now. Left: families beyond the PDF standard three need embedded fonts in
  PdfCraft's FreeText (not done).
- **C-annotate-019** Dated stamps had one format: Preferences ▸ Annotations ▸ "2:05 pm, Oct 25, 2026" (Acrobat's),
  "25/10/2026 14:05" or "2026-10-25 14:05". Proof: `edits::tests::dated_stamps_write_the_time_in_the_format_chosen`.
- **C-annotate-022** Five preset colours only: a sixth dot opens a palette of 18 and a RRGGBB field (Acrobat's
  Other color); the colour stays in the row (preferences, one for strokes and text, one for highlights and notes)
  and recolours a selected comment. Proof: gui-smoke 50 (a stroke in the palette's orange, /C saved).
- **C-reader-026** Bookmarks only for the page in view: + Bookmark takes the selected text as its title (Acrobat)
  and its page; on hover a bookmark moves up or down among its siblings, or points at the page in view (Set
  destination). Proof: `tools::bookmark_tests::bookmark_titles_are_one_short_line`, gui-smoke 51.
