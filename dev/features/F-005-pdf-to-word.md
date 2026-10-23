# F-005 — PDF to Word as editable text and tables

Status: open
Components: C-convert-009 (PDF to Word; at 4/3/7 on 23/10/2026), then C-convert-010/011 (PowerPoint, Excel)
Why: iLovePDF's PDF to Word gives flowing paragraphs and real Word tables. Both engines Sheaf has (PdfCraft's
`pdfcraft-export::docx`, LibreOffice's PDF import) give pages of floating text boxes and no table: on a real
Word fiche page 1 is unreadable once opened in Word (dev/HANDOFF.md, reviewer 23/10).

## Target (main app iLovePDF; Acrobat's flowing vs exact layout later)
- Paragraphs as Word paragraphs, lines joined (Sheaf's `text_blocks`, `soft_breaks`: hardened for C-edit-002),
  their runs styled: font name (subset tag dropped), size, bold, italic, colour, underline.
- Tables as Word tables, rebuilt from the page's ruled cells (stroked lines, filled rectangles): cell text,
  fills, merged cells.
- Images inline, in reading order. Two-column pages read column by column.
- A scanned page offers OCR first (the OCR tool exists).

## Corpus and checks, before code
`dev/scripts/word-corpus.py` builds the corpus from Word originals (python-docx), turns them into PDFs with
LibreOffice, exports them with Sheaf and measures each against its original:
1. words in order: the export's text equals the original's (normalised whitespace), in order;
2. tables: as many tables, with as many rows and cells, as the original;
3. no floating text boxes in `word/document.xml`;
4. the export rendered back by LibreOffice has the original's page count (± 1).
Documents: a letter (paragraphs, bold, a link), a fiche (table with fills and merged cells, as MICI), an
invoice (ruled table with numbers), a two-column article, a report with headings and an image.
Plus the MICI fiche itself (no Word original: checks 2–4 by eye and counts).

## Done when
C-convert-009 at 9 on the three axes with the corpus checks green, reviewer agreeing.

## Journal
- 23/10 03:30, corpus first run (PdfCraft's export as is): 14 of 20 checks pass. On these simple pages it
  flows and makes tables; MICI turns to 738 text boxes because one rotated label (`positioned()`) sends the
  whole page to positioned boxes. Faults found: (1) paragraphs: lines merged across paragraphs, a bold run
  made its own paragraph (letter); (2) tables found from text alignment swallow the paragraphs around them
  (invoice heading and "Billed to" inside the table; fiche's last line); (3) two columns read across, not
  column by column; (4) rotated text forces positioned boxes. Decision: vendor pdfcraft-export and fix these
  in its flow (it already writes paragraphs, tables, images, headings) rather than a new writer.
- 23/10 04:10, pdfcraft-export vendored (third_party/pdfcraft-export; upstream's inline tests kept and run
  by the unit layer; its LibreOffice test skips without Impress, which vpsn lacks: libreoffice-writer-nogui).
  Where the faults come from, and the steps:
  1. Paragraphs: the engine (`compare.rs` `export_pages`) feeds `pdfcraft_edit::reading_blocks`, which
     groups by single style (`group_layout(lines, false)`), so a bold run is its own block. Step: export
     `Block` gains styled runs; the engine fills them from the mixed grouping (`text_blocks`: spans, rows,
     `soft_breaks`); docx writes one `w:p` per paragraph with its runs, a typed break as `w:br`.
  2. Tables: `tables()` (lib.rs) clusters left edges of short blocks, no ruling seen. Step: `Page` gains the
     page's drawn segments, from `measure::snap::geometry` (user space, mapped like `visible_rect`); where
     horizontal and vertical segments make a grid, cells come from it (text assigned by centre, merged
     cells where a rule is missing, fills later); the text-alignment detector stays for unruled tables.
  3. Columns: `items()` sorts every part by top edge. Step: split the page at a vertical gutter (no text
     crossing it, both sides tall) and read left column then right.
  4. Rotated text: `positioned()` sends the whole page to boxes for one rotated block. Step: a rotated
     block inside a table cell becomes that cell's text with `w:textDirection btLr`; elsewhere a paragraph.
- 23/10 05:30, step 2 done (ruled tables): `Page::rules` from `measure::snap::geometry`; `ruled_tables` builds
  the grid (lines within 8 pt across / 4 pt down are one: fills inset in cells), merges cells where a rule
  is missing (gridSpan, vMerge), puts each block in its cell, a rotated label as `btLr` text; borders kept;
  the text-layout detector then runs on what is left; a rotated block in a ruled table no longer makes the
  page positioned. Test `ruled_table_with_merged_cells_and_a_vertical_label`. Corpus 18/20 (fiche and invoice
  tables exact). MICI: no text box left (14 tables, 26 vertical labels) but still poor: 10 pages for 4, the
  first page's stacked tables fragmented. Next: MICI page 1 (why tables split, cells lose text), then
  paragraphs (step 1), columns (step 3).
- 23/10 08:20, MICI page 1 is a Word table with its text in the cells (was 10 pages of fragments). Fixes: on a
  page with ruled tables the text-layout detector no longer runs (it made a 9×12 grid of nothing); border
  pieces join across a corner gap of up to 4 pt and pieces from 2 pt count (Word draws borders in pieces);
  the biggest grid first, a grid nested in it (a label's own box) is part of its cell; no vertical text in a
  cell merged down (LibreOffice aborts the page layout on it: tested by removing either). Corpus 18/20.
  Next: the table is wider than the page (scale columns or narrow the margins), a cell's lines become one
  run (keep one paragraph per line, each block's own style), letter-by-letter labels in narrow merged cells;
  then paragraphs (step 1), columns (step 3).
- 23/10 10:00, cells: one paragraph per line (blocks grouped into lines by their middles, then left to right),
  each block's own style (bold, colour), tight spacing; tables wider than the text column scaled to it. MICI:
  9 pages for 4 (was 10), styles kept. Much of the length is the font: runs name no font and vpsn has no
  Calibri or Carlito, so LibreOffice uses DejaVu (wider). Next: font names in runs, Carlito/Caladea on vpsn
  for fair measuring; labels in narrow merged cells (letter by letter); paragraphs (step 1); columns (step 3).
- 23/10 11:30, fonts and margins: runs name the source family (`w:rFonts`, "Calibri" from "ABCDEF+Calibri,
  Bold"); vpsn now has Carlito/Caladea (fonts-crosextra-*), so LibreOffice renders Calibri's metrics. Page
  margins follow the content (between a quarter inch and Word's usual eighth; content past the page's edges
  keeps the usual ones). MICI 7 pages for 4. Corpus 19/20 (columns now pass). Next: the column of tiny empty
  paragraphs on MICI page 2 (footer pieces), letter-by-letter labels in narrow merged cells, letter (step 1).
- 23/10 12:40, PdfCraft bug fixed: `reading_lines` (export) parsed the page's content streams one by one;
  a TJ whose operands end one stream lost its text state and its text came out at 1 pt in the page's
  corner (the "dashes" after MICI's tables, the lines missing from its cells). It now parses them joined,
  as `text_lines` does (§7.8.2). Test `text_cut_between_two_content_streams_is_exported_where_it_is_drawn`
  (red then green). MICI page 1 nearly whole in its table. Next: fills (white titles on green bands are
  invisible without them: MICI's "ITEM 282", "MALADIE DE CROHN"), then the letter (step 1).
- 23/10 13:30, fills: `pdfcraft_edit::page_fills` (re + fill, gray/RGB/CMYK); a cell takes the fill covering half
  of it (`w:shd`); a single shaded ruled box (a title band) is a one-cell table. MICI page 1 reads as the fiche.
  Reviewer 6/6/7 (was 4/3/7). Its order next: (1) spaces lost between style runs ("inconnue**pouvant**") with a
  word-level corpus check against pdftotext; (2) paragraphs outside tables (header, letter); (3) page length
  (cell padding, row heights); (4) labels in narrow merged cells; (5) OCR offer; (6) timing (4-page, 100-page).
- 23/10 15:00, step 1 (paragraphs) and the space bug: the export now reads runs (`reading_paragraphs`: the
  export's lines, forms included, grouped as the editor groups them, styles mixed) and each run carries its
  paragraph; a row break is a space when the row wrapped, a line break when the author ended it, a new
  paragraph where the row stands further below than the usual pitch. Spaces live in the runs, so none is
  lost at a change of style ("inconnue pouvant"). The corpus reader counted w:br as nothing ("Doe12"): fixed.
  Corpus 20/20. MICI page 1 ≈ the original (header in order, table, colours); the document is 7 pages for 4.
  Tests: `a_space_before_a_word_in_another_style_is_kept_in_the_export` (red then green; one paragraph holds
  both runs). Next: page length, narrow merged labels, OCR offer, timing.
- 23/10 16:30: vertical labels in merged cells again (works now), ruled cells 3 pt each side, 1 pt between tables;
  MICI still 7 pages for 4 (each source page, a Word section, spills a few lines). Timing (debug build):
  MICI 4 pages 0.12 s / 25 MB; 100 pages of Greenberg 6.6 s / 101 MB. Scanned PDFs: the probe flags a file
  whose first 3 pages carry under 20 characters each; PDF to Word/PowerPoint/Excel then say it looks scanned
  and to choose OCR (the OCR choice itself existed). gui-smoke 17; files test.
