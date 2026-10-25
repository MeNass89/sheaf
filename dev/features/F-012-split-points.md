# F-012 · Split: cut between pages in a grid (C-organize-015)

Main app: iLovePDF (Split: custom ranges shown as groups of pages, fixed ranges, "Merge all ranges in one PDF").
PDF Expert: split points placed between pages of the grid.

## Closing criteria
- **Function.** A PDF in Split shows its pages; a click in the gap between two pages cuts there (scissors), a second
  click joins again; the Ranges field follows ("1-3,4-7,8-10"), and typed ranges show on the pages (pages no range
  takes are faded). The panel says what will be made ("3 PDFs will be created", every-N count) and refuses ranges
  outside the document. "Merge all ranges into one PDF file" writes `<name>-split.pdf` with the ranges in order.
  Proof: `ranges::tests::a_click_between_pages_cuts_a_range_and_a_second_click_joins_it_back`, gui-smoke 22 (cut,
  join, three files written).
- **Aesthetics.** Each output file has its colour (border and numbered badge, six tints); the gap shows a dashed line
  and a scissors on hover, an accent line and filled scissors once cut.
- **Performance.** Thumbnails rendered on the probe pool as Organize's; the grid's ranges read through one parse per
  page (pure Slint callbacks); the job is one pdfium pass per range.

## Journal
- 25/10: built. Manual run: ten coloured pages, cuts after 3 and 7: ten-1-3.pdf, ten-4-7.pdf (4 pages), ten-8-10.pdf;
  a second click after 3 gives 1-7,8-10. Became gui-smoke 22.
  Left: very long documents render every thumbnail (as Organize).
