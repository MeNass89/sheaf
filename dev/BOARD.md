# Parity board

Inspirations: Adobe Acrobat (Reader + Pro tools) (), PDF Expert (Readdle) (), iLovePDF Desktop (2.3.0), Stirling-PDF (), macOS Preview ()
331 components · at parity (≥9): 63 · open 269, closed 62
Rendered by `dev/scripts/board.py render` from `dev/board/components.json`. Do not edit by hand.

## Queue (next first)
| # | id | weight | component | main | F · A · P → overall | next |
|---|---|---|---|---|---|---|
| 1 | C-reader-010 | strong | Gesture zoom and pan | pdfexpert | 0 · 0 · 0 → **0** | pinch zoom and drag-pan (hand tool / space-drag) |
| 2 | C-edit-002 | strong | Edit existing text in place | pdfexpert | 5 · 6 · 9 → **5** | engine EditTextBlock/EditTextLine wired to an in-place editor |
| 3 | C-edit-006 | strong | Add an image to a page | pdfexpert | 5 · 5 · 7 → **5** | free resize handles, more formats |
| 4 | C-convert-002 | strong | Convert Office documents to PDF | ilovepdf | 6 · 6 · 6 → **6** | needs LibreOffice installed; no sheet/print-area options |
| 5 | C-convert-003 | strong | Convert images to PDF | ilovepdf | 6 · 7 · 8 → **6** | one PDF per image; Letter and other sizes |
| 6 | C-convert-009 | strong | Convert PDF to Word | ilovepdf | 6 · 6 · 7 → **6** | layout fidelity |
| 7 | C-convert-012 | strong | Export pages as images | ilovepdf | 6 · 7 · 8 → **6** | PNG/TIFF, DPI choice |
| 8 | C-optimize-006 | strong | OCR a scanned PDF | ilovepdf | 7 · 6 · 6 → **6** | speed on big scans; deskew |
| 9 | C-security-010 | strong | Mark an area or text for redaction | acrobat | 6 · 7 · 8 → **6** | mark by text selection |
| 10 | C-edit-001 | strong | Enter edit mode | pdfexpert | 7 · 8 · 9 → **7** | edit mode inside the reader, not a separate tool flow |
| 11 | C-edit-007 | strong | Move, resize and select page objects | acrobat | 7 · 8 · 9 → **7** | resize handles, move existing page objects (engine MoveObjects) |
| 12 | C-edit-016 | strong | Add page numbers | ilovepdf | 8 · 7 · 8 → **7** | font, size and colour in the app |
| 13 | C-files-003 | strong | Home screen | pdfexpert | 7 · 7 · 8 → **7** | recents with thumbnails; open-file drop zone |
| 14 | C-files-015 | strong | Manage a tool's input files | ilovepdf | 7 · 7 · 8 → **7** | drag reorder |
| 15 | C-organize-015 | strong | Split by page ranges or split points | ilovepdf | 8 · 7 · 8 → **7** | split points chosen visually |
| 16 | C-organize-021 | strong | Merge files into one PDF | ilovepdf | 7 · 7 · 8 → **7** | per-file page ranges |
| 17 | C-output-004 | strong | Run progress and cancel | ours | 8 · 7 · 8 → **7** | — |
| 18 | C-output-005 | strong | Results screen | ilovepdf | 8 · 7 · 8 → **7** | — |
| 19 | C-platform-011 | strong | Open files from the system | ours | 7 · 7 · 9 → **7** | registered as Alternate viewer on macOS |
| 20 | C-platform-012 | strong | Native desktop builds | ours | 7 · 7 · 9 → **7** | macOS notarisation (needs Nassim's Apple account) |
| 21 | C-security-002 | strong | Protect a PDF with a password | ilovepdf | 7 · 7 · 9 → **7** | strength meter, show-password toggle |
| 22 | C-security-006 | strong | Remove a password | ilovepdf | 8 · 7 · 9 → **7** | — |
| 23 | C-security-012 | strong | Find text and redact | acrobat | 7 · 7 · 8 → **7** | regex, whole words, custom patterns |
| 24 | C-security-015 | strong | Apply redactions | acrobat | 8 · 7 · 8 → **7** | preview of the redacted output |
| 25 | C-annotate-001 | strong | Show annotation toolbar | pdfexpert | 8 · 8 · 9 → **8** | annotate mode with a tool strip |
| 26 | C-annotate-007 | strong | Add text box | pdfexpert | 8 · 8 · 9 → **8** | FreeText box |
| 27 | C-files-002 | strong | Drag and drop files to open | ilovepdf | 8 · 8 · 9 → **8** | drop on the reader opens the document |
| 28 | C-files-010 | strong | Documents in tabs or windows | ilovepdf | 8 · 9 · 9 → **8** | document tabs |
| 29 | C-files-012 | strong | Browse and open tools | ilovepdf | 8 · 8 · 9 → **8** | — |
| 30 | C-optimize-001 | strong | Compress a PDF (levels) | ilovepdf | 8 · 8 · 8 → **8** | seed bug: Save after Compress needs Enter; fonts/vector recompression |
| 31 | C-organize-001 | strong | Open page organizer grid | pdfexpert | 8 · 8 · 9 → **8** | grid in the reader; multi-select; several files |
| 32 | C-organize-002 | strong | Reorder pages by dragging thumbnails | pdfexpert | 9 · 8 · 9 → **8** | drag-and-drop with multi-select |
| 33 | C-output-001 | strong | Save the document | pdfexpert | 9 · 8 · 9 → **8** | save in place from the reader |
| 34 | C-output-002 | strong | Save as a new file | pdfexpert | 8 · 9 · 9 → **8** | seed bug: the Save panel's Save button ignores clicks, only Enter works |
| 35 | C-output-007 | strong | Print a document | pdfexpert | 8 · 8 · 9 → **8** | print via the OS |
| 36 | C-reader-001 | strong | Open a PDF in the reader | pdfexpert | 8 · 8 · 8 → **8** | a Reader tab opened by Open, drop and recents; tools act on the open document |
| 37 | C-reader-025 | strong | Browse the document outline | pdfexpert | 9 · 8 · 9 → **8** | outline panel from the PDF's bookmarks |
| 38 | C-sign-002 | strong | Draw a signature | pdfexpert | 8 · 8 · 9 → **8** | pen colour and thickness |
| 39 | C-sign-003 | strong | Type a signature | stirling | 8 · 8 · 9 → **8** | several script fonts |
| 40 | C-annotate-008 | normal | Add callout or speech bubble | acrobat | 0 · 0 · 0 → **0** | after core |
| 41 | C-annotate-018 | normal | Add check, cross or symbol | pdfexpert | 0 · 0 · 0 → **0** | with forms |
| 42 | C-annotate-021 | normal | Add link annotation | preview | 0 · 0 · 0 → **0** | with edit links |
| 43 | C-annotate-031 | normal | Sort and filter comments | acrobat | 0 · 0 · 0 → **0** | filter by type/colour |
| 44 | C-annotate-033 | normal | Reply to a comment | acrobat | 0 · 0 · 0 → **0** | engine ReplyToAnnotation |
| 45 | C-compare-005 | normal | View documents side by side, synced | stirling | 0 · 0 · 0 → **0** | synced side-by-side view |
| 46 | C-convert-004 | normal | Create PDF from clipboard | preview | 0 · 0 · 0 → **0** | light |
| 47 | C-convert-006 | normal | Create PDF from scanner or screenshot | acrobat | 0 · 0 · 0 → **0** | light |
| 48 | C-convert-008 | normal | Create an empty document | pdfexpert | 0 · 0 · 0 → **0** | engine create_blank |
| 49 | C-convert-013 | normal | Extract embedded images | ilovepdf | 0 · 0 · 0 → **0** | after core |
| 50 | C-convert-014 | normal | Export PDF as text or RTF | acrobat | 0 · 0 · 0 → **0** | export text (engine text layer) |
| 51 | C-edit-008 | normal | Replace, rotate or edit an image | acrobat | 0 · 0 · 0 → **0** | engine EditPageImage |
| 52 | C-edit-012 | normal | Find and replace text | acrobat | 0 · 0 · 0 → **0** | engine ReplaceText |
| 53 | C-edit-015 | normal | Add header and footer | acrobat | 0 · 0 · 0 → **0** | engine has page marks; add header/footer tool |
| 54 | C-edit-021 | normal | Add and edit links | acrobat | 0 · 0 · 0 → **0** | engine links |
| 55 | C-edit-023 | normal | Edit bookmarks and table of contents | acrobat | 0 · 0 · 0 → **0** | with outline editing (reader) |
| 56 | C-files-006 | normal | Star or pin favourite files | pdfexpert | 0 · 0 · 0 → **0** | light |
| 57 | C-files-007 | normal | Search, sort and select listed files | stirling | 0 · 0 · 0 → **0** | light |
| 58 | C-files-009 | normal | Cloud, network and shared files | acrobat | 0 · 0 · 0 → **0** | out of local-first scope (needs Nassim) |
| 59 | C-files-014 | normal | Favourite and recent tools | ilovepdf | 0 · 0 · 0 → **0** | light |
| 60 | C-forms-003 | normal | Required field warnings | acrobat | 0 · 0 · 0 → **0** | after fill |

## By domain
| domain | components | avg overall | at ≥9 |
|---|---|---|---|
| ai | 7 | 0.0 | 0 |
| annotate | 38 | 4.8 | 14 |
| batch | 9 | 2.3 | 0 |
| compare | 7 | 5.4 | 0 |
| convert | 20 | 3.3 | 0 |
| edit | 29 | 3.4 | 4 |
| files | 19 | 4.9 | 3 |
| forms | 17 | 2.1 | 4 |
| optimize | 15 | 2.9 | 0 |
| organize | 27 | 4.2 | 6 |
| output | 15 | 3.9 | 0 |
| platform | 14 | 3.4 | 1 |
| preferences | 14 | 2.4 | 1 |
| reader | 64 | 4.3 | 25 |
| security | 18 | 4.3 | 1 |
| sign | 18 | 3.8 | 4 |

## Closed
| id | component | overall | last proof |
|---|---|---|---|
| C-annotate-002 | Highlight selected text | 9 | Highlight: 1,982-page 70 MB book (reader_doc::tests::edit_cost_on_a_big_file): engine 0.19 s per edit (was 2.1 s), drawn at once provisionally; gui-smoke saves and checks the file |
| C-annotate-003 | Underline selected text | 9 | Underline: 1,982-page 70 MB book (reader_doc::tests::edit_cost_on_a_big_file): engine 0.19 s per edit (was 2.1 s), drawn at once provisionally; gui-smoke saves and checks the file |
| C-annotate-004 | Strike through selected text | 9 | Strikethrough: 1,982-page 70 MB book (reader_doc::tests::edit_cost_on_a_big_file): engine 0.19 s per edit (was 2.1 s), drawn at once provisionally; gui-smoke saves and checks the file |
| C-annotate-006 | Add sticky note | 9 | N, click, type: Note with popup; a click on it opens its text to read and edit (SetAnnotationContents), Notes panel follows (dev/re/evidence/F-002/06, dev/re/evidence/F-002/36) |
| C-annotate-010 | Draw freehand ink | 9 | Pen with 5 colours and 3 widths, strokes smoothed (Chaikin ×2) as PDF Expert's pen (dev/re/evidence/F-002/37) |
| C-annotate-011 | Erase ink strokes | 9 | Eraser (X, toolbar): drag across drawings, rubbed out along the path (PdfCraft EraseInk; a stroke split or removed), three sizes, one undo step, signatures left alone; gui-smoke erases a pen stroke entirely |
| C-annotate-012 | Draw rectangle or oval | 9 | Shapes tool (R): drag a rectangle or oval with a live outline; colour, width, fill, opacity; moved by drag, resized by the corner handle; gui-smoke saves a rectangle and a filled oval at 75 % |
| C-annotate-016 | Place a stamp | 9 | Stamp tool (M): Acrobat's 22 stamps (standard, sign here, dynamic) from a list in the options bar, placed with a click, 160 × 42 pt; moved and resized like other comments; gui-smoke places Approved and a dated Received |
| C-annotate-019 | Add date stamp | 9 | Dated stamps (Approved, Confidential, Received, Reviewed, Revised · DATED): 'By <author> at 11:32 am, Oct 21, 2026' in the stamp and /PCByLine (gui-smoke checks it) |
| C-annotate-022 | Change annotation colour | 9 | Selected annotation recoloured from the colour row, whose dot shows its current colour (dev/re/evidence/F-002/14) |
| C-annotate-025 | Delete an annotation | 9 | Click selects (blue outline), Delete or the panel's trash removes, ⌘Z restores (dev/re/evidence/F-002/15) |
| C-annotate-029 | Remove all annotations | 9 | Notes panel trash: Remove all comments in one step, ⌘Z brings them back (gui-smoke checks the saved file both ways) |
| C-annotate-030 | List comments in a panel | 9 | Notes panel: kind, page, text, then 'author · date' as the file records them (PDF Expert's comment list); click jumps, delete button. Author/date row checked by screenshot (not automatable: reading text off the panel); author written into new annotations checked by gui-smoke |
| C-annotate-032 | Hide or show all comments | 9 | Notes panel Hide all / Show all: pages render without comments (form fields still drawn), hidden comments not selectable; gui-smoke samples a stamp hidden and shown |
| C-edit-003 | Add a new text box | 9 | E on empty space adds text with the format row (Sans/Serif/Mono, bold, 9–24 pt, 5 ink colours); gui-smoke checks the text (pdftotext) and the font (pdffonts: Times-Bold) |
| C-edit-011 | Undo and redo edits | 9 | Undo/redo ⌘Z ⇧⌘Z: 0.19 s on the 1,982-page book (was 2.0 s); gui-smoke: undo then redo restores a note, undo brings a deleted page back |
| C-edit-014 | Save edited PDF | 9 | ⌘S saves edits in place (incremental, validated by check_save), Save As copies; gui-smoke checks every saved file with qpdf/pdftotext |
| C-edit-026 | Insert a blank page while editing | 9 | Pages panel: Insert blank page after the selection; gui-smoke checks an empty page 2 in the saved file and that ⌘Z removes it |
| C-files-001 | Open a PDF from a file picker | 9 | Home ▸ Open file: system dialog (GTK portal on Linux) → the PDF opens in a reader tab (dev/re/evidence/F-002/29, dev/re/evidence/F-002/30) |
| C-files-004 | Reopen a recent file | 9 | Recents with thumbnails reopen a PDF in the reader at the page it was left on (777, dev/re/evidence/F-002/34) |
| C-files-011 | Close documents, tabs and windows | 9 | ⌘W closes the document tab; with unsaved edits Save / Don't Save / Cancel (dev/re/evidence/F-002/16, dev/re/evidence/F-002/17); window close asks for each dirty document |
| C-forms-001 | Fill form fields | 9 | Text fields edited on the field (↩ fills, Tab/⇧Tab to the next text field, check boxes untouched), check boxes, radios, combo lists; values saved and read back with qpdf (dev/re/evidence/F-004/01–04) |
| C-forms-002 | Clear a form | 9 | Clear form resets every field, undoable (dev/re/evidence/F-004/05) |
| C-forms-005 | Highlight fillable fields | 9 | Fillable fields tinted light blue in the reader, as PDF Expert/Acrobat (dev/re/evidence/F-004/01) |
| C-forms-017 | Flatten form fields | 9 | Flatten bakes field values into the page: 0 fields left, 'Jane Doe' in the page text (pdftotext) |
| C-organize-003 | Rotate a page left or right | 9 | Rotate left/right from the strip on the selection or the page shown; thumbnail and page follow (dev/re/evidence/F-003/01) |
| C-organize-007 | Delete pages | 9 | Delete the selected pages (strip, refused for every page), undoable; 6 → 5 pages checked with qpdf |
| C-organize-009 | Insert a blank page | 9 | Insert a blank page of the same size after the selection; 4 → 5 pages checked with qpdf |
| C-organize-013 | Copy or duplicate pages | 9 | Duplicate the selected pages after them; page count checked with qpdf |
| C-organize-014 | Extract pages to a new PDF | 9 | Extract the selection to a new PDF named 'name - page 3.pdf' in a Save dialog; it opens in its own tab (dev/re/evidence/F-003/03, dev/re/evidence/F-003/04) |
| C-organize-027 | Undo and redo page edits | 9 | Page edits (duplicate, delete, rotate, move) are undo steps in the reader's Pages panel; gui-smoke: undo brings the deleted page back, undo again removes the duplicate |
| C-preferences-008 | Set author name | 9 | Preferences ▸ Annotations ▸ Author name (placeholder: the login name, used when empty, as Acrobat/PDF Expert); gui-smoke: typed name kept in prefs.json, default author written into a saved note |
| C-reader-002 | Continuous vertical scroll | 9 | Continuous scroll; keyboard: arrows (row by row with two pages), Space / ⇧Space, PgUp/PgDn, Home/End (gui-smoke reads the page in view after each); 1,982-page book scrolls with renders on demand |
| C-reader-005 | Zoom in and out | 9 | Zoom 25–400 % in actual-size steps like PDF Expert, ⌘+/⌘−, ⌘-wheel, toolbar (dev/re/evidence/F-002/26); pages re-render 150 ms after the zoom settles |
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
| C-reader-028 | Highlights and notes sidebar | 9 | Same Notes panel (C-annotate-030): highlights, notes, ink, signatures with author and date; click jumps |
| C-reader-032 | Find text in the document | 9 | Search on the 1,982-page book (reader_doc::tests::search_cost_on_a_big_file): results stream from the first pages within 0.5 s; the whole book in 12.6 s on 2 vCPUs (was 15.8 s; one reader per core, up to 4); every later search 1.5 ms (texts kept). gui-smoke runs a search |
| C-reader-033 | Next and previous search match | 9 | Enter / ⇧Enter / ⌘G / ⇧⌘G step through matches: 1/9 → 2/9 → 3/9 → 2/9 → 3/9 (dev/re/evidence/F-002/31) |
| C-reader-034 | Search options | 9 | Aa / Whole words toggles re-run the search (dev/re/evidence/F-002/12) |
| C-reader-035 | Search results list | 9 | Results panel: page, context with the match in bold, wraps, click jumps even while the search streams (dev/re/evidence/F-002/20, dev/re/evidence/F-002/21) |
| C-reader-043 | Dark and sepia reading themes | 9 | Day / Sepia / Night on pages only, remembered, Night keeps hues (yellow highlight, green headings) (dev/re/evidence/F-002/08, dev/re/evidence/F-002/18); test themes_map_white_* |
| C-reader-046 | Tabs and windows | 9 | Each PDF opens in its own tab next to tool tabs, title = file name, × closes (dev/re/evidence/F-002/12, dev/re/evidence/F-002/17) |
| C-reader-047 | Switch between open documents | 9 | ⇧⌘[ / ⇧⌘] previous / next tab (PDF Expert's shortcut), wraps around |
| C-reader-048 | Select and copy text | 9 | Drag selects in reading order (also over a highlight), double-click word, ⌘A page, ⌘C → clipboard verified with xclip (dev/re/evidence/F-002/03, dev/re/evidence/F-002/19); test a_drag_selects_* |
| C-reader-049 | Select all text | 9 | ⌘A selects the page's text (PDF Expert: same), ⌘C copies; gui-smoke checks the clipboard |
| C-reader-051 | Undo the last action | 9 | ⌘Z / toolbar undo for every reader edit (markup, notes, pages, forms); gui-smoke undo/redo checks; 0.19 s on 1,982 pages |
| C-security-005 | Open a password-protected PDF | 9 | A protected PDF asks for its password in the reader; a wrong one says so, the right one opens it (dev/re/evidence/F-002/35) |
| C-sign-001 | Open the sign tool | 9 | Sign tool (S) in the reader toolbar, PDF Expert's Fill & Sign place; gui-smoke.sh 'a drawn signature is placed and saved' PASS |
| C-sign-005 | Place a signature on the page | 9 | Select a placed signature, drag its corner handle: scales from its top-left with proportions kept (pull projected on the diagonal); several placements per run with the kept signature. gui-smoke checks the saved Ink Rect; dev/re/evidence/F-002/41-signature-resized.png |
| C-sign-008 | Delete a placed signature | 9 | Click a placed signature, Delete (or the Notes panel's delete): gone from the saved file (gui-smoke) |
| C-sign-009 | Save and manage signatures | 9 | The signature is kept in the preferences and placed again with one click on any document (gui-smoke 'kept signature' PASS); New signature replaces it |
