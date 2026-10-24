# F-007 · Mark text for redaction by selecting it (C-security-010)

Main app: Acrobat (Redact ▸ drag over content: text is selected, an empty area is boxed). Also iLovePDF and Stirling
(select text, or draw areas, then apply), PDF Expert (redact by text selection).

## Closing criteria
- **Function.** In the Redact tool, a drag that starts on text marks the words it crosses (one box per line, its
  text listed and also scrubbed from metadata and bookmarks); a drag elsewhere marks an area; applying removes
  exactly the marked words. Proof: `reader::tests::a_drag_over_text_marks_its_lines_for_redaction`, gui-smoke 18.
- **Aesthetics.** The marked lines boxed on the page as search matches are; the list says the words ("“Prévalence
  = 1/100…”"); the hint says both ways.
- **Performance.** A page's text read once per page (cached), on the first drag.

## Journal
- 24/10 17:00: the Redact viewer only drew rectangles (its hint promised "Select text"). A drag starting on a
  glyph now selects as the reader does (`reader::redact_selection`: `selection`, `line_rects`, `quads`,
  `selected_text`) and becomes a text mark labelled with its words; elsewhere it is an area as before. The viewer
  reads page text from its document (a tool tab's viewer has no bytes in memory: read from the file). MICI fiche:
  "Prévalence = 1/1000, début à tout âge avec un pic de fréquenc" marked and removed, the rest of the line kept
  (/tmp evidence). Hint: "Drag over text to mark it, or elsewhere on a page to mark an area."
  Tests: `a_drag_over_text_marks_its_lines_for_redaction` (unit), gui-smoke 18 (drag "quick" → "fox", applied:
  "Page 1 the" kept, the three words gone).
- 24/10 19:00, reviewer 8/8/9 on 0720459; its blockers:
  1. Live selection: the viewer sends pointer moves to Rust while a drag that began on text goes on; the text it
     selects shows as a selection (overlay kind 3) and no rubber band is drawn. The selection's ends are carets
     between letters: a letter is in once the pointer passes its middle (`reader::caret`; the drag of the first
     evidence stopped on the "e" of "fréquence" and left it readable).
  2. A double click marks a word; a third click soon after, the line (`reader::line`, `redact_range`).
  3. The result counts kinds: "2 text passages, 1 page and 1 area redacted" (`redact::MarkCounts`; the CLI's
     "7 areas redacted" for 6 matches and a page is now "6 text passages and 1 page redacted").
  Evidence (real app, Xvfb): mid-drag highlight, two marks (a dragged line, a double-clicked word), result
  "2 text passages redacted". Tests: `a_drag_over_text_marks_its_lines_for_redaction` (carets, line, word),
  `tools::tests` summary lines, cli `redact_subcommand_counts_the_areas_and_removes_the_text`, gui-smoke 18 (drag
  "quick"→"fox" from edge to edge + double click "Page"; "1 the" kept; "2 text passages redacted").
