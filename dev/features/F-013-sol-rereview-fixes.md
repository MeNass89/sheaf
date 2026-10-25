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

## Round 4 (25/10, after Sol's round 3: 3 agree, 3 disagree)
- **C-annotate-031** Acrobat's check mark (state model Marked) was read as a status: it neither sets nor overwrites
  one now; "None" clears it. Proof: `reader_doc::tests::a_check_mark_is_not_a_review_status`.
- **C-reader-052** An ordinary page showed no box: media and crop boxes always, trim/bleed/art when set. Proof:
  `facts::properties_read_restrictions_pdfa_and_dates_from_the_file`.
- **C-reader-063** High-quality printing and accessibility copying were not named: all eight /P bits are; "no
  printing" covers the high-quality one. Proof: `facts::tests::every_permission_bit_is_named`.
- **C-reader-030** Attachments only saved: a click opens one (a PDF in its own tab, any other file in its app after
  "Open name?"), the arrow on hover saves a copy, past three a Find field filters them by name. Proof: gui-smoke 25.
- **C-files-015** (Sol's re-review) Compare could not take open documents: each slot's Open file lists them (one
  per slot, the other slot's excluded). Proof: gui-smoke 26. Popups now carry their own size (without it they took
  their parent's and centred their list in it: the + menu and the slot menu sat off to the side).
- **C-security-004** Restrictions needed an open password: "Require a password to open" can be turned off, the
  permissions password then restricts alone (`security::protect_with_permissions` accepts an empty open password
  with an owner one). Proof: sheaf-core `security::restrictions_alone_open_without_a_password`, gui-smoke 27.
- **C-reader-013** No default zoom: Preferences ▸ Documents ▸ Open new documents at Fit width / Fit page / Actual
  size. Proof: gui-smoke 28.
- The opening size "Actual size" was computed on the view's first, provisional width (600 px) and showed 176 %:
  it is kept pending and set again as the width comes, until the user zooms (`ViewerUi::pending_actual`).
- gui-smoke 6h failed twice in five runs (961 → 889 px, at 263 %): a wheel event handled before the pointer's
  move into the page anchors where the pointer was. The smoke now moves the pointer in as a hand does (two
  steps, a pause). The app reads the pointer from the last move; a wheel without a move first is not a real case.
- Found 25/10 (was a flake in 5 of 11 full runs): gui-smoke 6h, ⌘ + wheel left the point under the pointer 72 px
  off. Diagnosis: the app kept its anchor through every wheel step (a probe recorded each step, written out 3 s
  later so as not to change the timing; the first probe, a log line in the step, made the bug vanish), and the view
  took each intermediate scroll; the last one was moved after the fact, the view cutting it to a width it had in
  between. Fix: when the zoom settles (150 ms), the anchored scroll is sent again (`viewer_ui::zoom_to_at`); the
  view's 2 s retry timer is restarted for each scroll. Proof: gui-smoke 6h green in the full runs since.

## Round 5 (25/10, after Sol's round 4: 4 agree, 3 disagree)
- **C-annotate-031** A "Checked" filter (Acrobat's check mark: the comment's last Marked/Unmarked reply).
  Proof: `reader::tests::notes_filter_groups`.
- **C-reader-013** Fit page as the opening size was saved but not applied: it is set once the view knows its
  height (`ViewerUi::pending_fit`). Proof: gui-smoke 28 (a fresh letter page opens under 120 %; 85 % by hand).
- **C-reader-030** Two attachments of one name shared a temporary file: each opens from its own folder.
- **C-files-011** Close Other / Left / Right left unsaved documents open with a toast: each is asked about in turn
  (Save / Don't Save / Cancel; Cancel stops), as closing it would (`flow::close_queue`, `next_close`). Proof:
  gui-smoke 29.
- gui-smoke 6h (⌘ + wheel around the pointer, 961 → 889 px in 5 of 11 full runs): the view cut the anchored
  horizontal scroll to a width it had while the page sizes were still changing. The anchored scroll is now sent
  again once the zoom settles (`viewer_ui::zoom_to_at`), and the 2 s `want-x` timer restarts on each request.
  Green in the three full runs since (961 → 959, 960, 960 px). The flake is closed.

## Round 6 (25/10, the components Sol's re-review left open, and strong ones below 9)
- **C-files-011** The tab menu gains Close All Tabs, and ⌥⌘W closes every tab; documents with unsaved edits are
  asked about in turn (round 5's queue).
- **C-organize-014** Extract made one PDF and kept the pages: the Pages panel's Extract… opens Acrobat's options,
  "One file per page" (a folder chosen, "name - page 3.pdf" each) and "Delete pages after extracting" (an edit
  that can be undone; refused when it would leave no page). Proof: `reader_doc::tests::extract_one_file_per_page_writes_each_and_names_their_folder`,
  gui-smoke 30. not automatable: choosing the folder or file — the Linux file dialog goes through the XDG
  portal, which Xvfb does not have.
- **C-reader-025** A bookmark went to its page's top: it goes to its /XYZ or /FitH top edge, or its /FitR
  rectangle (links share the rule, `reader::dest_focus`). Proof: `reader::tests::a_destination_points_at_its_top_edge_or_rectangle`.
- **C-reader-022** The side panel kept 248 px: its edge drags it from 200 to 480 px, the thumbnails take the
  width once let go, and it is remembered. Proof: gui-smoke 31.
- **C-reader-021** ⌘[ came back to the page's top: it comes back to the point left in that page, across zooms
  (`ViewerUi::place`). Proof: `viewer_ui::tests::a_place_in_a_page_is_found_again_after_a_zoom`.
- **C-reader-028** A highlight saved without a note showed no words until its page was read: with the Notes panel
  open, the pages holding such highlights are read. Proof: gui-smoke 32 (its page moved third, never shown).
- **C-reader-015** Pairs only read left to right: More ▸ Right to left (Acrobat's page order), kept with the
  document's view. Proof: `viewer::tests::two_pages_side_by_side_and_the_cover_alone`, gui-smoke 33.
- **C-annotate-033** Replies were one block of text: each is its own line with its own bin; deleting one keeps the
  comment and its status. Proof: `reader_doc::tests` (reply test), gui-smoke 6c.
- **C-edit-018** Watermark always added: "Replace the watermarks already there", and a Remove mode (Acrobat's
  Remove Watermark; a file without one says so). Proof: sheaf-core
  `watermark::a_watermark_replaces_the_one_there_and_can_be_removed`, gui-smoke 10c.
- **C-organize-013** Pages could only be duplicated in their own document: ⌘C on selected thumbnails keeps them
  (as the document is, unsaved edits included), ⌘V in another document's Pages panel puts them after its selected
  page, an edit that can be undone (Acrobat's and PDF Expert's page copy). Proof: gui-smoke 34.
- **C-reader-061** Page labels were read only: the Pages panel's Number pages… (Acrobat's Page Labels) numbers the
  selected page and those after in 1 / i / I / a / A or none, with a prefix and a first number; an edit that can be
  undone. Proof: gui-smoke 35 (/PageLabels /S /r saved). The Pages strip has seven buttons now (smoke 3's places
  moved with them).
- **C-reader-032** The search timing was a bench left out of the suite: a 300-page file of full text pages is read
  as the search thread reads it, its first page under 0.5 s, every page read. Proof:
  `reader_doc::tests::a_search_shows_its_first_page_at_once_on_a_long_file`.
- **C-compare-007** The report compared words only: the areas that look different though their words are the same
  (pictures, drawings, scanned pages) are listed on a last page, page by page, and the result line counts them
  ("1 other changed area listed"). Proof: sheaf-core `compare::the_report_lists_a_changed_picture_whose_words_are_the_same`.
- **C-files-016** Sign refused a protected PDF: it asks for the password as every tool does, and the signed file
  keeps its protection (the save keeps the encryption, as Acrobat's does); a file whose owner password forbids
  both filling in forms and adding comments is refused ("does not allow signing"). The app's refusal path
  (`files::refuses_protected`, `SIGN_PROTECTED`, `ProbeFailure::RefuseProtected`, `Probe::protected`) is
  deleted. Proof: sheaf-core `sign::a_protected_input_is_signed_and_stays_protected`,
  `sign::signing_is_refused_where_the_owner_password_forbids_it`, sheaf-cli `cli` (sign test).
- **C-convert-012** PDF to JPG only rendered pages: an "Extract images" mode (iLovePDF's) saves every picture placed
  in the pages as stored, each once ("name-p2-1.png"), JPG or PNG; a PDF without pictures says so. Proof:
  sheaf-core `images::extract_images_writes_each_picture_as_stored_once`.
- **C-preferences-002** Only system/light/dark: Preferences ▸ Appearance adds the accent colour (Sheaf red, blue,
  green, purple, orange, graphite; a lighter tint in dark mode) and the background behind pages (as the theme, light
  grey, mid grey, dark, black: Preview's window background; Night and Sepia keep their own). Proof: gui-smoke 36.
- **C-sign-009** Every signature drawn was kept: the pad's "Keep for next time" (on by default), unticked, places
  it once and stores nothing of it (PDF Expert's). Proof: gui-smoke 37. Left: the kept ones live in prefs.json, not
  the system keychain.
- **C-files-002** Nothing showed while files were dragged over the window: it lights up as a drop zone saying what
  the drop will do ("Drop to open", "Drop to add to Merge", "Drop to open in a new tab"), in the warning colour with
  the reason when the tab shown refuses the file (`flow::drop_hint`, winit's HoveredFile). not automatable: a drag
  from a file manager — xdotool cannot start an XDND drag on Xvfb; the routing it states is
  `files::tests::drops_are_routed_by_tool_and_extension`'s.
- **C-output-007** The print sheet had no paper or orientation: Paper (the printer's own, A4, Letter, Legal) and
  Orientation (Auto, Portrait, Landscape) go to CUPS as `media=` and `orientation-requested=`. Proof:
  `print::tests::lp_gets_every_option`.
- **C-security-015** The result said "Verified" without showing the output: the result screen shows the first
  redacted page as written ("Page 3 as redacted": what is gone shows black), rendered on the job's thread. Proof:
  `state::tests::redact_result_says_how_many_areas_and_that_the_audit_passed`, gui-smoke 18.
- **C-output-004** The progress line was a bare percentage: it says the file worked on in a batch, how far, and the
  time left once enough is done to tell ("File 3 of 5 · 40% · about 30 s left"). Proof:
  `state::tests::progress_says_the_file_how_far_and_the_time_left`.
- **C-convert-020** Only Office conversion said where it ran: every tool's options end with "Runs on this
  computer. Nothing is uploaded." (Sheaf has no server to choose: offline is its one mode, by design.)

## Round 7 (25/10)
- **C-optimize-001** Compress only re-encoded pictures: identical streams (a picture or a font program embedded
  again for each page) are now kept once, every reference sent to the first. Proof: sheaf-core
  `compress::the_same_picture_embedded_twice_is_kept_once`.
- **C-preferences-008** The name only signed comments: Preferences ▸ Identity ▸ Your name is also written as the
  Author of the PDFs Sheaf makes from images and documents, when they have none (Acrobat's Create PDF). Proof:
  sheaf-core `images::a_pdf_made_from_images_takes_the_author_given_unless_it_has_one`.
- **C-convert-009** One rotated text or watermark turned the whole page into text boxes, its tables too: only what
  lies over the page floats (rotated text outside ruled tables, watermark-sized text over other text, a picture
  covering half the page under the text); the rest flows as paragraphs and Word tables
  (`pdfcraft-export::floating`, `layers`). A picture over text keeps the page positioned, as before. Proof:
  pdfcraft-export `word_a_diagonal_watermark_floats_and_the_table_stays_a_table`.
- **C-security-011** Only the page shown could be redacted whole: a range typed beside the button ("2-4, 7", ↩)
  marks each of those pages, one mark each in the list (Acrobat's Redact Pages). Proof: gui-smoke 18b.
- **C-security-003** Changing a password had no name: Protect on a file opened with its password says "This PDF
  already has a password: the new ones replace it" (Acrobat's Change Password), and does. Proof: sheaf-core
  `security::protecting_a_protected_file_changes_its_password`.
- **C-organize-004** Rotate offered 90/180/270 clockwise and no preview: Left 90°, Right 90° and 180° (Acrobat's
  directions), and each file's card shows its page 1 turned as chosen (iLovePDF). Proof: gui-smoke 39.
- **C-edit-005** The Edit tool's text took one of four colours: any other is typed as RRGGBB under them (Acrobat's
  Other Color); the field shows the colour of a selected text that is not a preset. Proof:
  `edit_ui::tests::a_colour_typed_in_hex_is_kept_and_shown_as_typed`.
- **C-organize-016** Split went by range or every N pages only, N by −/+: Acrobat's other two ways, by top-level
  bookmarks (one file each, named after it; the pages before the first go with it) and by file size (pages grouped
  while under the MB typed, each measured alone), and the N can be typed. Proof: sheaf-core
  `pages::split_by_top_level_bookmarks_names_each_file_after_its_bookmark`, `pages::split_by_size_groups_pages_under_the_limit`.
- **C-compare-003 / C-compare-004 / C-compare-006** Compare's results opened in another app, its changes only as a
  count: the report and the marked copy open in Sheaf's reader, the marked copy on its Notes panel, where each
  difference (replaced, inserted, deleted words; changed areas) is a comment to step through, filter and sort
  (Acrobat's in-app comparison and its change list). Proof: gui-smoke 26 (after the build: Overlay run, Open, the
  Notes list).
- **C-sign-004** A signature image kept its paper: a picture without transparency has its background removed when
  chosen, the ink kept in its colour with soft edges (PDF Expert's). Proof: sheaf-core
  `sign::a_signature_on_paper_loses_its_background_and_keeps_its_ink`. Left: no camera capture.
- **C-annotate-026** Comments could be undone and redone but not copied: ⌘C / ⌘X on a selected comment copies (or
  cuts) it, ⌘V pastes it on the page shown of any open document, a little below and right when on its own page
  (Acrobat's). The comment travels as its XFDF element, renamed so it never replaces the original; a copy made
  elsewhere after it wins the next ⌘V. Proof: `reader::tests::a_copied_comment_is_pasted_on_another_page_moved_and_renamed`,
  `reader_doc::tests::a_comment_is_copied_and_pasted_as_a_new_one`.
- **C-edit-010** Delete did not remove what the Edit tool picked: Delete takes out the picked drawings, paragraphs,
  images and added items in one step that ⌘Z undoes (PdfCraft patched: `Edit::DeleteObjects`, `delete_objects`,
  `delete_paths`). Proof: sheaf-core `edit_text::picked_drawings_and_words_are_deleted_together`. Left: copying
  page objects.
- **C-batch-009** A task ran in its tab with nothing to show it elsewhere: its tab carries the task's progress
  along its foot, seen while another document is read (the task goes on; Cancel stays in its tab). not
  automatable: a bar 3 px high under a tab during a task of a few seconds — the smoke's screenshots cannot catch it
  reliably on Xvfb; the progress itself is `state::tests::progress_says_the_file_how_far_and_the_time_left`'s.
- **C-convert-017** Export had only OCR and Excel's sheets: PDF to Word gains Acrobat's layout setting, Flowing text
  (paragraphs and Word tables, the default) or Page layout (each page as it looks, its text in editable boxes;
  pdfcraft-export `docx_with`, engine `OfficeFormat::DocxLayout`). Proof: pdfcraft-export
  `word_retain_page_layout_keeps_every_page_positioned`.
- **C-convert-001** No one entry to convert: Home's Open file takes PDFs, images, Office and HTML files and sends
  each where a drop would: PDFs to the reader, the others to the tool that makes them PDFs (Acrobat's Open /
  Create PDF; `open_with::plan`, tested by its `plan` tests). The drop zone says so.
- **C-organize-018** Crop took typed margins only: a box dragged on the page preview sets the four margins, in mm to
  a tenth, the cut parts dimmed as it goes (iLovePDF's crop box). Proof: gui-smoke 40.
- **C-sign-017** Signatures could not be inspected: Document Properties lists each one, its signer, Acrobat's
  verdict line (valid, unknown, invalid), when and why it was signed, and the sentences the verdict rests on
  (changed since, certificate not trusted…). Proof: `reader_doc::tests::document_properties_list_the_signatures_with_their_verdict`.
  The date reading is `facts::pdf_date`, shared with the document's own dates.
- **C-platform-010** The MCP server offered tools only: it also offers prompts (summarise a PDF, redact its personal
  data, prepare it to share; each names the tools to use) and resources (`sheaf://text/<path>`,
  `sheaf://info/<path>`, as templates). Proof: sheaf-cli `cli::mcp_offers_prompts_and_reads_resources`.
