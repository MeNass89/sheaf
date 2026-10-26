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
  gui-smoke 30, 44 (one file per page in the folder chosen; debug builds answer a dialog from
  `SHEAF_TEST_DIALOG`, `dialog::test_answer`, since Xvfb has no XDG portal).
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

## Round 8 (25/10, components at 0 the engine already carried, and fixes from rounds 6–7)
- **C-annotate-014** No polygon, polyline or cloud: the Shapes tool gains Polygon, Cloud and Polyline, clicked out
  vertex by vertex, a double-click to end, Escape to drop (Acrobat's tools; PdfCraft's Polygon and PolyLine). Proof:
  gui-smoke 41.
- **C-annotate-038** No measuring: the Shapes tool gains Distance (two clicks), Perimeter and Area (clicked out, a
  double-click to end), labelled in true size in cm (Acrobat's Measuring tools; PdfCraft's AddMeasurement). Proof:
  `reader::tests::measures_read_in_true_size`, gui-smoke 42.
- **C-annotate-017** No custom stamp: Stamp ▸ Image… takes a PNG or JPEG as the stamp, placed at each click,
  150 pt wide (Acrobat's custom stamp; PdfCraft's AddCustomStamp). Proof: gui-smoke 43.
- Test seam: debug builds answer file dialogs from the file `SHEAF_TEST_DIALOG` names (`dialog::test_answer`;
  release builds never read it), so the smoke chooses files and folders (gui-smoke 43, 44).
- **C-organize-008** No way to drop blank pages: Remove pages ▸ "The blank pages (found for you)": pages with no
  text and nothing drawn but specks (under 0.3 % dark) are removed (Acrobat's Remove Blank Pages); a file without
  any says so. Proof: sheaf-core `pages::blank_pages_are_found_and_text_or_drawn_ones_are_not`.
- Fixes found by the suite after rounds 6–7: the Redact page preview drew over the window's top-left (a layout
  sized by a computed width: now a plain column, its caption above); the export's floating layer measured its
  "body size" on itself (a watermark lost its place behind the text); Preferences ▸ Identity moved up under
  Appearance (it was below the fold); the grid now clears a pending opening size (Fit page could come back over it). Smoke
  fixes: X screenshots are read as 16-bit (`#E0E020202020`), so pixel colours are taken from the 8-bit `srgb()`
  field (the grid step read green as blue); a `key ctrl+s` sat behind a `#` and never ran; the damaged-PDF toast
  is read while the app opens (the wait for its first frame outlasted the toast's 3 s).
- gui-smoke 6h's "drift" (⌘ + wheel, 961 → 889 px, 5 of 11 runs; rounds 4 and 5 fixed the scroll for it) was
  the test's: traced (the anchor Rust computes, the scroll the view shows: 425.17 px both) and screenshotted, the
  point under the pointer stayed put; OCR read "fox" with the end of the arrow drawn beside it at 6c as one word,
  its centre 63 px right of the word, so the pointer went to empty space and "fox" was then found where it
  really was. The step anchors on "quick" now. Rounds 4–5's scroll resends stay (they cover a width that comes
  late), their comments no longer cite this.
- Document Properties list a file's signatures after its facts: their long lines pushed PDF/A below the sheet's
  fold (`reader_ui::properties::tests`). The 300-page search test built its fixture through the engine, 13,500
  text objects one by one, which never ended in a debug build (the unit layer hung): it is written by hand now,
  read in 4 s, its first page in 28 ms.

## Round 10 (25/10, Prepare form, and Sol's gaps from batch b12)
- **C-forms-006 / 007 / 008 / 009 / 010 / 011 / 012 / 014 / 003** No way to make a form: the Form tool (F, Acrobat's
  Prepare a Form) adds a field of the kind chosen in its bar (Text, Check box, Radio, Dropdown, List, Date,
  Button, Signature, Image) where a box is dragged, or at its own size where the page is clicked; radio buttons
  placed one after another make one group. Every field shows over its page, named; a click selects one: it is
  dragged to move, Delete removes it, and its card sets the name, the tooltip, Required, Read-only, Several lines
  and a dropdown's or list's items, copies it to every page (Acrobat's Duplicate across pages), as one undo step
  each. Detect fields turns a flat form's blanks into fields named from their labels (PdfCraft's detection,
  Acrobat's automatic field detection). Proof: `reader_doc::tests::a_form_is_prepared_fields_added_set_up_copied_and_deleted`,
  `a_flat_forms_blanks_become_fields_named_from_their_labels`, gui-smoke 53.
- **C-reader-013** (Sol b12) A chosen zoom was kept as a share of the window's width, so a document reopened in
  a wider or narrower window came back at another percentage: it is kept as its actual size (preferences mode 3;
  older entries read as before).
- **C-reader-030** (Sol b12) Two documents' attachments of one name at one index shared their temporary path,
  so opening the second showed the first's tab: the folder is per document tab and attachment.
- Sol's re-review of rounds 5–7 (batches b12–b13c, 25/10): 23 components agreed at 9, 34 reopened with their
  gaps (board history). Fixed here:
  - **C-files-011** Closing a document whose paragraph was being retyped lost the text without a word: a
    paragraph open in Edit text whose text changed counts as unsaved (Save applies it first).
  - **C-organize-014** One file per page wrote over files of the same name in the folder chosen: a name taken
    gets " (1)". Proof: gui-smoke 44 (a file already there is kept).
  - **C-reader-025** /XYZ destinations ignored their left edge and zoom: a bookmark or link now brings its zoom
    (1.0 = actual size), its top a third down and its left edge at the view's left (`ViewerUi::show_dest`).
    Proof: `viewer_ui::tests::a_destination_brings_its_zoom_its_top_and_its_left_edge`.
  - **C-reader-021** ⌘[ restored the height only: the view reports how far across it is scrolled, and a place
    keeps it. Proof: `viewer_ui::tests::a_place_keeps_how_far_across_a_wide_page_was_scrolled`.
  - **C-reader-028** The Notes panel read highlighted words on the window's thread: read on a thread of their own,
    the rows fill in as it ends.
  - **C-organize-013** Pages copied from a document opened with its password could not be pasted (the copy kept
    the encrypted bytes): they are written out as a new, unprotected PDF on the document's thread first.
  - **C-reader-032** The first search result waited for a batch of 16 pages: it is sent as soon as it is found.
    Proof: `search::tests::the_first_result_is_shown_at_once_then_every_sixteen_pages`.
  - **C-compare-004 / C-compare-007** A page added or removed at the end went unreported (the visual comparison
    stops at the pages both have): a page only the changed document has is a changed area of its own, pages only
    the original has are said, and the result line counts both. Proof: sheaf-core
    `compare::a_page_added_or_removed_at_the_end_is_reported`.
  - **C-output-007** Print refused a PDF opened with its password: an unprotected copy goes to the print service
    when the owner allows printing (else it says so). Proof: `print::tests::a_protected_pdf_prints_from_an_unprotected_copy_unless_printing_is_forbidden`.
  - **C-convert-012** Extract images missed pictures drawn inside forms: form XObjects are walked at any depth.
    Proof: sheaf-core `images::extract_images_finds_a_picture_drawn_inside_a_form`.
  - **C-security-015** Closing Redact dropped the marks not applied without a word: it asks first ("Close … without
    redacting?", Cancel keeps the tab). Proof: gui-smoke 54.
  - **C-files-011** proof: gui-smoke 54 (a retyped paragraph asks before its tab closes).
  - **C-annotate-026** A comment without a name in the file (/NM) could not be copied: it is found by its page and
    box. Proof: `reader::tests::a_copied_comment_is_pasted_on_another_page_moved_and_renamed`.
  - **C-reader-053** Measures ignored /UserUnit (a page at 2 read half its lengths). Proof:
    `reader::tests::measures_read_in_true_size`.
  - **C-edit-025** Number pages ran to the end whatever was selected: several pages selected are numbered alone.
  - **C-convert-013** Same fix as C-convert-012 (pictures inside forms).
- Prepare form lives in More ▸ Prepare form and on F: a toolbar button moved the bar's right half, which the
  smoke's coordinates (zoom field, More, the wrench) are measured on.

## Round 11 (25/10, the forms' remaining gaps and more of Sol's)
- **C-forms-012** The field card sets the text's size ("Auto" or points) and alignment for text fields, dropdowns
  and lists (Acrobat's Appearance ▸ Font size, Options ▸ Alignment).
- **C-forms-010** A button's card says what a click does: nothing, clear the form, print, or open a web address
  (Acrobat's Actions ▸ Mouse Up). Proof: `prepare::tests::a_size_typed_and_a_click_chosen_become_the_fields_properties`.
- **C-forms-003** Saving says which required fields are still empty ("Saved form.pdf. 2 required fields are
  empty: Name, City"). Proof: `prepare::tests::required_fields_left_empty_are_named_on_save`.
- **C-organize-008** (Sol b14) Remove blank pages had one fixed threshold: Strict, Normal (Acrobat's) or Scans
  (grey paper, white from 200; Stirling's threshold in three steps). Proof: sheaf-core
  `pages::blank_pages_are_found_and_text_or_drawn_ones_are_not` (a grey scan blank only at Scans).
- **C-organize-004** (Sol b13b) Rotate turned every page of the range: the even or odd ones only, the portrait or
  landscape ones only (as shown, their rotation counted), as Acrobat's Rotate Pages. Proof: sheaf-core
  `rotate::only_the_odd_or_even_and_the_portrait_or_landscape_pages_turn_when_asked`.
- **C-edit-005** (Sol b13b) A paragraph open in Edit text kept its own style whatever the format row said: what
  is changed in the row while it is open (colour, size, font, alignment) applies to it, the rest stays its own.
- **C-preferences-004** (Sol b14) Only the opening zoom could be chosen: Preferences ▸ Documents also sets the
  pages of a document never opened before: one column, two side by side, or two with the cover alone (PDF
  Expert's and Preview's default view). Proof: `tests/history.rs` (the preference is kept).
- **C-organize-016** (Sol b13b) Split by size wrote a page larger than the limit without a word: the result says
  which pages are over it alone (a page cannot be cut). Proof: sheaf-core `pages::split_by_size_groups_pages_under_the_limit`.
- **C-security-016** No Sanitize: More ▸ Sanitize document (Acrobat's) removes the hidden information in one step
  (metadata, scripts and actions, attachments, links, hidden text, layers: PdfCraft's Sanitize), undone by ⌘Z
  until saved. Proof: sheaf-core `security::sanitize_removes_the_metadata_the_scripts_and_the_attachments`.
- **C-security-001** (Sol b13c) The Protect panel had passwords and permissions only: it also removes the hidden
  information first when asked (Acrobat's Protect tool gathers both).
- **C-optimize-011** (Sol b13c) A damaged PDF closed its tab with an error: it opens in Repair PDF, ready to
  rebuild it (a file that does not start as a PDF is still refused). Proof: gui-smoke 11.
- **C-annotate-013** (Sol b13c) The arrow had one head, at one end: its head is chosen (open or closed arrow,
  circle, square, diamond: a click on the options bar's name goes to the next) and Both ends puts it at the start
  too (Acrobat's Line Ends).
- **C-security-017** (Sol b13c) Document Properties had only the four standard entries to edit: its own (custom)
  properties are listed and edited too, "Name: value; …" (Acrobat's Custom tab); one taken out is emptied.
  Proof: sheaf-core `facts::custom_properties_are_read_past_the_standard_ones`,
  `reader_ui::properties::tests::custom_properties_are_typed_as_name_and_value_pairs`.
- **C-compare-006** (Sol b13b) The proof never ran a comparison: gui-smoke 26 now runs one (Overlay) and opens
  its result, the marked copy in the reader with its differences listed in the Notes panel.

## Round 14 (25/10, Sol's b15 follow-ups)
- **C-output-007** (Sol b15b) A PDF that allows printing but forbids changes could not be printed: the copy for the
  print service was made through Remove security, which needs the right to change security. It is now decrypted
  with the open password alone (`security::printable_copy`), as Acrobat prints such a file. Proof:
  `print::tests::a_protected_pdf_prints_from_an_unprotected_copy_unless_printing_is_forbidden` (changes forbidden).
- **C-annotate-026** (Sol b15b) A copied picture stamp was pasted blank: XFDF carries no appearance. The copy keeps
  the stamp's picture (pdfcraft-engine `Document::stamp_picture_file`, patched in third_party) and ⌘V places a
  picture stamp in the same box. Proof: `reader_doc::tests::a_picture_stamp_is_copied_with_its_picture`.
- **C-output-012** (Sol b13c) Run again started from the default options: each task's options are kept with its
  history entry (passwords never: `saved_options`) and Run again starts the panel from them. Proof:
  `saved_options::tests::options_come_back_as_they_were_without_their_passwords`, `tests/history.rs`
  `a_tasks_options_are_kept_with_it_and_older_files_still_load`, gui-smoke 10 (Low compression comes back).
- **C-forms-014** (Sol b15a) Fields could be moved, deleted and copied one at a time only: ⇧-click selects
  several; dragged, they move together; their card aligns them on the first one picked (left, centre, right,
  top, middle, bottom), gives them its width, height or both, and spaces them evenly across or down (Acrobat's
  Align, Center, Match Size, Distribute); the arrow keys move the fields selected by 1 point (10 with ⇧), and
  Delete removes them all, one undo step each; Escape lets them go without leaving the tool. Proof:
  `reader::tests::fields_selected_together_align_match_and_spread_after_the_first`, gui-smoke 53 (two check boxes
  ⇧-clicked and aligned by Left).
- **C-forms-014** (Sol b15a, grids) The options bar's Grid (Acrobat's Grid with Snap to Grid) draws lines every
  9 pt over the pages (a stronger one every inch); fields drawn, placed by a click or dragged land on them.
  Proof: gui-smoke 53 (a field drawn with the grid on has every edge on it).
- Bug found on the way: a field placed by a click was its kind's size divided by the zoom (150 pt wide at 100 %,
  240 at 160 %); it is now its size in points whatever the zoom. Proof: gui-smoke 53 (a check box is 14 pt).
- **C-organize-022** (Sol b13c) Merge kept its files as cards with page ranges typed: Files | Pages opens them out
  into one card per page (its file and number under it, each file's page choice kept), moved with the arrows or
  dropped with ×, files mixed; the merge follows that order (Acrobat's Combine Files with its pages expanded).
  Files added meanwhile come in at the end; a file removed takes its pages. Proof: sheaf-core
  `pages::assemble_mixes_the_pages_of_several_files_in_the_order_given`, gui-smoke 21e.
- **C-forms-007** (Sol) Detect fields found nothing on a scanned form (its lines are a picture): the page is
  rendered at 144 dpi, its lines and boxes are found in the pixels (`form_scan::lines_in`: dark bands one to six
  pixels thick; a box is two of the same span joined by dark sides), its words read by OCR for the labels, and
  PdfCraft's detection runs on them (Acrobat's Prepare Form on a scan). Proof: sheaf-core
  `form_scan::tests::a_scans_rules_and_boxes_are_found_and_its_dark_areas_and_strokes_are_not`,
  `two_lines_without_sides_are_two_rules`, gui-smoke 53 (a scanned form: two text fields and a check box).
- **C-organize-010, C-organize-012, C-organize-020** (never built: the audit of 25/10 found the engine ready) A
  thumbnail's right-click menu (Acrobat's and Preview's) gathers the page actions and adds Insert pages from file…
  (a PDF after the page, `Edit::InsertPagesFrom`), Replace with a file's pages… (its first pages in the selected
  pages' places, refused when it has fewer, `Edit::ReplacePages`) and Reverse page order (the selected run, else
  every page, one undo step). Proof: gui-smoke 35b.
- **C-edit-019** (never built; engine ready) More ▸ Page background… (Acrobat's Edit a PDF ▸ Background): six pale
  colours or a picture (image or PDF page) behind every page's content, at 100, 75 or 50 %, replacing an earlier
  background; Remove takes out the ones a file has; one undo step. Proof: gui-smoke 35c.
- **C-edit-015, C-edit-017** (never built; engine ready) The Page numbers tool (found by "header", "footer" or
  "bates") adds Acrobat's Header & Footer: text in any of the six places (top and bottom, left, centre, right) with
  {n}, {p}, {date} and {bates}; Bates numbers with a prefix, a first number and 3 to 15 digits; "No page number"
  for text alone; Replace takes out the file's own headers and footers first. Proof: sheaf-core
  `numbers::tests::header_and_footer_tokens_take_the_date_and_the_bates_number`, `tests/numbers.rs`
  `header_and_footer_text_and_bates_numbers_without_a_page_number`, gui-smoke 20d.
- **C-annotate-008** (never built; engine ready) Text box ▸ Callout (Acrobat's Callout tool): the click is where the
  callout points; its box, 60 pt up and right (left or down near the page's edges), is joined to it by a bent line
  with an open arrow; the text typed goes in the box. Proof: gui-smoke 60.
- **C-output-014** (partial before: undo and redo only) More ▸ Versions… (Acrobat's File ▸ Revert, and the
  versions an incremental save keeps): Revert to saved drops the unsaved edits and their undo history (said in red
  first); each version saved in the file opens in a tab of its own; Undo and Redo name what they take back
  ("Undo Add highlight"), as Acrobat's Edit menu. Proof: gui-smoke 35d.
- **C-convert-004** (never built) Home ▸ From clipboard (Acrobat's Create PDF from Clipboard, Preview's New from
  Clipboard): a picture on the clipboard becomes a page of its size, text becomes US Letter pages in 11 pt
  Helvetica; named in the Save dialog, then opened. Proof: sheaf-core `pages::a_pdf_is_made_from_a_pasted_picture_or_text`,
  gui-smoke 45b.

## Round 15 (26/10, Sol's b16 gaps and the audit's)
- **C-reader-025** (Sol b16c) /FitR kept the zoom and could leave its rectangle cut off: the rectangle fills the
  view (`ViewerUi::zoom_to_fit`), shown whole when it fits. Proof: `viewer_ui::tests::a_destination_brings_its_zoom_its_top_and_its_left_edge` (FitR).
- **C-reader-011** (never built) Zoom to area (Z, or More ▸ Zoom to area: Acrobat's Marquee Zoom): the next box
  dragged on a page fills the view, ⌘[ comes back; Z again or Escape lets it go. Proof: gui-smoke 61.
- **C-security-016** (Sol b16b) A refused Sanitize (pages being rearranged) was covered by the success message:
  the document's thread says what was removed only once it was. Proof:
  `reader_doc::tests::sanitize_says_what_it_removed_only_when_it_did`.
- **C-security-015** (Sol b16b) Quitting dropped Redact's marks not applied: the window's close asks about them
  as closing their tab does. not automatable: the window's close button in Xvfb (no window manager sends it) —
  the same dialog's path is gui-smoke 54 (closing the tab).
- **C-files-011** (Sol b16b) A paragraph open in Edit text with only its format changed was dropped on close: it
  counts as an unsaved change. Proof: gui-smoke 54 (made bold, then closed: asked).
- **C-convert-012, C-convert-013** (Sol b16b) Extract images held the pdfium lock for the whole document: it is
  held while a page's pictures are read, not while they are encoded and written (as PDF to JPG).
- **C-compare-004, C-compare-007** (Sol b16c) A moved line went unreported when a word of another column changed at
  the same height: a change on the line counts only within the run of words it belongs to (neighbours closer than
  an 18 pt gutter). Proof: sheaf-core `compare::a_line_moved_with_the_same_words_is_reported` (two columns).
- **C-compare-003** (Sol b16b) Semantic Text said pictures were not part of the report while it listed their
  changes, and two unrelated documents were compared without a word: the panel says the report lists the words,
  then pictures, drawings and scanned pages (Acrobat's report does both), and documents sharing almost no words
  (80 % or more changed) are said to differ almost everywhere. Proof: sheaf-core
  `compare::documents_that_share_almost_no_words_are_said_to`.
- **C-preferences-004** (Sol b16b) No page to start on and no opening layout: Document Properties ▸ When opened
  (Acrobat's Initial View) sets the page a document opens at, its layout (one page, two, cover alone), its zoom
  (fit page, fit width, actual size) and its side panel (bookmarks, pages), written in the file for every reader;
  a document never opened in Sheaf opens as it asks, over Preferences ▸ Documents. Proof: gui-smoke 62.
- **C-sign-017** (Sol b16a) A certificate's summary was cut short with no way to read it: a click on any About
  line of Document Properties shows it in full (issuer, subject and dates), a second click folds it.
  not automatable: the click on a cut-off line needs a signed PDF in the GUI suite, which has none (its text,
  issuer and dates, is `reader_doc::tests::document_properties_list_the_signatures_with_their_verdict`'s).
- **C-output-004** (Sol b16a) A file without pictures showed no progress and Cancel waited for the end:
  compressing each stream reports its progress too (pictures to 60 %, streams to 90 %), and Cancel is heard
  every 64 streams and before writing. Proof: sheaf-core `compress::compress_says_how_far_it_is_and_stops_when_cancelled`
  (a file without pictures).
- **C-organize-018** (Sol b16a) Crop's preview showed page 1 only: ‹ › under it step through the file's pages,
  the box drawn on any of them. Proof: gui-smoke 40 (page 2 shown).
- **C-reader-050** (partial before) More ▸ Copy an area as a picture (Acrobat's Snapshot): the next box dragged
  on a page goes to the clipboard as the page shows it, at 216 dpi (less past 40 megapixels). Proof:
  `reader_doc::tests::an_area_of_a_page_is_copied_as_its_pixels`, gui-smoke 63.
- **C-edit-010** (Sol b16a) ⌘C/⌘X copied a picked picture only: drawings, paragraphs and several objects picked
  with the Edit tool are copied as a picture of what they cover (⌘X then deletes them), as Acrobat's Snapshot of a
  selection; pasting brings them back as a picture (PdfCraft has no object copy). Proof: gui-smoke 20c (a
  drawing picked, ⌘C), `reader_doc::tests::an_area_of_a_page_is_copied_as_its_pixels`.
- **C-forms-012** (Sol b16c) A field whose /DA names its font by a resource key (/F1) read as Helvetica, so
  choosing Helvetica changed nothing: the key is looked up in the form's resources (pdfcraft-engine
  `Document::form_font_base`, patched in third_party). Proof:
  `reader_doc::tests::a_fields_font_named_by_a_resource_key_is_read_through_the_forms_resources`.
- **C-convert-014** (Sol b16a) RTF gave a table cell its first block's style: each block keeps its own (bold,
  italic, size, colour), a block below the one before on a new line, as the Word export lays cells out
  (pdfcraft-export `rtf_cell`, patched in third_party). Proof: pdfcraft-export `tests::rtf_table_cells_keep_each_blocks_style`.
- **C-convert-015** (Sol b16a) The XML export wrote a picture's size only: the picture goes in, in base64 lines
  with its type (PNG or JPEG), so the file holds the whole document (pdfcraft-export `xml`, patched). Proof:
  pdfcraft-export `tests::xml_holds_each_page_with_its_headings_paragraphs_and_pictures`.
- **C-annotate-038** (Sol b16a) Eight preset scales and no way to calibrate on a resized plan: Distance ▸
  Calibrate (Acrobat's, Bluebeam's): the next line drawn is a length you know, its real length is typed ("4.5 m",
  "12 ft", mm to mi), and the scale it gives measures from then on ("Calibrated · m"; the scale button goes back to
  the presets). Proof: `reader::tests::a_known_length_calibrates_the_scale`, gui-smoke 42.
- **C-reader-004** (never built) More ▸ Scroll automatically (Acrobat's Automatically Scroll): the pages move up by
  themselves (the view is moved in the UI, 60 times a second), ↑ and ↓ slow it down or speed it up, Escape stops.
  Proof: gui-smoke 64.
- **C-sign-006** (never built) Signature ▸ Initials (Acrobat's Fill & Sign ▸ Add Initials): initials are drawn or
  typed on their own pad ("Create your initials"), kept apart from the signatures, and placed 60 pt wide by a click;
  New initials forgets them. Proof: gui-smoke 65.
- **C-sign-007** (never built) Signature ▸ Every page: one click places the signature (or initials) at the same
  place on every page, one undo step. Proof: gui-smoke 65 (initials on all four pages).
- **C-optimize-003** (never built) Compress ▸ To a size (iLovePDF's and Smallpdf's target size): stronger settings
  are tried in turn (Low, Recommended, Extreme, then two beyond) until the file is the size typed or smaller; out of
  reach, the smallest made is kept and the result says so. Proof: sheaf-core
  `compress::a_target_size_tries_stronger_settings_until_the_file_fits`, gui-smoke 66.
- **C-edit-005** (Sol b13b) A paragraph edited with a colour of the palette's (F-014) came out in a preset one:
  Edit text's format row, carets and Replace text take the colour chosen, palette included (`ink_at`).
