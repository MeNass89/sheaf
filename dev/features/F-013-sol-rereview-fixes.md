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
