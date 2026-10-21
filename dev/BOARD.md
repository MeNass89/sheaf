# Parity board

Inspirations: Adobe Acrobat (Reader + Pro tools) (), PDF Expert (Readdle) (), iLovePDF Desktop (2.3.0), Stirling-PDF (), macOS Preview ()
331 components · at parity (≥9): 5 · open 327, closed 4
Rendered by `dev/scripts/board.py render` from `dev/board/components.json`. Do not edit by hand.

## Queue (next first)
| # | id | weight | component | main | F · A · P → overall | next |
|---|---|---|---|---|---|---|
| 1 | C-annotate-001 | strong | Show annotation toolbar | pdfexpert | 0 · 0 · 0 → **0** | annotate mode with a tool strip |
| 2 | C-annotate-007 | strong | Add text box | pdfexpert | 0 · 0 · 0 → **0** | FreeText box |
| 3 | C-edit-011 | strong | Undo and redo edits | pdfexpert | 0 · 0 · 0 → **0** | engine undo/redo |
| 4 | C-forms-001 | strong | Fill form fields | pdfexpert | 0 · 0 · 0 → **0** | fill AcroForm fields in the reader (engine SetFieldValue) |
| 5 | C-output-007 | strong | Print a document | pdfexpert | 0 · 0 · 0 → **0** | print via the OS |
| 6 | C-reader-007 | strong | Fit whole page to window | pdfexpert | 0 · 0 · 0 → **0** | fit-page mode |
| 7 | C-reader-010 | strong | Gesture zoom and pan | pdfexpert | 0 · 0 · 0 → **0** | pinch zoom and drag-pan (hand tool / space-drag) |
| 8 | C-reader-018 | strong | Next and previous page | pdfexpert | 0 · 0 · 0 → **0** | keyboard and toolbar paging |
| 9 | C-reader-033 | strong | Next and previous search match | pdfexpert | 0 · 0 · 0 → **0** | next/previous match |
| 10 | C-edit-001 | strong | Enter edit mode | pdfexpert | 3 · 4 · 6 → **3** | edit mode inside the reader, not a separate tool flow |
| 11 | C-organize-002 | strong | Reorder pages by dragging thumbnails | pdfexpert | 3 · 5 · 7 → **3** | drag-and-drop with multi-select |
| 12 | C-organize-003 | strong | Rotate a page left or right | pdfexpert | 3 · 5 · 8 → **3** | per-page rotate left/right in the grid |
| 13 | C-edit-003 | strong | Add a new text box | pdfexpert | 4 · 5 · 7 → **4** | multi-line text, fonts, undo |
| 14 | C-edit-007 | strong | Move, resize and select page objects | acrobat | 4 · 5 · 7 → **4** | resize handles, move existing page objects (engine MoveObjects) |
| 15 | C-edit-014 | strong | Save edited PDF | ilovepdf | 4 · 5 · 7 → **4** | ⌘S saves in place; Save dialog click bug (seed) |
| 16 | C-output-002 | strong | Save as a new file | pdfexpert | 4 · 6 · 8 → **4** | seed bug: the Save panel's Save button ignores clicks, only Enter works |
| 17 | C-edit-002 | strong | Edit existing text in place | pdfexpert | 5 · 5 · 9 → **5** | engine EditTextBlock/EditTextLine wired to an in-place editor |
| 18 | C-edit-006 | strong | Add an image to a page | pdfexpert | 5 · 5 · 7 → **5** | free resize handles, more formats |
| 19 | C-files-010 | strong | Documents in tabs or windows | ilovepdf | 5 · 6 · 8 → **5** | document tabs |
| 20 | C-organize-001 | strong | Open page organizer grid | pdfexpert | 5 · 6 · 7 → **5** | grid in the reader; multi-select; several files |
| 21 | C-reader-005 | strong | Zoom in and out | pdfexpert | 5 · 6 · 7 → **5** | pinch/ctrl+wheel zoom centred on the pointer, range 10–800 % |
| 22 | C-sign-001 | strong | Open the sign tool | pdfexpert | 5 · 6 · 8 → **5** | sign from the reader, not only a tool tab |
| 23 | C-sign-005 | strong | Place a signature on the page | pdfexpert | 5 · 6 · 8 → **5** | free resize; several placements per run |
| 24 | C-convert-002 | strong | Convert Office documents to PDF | ilovepdf | 6 · 6 · 6 → **6** | needs LibreOffice installed; no sheet/print-area options |
| 25 | C-convert-003 | strong | Convert images to PDF | ilovepdf | 6 · 7 · 8 → **6** | one PDF per image; Letter and other sizes |
| 26 | C-convert-009 | strong | Convert PDF to Word | ilovepdf | 6 · 6 · 7 → **6** | layout fidelity |
| 27 | C-convert-012 | strong | Export pages as images | ilovepdf | 6 · 7 · 8 → **6** | PNG/TIFF, DPI choice |
| 28 | C-files-001 | strong | Open a PDF from a file picker | pdfexpert | 6 · 6 · 8 → **6** | open into the reader, not only into tools |
| 29 | C-files-004 | strong | Reopen a recent file | pdfexpert | 6 · 6 · 8 → **6** | reopen into the reader at the last page; Open Recent menu |
| 30 | C-files-011 | strong | Close documents, tabs and windows | pdfexpert | 6 · 6 · 8 → **6** | ⌘W, unsaved-changes prompt for documents |
| 31 | C-optimize-006 | strong | OCR a scanned PDF | ilovepdf | 7 · 6 · 6 → **6** | speed on big scans; deskew |
| 32 | C-organize-007 | strong | Delete pages | pdfexpert | 6 · 6 · 8 → **6** | pick pages visually in Remove |
| 33 | C-organize-014 | strong | Extract pages to a new PDF | pdfexpert | 7 · 6 · 8 → **6** | each page as its own file |
| 34 | C-reader-008 | strong | Fit page width | pdfexpert | 7 · 6 · 8 → **6** | keep fit width on window resize; shortcut |
| 35 | C-security-005 | strong | Open a password-protected PDF | pdfexpert | 6 · 6 · 9 → **6** | prompt in the reader |
| 36 | C-security-010 | strong | Mark an area or text for redaction | acrobat | 6 · 7 · 8 → **6** | mark by text selection |
| 37 | C-sign-002 | strong | Draw a signature | pdfexpert | 6 · 6 · 8 → **6** | pen colour and thickness |
| 38 | C-sign-003 | strong | Type a signature | stirling | 6 · 6 · 8 → **6** | several script fonts |
| 39 | C-annotate-006 | strong | Add sticky note | pdfexpert | 8 · 7 · 8 → **7** | sticky note with popup |
| 40 | C-annotate-010 | strong | Draw freehand ink | pdfexpert | 7 · 7 · 8 → **7** | ink tool |
| 41 | C-edit-016 | strong | Add page numbers | ilovepdf | 8 · 7 · 8 → **7** | font, size and colour in the app |
| 42 | C-files-002 | strong | Drag and drop files to open | ilovepdf | 7 · 7 · 8 → **7** | drop on the reader opens the document |
| 43 | C-files-003 | strong | Home screen | pdfexpert | 7 · 7 · 8 → **7** | recents with thumbnails; open-file drop zone |
| 44 | C-files-015 | strong | Manage a tool's input files | ilovepdf | 7 · 7 · 8 → **7** | drag reorder |
| 45 | C-optimize-001 | strong | Compress a PDF (levels) | ilovepdf | 7 · 7 · 8 → **7** | seed bug: Save after Compress needs Enter; fonts/vector recompression |
| 46 | C-organize-015 | strong | Split by page ranges or split points | ilovepdf | 8 · 7 · 8 → **7** | split points chosen visually |
| 47 | C-organize-021 | strong | Merge files into one PDF | ilovepdf | 7 · 7 · 8 → **7** | per-file page ranges |
| 48 | C-output-001 | strong | Save the document | pdfexpert | 9 · 8 · 7 → **7** | save in place from the reader |
| 49 | C-output-004 | strong | Run progress and cancel | ours | 8 · 7 · 8 → **7** | — |
| 50 | C-output-005 | strong | Results screen | ilovepdf | 8 · 7 · 8 → **7** | — |
| 51 | C-platform-011 | strong | Open files from the system | ours | 7 · 7 · 9 → **7** | registered as Alternate viewer on macOS |
| 52 | C-platform-012 | strong | Native desktop builds | ours | 7 · 7 · 9 → **7** | macOS notarisation (needs Nassim's Apple account) |
| 53 | C-security-002 | strong | Protect a PDF with a password | ilovepdf | 7 · 7 · 9 → **7** | strength meter, show-password toggle |
| 54 | C-security-006 | strong | Remove a password | ilovepdf | 8 · 7 · 9 → **7** | — |
| 55 | C-security-012 | strong | Find text and redact | acrobat | 7 · 7 · 8 → **7** | regex, whole words, custom patterns |
| 56 | C-security-015 | strong | Apply redactions | acrobat | 8 · 7 · 8 → **7** | preview of the redacted output |
| 57 | C-annotate-002 | strong | Highlight selected text | pdfexpert | 9 · 9 · 8 → **8** | highlight via engine TextMarkup over PageText line rects |
| 58 | C-annotate-022 | strong | Change annotation colour | pdfexpert | 8 · 8 · 9 → **8** | recolour selected annotation |
| 59 | C-annotate-030 | strong | List comments in a panel | pdfexpert | 9 · 8 · 9 → **8** | annotations panel |
| 60 | C-files-012 | strong | Browse and open tools | ilovepdf | 8 · 8 · 9 → **8** | — |

## By domain
| domain | components | avg overall | at ≥9 |
|---|---|---|---|
| ai | 7 | 0.0 | 0 |
| annotate | 38 | 1.4 | 1 |
| batch | 9 | 2.3 | 0 |
| compare | 7 | 5.4 | 0 |
| convert | 20 | 3.3 | 0 |
| edit | 29 | 2.1 | 0 |
| files | 19 | 4.2 | 0 |
| forms | 17 | 0.0 | 0 |
| optimize | 15 | 2.9 | 0 |
| organize | 27 | 2.7 | 0 |
| output | 15 | 3.1 | 0 |
| platform | 14 | 3.4 | 1 |
| preferences | 14 | 1.8 | 0 |
| reader | 64 | 1.8 | 3 |
| security | 18 | 4.1 | 0 |
| sign | 18 | 2.4 | 0 |

## Closed
| id | component | overall | last proof |
|---|---|---|---|
| C-annotate-025 | Delete an annotation | 9 | Click selects (blue outline), Delete or the panel's trash removes, ⌘Z restores (dev/re/evidence/F-002/15) |
| C-reader-023 | Page thumbnails panel | 9 | Pages tab: 150 px thumbnails like PDF Expert's, page number, current ringed, click jumps, lazy renders (dev/re/evidence/F-002/11, dev/re/evidence/F-002/18) |
| C-reader-043 | Dark and sepia reading themes | 9 | Day / Sepia / Night on pages only, remembered, Night keeps hues (yellow highlight, green headings) (dev/re/evidence/F-002/08, dev/re/evidence/F-002/18); test themes_map_white_* |
| C-reader-048 | Select and copy text | 9 | Drag selects in reading order (also over a highlight), double-click word, ⌘A page, ⌘C → clipboard verified with xclip (dev/re/evidence/F-002/03, dev/re/evidence/F-002/19); test a_drag_selects_* |
