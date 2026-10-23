# Parity board

Inspirations: Adobe Acrobat (Reader + Pro tools) (), PDF Expert (Readdle) (), iLovePDF Desktop (2.3.0), Stirling-PDF (), macOS Preview ()
331 components · at parity (≥9): 126 · open 205, closed 126
Rendered by `dev/scripts/board.py render` from `dev/board/components.json`. Do not edit by hand.

## Queue (next first)
| # | id | weight | component | main | F · A · P → overall | next |
|---|---|---|---|---|---|---|
| 1 | C-optimize-006 | strong | OCR a scanned PDF | ilovepdf | 7 · 6 · 6 → **6** | speed on big scans; deskew |
| 2 | C-security-010 | strong | Mark an area or text for redaction | acrobat | 6 · 7 · 8 → **6** | mark by text selection |
| 3 | C-convert-009 | strong | Convert PDF to Word | ilovepdf | 8 · 7 · 9 → **7** | layout fidelity |
| 4 | C-edit-001 | strong | Enter edit mode | pdfexpert | 7 · 8 · 9 → **7** | edit mode inside the reader, not a separate tool flow |
| 5 | C-edit-007 | strong | Move, resize and select page objects | acrobat | 7 · 8 · 9 → **7** | resize handles, move existing page objects (engine MoveObjects) |
| 6 | C-files-015 | strong | Manage a tool's input files | ilovepdf | 7 · 7 · 8 → **7** | drag reorder |
| 7 | C-organize-015 | strong | Split by page ranges or split points | ilovepdf | 8 · 7 · 8 → **7** | split points chosen visually |
| 8 | C-organize-021 | strong | Merge files into one PDF | ilovepdf | 7 · 7 · 8 → **7** | per-file page ranges |
| 9 | C-output-004 | strong | Run progress and cancel | ours | 8 · 7 · 8 → **7** | — |
| 10 | C-output-007 | strong | Print a document | pdfexpert | 8 · 7 · 9 → **7** | print via the OS |
| 11 | C-platform-011 | strong | Open files from the system | ours | 7 · 7 · 9 → **7** | registered as Alternate viewer on macOS |
| 12 | C-platform-012 | strong | Native desktop builds | ours | 7 · 7 · 9 → **7** | macOS notarisation (needs Nassim's Apple account) |
| 13 | C-security-015 | strong | Apply redactions | acrobat | 8 · 7 · 8 → **7** | preview of the redacted output |
| 14 | C-convert-002 | strong | Convert Office documents to PDF | ilovepdf | 8 · 9 · 9 → **8** | needs LibreOffice installed; no sheet/print-area options |
| 15 | C-files-002 | strong | Drag and drop files to open | ilovepdf | 8 · 8 · 9 → **8** | drop on the reader opens the document |
| 16 | C-optimize-001 | strong | Compress a PDF (levels) | ilovepdf | 8 · 8 · 8 → **8** | seed bug: Save after Compress needs Enter; fonts/vector recompression |
| 17 | C-organize-001 | strong | Open page organizer grid | pdfexpert | 8 · 8 · 9 → **8** | grid in the reader; multi-select; several files |
| 18 | C-output-002 | strong | Save as a new file | pdfexpert | 8 · 9 · 9 → **8** | seed bug: the Save panel's Save button ignores clicks, only Enter works |
| 19 | C-reader-010 | strong | Gesture zoom and pan | pdfexpert | 8 · 8 · 8 → **8** | pinch zoom and drag-pan (hand tool / space-drag) |
| 20 | C-annotate-008 | normal | Add callout or speech bubble | acrobat | 0 · 0 · 0 → **0** | after core |
| 21 | C-annotate-021 | normal | Add link annotation | preview | 0 · 0 · 0 → **0** | with edit links |
| 22 | C-compare-005 | normal | View documents side by side, synced | stirling | 0 · 0 · 0 → **0** | synced side-by-side view |
| 23 | C-convert-004 | normal | Create PDF from clipboard | preview | 0 · 0 · 0 → **0** | light |
| 24 | C-convert-006 | normal | Create PDF from scanner or screenshot | acrobat | 0 · 0 · 0 → **0** | light |
| 25 | C-convert-008 | normal | Create an empty document | pdfexpert | 0 · 0 · 0 → **0** | engine create_blank |
| 26 | C-convert-013 | normal | Extract embedded images | ilovepdf | 0 · 0 · 0 → **0** | after core |
| 27 | C-convert-014 | normal | Export PDF as text or RTF | acrobat | 0 · 0 · 0 → **0** | export text (engine text layer) |
| 28 | C-edit-008 | normal | Replace, rotate or edit an image | acrobat | 0 · 0 · 0 → **0** | engine EditPageImage |
| 29 | C-edit-012 | normal | Find and replace text | acrobat | 0 · 0 · 0 → **0** | engine ReplaceText |
| 30 | C-edit-015 | normal | Add header and footer | acrobat | 0 · 0 · 0 → **0** | engine has page marks; add header/footer tool |
| 31 | C-edit-021 | normal | Add and edit links | acrobat | 0 · 0 · 0 → **0** | engine links |
| 32 | C-files-006 | normal | Star or pin favourite files | pdfexpert | 0 · 0 · 0 → **0** | light |
| 33 | C-files-007 | normal | Search, sort and select listed files | stirling | 0 · 0 · 0 → **0** | light |
| 34 | C-files-009 | normal | Cloud, network and shared files | acrobat | 0 · 0 · 0 → **0** | out of local-first scope (needs Nassim) |
| 35 | C-forms-003 | normal | Required field warnings | acrobat | 0 · 0 · 0 → **0** | after fill |
| 36 | C-forms-006 | normal | Enter form editing mode | acrobat | 0 · 0 · 0 → **0** | after core |
| 37 | C-forms-007 | normal | Auto-detect form fields | pdfexpert | 0 · 0 · 0 → **0** | after core |
| 38 | C-forms-008 | normal | Add a text field | acrobat | 0 · 0 · 0 → **0** | engine AddField |
| 39 | C-forms-009 | normal | Add choice fields | acrobat | 0 · 0 · 0 → **0** | engine AddField |
| 40 | C-forms-012 | normal | Edit field properties | acrobat | 0 · 0 · 0 → **0** | after core |
| 41 | C-forms-014 | normal | Move, duplicate and arrange fields | acrobat | 0 · 0 · 0 → **0** | after core |
| 42 | C-optimize-003 | normal | Compress to a target size | stirling | 0 · 0 · 0 → **0** | after core |
| 43 | C-optimize-010 | normal | Enhance scanned pages | acrobat | 0 · 0 · 0 → **0** | after core |
| 44 | C-organize-010 | normal | Insert pages from another file | pdfexpert | 0 · 0 · 0 → **0** | insert pages from another PDF |
| 45 | C-output-009 | normal | Share a document | pdfexpert | 0 · 0 · 0 → **0** | OS share (macOS only) |
| 46 | C-output-010 | normal | Email as attachment | acrobat | 0 · 0 · 0 → **0** | mailto with attachment |
| 47 | C-output-014 | normal | Revert, versions and undo history | preview | 0 · 0 · 0 → **0** | revert to saved |
| 48 | C-platform-001 | normal | Check for updates | ilovepdf | 0 · 0 · 0 → **0** | light; release channel decision is Nassim's |
| 49 | C-preferences-003 | normal | Choose interface language | ilovepdf | 0 · 0 · 0 → **0** | localisation (after core) |
| 50 | C-preferences-004 | normal | Set default view on open | acrobat | 0 · 0 · 0 → **0** | with reader view modes |
| 51 | C-reader-011 | normal | Zoom to a selected area | acrobat | 0 · 0 · 0 → **0** | after core |
| 52 | C-reader-014 | normal | Single page view | pdfexpert | 0 · 0 · 0 → **0** | page modes: single, continuous |
| 53 | C-reader-017 | normal | Rotate the view | acrobat | 0 · 0 · 0 → **0** | view rotation |
| 54 | C-reader-045 | normal | Split view | pdfexpert | 0 · 0 · 0 → **0** | after core |
| 55 | C-reader-050 | normal | Select or snapshot an area | pdfexpert | 0 · 0 · 0 → **0** | after core |
| 56 | C-security-016 | normal | Sanitize a document | acrobat | 0 · 0 · 0 → **0** | engine hidden_info; add sanitize |
| 57 | C-sign-006 | normal | Add initials | acrobat | 0 · 0 · 0 → **0** | initials |
| 58 | C-sign-007 | normal | Repeat a signature on several pages | ilovepdf | 0 · 0 · 0 → **0** | after core |
| 59 | C-sign-016 | normal | Validate signatures | acrobat | 0 · 0 · 0 → **0** | engine signatures panel |
| 60 | C-platform-014 | normal | Native menu bar and shortcuts | ours | 2 · 4 · 9 → **2** | menu bar and shortcuts (File/Edit/View/Go/Tools) |

## By domain
| domain | components | avg overall | at ≥9 |
|---|---|---|---|
| ai | 7 | 0.0 | 0 |
| annotate | 38 | 6.2 | 24 |
| batch | 9 | 2.6 | 1 |
| compare | 7 | 6.0 | 2 |
| convert | 20 | 3.8 | 3 |
| edit | 29 | 4.4 | 8 |
| files | 19 | 6.2 | 11 |
| forms | 17 | 2.1 | 4 |
| optimize | 15 | 3.3 | 2 |
| organize | 27 | 4.3 | 8 |
| output | 15 | 4.3 | 3 |
| platform | 14 | 3.5 | 2 |
| preferences | 14 | 2.8 | 4 |
| reader | 64 | 5.9 | 41 |
| security | 18 | 5.4 | 6 |
| sign | 18 | 4.4 | 7 |

## Closed
| id | component | overall | last proof |
|---|---|---|---|
| C-annotate-001 | Show annotation toolbar | 9 | All tools in one visible row with letter shortcuts, a floating options bar per tool; colours kept per tool. Joint 9/10: independent visual audit (interim reviewer) against PDF Expert / iLovePDF captures, from gui-smoke screenshots, agreed (vs pdfexpert/05-annotate-toolbar) |
| C-annotate-002 | Highlight selected text | 9 | Highlight: 1,982-page 70 MB book (reader_doc::tests::edit_cost_on_a_big_file): engine 0.19 s per edit (was 2.1 s), drawn at once provisionally; gui-smoke saves and checks the file |
| C-annotate-003 | Underline selected text | 9 | Underline: 1,982-page 70 MB book (reader_doc::tests::edit_cost_on_a_big_file): engine 0.19 s per edit (was 2.1 s), drawn at once provisionally; gui-smoke saves and checks the file |
| C-annotate-004 | Strike through selected text | 9 | Strikethrough: 1,982-page 70 MB book (reader_doc::tests::edit_cost_on_a_big_file): engine 0.19 s per edit (was 2.1 s), drawn at once provisionally; gui-smoke saves and checks the file |
| C-annotate-006 | Add sticky note | 9 | N, click, type: Note with popup; a click on it opens its text to read and edit (SetAnnotationContents), Notes panel follows (dev/re/evidence/F-002/06, dev/re/evidence/F-002/36) |
| C-annotate-007 | Add text box | 9 | Text box in the format row's font (12 standard fonts, bold, italic), size and alignment, grows to keep its text, one undo step (Edit::AddTextBox, vendored pdfcraft-annot); reader_doc::tests::a_text_box_takes_a_font_grows_to_fit_and_undoes_in_one_step; gui-smoke 'a text box takes the format row's font'; 300 boxes on the 1,982-page book: edit 0.19 → 0.27 s; joint 9, 3 rounds, 22/10 |
| C-annotate-009 | Change text annotation font | 9 | Selected text box: the row shows its font/size/alignment, each control restyles only its own setting (Edit::SetTextFont), free size typed or stepped (8–72 presets), no-op clicks add no undo, palette follows the comment; tools::tests::text_sizes_step_through_the_presets_and_can_be_typed; gui-smoke 'a selected text box takes a new font, alignment and a typed size, and grows'; joint 9 round 3 |
| C-annotate-010 | Draw freehand ink | 9 | Pen with 5 colours and 3 widths, strokes smoothed (Chaikin ×2) as PDF Expert's pen (dev/re/evidence/F-002/37) |
| C-annotate-011 | Erase ink strokes | 9 | Eraser (X, toolbar): drag across drawings, rubbed out along the path (PdfCraft EraseInk; a stroke split or removed), three sizes, one undo step, signatures left alone; gui-smoke erases a pen stroke entirely |
| C-annotate-012 | Draw rectangle or oval | 9 | Shapes tool (R): drag a rectangle or oval with a live outline; colour, width, fill, opacity; moved by drag, resized by the corner handle; gui-smoke saves a rectangle and a filled oval at 75 % |
| C-annotate-016 | Place a stamp | 9 | Stamp tool (M): Acrobat's 22 stamps (standard, sign here, dynamic) from a list in the options bar, placed with a click, 160 × 42 pt; moved and resized like other comments; gui-smoke places Approved and a dated Received |
| C-annotate-018 | Add check, cross or symbol | 9 | Stamp tool list starts with Acrobat's Fill & Sign marks (✓ ✗ • —), placed with a click at 18 pt; gui-smoke places a checkmark |
| C-annotate-019 | Add date stamp | 9 | Dated stamps (Approved, Confidential, Received, Reviewed, Revised · DATED): 'By <author> at 11:32 am, Oct 21, 2026' in the stamp and /PCByLine (gui-smoke checks it) |
| C-annotate-022 | Change annotation colour | 9 | Selected annotation recoloured from the colour row, whose dot shows its current colour (dev/re/evidence/F-002/14) |
| C-annotate-024 | Edit annotation properties and defaults | 9 | Each tool keeps its colour, width, opacity between sessions; new tools start from defaults; the selection bar's highlight uses the highlighter's colour. Joint 9/10: Sol agreed in round 3 (gui-smoke: Shapes style in the preferences) |
| C-annotate-025 | Delete an annotation | 9 | Click selects (blue outline), Delete or the panel's trash removes, ⌘Z restores (dev/re/evidence/F-002/15) |
| C-annotate-029 | Remove all annotations | 9 | Notes panel trash: Remove all comments in one step, ⌘Z brings them back (gui-smoke checks the saved file both ways) |
| C-annotate-030 | List comments in a panel | 9 | Notes panel: kind, page, text, then 'author · date' as the file records them (PDF Expert's comment list); click jumps, delete button. Author/date row checked by screenshot (not automatable: reading text off the panel); author written into new annotations checked by gui-smoke |
| C-annotate-031 | Sort and filter comments | 9 | Notes panel: Show (All, Markup, Notes, Drawings, Stamps) and Sort (page, newest, author, type), rows keep their comment's identity (gui-smoke: the first row of Stamps deletes a stamp, shapes kept; reader::tests::notes_filter_groups) |
| C-annotate-032 | Hide or show all comments | 9 | Notes panel Hide all / Show all: pages render without comments (form fields still drawn), hidden comments not selectable; gui-smoke samples a stamp hidden and shown |
| C-annotate-033 | Reply to a comment | 9 | Notes panel: Reply on the selected comment opens the editor (↩ sends, ⇧↩ new line); replies listed under their comment and saved as /IRT replies (reader_doc::tests::a_reply_is_listed_under_its_comment; gui-smoke saves one); review status shown when present |
| C-annotate-034 | Set comment review status | 9 | Selected comment in Notes: status list (No status, Accepted, Rejected, Cancelled, Completed) as Acrobat's Set Status; shown after the author and date; saved as a status reply (reader_doc::tests::a_review_status_shows_on_its_comment; gui-smoke saves Accepted) |
| C-annotate-035 | Export or summarise comments | 9 | Notes panel: Summary of comments (PDF) and Export comments to XFDF or FDF, as Acrobat (reader_doc::tests::comments_are_exported_and_imported, a_summary_of_comments_is_written). Dialogs not automatable here (portal) |
| C-annotate-036 | Import comments | 9 | Notes panel: Import comments from XFDF or FDF (one undo step; every page renders again); unit test exports from one copy and imports into another |
| C-batch-007 | Chain another tool from results | 9 | Continue to… on the result screen chains any tool with the outputs, as iLovePDF; dev/re/evidence/F-001/sheaf-compress-result.png beside dev/re/evidence/ilovepdf/05c-compress-result.png |
| C-compare-002 | Swap the compared files | 9 | Swap the compared files in one click; Joint 9/10: toolbox audit (interim reviewer) against iLovePDF's QML, agreed |
| C-compare-007 | Download a change report | 9 | The written comparison PDF is the change report; Joint 9/10: toolbox audit (interim reviewer) against iLovePDF's QML, agreed |
| C-convert-003 | Convert images to PDF | 9 | iLovePDF's options: Fit / A4 / US Letter, orientation and margin for paper, Merge all images into one PDF (off: a PDF next to each image); PNG JPEG WebP TIF/TIFF BMP GIF; images opened from outside open the tool; JPEGs embedded as they are, EXIF rotation applied. Tests: images (Letter landscape, tif+gif), run, dest, open_with, route_drop; gui-smoke 16. Reviewer (Opus) agreed 9/9/9. |
| C-convert-012 | Export pages as images | 9 | 150/300/600 dpi, JPG or PNG, count in the result title; 72 Mpx cap, pdfium lock released while encoding (images.rs); tests/images.rs pdf_to_jpg_one_file_per_page_at_dpi; TIFF a known gap (iLovePDF has none); joint 9 round 3 |
| C-convert-020 | Choose online or offline conversion | 9 | Offline only by design, said in the converters' info box; Joint 9/10: toolbox audit (interim reviewer) against iLovePDF's QML, agreed |
| C-edit-002 | Edit existing text in place | 9 | Click a paragraph, edit: the page shows it as typed after a 300 ms pause (reader_doc Op::Preview; PdfCraft Session::preview/withdraw keep redo, oldest undo and dirty), whole paragraph rewraps (soft_breaks), styles kept across edits (Myers), runs touching mid-word kept together, substitute-font and '?' notices; ⌘↩ one undo step, Escape leaves no trace, ⌘S commits typed text, print refused mid-edit. Tests: edit_text.rs, reader_doc preview test, gui-smoke 2e. Reviewer (Opus) agreed 9/9/9. |
| C-edit-003 | Add a new text box | 9 | E on empty space adds text with the format row (Sans/Serif/Mono, bold, 9–24 pt, 5 ink colours); gui-smoke checks the text (pdftotext) and the font (pdffonts: Times-Bold) |
| C-edit-006 | Add an image to a page | 9 | Edit tool: select a page image, corners resize it live keeping proportions (the picture itself follows, one preview in flight), drag moves it live, bar rotates/flips/replaces/deletes, Delete key; Add image and ⌘V paste place it (80 % cap each way) selected; WebP/TIFF/BMP/GIF. gui-smoke 14 on the saved file: resize ratio, flip (red right), rotate right (red down), Delete takes only the selected of two, ⌘V 120×60 then Delete. Reviewer (Opus) agreed 9/9/9 (dev: /tmp/rv/edit006-live.png). |
| C-edit-011 | Undo and redo edits | 9 | Undo/redo ⌘Z ⇧⌘Z: 0.19 s on the 1,982-page book (was 2.0 s); gui-smoke: undo then redo restores a note, undo brings a deleted page back |
| C-edit-014 | Save edited PDF | 9 | ⌘S saves edits in place (incremental, validated by check_save), Save As copies; gui-smoke checks every saved file with qpdf/pdftotext |
| C-edit-016 | Add page numbers | 9 | Font section (families in their own look, size pt, B/I/U), presets + RRGGBB with swatch, batch range clamped per file; tests/numbers.rs (font, underline, custom colour, clamp, redaction of a font name); 1,982-page book numbered in 12.9 s (was 83 s: O(n²) page re-listing in pdfcraft-edit, per-read cache sum in pdfcraft-cos); joint review round 3 |
| C-edit-018 | Add a watermark | 9 | Live preview pinned above the options (640 px, redrawn 300 ms after a change, one page copy per file, reason shown when no mark); gui-smoke 'the watermark preview shows the mark as typed, in its colour' (0 → 116/10000 red); mosaic on the 1,982-page book 15.2 s; joint review round 5 |
| C-edit-026 | Insert a blank page while editing | 9 | Pages panel: Insert blank page after the selection; gui-smoke checks an empty page 2 in the saved file and that ⌘Z removes it |
| C-files-001 | Open a PDF from a file picker | 9 | Home ▸ Open file: system dialog (GTK portal on Linux) → the PDF opens in a reader tab (dev/re/evidence/F-002/29, dev/re/evidence/F-002/30) |
| C-files-003 | Home screen | 9 | Home as iLovePDF's: drop zone with Open file, Recent tools (latest runs' tools, wrapping), Recent files with thumbnails, remove and clear. Joint 9/10: independent Opus reviewer (interim for Sol, see DECISIONS) agreed in round 3 |
| C-files-004 | Reopen a recent file | 9 | Recents with thumbnails reopen a PDF in the reader at the page it was left on (777, dev/re/evidence/F-002/34) |
| C-files-005 | Manage the recent files list | 9 | Recents: × on hover removes one, Clear recents removes all, Tasks History untouched (hidden-recents.json; unit tests). Joint 9/10: Sol built it and set on-screen validation as its condition; Claude reviewed, validated on screen and fixed (removal checked on screen) |
| C-files-010 | Documents in tabs or windows | 9 | Tabs drag to reorder (drop line), right-click Close Other/Left/Right, 1px dividers as iLovePDF; only the bar order moves so tab pages keep options and scroll. gui-smoke 13: drag reorders, dragged document keeps Page 4, Close Other Tabs leaves one. Reviewer (Opus) agreed 9/9/9. |
| C-files-011 | Close documents, tabs and windows | 9 | ⌘W closes the document tab; with unsaved edits Save / Don't Save / Cancel (dev/re/evidence/F-002/16, dev/re/evidence/F-002/17); window close asks for each dirty document |
| C-files-012 | Browse and open tools | 9 | Tools grid by category with colour tiles, collapsible, sort, search, favourites, recent tools. Joint 9/10: independent visual audit (interim reviewer) against PDF Expert / iLovePDF captures, from gui-smoke screenshots, agreed (vs ilovepdf/01c) |
| C-files-013 | Search tools | 9 | Tools search: ranked (name start, name, description word, letters in order: 'cmprs' finds Compress), results as a list, ↑↓ with the chosen row kept in view, ↩ opens, Escape clears; focused on Tools, + and ⌘F (sheaf_core tests; gui-smoke: 'pdf to', ↓, ↩ opens PDF to Word without clicking the field). Joint 9/10: independent Opus reviewer (interim for Sol, see DECISIONS) agreed in round 3 |
| C-files-014 | Favourite and recent tools | 9 | Star any tool (Tools and Home cards; prefs favourite_tools), Favourites first on Tools; Home's Recent tools from the latest runs. gui-smoke stars on Tools and unstars on Home. Joint 9/10: interim Opus reviewer agreed (round 4) |
| C-files-016 | Unlock password-protected input | 9 | Same PasswordDialog as the reader (proven by smoke), probe_failures_are_routed_per_tool, jobs encrypted-input tests; joint review |
| C-files-019 | Unreadable file feedback | 9 | files::open_error one wording reader+tools, unit test, gui-smoke damaged PDF refused, app stays open; joint review (commit 43f1fff) |
| C-forms-001 | Fill form fields | 9 | Text fields edited on the field (↩ fills, Tab/⇧Tab to the next text field, check boxes untouched), check boxes, radios, combo lists; values saved and read back with qpdf (dev/re/evidence/F-004/01–04) |
| C-forms-002 | Clear a form | 9 | Clear form resets every field, undoable (dev/re/evidence/F-004/05) |
| C-forms-005 | Highlight fillable fields | 9 | Fillable fields tinted light blue in the reader, as PDF Expert/Acrobat (dev/re/evidence/F-004/01) |
| C-forms-017 | Flatten form fields | 9 | Flatten bakes field values into the page: 0 fields left, 'Jane Doe' in the page text (pdftotext) |
| C-optimize-002 | See compression savings | 9 | state::compress_note test; joint review (commit 43f1fff) |
| C-optimize-008 | OCR multiple files | 9 | OCR takes several files, per-file progress and cancel (jobs.rs each()); Joint 9/10: toolbox audit (interim reviewer) against iLovePDF's QML, agreed |
| C-organize-002 | Reorder pages by dragging thumbnails | 9 | Drag thumbnails (multi-select) with a drop line, a page-count badge, auto-scroll at the strip's edges; one undo step; gui-smoke reorders and checks the saved order. Joint 9/10: interim reviewer agreed in round 3 (screenshots 02b, 02e, 03b) |
| C-organize-003 | Rotate a page left or right | 9 | Rotate left/right from the strip on the selection or the page shown; thumbnail and page follow (dev/re/evidence/F-003/01) |
| C-organize-007 | Delete pages | 9 | Delete the selected pages (strip, refused for every page), undoable; 6 → 5 pages checked with qpdf |
| C-organize-009 | Insert a blank page | 9 | Insert a blank page of the same size after the selection; 4 → 5 pages checked with qpdf |
| C-organize-013 | Copy or duplicate pages | 9 | Duplicate the selected pages after them; page count checked with qpdf |
| C-organize-014 | Extract pages to a new PDF | 9 | Extract the selection to a new PDF named 'name - page 3.pdf' in a Save dialog; it opens in its own tab (dev/re/evidence/F-003/03, dev/re/evidence/F-003/04) |
| C-organize-023 | Name the merged output file | 9 | dest::tests::typed_output_names_are_checked; inline error + disabled action (options.slint); joint 9 with interim reviewer 22/10 (round 1) |
| C-organize-027 | Undo and redo page edits | 9 | Page edits (duplicate, delete, rotate, move) are undo steps in the reader's Pages panel; gui-smoke: undo brings the deleted page back, undo again removes the duplicate |
| C-output-001 | Save the document | 9 | ⌘S incremental save 1.1 s on 70 MB, Saved toast, save button state, unsaved prompt. Joint 9/10: independent visual audit (interim reviewer) against PDF Expert / iLovePDF captures, from gui-smoke screenshots, agreed |
| C-output-005 | Results screen | 9 | Result screen at parity with iLovePDF's: output card (name, size, pages, '-17 %'), Continue to… list, size before → after, restart, Open folder; dev/re/evidence/F-001/sheaf-compress-result.png beside dev/re/evidence/ilovepdf/05c-compress-result.png |
| C-output-006 | Show the output in its folder | 9 | Open folder on the result screen and per-output file/folder buttons, as iLovePDF; dev/re/evidence/F-001/sheaf-compress-result.png beside dev/re/evidence/ilovepdf/05c-compress-result.png |
| C-platform-009 | Command-line interface | 9 | Help on every option and value (args::help_tests every_option_and_value_has_help, page_number_help_reads_as_written); {page}/{pages} placeholders; joint review |
| C-platform-013 | Work offline without account | 9 | Stale-grade audit 22/10 (independent reviewer): no network library at run time (reqwest only a Windows build dep of winprint); outgoing URLs only opened by the user (main.rs, reader_ui/pointer.rs) |
| C-preferences-001 | Open preferences | 9 | ⌘, from anywhere (Home, tool tabs, documents, their fields) opens Preferences, leaving full screen and a presentation; Home sidebar entry. gui-smoke: from a tool tab and a document. Joint 9/10: independent Opus reviewer (interim for Sol, see DECISIONS) agreed in round 3 |
| C-preferences-002 | Choose theme and colours | 9 | gui-smoke Dark then Light switch the window; joint review (commit 43f1fff) |
| C-preferences-008 | Set author name | 9 | Preferences ▸ Annotations ▸ Author name (placeholder: the login name, used when empty, as Acrobat/PDF Expert); gui-smoke: typed name kept in prefs.json, default author written into a saved note |
| C-preferences-009 | Choose default output location | 9 | Preferences screenshot beside iLovePDF 07 (cleaner, more choices); dest.rs test; smoke Compress next to original; joint review |
| C-reader-001 | Open a PDF in the reader | 9 | PDFs open in document tabs from Open, drop, recents and the system; pages in 1.3 s on 1,982 pages; password, unsaved prompt. Joint 9/10: independent visual audit (interim reviewer) against PDF Expert / iLovePDF captures, from gui-smoke screenshots, agreed |
| C-reader-002 | Continuous vertical scroll | 9 | Continuous scroll; keyboard: arrows (row by row with two pages), Space / ⇧Space, PgUp/PgDn, Home/End (gui-smoke reads the page in view after each); 1,982-page book scrolls with renders on demand |
| C-reader-005 | Zoom in and out | 9 | Zoom 25–400 % in actual-size steps like PDF Expert, ⌘+/⌘−, ⌘-wheel, toolbar (dev/re/evidence/F-002/26); pages re-render 150 ms after the zoom settles |
| C-reader-006 | Set an exact zoom percentage | 9 | Zoom field in the toolbar: type a percentage (↩), or pick 50–400 %, Fit width, Fit page from its list, as PDF Expert's zoom menu; actual-size percentages (reader::tests::zoom_typed_by_the_user; gui-smoke types 200 then picks Fit page) |
| C-reader-007 | Fit whole page to window | 9 | ⌘8 / toolbar fit page; kept when the window is resized (80 % → 63 %, dev/re/evidence/F-002/38) |
| C-reader-008 | Fit page width | 9 | ⌘9 / toolbar fit width (168 %), kept when the window or panel changes size (dev/re/evidence/F-002/26, dev/re/evidence/F-002/28) |
| C-reader-009 | Zoom to actual size | 9 | ⌘0 zoom to 100 % actual size, PDF Expert's shortcut (dev/re/evidence/F-002/26) |
| C-reader-013 | Default zoom and remembered position | 9 | Reopens on the page, zoom (a chosen zoom; Fit width/page refit) and two-page mode it was left in, as PDF Expert (gui-smoke: closed in two pages, reopens in two pages); fixed: small files lost the page (document thread answered first) |
| C-reader-015 | Two-page spread view | 9 | Toolbar: Two pages — pages in pairs, the page in view kept in view, Fit width / Fit page per page, hit testing, edits and search unchanged (viewer::tests two_pages_side_by_side_and_the_cover_alone; gui-smoke samples the gap between the pages). On the 1,982-page book: instant |
| C-reader-016 | Cover page alone in spreads | 9 | Two pages with the cover alone in the right column, then pairs (Acrobat's and PDF Expert's book mode); unit-tested layout, gui-smoke cycles through it |
| C-reader-018 | Next and previous page | 9 | ← → page, PgUp/PgDn/space screen, scroll by arrows (GUI 21/10) |
| C-reader-019 | First and last page | 9 | Home/End and ⌘↑/⌘↓ to first/last page (End → 1982/1982, dev/re/evidence/F-002/25) |
| C-reader-020 | Go to a page by number | 9 | Page field: number or label ↵ jumps (420), field returns to the page shown; history recorded (dev/re/evidence/F-002/22) |
| C-reader-021 | Back and forward in view history | 9 | ⌘[ / ⌘] after a page jump, link or outline: 27 → 420 → 27 → 420 (dev/re/evidence/F-002/22) |
| C-reader-022 | Show or hide the side panel | 9 | Panel button and ⌥⌘0 hide/show the side panel (dev/re/evidence/F-002/28) |
| C-reader-023 | Page thumbnails panel | 9 | Pages tab: 150 px thumbnails like PDF Expert's, page number, current ringed, click jumps, lazy renders (dev/re/evidence/F-002/11, dev/re/evidence/F-002/18) |
| C-reader-025 | Browse the document outline | 9 | Outline tree with chevrons, page labels, current entry; add, rename, delete bookmarks. Joint 9/10: independent visual audit (interim reviewer) against PDF Expert / iLovePDF captures, from gui-smoke screenshots, agreed (vs pdfexpert/04a) |
| C-reader-026 | Add and edit outline entries | 9 | Outline panel: + Bookmark (the page in view, in page order), rename in place (double-click or pencil; ↩ / Escape), delete (with what is under it); PdfCraft Add/Rename/DeleteBookmark (reader_doc::tests::bookmarks_are_added_renamed_and_deleted; gui-smoke adds and renames one) |
| C-reader-027 | Bookmark pages for yourself | 9 | Bookmark the page in view with + Bookmark; stored in the PDF's outline, so other readers see it; Outline panel: + Bookmark (the page in view, in page order), rename in place (double-click or pencil; ↩ / Escape), delete (with what is under it); PdfCraft Add/Rename/DeleteBookmark (reader_doc::tests::bookmarks_are_added_renamed_and_deleted; gui-smoke adds and renames one) |
| C-reader-028 | Highlights and notes sidebar | 9 | Same Notes panel (C-annotate-030): highlights, notes, ink, signatures with author and date; click jumps |
| C-reader-030 | View and save attachments | 9 | Files panel (paperclip): every attached file (document and page level) with its size; a click saves it under its name (reader_doc::tests::a_file_is_attached_listed_and_saved_back reads the bytes back). Save dialog not automatable here (xdg portal on another display) |
| C-reader-031 | Add an attachment | 9 | Files panel ▸ Attach…: a file attached to the page in view (Acrobat's Attach File comment), listed at once; unit-tested |
| C-reader-032 | Find text in the document | 9 | Search on the 1,982-page book (reader_doc::tests::search_cost_on_a_big_file): results stream from the first pages within 0.5 s; the whole book in 12.6 s on 2 vCPUs (was 15.8 s; one reader per core, up to 4); every later search 1.5 ms (texts kept). gui-smoke runs a search |
| C-reader-033 | Next and previous search match | 9 | Enter / ⇧Enter / ⌘G / ⇧⌘G step through matches: 1/9 → 2/9 → 3/9 → 2/9 → 3/9 (dev/re/evidence/F-002/31) |
| C-reader-034 | Search options | 9 | Aa / Whole words toggles re-run the search (dev/re/evidence/F-002/12) |
| C-reader-035 | Search results list | 9 | Results panel: page, context with the match in bold, wraps, click jumps even while the search streams (dev/re/evidence/F-002/20, dev/re/evidence/F-002/21) |
| C-reader-036 | Search for the selected text | 9 | Selection bar Search and ⇧⌘E: the selection searched, results listed; shows the toolbar again if hidden; bar kept inside the view. Joint 9/10: Claude built it, Sol agreed after its second review round (gui-smoke: page 4's text searched from page 1) |
| C-reader-039 | Hide toolbars for reading | 9 | ⇧T or More ▸ Hide toolbar: toolbar and panel hidden for reading, kept hidden through full screen; Escape restores. Joint 9/10: Sol agreed (round 5); gui-smoke checks hide, restore, and full screen keeping it hidden |
| C-reader-040 | Full screen mode | 9 | ⌃⌘F and More ▸ Full screen; Escape leaves it and keeps a hidden toolbar hidden. Joint 9/10: Sol agreed (round 4). Window state itself not checkable without a window manager here |
| C-reader-041 | Presentation slideshow | 9 | Presentation (⇧⌘P, More menu): full screen, no toolbar, one page at a time fitted; Space, arrows and Page keys turn whole pages; Escape ends and restores the view and two-page mode. Joint 9/10: Sol agreed (round 7); gui-smoke: Space turns the page while presenting |
| C-reader-043 | Dark and sepia reading themes | 9 | Day / Sepia / Night on pages only, remembered, Night keeps hues (yellow highlight, green headings) (dev/re/evidence/F-002/08, dev/re/evidence/F-002/18); test themes_map_white_* |
| C-reader-046 | Tabs and windows | 9 | Each PDF opens in its own tab next to tool tabs, title = file name, × closes (dev/re/evidence/F-002/12, dev/re/evidence/F-002/17) |
| C-reader-047 | Switch between open documents | 9 | ⇧⌘[ / ⇧⌘] previous / next tab (PDF Expert's shortcut), wraps around |
| C-reader-048 | Select and copy text | 9 | Drag selects in reading order (also over a highlight), double-click word, ⌘A page, ⌘C → clipboard verified with xclip (dev/re/evidence/F-002/03, dev/re/evidence/F-002/19); test a_drag_selects_* |
| C-reader-049 | Select all text | 9 | ⌘A selects the page's text (PDF Expert: same), ⌘C copies; gui-smoke checks the clipboard |
| C-reader-051 | Undo the last action | 9 | ⌘Z / toolbar undo for every reader edit (markup, notes, pages, forms); gui-smoke undo/redo checks; 0.19 s on 1,982 pages |
| C-reader-052 | View document properties | 9 | ⌘D or the info button: Document properties (Acrobat's sheet) — title, author, subject, keywords editable; pages, page size, file size, PDF version, creator, producer, security, tagged, fonts shown; Return saves, Escape cancels; gui-smoke saves a title |
| C-reader-057 | Follow links | 9 | Internal and web links followed, ⌘[ ⌘] history; hovering shows where a link goes, cleared on scroll and kept in view. Joint 9/10: Sol agreed in its third review round (gui-smoke: tooltip shown, then gone after scrolling) |
| C-reader-058 | Highlight links and form fields | 9 | More ▸ Highlight links (⇧⌘L) tints every link; fillable fields tinted in Select mode, as PDF Expert. Joint 9/10: Sol agreed (round 4); gui-smoke samples the tint |
| C-reader-061 | Page labels | 9 | Page field shows the page label (iv) with '(4 / 12)' beside it, as Acrobat; a typed label goes to its page before a page number (gui-smoke: '1' in a book labelled i, ii, 1, 2 goes to the third page); labels in outline, notes and results rows |
| C-reader-063 | Unlock a protected PDF to view | 9 | Prompt + wrong password (red border, message) screenshots; smoke: wrong then right password, Escape skips; joint review |
| C-security-002 | Protect a PDF with a password | 9 | Protect: strength meter (weak/fair/strong from length and character classes, unit-tested) and show/hide on every password field. Joint 9/10: Sol built it and set on-screen validation as its condition; Claude reviewed, validated on screen and fixed (eye-off icon was a bare slash) |
| C-security-004 | Set permissions restrictions | 9 | Protect: allow printing, copying, editing, commenting, filling forms; qpdf-checked per permission. Joint 9/10: Sol built it and set on-screen validation as its condition; Claude reviewed, validated on screen and fixed: with restrictions and no permissions password a random one is set, so the open password no longer grants every right (test inverted to check it) |
| C-security-005 | Open a password-protected PDF | 9 | A protected PDF asks for its password in the reader; a wrong one says so, the right one opens it (dev/re/evidence/F-002/35) |
| C-security-006 | Remove a password | 9 | Unlock: password field with reveal, wrong-password feedback, batch; Joint 9/10: toolbox audit (interim reviewer) against iLovePDF's QML, agreed |
| C-security-012 | Find text and redact | 9 | Match case / Whole words in the panel, engine and CLI, honoured by the clean-up and audit outside the page (redact::Needle); tests/redact.rs whole-words count, redact::tests scrub Ann; joint 9 round 3 (regex a Stirling extra) |
| C-security-017 | Edit document metadata | 9 | Metadata edited in Document properties (PdfCraft SetInfo, one undo step); gui-smoke checks /Title in the saved file |
| C-sign-001 | Open the sign tool | 9 | Sign tool (S) in the reader toolbar, PDF Expert's Fill & Sign place; gui-smoke.sh 'a drawn signature is placed and saved' PASS |
| C-sign-002 | Draw a signature | 9 | Pad with 3 inks and 3 widths shown while drawing, smoothed strokes; up to 4 kept signatures with their own pen, picked or forgotten from thumbnails in their ink; gui-smoke checks ink and width in the saved file. Joint 9/10: interim reviewer agreed in round 3 (screenshots 02b, 02e, 03b) |
| C-sign-003 | Type a signature | 9 | Four handwriting fonts (Dancing Script, Great Vibes, Satisfy, Caveat; sheaf-core/src/script.rs, Cyrillic/Greek fallback), live 2×2 previews on the pad, kept per signature, Sign tool and CLI --script-font; script::tests, tests/sign.rs a_typed_name_takes_its_handwriting_font + same ink visual/digital for every font, history compat test, gui-smoke typed signature in Great Vibes; Stirling offers no script fonts; joint review 2 rounds |
| C-sign-005 | Place a signature on the page | 9 | Select a placed signature, drag its corner handle: scales from its top-left with proportions kept (pull projected on the diagonal); several placements per run with the kept signature. gui-smoke checks the saved Ink Rect; dev/re/evidence/F-002/41-signature-resized.png |
| C-sign-008 | Delete a placed signature | 9 | Click a placed signature, Delete (or the Notes panel's delete): gone from the saved file (gui-smoke) |
| C-sign-009 | Save and manage signatures | 9 | The signature is kept in the preferences and placed again with one click on any document (gui-smoke 'kept signature' PASS); New signature replaces it |
| C-sign-010 | Add marks to fill a form | 9 | Marks to fill a form: checkmark, cross, dot, line (Stamp tool, M), plus typed text (T, E) and the pen; gui-smoke places a checkmark |
