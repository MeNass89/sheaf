# Parity board

Inspirations: Adobe Acrobat (Reader + Pro tools) (), PDF Expert (Readdle) (), iLovePDF Desktop (2.3.0), Stirling-PDF (), macOS Preview ()
331 components · at parity (≥9): 30 · open 302, closed 29
Rendered by `dev/scripts/board.py render` from `dev/board/components.json`. Do not edit by hand.

## Queue (next first)
| # | id | weight | component | main | F · A · P → overall | next |
|---|---|---|---|---|---|---|
| 1 | C-output-007 | strong | Print a document | pdfexpert | 0 · 0 · 0 → **0** | print via the OS |
| 2 | C-reader-010 | strong | Gesture zoom and pan | pdfexpert | 0 · 0 · 0 → **0** | pinch zoom and drag-pan (hand tool / space-drag) |
| 3 | C-edit-001 | strong | Enter edit mode | pdfexpert | 3 · 4 · 6 → **3** | edit mode inside the reader, not a separate tool flow |
| 4 | C-edit-003 | strong | Add a new text box | pdfexpert | 4 · 5 · 7 → **4** | multi-line text, fonts, undo |
| 5 | C-edit-007 | strong | Move, resize and select page objects | acrobat | 4 · 5 · 7 → **4** | resize handles, move existing page objects (engine MoveObjects) |
| 6 | C-edit-014 | strong | Save edited PDF | ilovepdf | 4 · 5 · 7 → **4** | ⌘S saves in place; Save dialog click bug (seed) |
| 7 | C-edit-002 | strong | Edit existing text in place | pdfexpert | 5 · 5 · 9 → **5** | engine EditTextBlock/EditTextLine wired to an in-place editor |
| 8 | C-edit-006 | strong | Add an image to a page | pdfexpert | 5 · 5 · 7 → **5** | free resize handles, more formats |
| 9 | C-files-010 | strong | Documents in tabs or windows | ilovepdf | 5 · 6 · 8 → **5** | document tabs |
| 10 | C-sign-001 | strong | Open the sign tool | pdfexpert | 5 · 6 · 8 → **5** | sign from the reader, not only a tool tab |
| 11 | C-sign-005 | strong | Place a signature on the page | pdfexpert | 5 · 6 · 8 → **5** | free resize; several placements per run |
| 12 | C-convert-002 | strong | Convert Office documents to PDF | ilovepdf | 6 · 6 · 6 → **6** | needs LibreOffice installed; no sheet/print-area options |
| 13 | C-convert-003 | strong | Convert images to PDF | ilovepdf | 6 · 7 · 8 → **6** | one PDF per image; Letter and other sizes |
| 14 | C-convert-009 | strong | Convert PDF to Word | ilovepdf | 6 · 6 · 7 → **6** | layout fidelity |
| 15 | C-convert-012 | strong | Export pages as images | ilovepdf | 6 · 7 · 8 → **6** | PNG/TIFF, DPI choice |
| 16 | C-optimize-006 | strong | OCR a scanned PDF | ilovepdf | 7 · 6 · 6 → **6** | speed on big scans; deskew |
| 17 | C-security-005 | strong | Open a password-protected PDF | pdfexpert | 6 · 6 · 9 → **6** | prompt in the reader |
| 18 | C-security-010 | strong | Mark an area or text for redaction | acrobat | 6 · 7 · 8 → **6** | mark by text selection |
| 19 | C-sign-002 | strong | Draw a signature | pdfexpert | 6 · 6 · 8 → **6** | pen colour and thickness |
| 20 | C-sign-003 | strong | Type a signature | stirling | 6 · 6 · 8 → **6** | several script fonts |
| 21 | C-annotate-006 | strong | Add sticky note | pdfexpert | 8 · 7 · 8 → **7** | sticky note with popup |
| 22 | C-annotate-010 | strong | Draw freehand ink | pdfexpert | 7 · 7 · 8 → **7** | ink tool |
| 23 | C-edit-016 | strong | Add page numbers | ilovepdf | 8 · 7 · 8 → **7** | font, size and colour in the app |
| 24 | C-files-002 | strong | Drag and drop files to open | ilovepdf | 7 · 7 · 8 → **7** | drop on the reader opens the document |
| 25 | C-files-003 | strong | Home screen | pdfexpert | 7 · 7 · 8 → **7** | recents with thumbnails; open-file drop zone |
| 26 | C-files-015 | strong | Manage a tool's input files | ilovepdf | 7 · 7 · 8 → **7** | drag reorder |
| 27 | C-organize-015 | strong | Split by page ranges or split points | ilovepdf | 8 · 7 · 8 → **7** | split points chosen visually |
| 28 | C-organize-021 | strong | Merge files into one PDF | ilovepdf | 7 · 7 · 8 → **7** | per-file page ranges |
| 29 | C-output-001 | strong | Save the document | pdfexpert | 9 · 8 · 7 → **7** | save in place from the reader |
| 30 | C-output-004 | strong | Run progress and cancel | ours | 8 · 7 · 8 → **7** | — |
| 31 | C-output-005 | strong | Results screen | ilovepdf | 8 · 7 · 8 → **7** | — |
| 32 | C-platform-011 | strong | Open files from the system | ours | 7 · 7 · 9 → **7** | registered as Alternate viewer on macOS |
| 33 | C-platform-012 | strong | Native desktop builds | ours | 7 · 7 · 9 → **7** | macOS notarisation (needs Nassim's Apple account) |
| 34 | C-security-002 | strong | Protect a PDF with a password | ilovepdf | 7 · 7 · 9 → **7** | strength meter, show-password toggle |
| 35 | C-security-006 | strong | Remove a password | ilovepdf | 8 · 7 · 9 → **7** | — |
| 36 | C-security-012 | strong | Find text and redact | acrobat | 7 · 7 · 8 → **7** | regex, whole words, custom patterns |
| 37 | C-security-015 | strong | Apply redactions | acrobat | 8 · 7 · 8 → **7** | preview of the redacted output |
| 38 | C-annotate-001 | strong | Show annotation toolbar | pdfexpert | 8 · 8 · 9 → **8** | annotate mode with a tool strip |
| 39 | C-annotate-002 | strong | Highlight selected text | pdfexpert | 9 · 9 · 8 → **8** | highlight via engine TextMarkup over PageText line rects |
| 40 | C-annotate-007 | strong | Add text box | pdfexpert | 8 · 8 · 9 → **8** | FreeText box |
| 41 | C-annotate-022 | strong | Change annotation colour | pdfexpert | 8 · 8 · 9 → **8** | recolour selected annotation |
| 42 | C-annotate-030 | strong | List comments in a panel | pdfexpert | 9 · 8 · 9 → **8** | annotations panel |
| 43 | C-edit-011 | strong | Undo and redo edits | pdfexpert | 9 · 9 · 8 → **8** | engine undo/redo |
| 44 | C-files-012 | strong | Browse and open tools | ilovepdf | 8 · 8 · 9 → **8** | — |
| 45 | C-optimize-001 | strong | Compress a PDF (levels) | ilovepdf | 8 · 8 · 8 → **8** | seed bug: Save after Compress needs Enter; fonts/vector recompression |
| 46 | C-organize-001 | strong | Open page organizer grid | pdfexpert | 8 · 8 · 9 → **8** | grid in the reader; multi-select; several files |
| 47 | C-organize-002 | strong | Reorder pages by dragging thumbnails | pdfexpert | 9 · 8 · 9 → **8** | drag-and-drop with multi-select |
| 48 | C-output-002 | strong | Save as a new file | pdfexpert | 8 · 9 · 9 → **8** | seed bug: the Save panel's Save button ignores clicks, only Enter works |
| 49 | C-reader-001 | strong | Open a PDF in the reader | pdfexpert | 8 · 8 · 8 → **8** | a Reader tab opened by Open, drop and recents; tools act on the open document |
| 50 | C-reader-002 | strong | Continuous vertical scroll | pdfexpert | 8 · 8 · 8 → **8** | standalone reader, keyboard scrolling (arrows, space, PgUp/PgDn) |
| 51 | C-reader-007 | strong | Fit whole page to window | pdfexpert | 8 · 9 · 9 → **8** | fit-page mode |
| 52 | C-reader-025 | strong | Browse the document outline | pdfexpert | 9 · 8 · 9 → **8** | outline panel from the PDF's bookmarks |
| 53 | C-reader-032 | strong | Find text in the document | pdfexpert | 9 · 9 · 8 → **8** | reader search: field, count, highlights, results panel (search exists only inside Redact) |
| 54 | C-annotate-008 | normal | Add callout or speech bubble | acrobat | 0 · 0 · 0 → **0** | after core |
| 55 | C-annotate-009 | normal | Change text annotation font | pdfexpert | 0 · 0 · 0 → **0** | font size, colour for text boxes |
| 56 | C-annotate-011 | normal | Erase ink strokes | pdfexpert | 0 · 0 · 0 → **0** | eraser via engine EraseInk |
| 57 | C-annotate-012 | normal | Draw rectangle or oval | pdfexpert | 0 · 0 · 0 → **0** | rectangle/oval |
| 58 | C-annotate-013 | normal | Draw line or arrow | pdfexpert | 0 · 0 · 0 → **0** | line/arrow |
| 59 | C-annotate-016 | normal | Place a stamp | pdfexpert | 0 · 0 · 0 → **0** | standard stamps |
| 60 | C-annotate-018 | normal | Add check, cross or symbol | pdfexpert | 0 · 0 · 0 → **0** | with forms |

## By domain
| domain | components | avg overall | at ≥9 |
|---|---|---|---|
| ai | 7 | 0.0 | 0 |
| annotate | 38 | 2.3 | 1 |
| batch | 9 | 2.3 | 0 |
| compare | 7 | 5.4 | 0 |
| convert | 20 | 3.3 | 0 |
| edit | 29 | 2.3 | 0 |
| files | 19 | 4.7 | 3 |
| forms | 17 | 2.1 | 4 |
| optimize | 15 | 2.9 | 0 |
| organize | 27 | 3.9 | 5 |
| output | 15 | 3.3 | 0 |
| platform | 14 | 3.4 | 1 |
| preferences | 14 | 1.8 | 0 |
| reader | 64 | 3.2 | 16 |
| security | 18 | 4.1 | 0 |
| sign | 18 | 2.4 | 0 |

## Closed
| id | component | overall | last proof |
|---|---|---|---|
| C-annotate-025 | Delete an annotation | 9 | Click selects (blue outline), Delete or the panel's trash removes, ⌘Z restores (dev/re/evidence/F-002/15) |
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
| C-reader-005 | Zoom in and out | 9 | Zoom 25–400 % in actual-size steps like PDF Expert, ⌘+/⌘−, ⌘-wheel, toolbar (dev/re/evidence/F-002/26); pages re-render 150 ms after the zoom settles |
| C-reader-008 | Fit page width | 9 | ⌘9 / toolbar fit width (168 %), kept when the window or panel changes size (dev/re/evidence/F-002/26, dev/re/evidence/F-002/28) |
| C-reader-009 | Zoom to actual size | 9 | ⌘0 zoom to 100 % actual size, PDF Expert's shortcut (dev/re/evidence/F-002/26) |
| C-reader-018 | Next and previous page | 9 | ← → page, PgUp/PgDn/space screen, scroll by arrows (GUI 21/10) |
| C-reader-019 | First and last page | 9 | Home/End and ⌘↑/⌘↓ to first/last page (End → 1982/1982, dev/re/evidence/F-002/25) |
| C-reader-020 | Go to a page by number | 9 | Page field: number or label ↵ jumps (420), field returns to the page shown; history recorded (dev/re/evidence/F-002/22) |
| C-reader-021 | Back and forward in view history | 9 | ⌘[ / ⌘] after a page jump, link or outline: 27 → 420 → 27 → 420 (dev/re/evidence/F-002/22) |
| C-reader-022 | Show or hide the side panel | 9 | Panel button and ⌥⌘0 hide/show the side panel (dev/re/evidence/F-002/28) |
| C-reader-023 | Page thumbnails panel | 9 | Pages tab: 150 px thumbnails like PDF Expert's, page number, current ringed, click jumps, lazy renders (dev/re/evidence/F-002/11, dev/re/evidence/F-002/18) |
| C-reader-033 | Next and previous search match | 9 | Enter / ⇧Enter / ⌘G / ⇧⌘G step through matches: 1/9 → 2/9 → 3/9 → 2/9 → 3/9 (dev/re/evidence/F-002/31) |
| C-reader-034 | Search options | 9 | Aa / Whole words toggles re-run the search (dev/re/evidence/F-002/12) |
| C-reader-035 | Search results list | 9 | Results panel: page, context with the match in bold, wraps, click jumps even while the search streams (dev/re/evidence/F-002/20, dev/re/evidence/F-002/21) |
| C-reader-043 | Dark and sepia reading themes | 9 | Day / Sepia / Night on pages only, remembered, Night keeps hues (yellow highlight, green headings) (dev/re/evidence/F-002/08, dev/re/evidence/F-002/18); test themes_map_white_* |
| C-reader-046 | Tabs and windows | 9 | Each PDF opens in its own tab next to tool tabs, title = file name, × closes (dev/re/evidence/F-002/12, dev/re/evidence/F-002/17) |
| C-reader-047 | Switch between open documents | 9 | ⇧⌘[ / ⇧⌘] previous / next tab (PDF Expert's shortcut), wraps around |
| C-reader-048 | Select and copy text | 9 | Drag selects in reading order (also over a highlight), double-click word, ⌘A page, ⌘C → clipboard verified with xclip (dev/re/evidence/F-002/03, dev/re/evidence/F-002/19); test a_drag_selects_* |
