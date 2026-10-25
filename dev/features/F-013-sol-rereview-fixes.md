# F-013 · Gaps found by Sol's re-review of 25/10 (dev/board/reviews/sol-rereview-2026-10-25.md)

Each line: the component, Sol's gap, what was done, its proof.

## Round 1 (25/10)
- **C-edit-002 / C-output-001** ⌘S skipped a paragraph emptied in the editor. The editor's text now starts as the
  paragraph's own, and ⌘S commits it as it stands: emptied, the paragraph is removed. Proof: sheaf-core
  `edit_text::a_paragraph_emptied_is_removed_from_the_page`.
- **C-edit-014** Save As copied the file without the paragraph still open in the editor: it commits it first
  (`saving::commit_open_paragraph`, shared with ⌘S).
- **C-forms-002** Clear form cleared at once: it asks "Clear every field?" with Clear / Cancel in the bar (Acrobat's
  Reset Form asks). Proof: gui-smoke 6 (Cancel keeps the value in the saved file, Clear empties it).
- **C-reader-033** Enter while a search was still running searched again: with results listed, Enter goes to the
  next one (`reader-toolbar.slint`).
- **C-annotate-012** ⇧ did not constrain shapes: ⇧ held at the press makes squares, circles, 45° lines
  (`reader::constrained`). Proof: `reader::tests::shift_makes_squares_circles_and_45_degree_lines`, gui-smoke 6c
  (a 160 × 50 drag with ⇧ is saved square).
- **C-output-005** A batch with failures announced full success: the heading says "2 of 3 files were converted;
  1 failed." Proof: `state::tests::failed_batch_items_are_listed`.
- **C-security-012** An older redaction search could overwrite a newer one's matches: each search has a number and
  a stale answer is dropped (`viewer_ui::search`). not automatable: the race needs two finder threads finishing in
  the wrong order — no seam to order them in a test.
- **C-edit-021** `file:` links did nothing on click: a linked PDF opens in its own tab, any other file is shown in
  its folder (never run; Acrobat asks before launching), a missing one says so. Proof:
  `reader::tests::file_links_are_decoded_and_taken_from_the_document_folder`.
- **C-reader-007** Fit page stopped at 25 %: a very tall page now fits whole (zoom floor 1 % of the fit width, page
  floor 16 px). Proof: `viewer::tests::fit_page_fits_a_very_tall_page_whole`.
- **C-reader-035** Results stopped at 2,000: the list is a ListView (only rows in view are made), every match is
  listed.
- **C-organize-021** (Sol on Merge) Merge took PDFs only: images and Office/HTML documents are converted and merged
  in order (iLovePDF). Proof: `tests/jobs.rs merge_takes_images_and_documents_with_pdfs`,
  `files::tests::drops_are_routed_by_tool_and_extension`.
- Flake seen 25/10 in gui-smoke 6h (⌘ + wheel around the pointer: 961 → 889 px, once, not reproduced alone nor
  under CPU load): a second notch arriving after the 150 ms settle but before the view showed the first notch's
  scroll anchored on the old scroll. The zoom now anchors on the scroll asked for while it is not shown yet
  (`reader.slint` `scroll-x-now`). Probable cause, fix kept; watch this check.

## Round 2 (25/10, after Sol's round 1: 9 agree, 4 disagree)
- **C-edit-021** A non-PDF linked file was only shown in its folder: it now opens in its own app after "Open
  “name”?" (Acrobat's Launch File warning; `LaunchFileDialog`, Cancel keeps it closed).
- **C-output-005** The partial heading counted outputs: it counts the files given (a PDF to JPG input making five
  images counts once). Proof: `state::tests::failed_batch_items_are_listed` (2 of 3 with 5 JPGs).
- **C-reader-007** A 200 × 14,400 pt page still overflowed at 1 %: narrow pages round their width down, floor 4 px.
  Proof: `viewer::tests::fit_page_fits_a_very_tall_page_whole` (that page, 700 px high).
- **C-reader-035** Results were a flat list without history: the page heads its group of results (PDF Expert), and
  the clock lists the session's last ten searches, one click runs it again into the field (⌘F in the field selects
  its words). Proof: `search::tests::a_search_goes_first_in_the_history_once_and_the_oldest_drop_off`, gui-smoke 23.
- Refactor: `viewer.rs` (1,004 lines) split into `viewer/mod.rs` (layout, zoom, mapping) and `viewer/renders.rs`
  (caches, render queue, drawing).

## Round 3 (25/10)
- **C-reader-035** (Sol round 2) Each page heads its results with their count ("Page 4 · 3 results"), the header
  says "12 results on 5 pages" (Preview). Counts from one map per update (no quadratic pass on 50k hits).
- **C-reader-052 / C-reader-063** Properties lacked page boxes, PDF/A and restrictions: `sheaf_core::facts` reads
  what the owner password forbids (from /P, or the encryption state when lopdf opened it with no password), PDF/A
  from XMP, creation and modification dates, page 1's media/trim/bleed/art boxes when they differ; read on the
  side when the sheet opens. Proof: sheaf-core `facts::properties_read_restrictions_pdfa_and_dates_from_the_file`,
  `facts::tests::xmp_and_dates_read_as_people_write_them`, gui-smoke 24 ("Restricted: no printing").
- **C-annotate-031** Notes filter by review status (Accepted, Rejected, Cancelled, Completed, No status), as
  Acrobat's. Proof: `reader::tests::notes_filter_groups`.
- **C-sign-010** Marks took a fixed colour: the colour row shows for marks (black first, as Acrobat) and the mark
  takes it. Proof: gui-smoke 6c'' (a blue checkmark, /C saved blue).
- **C-forms-005** Fields were always tinted: More ▸ Highlight form fields turns it off and on, kept in the
  preferences. Proof: gui-smoke 24.
