# Parity board

Inspirations: Adobe Acrobat (Reader + Pro tools) (), PDF Expert (Readdle) (), iLovePDF Desktop (2.3.0), Stirling-PDF (), macOS Preview ()
331 components · at parity (≥9): 1 · open 331
Rendered by `dev/scripts/board.py render` from `dev/board/components.json`. Do not edit by hand.

## Queue (next first)
| # | id | weight | component | main | F · A · P → overall | next |
|---|---|---|---|---|---|---|
| 1 | C-annotate-001 | strong | Show annotation toolbar | pdfexpert | 0 · 0 · 0 → **0** | annotate mode with a tool strip |
| 2 | C-annotate-002 | strong | Highlight selected text | pdfexpert | 0 · 0 · 0 → **0** | highlight via engine TextMarkup over PageText line rects |
| 3 | C-annotate-006 | strong | Add sticky note | pdfexpert | 0 · 0 · 0 → **0** | sticky note with popup |
| 4 | C-annotate-007 | strong | Add text box | pdfexpert | 0 · 0 · 0 → **0** | FreeText box |
| 5 | C-annotate-010 | strong | Draw freehand ink | pdfexpert | 0 · 0 · 0 → **0** | ink tool |
| 6 | C-annotate-022 | strong | Change annotation colour | pdfexpert | 0 · 0 · 0 → **0** | recolour selected annotation |
| 7 | C-annotate-025 | strong | Delete an annotation | acrobat | 0 · 0 · 0 → **0** | select and delete annotations |
| 8 | C-annotate-030 | strong | List comments in a panel | pdfexpert | 0 · 0 · 0 → **0** | annotations panel |
| 9 | C-edit-002 | strong | Edit existing text in place | pdfexpert | 0 · 0 · 0 → **0** | engine EditTextBlock/EditTextLine wired to an in-place editor |
| 10 | C-edit-011 | strong | Undo and redo edits | pdfexpert | 0 · 0 · 0 → **0** | engine undo/redo |
| 11 | C-forms-001 | strong | Fill form fields | pdfexpert | 0 · 0 · 0 → **0** | fill AcroForm fields in the reader (engine SetFieldValue) |
| 12 | C-output-001 | strong | Save the document | pdfexpert | 0 · 0 · 0 → **0** | save in place from the reader |
| 13 | C-output-007 | strong | Print a document | pdfexpert | 0 · 0 · 0 → **0** | print via the OS |
| 14 | C-reader-007 | strong | Fit whole page to window | pdfexpert | 0 · 0 · 0 → **0** | fit-page mode |
| 15 | C-reader-010 | strong | Gesture zoom and pan | pdfexpert | 0 · 0 · 0 → **0** | pinch zoom and drag-pan (hand tool / space-drag) |
| 16 | C-reader-018 | strong | Next and previous page | pdfexpert | 0 · 0 · 0 → **0** | keyboard and toolbar paging |
| 17 | C-reader-020 | strong | Go to a page by number | pdfexpert | 0 · 0 · 0 → **0** | editable page field in the toolbar |
| 18 | C-reader-025 | strong | Browse the document outline | pdfexpert | 0 · 0 · 0 → **0** | outline panel from the PDF's bookmarks |
| 19 | C-reader-033 | strong | Next and previous search match | pdfexpert | 0 · 0 · 0 → **0** | next/previous match |
| 20 | C-reader-048 | strong | Select and copy text | pdfexpert | 0 · 0 · 0 → **0** | text selection layer from the engine's PageText |
| 21 | C-reader-001 | strong | Open a PDF in the reader | pdfexpert | 2 · 4 · 6 → **2** | a Reader tab opened by Open, drop and recents; tools act on the open document |
| 22 | C-edit-001 | strong | Enter edit mode | pdfexpert | 3 · 4 · 6 → **3** | edit mode inside the reader, not a separate tool flow |
| 23 | C-organize-002 | strong | Reorder pages by dragging thumbnails | pdfexpert | 3 · 5 · 7 → **3** | drag-and-drop with multi-select |
| 24 | C-organize-003 | strong | Rotate a page left or right | pdfexpert | 3 · 5 · 8 → **3** | per-page rotate left/right in the grid |
| 25 | C-reader-032 | strong | Find text in the document | pdfexpert | 3 · 4 · 6 → **3** | reader search: field, count, highlights, results panel (search exists only inside Redact) |
| 26 | C-edit-003 | strong | Add a new text box | pdfexpert | 4 · 5 · 7 → **4** | multi-line text, fonts, undo |
| 27 | C-edit-007 | strong | Move, resize and select page objects | acrobat | 4 · 5 · 7 → **4** | resize handles, move existing page objects (engine MoveObjects) |
| 28 | C-edit-014 | strong | Save edited PDF | ilovepdf | 4 · 5 · 7 → **4** | ⌘S saves in place; Save dialog click bug (seed) |
| 29 | C-output-002 | strong | Save as a new file | pdfexpert | 4 · 6 · 8 → **4** | seed bug: the Save panel's Save button ignores clicks, only Enter works |
| 30 | C-edit-006 | strong | Add an image to a page | pdfexpert | 5 · 5 · 7 → **5** | free resize handles, more formats |
| 31 | C-files-010 | strong | Documents in tabs or windows | ilovepdf | 5 · 6 · 8 → **5** | document tabs |
| 32 | C-organize-001 | strong | Open page organizer grid | pdfexpert | 5 · 6 · 7 → **5** | grid in the reader; multi-select; several files |
| 33 | C-reader-002 | strong | Continuous vertical scroll | pdfexpert | 5 · 6 · 7 → **5** | standalone reader, keyboard scrolling (arrows, space, PgUp/PgDn) |
| 34 | C-reader-005 | strong | Zoom in and out | pdfexpert | 5 · 6 · 7 → **5** | pinch/ctrl+wheel zoom centred on the pointer, range 10–800 % |
| 35 | C-reader-023 | strong | Page thumbnails panel | pdfexpert | 5 · 6 · 7 → **5** | thumbnails as a panel tab; current page follows scroll |
| 36 | C-sign-001 | strong | Open the sign tool | pdfexpert | 5 · 6 · 8 → **5** | sign from the reader, not only a tool tab |
| 37 | C-sign-005 | strong | Place a signature on the page | pdfexpert | 5 · 6 · 8 → **5** | free resize; several placements per run |
| 38 | C-convert-002 | strong | Convert Office documents to PDF | ilovepdf | 6 · 6 · 6 → **6** | needs LibreOffice installed; no sheet/print-area options |
| 39 | C-convert-003 | strong | Convert images to PDF | ilovepdf | 6 · 7 · 8 → **6** | one PDF per image; Letter and other sizes |
| 40 | C-convert-009 | strong | Convert PDF to Word | ilovepdf | 6 · 6 · 7 → **6** | layout fidelity |
| 41 | C-convert-012 | strong | Export pages as images | ilovepdf | 6 · 7 · 8 → **6** | PNG/TIFF, DPI choice |
| 42 | C-files-001 | strong | Open a PDF from a file picker | pdfexpert | 6 · 6 · 8 → **6** | open into the reader, not only into tools |
| 43 | C-files-004 | strong | Reopen a recent file | pdfexpert | 6 · 6 · 8 → **6** | reopen into the reader at the last page; Open Recent menu |
| 44 | C-files-011 | strong | Close documents, tabs and windows | pdfexpert | 6 · 6 · 8 → **6** | ⌘W, unsaved-changes prompt for documents |
| 45 | C-optimize-006 | strong | OCR a scanned PDF | ilovepdf | 7 · 6 · 6 → **6** | speed on big scans; deskew |
| 46 | C-organize-007 | strong | Delete pages | pdfexpert | 6 · 6 · 8 → **6** | pick pages visually in Remove |
| 47 | C-organize-014 | strong | Extract pages to a new PDF | pdfexpert | 7 · 6 · 8 → **6** | each page as its own file |
| 48 | C-reader-008 | strong | Fit page width | pdfexpert | 7 · 6 · 8 → **6** | keep fit width on window resize; shortcut |
| 49 | C-security-005 | strong | Open a password-protected PDF | pdfexpert | 6 · 6 · 9 → **6** | prompt in the reader |
| 50 | C-security-010 | strong | Mark an area or text for redaction | acrobat | 6 · 7 · 8 → **6** | mark by text selection |
| 51 | C-sign-002 | strong | Draw a signature | pdfexpert | 6 · 6 · 8 → **6** | pen colour and thickness |
| 52 | C-sign-003 | strong | Type a signature | stirling | 6 · 6 · 8 → **6** | several script fonts |
| 53 | C-edit-016 | strong | Add page numbers | ilovepdf | 8 · 7 · 8 → **7** | font, size and colour in the app |
| 54 | C-files-002 | strong | Drag and drop files to open | ilovepdf | 7 · 7 · 8 → **7** | drop on the reader opens the document |
| 55 | C-files-003 | strong | Home screen | pdfexpert | 7 · 7 · 8 → **7** | recents with thumbnails; open-file drop zone |
| 56 | C-files-015 | strong | Manage a tool's input files | ilovepdf | 7 · 7 · 8 → **7** | drag reorder |
| 57 | C-optimize-001 | strong | Compress a PDF (levels) | ilovepdf | 7 · 7 · 8 → **7** | seed bug: Save after Compress needs Enter; fonts/vector recompression |
| 58 | C-organize-015 | strong | Split by page ranges or split points | ilovepdf | 8 · 7 · 8 → **7** | split points chosen visually |
| 59 | C-organize-021 | strong | Merge files into one PDF | ilovepdf | 7 · 7 · 8 → **7** | per-file page ranges |
| 60 | C-output-004 | strong | Run progress and cancel | ours | 8 · 7 · 8 → **7** | — |

## By domain
| domain | components | avg overall | at ≥9 |
|---|---|---|---|
| ai | 7 | 0.0 | 0 |
| annotate | 38 | 0.0 | 0 |
| batch | 9 | 2.3 | 0 |
| compare | 7 | 5.4 | 0 |
| convert | 20 | 3.3 | 0 |
| edit | 29 | 1.9 | 0 |
| files | 19 | 4.2 | 0 |
| forms | 17 | 0.0 | 0 |
| optimize | 15 | 2.9 | 0 |
| organize | 27 | 2.7 | 0 |
| output | 15 | 2.6 | 0 |
| platform | 14 | 3.4 | 1 |
| preferences | 14 | 1.8 | 0 |
| reader | 64 | 0.7 | 0 |
| security | 18 | 4.1 | 0 |
| sign | 18 | 2.4 | 0 |
