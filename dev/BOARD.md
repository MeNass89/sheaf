# Parity board

Inspirations: Adobe Acrobat (Reader + Pro tools) (), PDF Expert (Readdle) (), iLovePDF Desktop (2.3.0), Stirling-PDF (), macOS Preview ()
331 components · at parity (≥9): 100 · open 231, closed 100
Rendered by `dev/scripts/board.py render` from `dev/board/components.json`. Do not edit by hand.

## Queue (next first)
| # | id | weight | component | main | F · A · P → overall | next |
|---|---|---|---|---|---|---|
| 1 | C-organize-014 | strong | Extract pages to a new PDF | pdfexpert | 7 · 9 · 9 → **7** | — |
| 2 | C-output-004 | strong | Run progress and cancel | ours | 8 · 7 · 8 → **7** | — |
| 3 | C-output-007 | strong | Print a document | pdfexpert | 8 · 7 · 9 → **7** | print via the OS |
| 4 | C-platform-011 | strong | Open files from the system | ours | 7 · 7 · 9 → **7** | registered as Alternate viewer on macOS |
| 5 | C-platform-012 | strong | Native desktop builds | ours | 7 · 7 · 9 → **7** | macOS notarisation (needs Nassim's Apple account) |
| 6 | C-security-015 | strong | Apply redactions | acrobat | 8 · 7 · 8 → **7** | preview of the redacted output |
| 7 | C-annotate-001 | strong | Show annotation toolbar | pdfexpert | 8 · 8 · 9 → **8** | — |
| 8 | C-annotate-007 | strong | Add text box | pdfexpert | 8 · 9 · 9 → **8** | — |
| 9 | C-annotate-022 | strong | Change annotation colour | pdfexpert | 8 · 9 · 9 → **8** | — |
| 10 | C-convert-002 | strong | Convert Office documents to PDF | ilovepdf | 8 · 9 · 9 → **8** | needs LibreOffice installed; no sheet/print-area options |
| 11 | C-convert-009 | strong | Convert PDF to Word | ilovepdf | 8 · 9 · 9 → **8** | — |
| 12 | C-convert-012 | strong | Export pages as images | ilovepdf | 8 · 9 · 9 → **8** | — |
| 13 | C-files-002 | strong | Drag and drop files to open | ilovepdf | 8 · 8 · 9 → **8** | drop on the reader opens the document |
| 14 | C-files-011 | strong | Close documents, tabs and windows | pdfexpert | 8 · 9 · 9 → **8** | — |
| 15 | C-files-015 | strong | Manage a tool's input files | ilovepdf | 8 · 9 · 9 → **8** | — |
| 16 | C-optimize-001 | strong | Compress a PDF (levels) | ilovepdf | 8 · 8 · 8 → **8** | seed bug: Save after Compress needs Enter; fonts/vector recompression |
| 17 | C-optimize-006 | strong | OCR a scanned PDF | ilovepdf | 9 · 9 · 8 → **8** | speed on big scans; deskew |
| 18 | C-organize-001 | strong | Open page organizer grid | pdfexpert | 8 · 8 · 9 → **8** | grid in the reader; multi-select; several files |
| 19 | C-output-002 | strong | Save as a new file | pdfexpert | 8 · 9 · 9 → **8** | seed bug: the Save panel's Save button ignores clicks, only Enter works |
| 20 | C-reader-010 | strong | Gesture zoom and pan | pdfexpert | 8 · 8 · 8 → **8** | pinch zoom and drag-pan (hand tool / space-drag) |
| 21 | C-reader-025 | strong | Browse the document outline | pdfexpert | 8 · 9 · 9 → **8** | — |
| 22 | C-reader-032 | strong | Find text in the document | pdfexpert | 9 · 9 · 8 → **8** | — |
| 23 | C-annotate-008 | normal | Add callout or speech bubble | acrobat | 0 · 0 · 0 → **0** | after core |
| 24 | C-compare-005 | normal | View documents side by side, synced | stirling | 0 · 0 · 0 → **0** | synced side-by-side view |
| 25 | C-convert-004 | normal | Create PDF from clipboard | preview | 0 · 0 · 0 → **0** | light |
| 26 | C-convert-006 | normal | Create PDF from scanner or screenshot | acrobat | 0 · 0 · 0 → **0** | light |
| 27 | C-convert-008 | normal | Create an empty document | pdfexpert | 0 · 0 · 0 → **0** | engine create_blank |
| 28 | C-convert-013 | normal | Extract embedded images | ilovepdf | 0 · 0 · 0 → **0** | after core |
| 29 | C-convert-014 | normal | Export PDF as text or RTF | acrobat | 0 · 0 · 0 → **0** | export text (engine text layer) |
| 30 | C-edit-008 | normal | Replace, rotate or edit an image | acrobat | 0 · 0 · 0 → **0** | engine EditPageImage |
| 31 | C-edit-012 | normal | Find and replace text | acrobat | 0 · 0 · 0 → **0** | engine ReplaceText |
| 32 | C-edit-015 | normal | Add header and footer | acrobat | 0 · 0 · 0 → **0** | engine has page marks; add header/footer tool |
| 33 | C-files-006 | normal | Star or pin favourite files | pdfexpert | 0 · 0 · 0 → **0** | light |
| 34 | C-files-007 | normal | Search, sort and select listed files | stirling | 0 · 0 · 0 → **0** | light |
| 35 | C-files-009 | normal | Cloud, network and shared files | acrobat | 0 · 0 · 0 → **0** | out of local-first scope (needs Nassim) |
| 36 | C-forms-003 | normal | Required field warnings | acrobat | 0 · 0 · 0 → **0** | after fill |
| 37 | C-forms-006 | normal | Enter form editing mode | acrobat | 0 · 0 · 0 → **0** | after core |
| 38 | C-forms-007 | normal | Auto-detect form fields | pdfexpert | 0 · 0 · 0 → **0** | after core |
| 39 | C-forms-008 | normal | Add a text field | acrobat | 0 · 0 · 0 → **0** | engine AddField |
| 40 | C-forms-009 | normal | Add choice fields | acrobat | 0 · 0 · 0 → **0** | engine AddField |
| 41 | C-forms-012 | normal | Edit field properties | acrobat | 0 · 0 · 0 → **0** | after core |
| 42 | C-forms-014 | normal | Move, duplicate and arrange fields | acrobat | 0 · 0 · 0 → **0** | after core |
| 43 | C-optimize-003 | normal | Compress to a target size | stirling | 0 · 0 · 0 → **0** | after core |
| 44 | C-optimize-010 | normal | Enhance scanned pages | acrobat | 0 · 0 · 0 → **0** | after core |
| 45 | C-organize-010 | normal | Insert pages from another file | pdfexpert | 0 · 0 · 0 → **0** | insert pages from another PDF |
| 46 | C-output-009 | normal | Share a document | pdfexpert | 0 · 0 · 0 → **0** | OS share (macOS only) |
| 47 | C-output-010 | normal | Email as attachment | acrobat | 0 · 0 · 0 → **0** | mailto with attachment |
| 48 | C-output-014 | normal | Revert, versions and undo history | preview | 0 · 0 · 0 → **0** | revert to saved |
| 49 | C-platform-001 | normal | Check for updates | ilovepdf | 0 · 0 · 0 → **0** | light; release channel decision is Nassim's |
| 50 | C-preferences-003 | normal | Choose interface language | ilovepdf | 0 · 0 · 0 → **0** | localisation (after core) |
| 51 | C-preferences-004 | normal | Set default view on open | acrobat | 0 · 0 · 0 → **0** | with reader view modes |
| 52 | C-reader-011 | normal | Zoom to a selected area | acrobat | 0 · 0 · 0 → **0** | after core |
| 53 | C-reader-014 | normal | Single page view | pdfexpert | 0 · 0 · 0 → **0** | page modes: single, continuous |
| 54 | C-reader-017 | normal | Rotate the view | acrobat | 0 · 0 · 0 → **0** | view rotation |
| 55 | C-reader-045 | normal | Split view | pdfexpert | 0 · 0 · 0 → **0** | after core |
| 56 | C-reader-050 | normal | Select or snapshot an area | pdfexpert | 0 · 0 · 0 → **0** | after core |
| 57 | C-security-016 | normal | Sanitize a document | acrobat | 0 · 0 · 0 → **0** | engine hidden_info; add sanitize |
| 58 | C-sign-006 | normal | Add initials | acrobat | 0 · 0 · 0 → **0** | initials |
| 59 | C-sign-007 | normal | Repeat a signature on several pages | ilovepdf | 0 · 0 · 0 → **0** | after core |
| 60 | C-sign-016 | normal | Validate signatures | acrobat | 0 · 0 · 0 → **0** | engine signatures panel |

## By domain
| domain | components | avg overall | at ≥9 |
|---|---|---|---|
| ai | 7 | 0.0 | 0 |
| annotate | 38 | 6.3 | 18 |
| batch | 9 | 2.6 | 1 |
| compare | 7 | 5.7 | 1 |
| convert | 20 | 3.6 | 1 |
| edit | 29 | 4.8 | 10 |
| files | 19 | 6.1 | 9 |
| forms | 17 | 2.1 | 4 |
| optimize | 15 | 3.5 | 2 |
| organize | 27 | 4.3 | 8 |
| output | 15 | 4.3 | 3 |
| platform | 14 | 3.5 | 2 |
| preferences | 14 | 2.6 | 2 |
| reader | 64 | 5.5 | 28 |
| security | 18 | 5.4 | 5 |
| sign | 18 | 4.3 | 6 |

## Closed
| id | component | overall | last proof |
|---|---|---|---|
| C-annotate-002 | Highlight selected text | 9 | Highlight: 1,982-page 70 MB book (reader_doc::tests::edit_cost_on_a_big_file): engine 0.19 s per edit (was 2.1 s), drawn at once provisionally; gui-smoke saves and checks the file |
| C-annotate-003 | Underline selected text | 9 | Underline: 1,982-page 70 MB book (reader_doc::tests::edit_cost_on_a_big_file): engine 0.19 s per edit (was 2.1 s), drawn at once provisionally; gui-smoke saves and checks the file |
| C-annotate-004 | Strike through selected text | 9 | Strikethrough: 1,982-page 70 MB book (reader_doc::tests::edit_cost_on_a_big_file): engine 0.19 s per edit (was 2.1 s), drawn at once provisionally; gui-smoke saves and checks the file |
| C-annotate-006 | Add sticky note | 9 | N, click, type: Note with popup; a click on it opens its text to read and edit (SetAnnotationContents), Notes panel follows (dev/re/evidence/F-002/06, dev/re/evidence/F-002/36) |
| C-annotate-010 | Draw freehand ink | 9 | Pen with 5 colours and 3 widths, strokes smoothed (Chaikin ×2) as PDF Expert's pen (dev/re/evidence/F-002/37) |
| C-annotate-011 | Erase ink strokes | 9 | Eraser (X, toolbar): drag across drawings, rubbed out along the path (PdfCraft EraseInk; a stroke split or removed), three sizes, one undo step, signatures left alone; gui-smoke erases a pen stroke entirely |
| C-annotate-012 | Draw rectangle or oval | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-annotate-016 | Place a stamp | 9 | Stamp tool (M): Acrobat's 22 stamps (standard, sign here, dynamic) from a list in the options bar, placed with a click, 160 × 42 pt; moved and resized like other comments; gui-smoke places Approved and a dated Received |
| C-annotate-018 | Add check, cross or symbol | 9 | Stamp tool list starts with Acrobat's Fill & Sign marks (✓ ✗ • —), placed with a click at 18 pt; gui-smoke places a checkmark |
| C-annotate-021 | Add link annotation | 9 | reviewer 24/10 on d613100: drag a box, page/URL/mailto target, saved /Link; reader_doc and gui-smoke 19 |
| C-annotate-024 | Edit annotation properties and defaults | 9 | Each tool keeps its colour, width, opacity between sessions; new tools start from defaults; the selection bar's highlight uses the highlighter's colour. Joint 9/10: Sol agreed in round 3 (gui-smoke: Shapes style in the preferences) |
| C-annotate-025 | Delete an annotation | 9 | Click selects (blue outline), Delete or the panel's trash removes, ⌘Z restores (dev/re/evidence/F-002/15) |
| C-annotate-029 | Remove all annotations | 9 | Notes panel trash: Remove all comments in one step, ⌘Z brings them back (gui-smoke checks the saved file both ways) |
| C-annotate-030 | List comments in a panel | 9 | Notes panel: kind, page, text, then 'author · date' as the file records them (PDF Expert's comment list); click jumps, delete button. Author/date row checked by screenshot (not automatable: reading text off the panel); author written into new annotations checked by gui-smoke |
| C-annotate-032 | Hide or show all comments | 9 | Notes panel Hide all / Show all: pages render without comments (form fields still drawn), hidden comments not selectable; gui-smoke samples a stamp hidden and shown |
| C-annotate-034 | Set comment review status | 9 | Selected comment in Notes: status list (No status, Accepted, Rejected, Cancelled, Completed) as Acrobat's Set Status; shown after the author and date; saved as a status reply (reader_doc::tests::a_review_status_shows_on_its_comment; gui-smoke saves Accepted) |
| C-annotate-035 | Export or summarise comments | 9 | Notes panel: Summary of comments (PDF) and Export comments to XFDF or FDF, as Acrobat (reader_doc::tests::comments_are_exported_and_imported, a_summary_of_comments_is_written). Dialogs not automatable here (portal) |
| C-annotate-036 | Import comments | 9 | Notes panel: Import comments from XFDF or FDF (one undo step; every page renders again); unit test exports from one copy and imports into another |
| C-batch-007 | Chain another tool from results | 9 | Continue to… on the result screen chains any tool with the outputs, as iLovePDF; dev/re/evidence/F-001/sheaf-compress-result.png beside dev/re/evidence/ilovepdf/05c-compress-result.png |
| C-compare-002 | Swap the compared files | 9 | Swap the compared files in one click; Joint 9/10: toolbox audit (interim reviewer) against iLovePDF's QML, agreed |
| C-convert-003 | Convert images to PDF | 9 | iLovePDF's options: Fit / A4 / US Letter, orientation and margin for paper, Merge all images into one PDF (off: a PDF next to each image); PNG JPEG WebP TIF/TIFF BMP GIF; images opened from outside open the tool; JPEGs embedded as they are, EXIF rotation applied. Tests: images (Letter landscape, tif+gif), run, dest, open_with, route_drop; gui-smoke 16. Reviewer (Opus) agreed 9/9/9. |
| C-edit-001 | Enter edit mode | 9 | reviewer 24/10 on d613100: edit group = Edit text, Add image, Link, Redact…, Sign (PDF Expert's strip); fresh captures; fits 1280 px |
| C-edit-002 | Edit existing text in place | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-edit-003 | Add a new text box | 9 | E on empty space adds text with the format row (Sans/Serif/Mono, bold, 9–24 pt, 5 ink colours); gui-smoke checks the text (pdftotext) and the font (pdffonts: Times-Bold) |
| C-edit-006 | Add an image to a page | 9 | Edit tool: select a page image, corners resize it live keeping proportions (the picture itself follows, one preview in flight), drag moves it live, bar rotates/flips/replaces/deletes, Delete key; Add image and ⌘V paste place it (80 % cap each way) selected; WebP/TIFF/BMP/GIF. gui-smoke 14 on the saved file: resize ratio, flip (red right), rotate right (red down), Delete takes only the selected of two, ⌘V 120×60 then Delete. Reviewer (Opus) agreed 9/9/9 (dev: /tmp/rv/edit006-live.png). |
| C-edit-007 | Move, resize and select page objects | 9 | reviewer 25/10 on 07c9d1c: multi-pick (⇧-click, marquee), drawings (PdfCraft patch) found by hover and picked by press, drag together, nudge, paragraph resize with matching box; gui-smoke 20/20b/20c |
| C-edit-011 | Undo and redo edits | 9 | Undo/redo ⌘Z ⇧⌘Z: 0.19 s on the 1,982-page book (was 2.0 s); gui-smoke: undo then redo restores a note, undo brings a deleted page back |
| C-edit-014 | Save edited PDF | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-edit-016 | Add page numbers | 9 | Font section (families in their own look, size pt, B/I/U), presets + RRGGBB with swatch, batch range clamped per file; tests/numbers.rs (font, underline, custom colour, clamp, redaction of a font name); 1,982-page book numbered in 12.9 s (was 83 s: O(n²) page re-listing in pdfcraft-edit, per-read cache sum in pdfcraft-cos); joint review round 3 |
| C-edit-021 | Add and edit links | 9 | 6edfb12: Sol AGREE 9 (round 2 25/10); F-013 round 2 |
| C-edit-026 | Insert a blank page while editing | 9 | Pages panel: Insert blank page after the selection; gui-smoke checks an empty page 2 in the saved file and that ⌘Z removes it |
| C-files-001 | Open a PDF from a file picker | 9 | Home ▸ Open file: system dialog (GTK portal on Linux) → the PDF opens in a reader tab (dev/re/evidence/F-002/29, dev/re/evidence/F-002/30) |
| C-files-003 | Home screen | 9 | Home as iLovePDF's: drop zone with Open file, Recent tools (latest runs' tools, wrapping), Recent files with thumbnails, remove and clear. Joint 9/10: independent Opus reviewer (interim for Sol, see DECISIONS) agreed in round 3 |
| C-files-004 | Reopen a recent file | 9 | Recents with thumbnails reopen a PDF in the reader at the page it was left on (777, dev/re/evidence/F-002/34) |
| C-files-005 | Manage the recent files list | 9 | Recents: × on hover removes one, Clear recents removes all, Tasks History untouched (hidden-recents.json; unit tests). Joint 9/10: Sol built it and set on-screen validation as its condition; Claude reviewed, validated on screen and fixed (removal checked on screen) |
| C-files-010 | Documents in tabs or windows | 9 | Tabs drag to reorder (drop line), right-click Close Other/Left/Right, 1px dividers as iLovePDF; only the bar order moves so tab pages keep options and scroll. gui-smoke 13: drag reorders, dragged document keeps Page 4, Close Other Tabs leaves one. Reviewer (Opus) agreed 9/9/9. |
| C-files-012 | Browse and open tools | 9 | Tools grid by category with colour tiles, collapsible, sort, search, favourites, recent tools. Joint 9/10: independent visual audit (interim reviewer) against PDF Expert / iLovePDF captures, from gui-smoke screenshots, agreed (vs ilovepdf/01c) |
| C-files-013 | Search tools | 9 | Tools search: ranked (name start, name, description word, letters in order: 'cmprs' finds Compress), results as a list, ↑↓ with the chosen row kept in view, ↩ opens, Escape clears; focused on Tools, + and ⌘F (sheaf_core tests; gui-smoke: 'pdf to', ↓, ↩ opens PDF to Word without clicking the field). Joint 9/10: independent Opus reviewer (interim for Sol, see DECISIONS) agreed in round 3 |
| C-files-014 | Favourite and recent tools | 9 | Star any tool (Tools and Home cards; prefs favourite_tools), Favourites first on Tools; Home's Recent tools from the latest runs. gui-smoke stars on Tools and unstars on Home. Joint 9/10: interim Opus reviewer agreed (round 4) |
| C-files-019 | Unreadable file feedback | 9 | files::open_error one wording reader+tools, unit test, gui-smoke damaged PDF refused, app stays open; joint review (commit 43f1fff) |
| C-forms-001 | Fill form fields | 9 | Text fields edited on the field (↩ fills, Tab/⇧Tab to the next text field, check boxes untouched), check boxes, radios, combo lists; values saved and read back with qpdf (dev/re/evidence/F-004/01–04) |
| C-forms-002 | Clear a form | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-forms-005 | Highlight fillable fields | 9 | 41b1756: Sol AGREE 9 (round 3 25/10); F-013 round 3 |
| C-forms-017 | Flatten form fields | 9 | Flatten bakes field values into the page: 0 fields left, 'Jane Doe' in the page text (pdftotext) |
| C-optimize-002 | See compression savings | 9 | state::compress_note test; joint review (commit 43f1fff) |
| C-optimize-008 | OCR multiple files | 9 | OCR takes several files, per-file progress and cancel (jobs.rs each()); Joint 9/10: toolbox audit (interim reviewer) against iLovePDF's QML, agreed |
| C-organize-002 | Reorder pages by dragging thumbnails | 9 | Drag thumbnails (multi-select) with a drop line, a page-count badge, auto-scroll at the strip's edges; one undo step; gui-smoke reorders and checks the saved order. Joint 9/10: interim reviewer agreed in round 3 (screenshots 02b, 02e, 03b) |
| C-organize-003 | Rotate a page left or right | 9 | Rotate left/right from the strip on the selection or the page shown; thumbnail and page follow (dev/re/evidence/F-003/01) |
| C-organize-007 | Delete pages | 9 | Delete the selected pages (strip, refused for every page), undoable; 6 → 5 pages checked with qpdf |
| C-organize-009 | Insert a blank page | 9 | Insert a blank page of the same size after the selection; 4 → 5 pages checked with qpdf |
| C-organize-015 | Split by page ranges or split points | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-organize-021 | Merge files into one PDF | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-organize-023 | Name the merged output file | 9 | dest::tests::typed_output_names_are_checked; inline error + disabled action (options.slint); joint 9 with interim reviewer 22/10 (round 1) |
| C-organize-027 | Undo and redo page edits | 9 | Page edits (duplicate, delete, rotate, move) are undo steps in the reader's Pages panel; gui-smoke: undo brings the deleted page back, undo again removes the duplicate |
| C-output-001 | Save the document | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-output-005 | Results screen | 9 | 6edfb12: Sol AGREE 9 (round 2 25/10); F-013 round 2 |
| C-output-006 | Show the output in its folder | 9 | Open folder on the result screen and per-output file/folder buttons, as iLovePDF; dev/re/evidence/F-001/sheaf-compress-result.png beside dev/re/evidence/ilovepdf/05c-compress-result.png |
| C-platform-009 | Command-line interface | 9 | Help on every option and value (args::help_tests every_option_and_value_has_help, page_number_help_reads_as_written); {page}/{pages} placeholders; joint review |
| C-platform-013 | Work offline without account | 9 | Stale-grade audit 22/10 (independent reviewer): no network library at run time (reqwest only a Windows build dep of winprint); outgoing URLs only opened by the user (main.rs, reader_ui/pointer.rs) |
| C-preferences-001 | Open preferences | 9 | ⌘, from anywhere (Home, tool tabs, documents, their fields) opens Preferences, leaving full screen and a presentation; Home sidebar entry. gui-smoke: from a tool tab and a document. Joint 9/10: independent Opus reviewer (interim for Sol, see DECISIONS) agreed in round 3 |
| C-preferences-009 | Choose default output location | 9 | Preferences screenshot beside iLovePDF 07 (cleaner, more choices); dest.rs test; smoke Compress next to original; joint review |
| C-reader-001 | Open a PDF in the reader | 9 | PDFs open in document tabs from Open, drop, recents and the system; pages in 1.3 s on 1,982 pages; password, unsaved prompt. Joint 9/10: independent visual audit (interim reviewer) against PDF Expert / iLovePDF captures, from gui-smoke screenshots, agreed |
| C-reader-002 | Continuous vertical scroll | 9 | Continuous scroll; keyboard: arrows (row by row with two pages), Space / ⇧Space, PgUp/PgDn, Home/End (gui-smoke reads the page in view after each); 1,982-page book scrolls with renders on demand |
| C-reader-005 | Zoom in and out | 9 | Zoom 25–400 % in actual-size steps like PDF Expert, ⌘+/⌘−, ⌘-wheel, toolbar (dev/re/evidence/F-002/26); pages re-render 150 ms after the zoom settles |
| C-reader-006 | Set an exact zoom percentage | 9 | Zoom field in the toolbar: type a percentage (↩), or pick 50–400 %, Fit width, Fit page from its list, as PDF Expert's zoom menu; actual-size percentages (reader::tests::zoom_typed_by_the_user; gui-smoke types 200 then picks Fit page) |
| C-reader-007 | Fit whole page to window | 9 | 6edfb12: Sol AGREE 9 (round 2 25/10); F-013 round 2 |
| C-reader-008 | Fit page width | 9 | ⌘9 / toolbar fit width (168 %), kept when the window or panel changes size (dev/re/evidence/F-002/26, dev/re/evidence/F-002/28) |
| C-reader-009 | Zoom to actual size | 9 | ⌘0 zoom to 100 % actual size, PDF Expert's shortcut (dev/re/evidence/F-002/26) |
| C-reader-016 | Cover page alone in spreads | 9 | Two pages with the cover alone in the right column, then pairs (Acrobat's and PDF Expert's book mode); unit-tested layout, gui-smoke cycles through it |
| C-reader-018 | Next and previous page | 9 | ← → page, PgUp/PgDn/space screen, scroll by arrows (GUI 21/10) |
| C-reader-019 | First and last page | 9 | Home/End and ⌘↑/⌘↓ to first/last page (End → 1982/1982, dev/re/evidence/F-002/25) |
| C-reader-020 | Go to a page by number | 9 | Page field: number or label ↵ jumps (420), field returns to the page shown; history recorded (dev/re/evidence/F-002/22) |
| C-reader-023 | Page thumbnails panel | 9 | Pages tab: 150 px thumbnails like PDF Expert's, page number, current ringed, click jumps, lazy renders (dev/re/evidence/F-002/11, dev/re/evidence/F-002/18) |
| C-reader-027 | Bookmark pages for yourself | 9 | Bookmark the page in view with + Bookmark; stored in the PDF's outline, so other readers see it; Outline panel: + Bookmark (the page in view, in page order), rename in place (double-click or pencil; ↩ / Escape), delete (with what is under it); PdfCraft Add/Rename/DeleteBookmark (reader_doc::tests::bookmarks_are_added_renamed_and_deleted; gui-smoke adds and renames one) |
| C-reader-031 | Add an attachment | 9 | Files panel ▸ Attach…: a file attached to the page in view (Acrobat's Attach File comment), listed at once; unit-tested |
| C-reader-033 | Next and previous search match | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-reader-034 | Search options | 9 | Aa / Whole words toggles re-run the search (dev/re/evidence/F-002/12) |
| C-reader-035 | Search results list | 9 | 41b1756: Sol AGREE 9 (round 3 25/10); F-013 round 3 |
| C-reader-036 | Search for the selected text | 9 | Selection bar Search and ⇧⌘E: the selection searched, results listed; shows the toolbar again if hidden; bar kept inside the view. Joint 9/10: Claude built it, Sol agreed after its second review round (gui-smoke: page 4's text searched from page 1) |
| C-reader-039 | Hide toolbars for reading | 9 | ⇧T or More ▸ Hide toolbar: toolbar and panel hidden for reading, kept hidden through full screen; Escape restores. Joint 9/10: Sol agreed (round 5); gui-smoke checks hide, restore, and full screen keeping it hidden |
| C-reader-040 | Full screen mode | 9 | ⌃⌘F and More ▸ Full screen; Escape leaves it and keeps a hidden toolbar hidden. Joint 9/10: Sol agreed (round 4). Window state itself not checkable without a window manager here |
| C-reader-041 | Presentation slideshow | 9 | Presentation (⇧⌘P, More menu): full screen, no toolbar, one page at a time fitted; Space, arrows and Page keys turn whole pages; Escape ends and restores the view and two-page mode. Joint 9/10: Sol agreed (round 7); gui-smoke: Space turns the page while presenting |
| C-reader-043 | Dark and sepia reading themes | 9 | Day / Sepia / Night on pages only, remembered, Night keeps hues (yellow highlight, green headings) (dev/re/evidence/F-002/08, dev/re/evidence/F-002/18); test themes_map_white_* |
| C-reader-047 | Switch between open documents | 9 | ⇧⌘[ / ⇧⌘] previous / next tab (PDF Expert's shortcut), wraps around |
| C-reader-048 | Select and copy text | 9 | Drag selects in reading order (also over a highlight), double-click word, ⌘A page, ⌘C → clipboard verified with xclip (dev/re/evidence/F-002/03, dev/re/evidence/F-002/19); test a_drag_selects_* |
| C-reader-049 | Select all text | 9 | ⌘A selects the page's text (PDF Expert: same), ⌘C copies; gui-smoke checks the clipboard |
| C-reader-051 | Undo the last action | 9 | ⌘Z / toolbar undo for every reader edit (markup, notes, pages, forms); gui-smoke undo/redo checks; 0.19 s on 1,982 pages |
| C-reader-057 | Follow links | 9 | Internal and web links followed, ⌘[ ⌘] history; hovering shows where a link goes, cleared on scroll and kept in view. Joint 9/10: Sol agreed in its third review round (gui-smoke: tooltip shown, then gone after scrolling) |
| C-reader-058 | Highlight links and form fields | 9 | More ▸ Highlight links (⇧⌘L) tints every link; fillable fields tinted in Select mode, as PDF Expert. Joint 9/10: Sol agreed (round 4); gui-smoke samples the tint |
| C-security-002 | Protect a PDF with a password | 9 | Protect: strength meter (weak/fair/strong from length and character classes, unit-tested) and show/hide on every password field. Joint 9/10: Sol built it and set on-screen validation as its condition; Claude reviewed, validated on screen and fixed (eye-off icon was a bare slash) |
| C-security-005 | Open a password-protected PDF | 9 | A protected PDF asks for its password in the reader; a wrong one says so, the right one opens it (dev/re/evidence/F-002/35) |
| C-security-006 | Remove a password | 9 | Unlock: password field with reveal, wrong-password feedback, batch; Joint 9/10: toolbox audit (interim reviewer) against iLovePDF's QML, agreed |
| C-security-010 | Mark an area or text for redaction | 9 | reviewer 24/10 on 42fcabb: live selection, caret ends, word/line clicks, result counts text passages; output scrubbed (fiche); gui-smoke 18 |
| C-security-012 | Find text and redact | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-sign-001 | Open the sign tool | 9 | Sign tool (S) in the reader toolbar, PDF Expert's Fill & Sign place; gui-smoke.sh 'a drawn signature is placed and saved' PASS |
| C-sign-002 | Draw a signature | 9 | Pad with 3 inks and 3 widths shown while drawing, smoothed strokes; up to 4 kept signatures with their own pen, picked or forgotten from thumbnails in their ink; gui-smoke checks ink and width in the saved file. Joint 9/10: interim reviewer agreed in round 3 (screenshots 02b, 02e, 03b) |
| C-sign-003 | Type a signature | 9 | Four handwriting fonts (Dancing Script, Great Vibes, Satisfy, Caveat; sheaf-core/src/script.rs, Cyrillic/Greek fallback), live 2×2 previews on the pad, kept per signature, Sign tool and CLI --script-font; script::tests, tests/sign.rs a_typed_name_takes_its_handwriting_font + same ink visual/digital for every font, history compat test, gui-smoke typed signature in Great Vibes; Stirling offers no script fonts; joint review 2 rounds |
| C-sign-005 | Place a signature on the page | 9 | Select a placed signature, drag its corner handle: scales from its top-left with proportions kept (pull projected on the diagonal); several placements per run with the kept signature. gui-smoke checks the saved Ink Rect; dev/re/evidence/F-002/41-signature-resized.png |
| C-sign-008 | Delete a placed signature | 9 | Click a placed signature, Delete (or the Notes panel's delete): gone from the saved file (gui-smoke) |
| C-sign-010 | Add marks to fill a form | 9 | 41b1756: Sol AGREE 9 (round 3 25/10); F-013 round 3 |
