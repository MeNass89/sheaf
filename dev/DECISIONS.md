# Decisions — Sheaf (dev skill)

Each entry: date · decision · alternatives · reason · what would change it. Nassim reviews and overrides here.

## 2026-10-20 · Brief
**Decision.** Sheaf becomes the end-all PDF app: the one-off toolbox it already is (compress, OCR, merge…)
**and** the daily hub for reading, annotating and editing PDFs, at PDF Expert's level of polish.
**Alternatives.** Stay a toolbox only (Smallpdf/iLovePDF model). **Reason.** Nassim's direction (20/10): "the central
hub for editing, annotating, reading PDFs", e.g. a student studying. **Would change it.** Nassim.

## 2026-10-20 · Inspirations (phase 2)
**Decision.** Acrobat (breadth), PDF Expert (aesthetics, performance, reading/annotating/editing), iLovePDF Desktop
(toolbox model), Stirling-PDF (open-source code to translate), macOS Preview (performance floor, reading UI).
**Alternatives considered.** Foxit, PDFelement, Nitro (redundant with Acrobat), UPDF/PDFgear (closed, unverifiable
stack), Smallpdf (web, overlaps iLovePDF), Sejda (Electron, easy to read — kept as a fallback for in-place text
editing), PDF Arranger (GPL), pdf.js (viewer only, a later reference for text layers).
**Reason.** One app per north star plus one readable codebase; Nassim approved the four and added Preview (20/10).
**Would change it.** A component where none of the five has a good version → add a specialist app for it.

## 2026-10-20 · Known polish bugs reported by Nassim (seed the board)
- Save dialog after Compress: clicking "Save" does nothing; only Enter works. Function bug, P1.
- Edit PDF has too few functions. P1, to be measured against PDF Expert.
- No reading mode for long study sessions (outline, thumbnails, continuous scroll, highlights, notes). P1.

## 2026-10-20 · Work split across machines
**Decision.** Acquisition and reverse engineering on the MacBook Pro (the apps are macOS); building, testing and
screenshots on the VPS from tomorrow, using the Linux build and the archives. **Reason.** Nassim's plan.

## 2026-10-20 · Phase 4: how the board was built
**Decision.** 1,254 raw gestures extracted from the five archives by Haiku agents, one per app (Acrobat in two
halves), to `dev/board/raw/*.json` (instructions `dev/board/EXTRACT.md`; every evidence string checked with `rg -F`);
merged per domain into 331 components (`dev/board/MERGE.md`), then `locate.py` (code found for 750 app×component
pairs; the rest say so in `how`). Main app, what to take, weight and Sheaf's grade were decided by Opus per component
and live in `dev/board/decisions/<domain>.py`; the table below is generated from them (`merge_prep.py decide`).
**Rules applied.** PDF Expert is the main app for reading, annotating and in-place editing wherever it has the gesture
(Nassim's direction and the aesthetic bar); iLovePDF for one-screen toolbox tools Sheaf already models on it (compress,
split, page numbers, watermark, results, history, preferences); Acrobat where only it goes deep (forms, digital
signatures, redaction, header/footer, measurement); Preview or Stirling for the few gestures only they have.
Grades are Sheaf 0.2.1 as inventoried in `ours.json`; an absent gesture is 0/0/0.
**Alternatives.** Letting the cheap agents pick main apps (rejected: the skill reserves it). **Would change it.** A
capture of the main app contradicting a choice, or Nassim.

## 2026-10-20 · Order of work in phase 5
**Decision.** Nassim's four seeds go first, before `board.py next`: (a) the Save dialog that ignores clicks
(C-output-002, C-optimize-001), (b) the reader and annotator (reader + annotate strong components), (c) editing
(C-edit-*), (d) polish and responsiveness. Then `board.py next` order. **Reason.** His explicit order.

## 2026-10-20 · Building on vpsn (Linux)
**Decision.** Every build goes through `dev/scripts/cargo.sh`: two jobs, `CARGO_TARGET_DIR=~/.cache/cargo-target/sheaf`,
line tables only for our crates, no debug info for dependencies, run under a 3 GB cgroup. **Reason.** With full debug
info the test binaries filled the disk (target 16 GB, disk 100 %, `ld` bus error); a 2 GB cap OOM-killed `-j 2`.
**Would change it.** A bigger machine.

## 2026-10-20 · F-001: every file dialog is asynchronous and attached to the window
**Decision.** All native dialogs go through `src/dialog.rs` (`rfd::AsyncFileDialog` + `slint::spawn_local`, parent =
our window: a sheet on macOS). **Alternatives.** Defer the blocking dialog with a zero timer (still a nested modal
loop inside winit); keep blocking dialogs and only fix Save. **Reason.** The click-deaf Save button comes from the
nested modal loop; PDF Expert's Save As is a sheet. **Would change it.** The Mac test failing.

## 2026-10-20 · F-002: the reader's architecture
**Decision.** A PDF opened from Home, a drop, Recents or "Open With" opens in its own document tab (the reader);
non-PDF files still wait on Home for a tool, and a "Use a tool" button sends the document to the Tools page.
The reader holds the document in a PdfCraft session (the working copy: annotations, undo/redo, text layer, outline,
incremental save) and renders pages with pdfium from the working copy's bytes; each page render carries a
revision, so an edit re-renders only its page and the old render stays on screen until then. ⌘S writes an
incremental update into the file itself (as PDF Expert and Preview do); Save As writes a full copy.
**Alternatives.** Rendering with PdfCraft (measured on the Mac earlier: 600–800 MB on a picture-heavy guide);
annotating through pdfium (no undo, weaker appearance streams); keeping "Choose a tool" for PDFs opened on Home
(the toolbox-first model; rejected by Nassim's "Sheaf must become a reader"). **Would change it.** Nassim preferring
the old Home behaviour, or pdfium failing to draw PdfCraft's appearances.

## 2026-10-20 · F-002: page themes are recoloured renders
**Decision.** Night inverts the page (white paper → #1E1E20, black ink → #DEDEDC), Sepia maps white → #F4ECD8 and
black → #5B4636, both by a per-channel lookup table on the RGBA render (≈1 ms per megapixel); pictures are recoloured
too, as in PDF Expert's Night (evidence 11). **Alternatives.** PdfCraft's reading colours (text and line art only,
not pdfium's path). **Would change it.** Users wanting photos left untouched in Night: then recolour only near-grey pixels.

<!-- board decisions: generated by merge_prep.py decide -->

### Board · ai (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-ai-001 | Open the AI assistant | light | pdfexpert | PDF Copilot panel |  | 0·0·0 |
| C-ai-002 | Ask AI about the document | light | acrobat | AI Assistant Q&A |  | 0·0·0 |
| C-ai-003 | Summarize with AI | light | pdfexpert | summarise selection or document |  | 0·0·0 |
| C-ai-004 | Translate with AI | light | pdfexpert | translate PDF |  | 0·0·0 |
| C-ai-005 | Generate podcast or presentation | light | acrobat | podcast/presentation |  | 0·0·0 |
| C-ai-006 | Group documents in AI spaces | light | acrobat | AI spaces |  | 0·0·0 |
| C-ai-007 | Classify documents with AI | light | stirling | classify |  | 0·0·0 |

### Board · annotate (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-annotate-001 | Show annotation toolbar | strong | pdfexpert | vertical tool strip always one click away (evidence 05), tools keep their last colour | preview: markup toolbar toggle ⇧⌘A | 0·0·0 |
| C-annotate-002 | Highlight selected text | strong | pdfexpert | select text → highlight from toolbar or context menu, 5 preset colours, multiply blend (evidence 06b) | preview: highlight stays active as a mode; acrobat: highlight with mouse drag across columns | 0·0·0 |
| C-annotate-003 | Underline selected text | normal | pdfexpert | same gesture as highlight |  | 0·0·0 |
| C-annotate-004 | Strike through selected text | normal | pdfexpert | same gesture as highlight |  | 0·0·0 |
| C-annotate-005 | Propose text insertion or replacement | light | acrobat | insert/replace text proposals are an Acrobat review feature |  | 0·0·0 |
| C-annotate-006 | Add sticky note | strong | pdfexpert | click to drop a note icon, popup editor, colour | acrobat: note icon choice | 0·0·0 |
| C-annotate-007 | Add text box | strong | pdfexpert | text box with font size and colour, drag to resize |  | 0·0·0 |
| C-annotate-008 | Add callout or speech bubble | normal | acrobat | callout with leader line |  | 0·0·0 |
| C-annotate-009 | Change text annotation font | normal | pdfexpert | font and size for text annotations in the inspector |  | 0·0·0 |
| C-annotate-010 | Draw freehand ink | strong | pdfexpert | pen with thickness and colour, smoothing |  | 0·0·0 |
| C-annotate-011 | Erase ink strokes | normal | pdfexpert | eraser removes stroke parts (EraseInkTool) |  | 0·0·0 |
| C-annotate-012 | Draw rectangle or oval | normal | pdfexpert | shapes with stroke/fill, drag to draw |  | 0·0·0 |
| C-annotate-013 | Draw line or arrow | normal | pdfexpert | line and arrow |  | 0·0·0 |
| C-annotate-014 | Draw polygon, polyline or cloud | light | acrobat | polygon, cloud |  | 0·0·0 |
| C-annotate-015 | Add loupe or mask | light | preview | loupe/mask (Preview only) |  | 0·0·0 |
| C-annotate-016 | Place a stamp | normal | pdfexpert | stamp picker | acrobat: dynamic stamps with date | 0·0·0 |
| C-annotate-017 | Create a custom stamp | light | acrobat | custom stamps from images |  | 0·0·0 |
| C-annotate-018 | Add check, cross or symbol | normal | pdfexpert | check/cross symbols in Fill & Sign |  | 0·0·0 |
| C-annotate-019 | Add date stamp | light | pdfexpert | date stamp |  | 0·0·0 |
| C-annotate-020 | Attach file or audio comment | light | acrobat | file/sound attachment comments |  | 0·0·0 |
| C-annotate-021 | Add link annotation | normal | preview | link annotation | stirling: link to page or URL | 0·0·0 |
| C-annotate-022 | Change annotation colour | strong | pdfexpert | colour row in the inspector and context menu applies to selection |  | 0·0·0 |
| C-annotate-023 | Set stroke, fill and opacity | normal | pdfexpert | inspector with stroke width, fill and opacity |  | 0·0·0 |
| C-annotate-024 | Edit annotation properties and defaults | light | acrobat | properties dialog and make-default |  | 0·0·0 |
| C-annotate-025 | Delete an annotation | strong | acrobat | select + Delete key, context menu delete | pdfexpert: delete from list | 0·0·0 |
| C-annotate-026 | Undo, redo, cut and paste annotations | normal | preview | ⌘Z/⌘X/⌘V on annotations |  | 0·0·0 |
| C-annotate-027 | Rotate an annotation | light | pdfexpert | rotate annotation |  | 0·0·0 |
| C-annotate-028 | Align and snap annotations | light | pdfexpert | align annotations |  | 0·0·0 |
| C-annotate-029 | Remove all annotations | normal | pdfexpert | remove all annotations | stirling: tool form | 0·0·0 |
| C-annotate-030 | List comments in a panel | strong | pdfexpert | Annotations tab lists highlights with their text, click to jump (evidence 04c) | acrobat: comment threads | 0·0·0 |
| C-annotate-031 | Sort and filter comments | normal | acrobat | sort and filter comments |  | 0·0·0 |
| C-annotate-032 | Hide or show all comments | normal | acrobat | hide all comments |  | 0·0·0 |
| C-annotate-033 | Reply to a comment | normal | acrobat | reply threads |  | 0·0·0 |
| C-annotate-034 | Set comment review status | light | acrobat | review status |  | 0·0·0 |
| C-annotate-035 | Export or summarise comments | normal | acrobat | summarise comments to PDF | pdfexpert: export annotations summary | 0·0·0 |
| C-annotate-036 | Import comments | light | acrobat | import comments (FDF/XFDF) |  | 0·0·0 |
| C-annotate-037 | Send for shared review | light | acrobat | shared review (cloud) |  | 0·0·0 |
| C-annotate-038 | Measure distance, perimeter, area | light | pdfexpert | measure tools |  | 0·0·0 |

### Board · batch (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-batch-001 | Process many files in one run | normal | ilovepdf | each tool accepts many files and writes one output each | acrobat: Action Wizard file lists | 7·7·8 |
| C-batch-002 | Build a multi-step action | light | acrobat | Action Wizard steps | stirling: automate pipelines | 0·0·0 |
| C-batch-003 | Run a saved action | light | acrobat | run saved actions |  | 0·0·0 |
| C-batch-004 | Import or export an action | light | stirling | import/export pipelines JSON |  | 0·0·0 |
| C-batch-005 | Use suggested actions | light | acrobat | suggested actions |  | 0·0·0 |
| C-batch-006 | Create a custom tool | light | acrobat | custom tools |  | 0·0·0 |
| C-batch-007 | Chain another tool from results | normal | ilovepdf | continue to another tool from the result (evidence 05c) |  | 8·7·9 |
| C-batch-008 | Automate a watched folder | light | stirling | watched folders |  | 0·0·0 |
| C-batch-009 | Minimize or cancel a running task | normal | ilovepdf | minimise or cancel a running task |  | 7·7·8 |

### Board · compare (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-compare-001 | Choose two documents to compare | normal | acrobat | pick old and new, recent-files suggestions | ilovepdf: two drop slots | 8·7·9 |
| C-compare-002 | Swap the compared files | light | ilovepdf | swap the two files |  | 9·8·9 |
| C-compare-003 | Compare text content | normal | acrobat | text compare with a summary of changes by type | stirling: word-level diff | 6·6·7 |
| C-compare-004 | Compare visually by overlay | normal | ilovepdf | visual overlay compare |  | 6·6·7 |
| C-compare-005 | View documents side by side, synced | normal | stirling | side by side synced scrolling |  | 0·0·0 |
| C-compare-006 | Browse differences and summary | normal | acrobat | change list, click to jump |  | 4·5·8 |
| C-compare-007 | Download a change report | light | ilovepdf | download report PDF |  | 8·7·8 |

### Board · convert (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-convert-001 | Open the conversion hub | normal | acrobat | Create PDF hub accepts any supported file in one place | stirling: hide unavailable conversions | 6·7·8 |
| C-convert-002 | Convert Office documents to PDF | strong | ilovepdf | Word/PPT/Excel to PDF with fidelity, batch; Acrobat needs Office installed on macOS | acrobat: keeps links and bookmarks | 6·6·6 |
| C-convert-003 | Convert images to PDF | strong | ilovepdf | images to PDF: orientation, margins, page size, one PDF or one per image | preview: export as PDF | 6·7·8 |
| C-convert-004 | Create PDF from clipboard | normal | preview | New from Clipboard ⌘N |  | 0·0·0 |
| C-convert-005 | Convert a web page or HTML to PDF | normal | ilovepdf | HTML/URL to PDF with page size and screen mode | acrobat: web capture | 5·6·7 |
| C-convert-006 | Create PDF from scanner or screenshot | normal | acrobat | scan to PDF |  | 0·0·0 |
| C-convert-007 | Convert other formats to PDF | light | stirling | other formats (ebook, markdown…) |  | 0·0·0 |
| C-convert-008 | Create an empty document | normal | pdfexpert | new blank document |  | 0·0·0 |
| C-convert-009 | Convert PDF to Word | strong | ilovepdf | PDF to editable Word, OCR when scanned | acrobat: flowing vs exact layout | 6·6·7 |
| C-convert-010 | Convert PDF to Excel | normal | ilovepdf | PDF to Excel tables | acrobat: one sheet per table option | 6·6·7 |
| C-convert-011 | Convert PDF to PowerPoint | normal | ilovepdf | PDF to PowerPoint |  | 6·6·7 |
| C-convert-012 | Export pages as images | strong | ilovepdf | pages to JPG with quality; extract images mode | acrobat: PNG/TIFF and DPI | 6·7·8 |
| C-convert-013 | Extract embedded images | normal | ilovepdf | extract embedded images |  | 0·0·0 |
| C-convert-014 | Export PDF as text or RTF | normal | acrobat | export as text/RTF |  | 0·0·0 |
| C-convert-015 | Export PDF as HTML or XML | light | acrobat | HTML/XML export |  | 0·0·0 |
| C-convert-016 | Export to PostScript or ebook | light | acrobat | PostScript/ebook |  | 0·0·0 |
| C-convert-017 | Set export options | normal | acrobat | per-format export settings |  | 4·6·8 |
| C-convert-018 | Convert PDF to PDF/A | normal | ilovepdf | PDF/A conversion with level choice | acrobat: preflight report | 7·7·7 |
| C-convert-019 | Save as PDF/X or PDF/E | light | acrobat | PDF/X, PDF/E |  | 0·0·0 |
| C-convert-020 | Choose online or offline conversion | light | ilovepdf | offline vs online |  | 9·8·9 |

### Board · edit (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-edit-001 | Enter edit mode | strong | pdfexpert | Edit mode is one toolbar tab with Text/Image/Link/Redact tools (evidence 07); Acrobat's is heavier | acrobat: bounding boxes toggle | 3·4·6 |
| C-edit-002 | Edit existing text in place | strong | pdfexpert | click a paragraph, edit with the original font kept, reflow inside the block | acrobat: paragraph reflow and font substitution notice | 0·0·0 |
| C-edit-003 | Add a new text box | strong | pdfexpert | click to add a text box, type multi-line, drag to move | ilovepdf: text box presets | 4·5·7 |
| C-edit-004 | Format text font, size and style | normal | acrobat | font, size, bold/italic, alignment in the format panel; PDF Expert has fewer options | pdfexpert: inline toolbar | 3·4·7 |
| C-edit-005 | Change text colour | normal | acrobat | colour picker in format panel |  | 4·5·8 |
| C-edit-006 | Add an image to a page | strong | pdfexpert | add image, place by click, drag corners to resize | acrobat: replace image | 5·5·7 |
| C-edit-007 | Move, resize and select page objects | strong | acrobat | select any page object (text, image, path), move and resize with handles | ilovepdf: snapping guides | 4·5·7 |
| C-edit-008 | Replace, rotate or edit an image | normal | acrobat | replace, rotate, flip, crop an image |  | 0·0·0 |
| C-edit-009 | Align and arrange objects | light | acrobat | align and distribute |  | 0·0·0 |
| C-edit-010 | Cut, copy, paste and delete objects | normal | pdfexpert | ⌘C/⌘V/⌫ on objects |  | 4·5·8 |
| C-edit-011 | Undo and redo edits | strong | pdfexpert | ⌘Z/⇧⌘Z across every edit |  | 0·0·0 |
| C-edit-012 | Find and replace text | normal | acrobat | find and replace in edit mode | stirling: regex replace | 0·0·0 |
| C-edit-013 | Spell check | light | acrobat | spell check while editing |  | 0·0·0 |
| C-edit-014 | Save edited PDF | strong | ilovepdf | edits saved in place (⌘S) or Save As; iLovePDF saves straight back | stirling: download/apply changes | 4·5·7 |
| C-edit-015 | Add header and footer | normal | acrobat | header/footer with 6 slots, fonts, page range, update/remove | pdfexpert: simpler panel | 0·0·0 |
| C-edit-016 | Add page numbers | strong | ilovepdf | page numbers with position grid, first number, range, single/double sided — same model as Sheaf | pdfexpert: font choice | 8·7·8 |
| C-edit-017 | Add Bates numbering | light | acrobat | Bates numbering |  | 0·0·0 |
| C-edit-018 | Add a watermark | normal | ilovepdf | text or image watermark, position grid, mosaic, transparency, rotation, layer | acrobat: live preview and update/remove | 7·6·8 |
| C-edit-019 | Add or remove a page background | light | acrobat | background colour or image, update/remove |  | 0·0·0 |
| C-edit-020 | Overlay PDFs | light | stirling | overlay one PDF on another |  | 0·0·0 |
| C-edit-021 | Add and edit links | normal | acrobat | link tool: draw a box, link to page or URL | pdfexpert: link tool in edit strip | 0·0·0 |
| C-edit-022 | Manage file attachments | light | acrobat | attachments panel add/remove |  | 0·0·0 |
| C-edit-023 | Edit bookmarks and table of contents | normal | acrobat | bookmarks panel edit | stirling: edit TOC tool | 0·0·0 |
| C-edit-024 | Create article threads | light | acrobat | articles |  | 0·0·0 |
| C-edit-025 | Edit page labels | light | acrobat | page labels |  | 0·0·0 |
| C-edit-026 | Insert a blank page while editing | light | ilovepdf | insert blank page while editing |  | 6·6·8 |
| C-edit-027 | Restrict editing | light | acrobat | restrict editing (permissions) |  | 5·6·8 |
| C-edit-028 | Adjust or invert colours | light | stirling | adjust colours / invert |  | 0·0·0 |
| C-edit-029 | Remove images or image background | light | stirling | remove images |  | 0·0·0 |

### Board · files (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-files-001 | Open a PDF from a file picker | strong | pdfexpert | ⌘O opens straight into reading; the open panel remembers the folder | ilovepdf: open into a chosen tool | 6·6·8 |
| C-files-002 | Drag and drop files to open | strong | ilovepdf | drop anywhere: home or tool; refuses non-PDFs with a message | stirling: drop many files to the workbench | 7·7·8 |
| C-files-003 | Home screen | strong | pdfexpert | home with Open Files drop zone, tools grid and recents (evidence 01) | ilovepdf: sidebar Recents/Tools/History/Preferences | 7·7·8 |
| C-files-004 | Reopen a recent file | strong | pdfexpert | recents with thumbnails, reopen at last page | preview: File ▸ Open Recent menu | 6·6·8 |
| C-files-005 | Manage the recent files list | normal | ilovepdf | remove from recents, reveal, clear |  | 5·6·8 |
| C-files-006 | Star or pin favourite files | normal | pdfexpert | star favourites |  | 0·0·0 |
| C-files-007 | Search, sort and select listed files | normal | stirling | search/sort the file list |  | 0·0·0 |
| C-files-008 | Local file library | light | stirling | local library |  | 0·0·0 |
| C-files-009 | Cloud, network and shared files | normal | acrobat | cloud storages |  | 0·0·0 |
| C-files-010 | Documents in tabs or windows | strong | ilovepdf | documents and tools open as tabs in one window | stirling: workbench file switcher | 5·6·8 |
| C-files-011 | Close documents, tabs and windows | strong | pdfexpert | ⌘W closes the tab, asks to save if dirty |  | 6·6·8 |
| C-files-012 | Browse and open tools | strong | ilovepdf | tools grid by category with colour tiles; same model | stirling: tool search | 8·8·9 |
| C-files-013 | Search tools | normal | stirling | tool search field |  | 6·7·9 |
| C-files-014 | Favourite and recent tools | normal | ilovepdf | favourite tools |  | 0·0·0 |
| C-files-015 | Manage a tool's input files | strong | ilovepdf | add/remove/reorder input files in a tool |  | 7·7·8 |
| C-files-016 | Unlock password-protected input | normal | ilovepdf | password prompt per file |  | 8·7·9 |
| C-files-017 | System integration and default app | light | ilovepdf | default app prompt, OS context menu |  | 3·5·8 |
| C-files-018 | Scan or capture into a PDF | light | ilovepdf | scan to PDF |  | 0·0·0 |
| C-files-019 | Unreadable file feedback | normal | ours | Sheaf-only: clear toasts for unreadable files |  | 8·7·9 |

### Board · forms (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-forms-001 | Fill form fields | strong | pdfexpert | click a field and type; tab order; checkboxes toggle | acrobat: field highlight | 0·0·0 |
| C-forms-002 | Clear a form | normal | acrobat | clear form |  | 0·0·0 |
| C-forms-003 | Required field warnings | normal | acrobat | required fields warning |  | 0·0·0 |
| C-forms-004 | Auto-complete form entries | light | acrobat | auto-complete |  | 0·0·0 |
| C-forms-005 | Highlight fillable fields | normal | acrobat | highlight fields |  | 0·0·0 |
| C-forms-006 | Enter form editing mode | normal | acrobat | Prepare Form mode |  | 0·0·0 |
| C-forms-007 | Auto-detect form fields | normal | pdfexpert | auto-detect form fields | stirling: auto form detection | 0·0·0 |
| C-forms-008 | Add a text field | normal | acrobat | text field |  | 0·0·0 |
| C-forms-009 | Add choice fields | normal | acrobat | checkbox, radio, list, dropdown |  | 0·0·0 |
| C-forms-010 | Add a button field | light | acrobat | buttons |  | 0·0·0 |
| C-forms-011 | Add special fields | light | acrobat | barcode, signature fields |  | 0·0·0 |
| C-forms-012 | Edit field properties | normal | acrobat | properties dialog |  | 0·0·0 |
| C-forms-013 | Format, validate and calculate fields | light | acrobat | format/validate/calculate |  | 0·0·0 |
| C-forms-014 | Move, duplicate and arrange fields | normal | acrobat | move, duplicate, align |  | 0·0·0 |
| C-forms-015 | Import and export form data | light | acrobat | import/export FDF/XFDF |  | 0·0·0 |
| C-forms-016 | Distribute and submit forms | light | acrobat | distribute |  | 0·0·0 |
| C-forms-017 | Flatten form fields | normal | acrobat | flatten | pdfexpert: save as flattened | 0·0·0 |

### Board · optimize (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-optimize-001 | Compress a PDF (levels) | strong | ilovepdf | three levels (extreme/recommended/less) on a one-screen tool, result shows savings (evidence 03–05c) | acrobat: font subsetting, object streams; stirling: custom quality | 7·7·8 |
| C-optimize-002 | See compression savings | normal | ilovepdf | savings shown on the result screen |  | 8·7·9 |
| C-optimize-003 | Compress to a target size | normal | stirling | compress to a target size |  | 0·0·0 |
| C-optimize-004 | Batch compress files | normal | ours | Sheaf-only batch compress of many files |  | 8·7·8 |
| C-optimize-005 | Advanced optimization | light | acrobat | PDF Optimizer audit space and settings |  | 0·0·0 |
| C-optimize-006 | OCR a scanned PDF | strong | ilovepdf | OCR to a searchable PDF | acrobat: deskew, image-only output | 7·6·6 |
| C-optimize-007 | Choose OCR language and level | normal | ilovepdf | OCR language list |  | 3·5·8 |
| C-optimize-008 | OCR multiple files | light | acrobat | OCR many files |  | 6·6·7 |
| C-optimize-009 | Correct recognized text | light | acrobat | correct recognised text |  | 0·0·0 |
| C-optimize-010 | Enhance scanned pages | normal | acrobat | enhance scans (deskew, background) |  | 0·0·0 |
| C-optimize-011 | Repair a damaged PDF | normal | ilovepdf | repair a damaged PDF |  | 8·7·8 |
| C-optimize-012 | Run preflight and output preview | light | acrobat | preflight |  | 0·0·0 |
| C-optimize-013 | Convert colours or grayscale | light | stirling | grayscale |  | 0·0·0 |
| C-optimize-014 | Prepare for print production | light | acrobat | print production |  | 0·0·0 |
| C-optimize-015 | Create a full-text index | light | acrobat | full-text index |  | 0·0·0 |

### Board · organize (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-organize-001 | Open page organizer grid | strong | pdfexpert | page grid with an ops strip (insert, rotate, copy, extract, split, delete), evidence 08 | stirling: page editor multi-select toolbar | 5·6·7 |
| C-organize-002 | Reorder pages by dragging thumbnails | strong | pdfexpert | drag thumbnails, multi-select, drop indicator | preview: drag between documents | 3·5·7 |
| C-organize-003 | Rotate a page left or right | strong | pdfexpert | rotate buttons on hover and in the strip, both directions |  | 3·5·8 |
| C-organize-004 | Rotate pages by angle and range | normal | ilovepdf | rotate tool with angle and page selection | acrobat: even/odd/landscape page filters | 7·6·8 |
| C-organize-005 | Auto-rotate pages by orientation | light | stirling | auto-rotate by text orientation |  | 0·0·0 |
| C-organize-006 | Flip pages horizontally or vertically | light | preview | flip pages |  | 0·0·0 |
| C-organize-007 | Delete pages | strong | pdfexpert | select pages in the grid, ⌫ | ilovepdf: remove pages by visual pick | 6·6·8 |
| C-organize-008 | Remove blank pages automatically | light | stirling | remove blank pages with threshold |  | 0·0·0 |
| C-organize-009 | Insert a blank page | normal | pdfexpert | insert blank at any position with size |  | 5·6·8 |
| C-organize-010 | Insert pages from another file | normal | pdfexpert | insert from file at position | acrobat: before/after page N | 0·0·0 |
| C-organize-011 | Insert pages from clipboard or scanner | light | preview | insert from scanner/clipboard |  | 0·0·0 |
| C-organize-012 | Replace pages from another PDF | light | acrobat | replace pages |  | 0·0·0 |
| C-organize-013 | Copy or duplicate pages | normal | pdfexpert | copy/paste pages, duplicate |  | 2·4·8 |
| C-organize-014 | Extract pages to a new PDF | strong | pdfexpert | extract selected pages; option each page a separate file | ilovepdf: extract by ranges tool | 7·6·8 |
| C-organize-015 | Split by page ranges or split points | strong | ilovepdf | split by ranges with custom/fixed modes, merge ranges option | pdfexpert: split point in the grid | 8·7·8 |
| C-organize-016 | Split by page count, size or bookmarks | normal | acrobat | split by page count, file size, top-level bookmarks | stirling: split by chapters | 5·6·8 |
| C-organize-017 | Split a page into parts | light | stirling | split a page into parts |  | 0·0·0 |
| C-organize-018 | Crop pages | normal | acrobat | crop tool: drag a box on the page, apply to range, remove white margins | ilovepdf: crop margins in mm | 6·6·8 |
| C-organize-019 | Resize or scale pages | light | stirling | scale page size | pdfexpert: page resize in Edit menu | 0·0·0 |
| C-organize-020 | Reverse or reshuffle page order | light | stirling | reverse/odd-even reorder |  | 0·0·0 |
| C-organize-021 | Merge files into one PDF | strong | ilovepdf | drop files, reorder cards, merge; same toolbox model as Sheaf | acrobat: page ranges per file | 7·7·8 |
| C-organize-022 | Arrange files and pages before merging | normal | acrobat | expand files to pages before combining, drag to reorder |  | 4·6·8 |
| C-organize-023 | Name the merged output file | normal | ilovepdf | name the merged output |  | 9·8·9 |
| C-organize-024 | Create a PDF portfolio | light | acrobat | portfolios |  | 0·0·0 |
| C-organize-025 | Impose pages: N-up, booklet, one long page | light | stirling | N-up / booklet |  | 0·0·0 |
| C-organize-026 | Set page transitions and page properties | light | acrobat | page transitions |  | 0·0·0 |
| C-organize-027 | Undo and redo page edits | normal | stirling | undo/redo in the page editor |  | 0·0·0 |

### Board · output (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-output-001 | Save the document | strong | pdfexpert | ⌘S saves annotations and edits in place, incremental | preview: autosave and versions | 0·0·0 |
| C-output-002 | Save as a new file | strong | pdfexpert | Save As sheet with name and folder | acrobat: save as other formats | 4·6·8 |
| C-output-003 | Choose the output folder | normal | ilovepdf | always ask / next to original / fixed folder |  | 8·7·9 |
| C-output-004 | Run progress and cancel | strong | ours | Sheaf-only progress overlay with cancel |  | 8·7·8 |
| C-output-005 | Results screen | strong | ilovepdf | result screen: open, reveal, start over, continue with another tool (evidence 05c) | stirling: chain tools | 8·7·8 |
| C-output-006 | Show the output in its folder | normal | ilovepdf | show in folder |  | 8·7·9 |
| C-output-007 | Print a document | strong | pdfexpert | print with scale options (evidence 12c) | acrobat: page range, booklet | 0·0·0 |
| C-output-008 | Print layout options | light | acrobat | print layout options |  | 0·0·0 |
| C-output-009 | Share a document | normal | pdfexpert | share sheet |  | 0·0·0 |
| C-output-010 | Email as attachment | normal | acrobat | email as attachment |  | 0·0·0 |
| C-output-011 | Export or mail flattened | light | pdfexpert | export flattened |  | 0·0·0 |
| C-output-012 | Tasks history | normal | ilovepdf | tasks history with reveal and rerun (evidence 06) |  | 7·7·8 |
| C-output-013 | Notify when a task finishes | light | ilovepdf | notify when done |  | 0·0·0 |
| C-output-014 | Revert, versions and undo history | normal | preview | revert to saved / versions | stirling: undo history | 0·0·0 |
| C-output-015 | Rename or trash the document | light | preview | rename / move to trash |  | 0·0·0 |

### Board · platform (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-platform-001 | Check for updates | normal | ilovepdf | update check |  | 0·0·0 |
| C-platform-002 | Onboarding and help guides | light | ilovepdf | onboarding tips |  | 0·0·0 |
| C-platform-003 | Send feedback or report issue | light | ilovepdf | send feedback |  | 0·0·0 |
| C-platform-004 | Accessibility check and RTL | light | acrobat | accessibility checker |  | 0·0·0 |
| C-platform-005 | Run JavaScript | light | acrobat | JavaScript |  | 0·0·0 |
| C-platform-006 | Manage plugins | light | acrobat | plugins |  | 0·0·0 |
| C-platform-007 | OS automation of PDF actions | light | preview | Shortcuts/AppleScript actions |  | 7·7·9 |
| C-platform-008 | Sign in with an account | light | ilovepdf | account sign-in |  | 0·0·0 |
| C-platform-009 | Command-line interface | light | ours | Sheaf-only CLI with --json |  | 9·8·9 |
| C-platform-010 | MCP server for AI agents | light | ours | Sheaf-only MCP server |  | 8·8·9 |
| C-platform-011 | Open files from the system | strong | ours | open files from Finder/command line |  | 7·7·9 |
| C-platform-012 | Native desktop builds | strong | ours | native builds on three OSes |  | 7·7·9 |
| C-platform-013 | Work offline without account | normal | ours | offline, no account |  | 9·9·9 |
| C-platform-014 | Native menu bar and shortcuts | normal | ours | native menu bar with shortcuts; every inspiration has one |  | 2·4·9 |

### Board · preferences (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-preferences-001 | Open preferences | normal | ilovepdf | preferences page with sections (evidence 07–07f), same model as Sheaf | pdfexpert: ⌘, shortcut | 6·7·9 |
| C-preferences-002 | Choose theme and colours | normal | stirling | light/dark/system theme | preview: follows system | 8·8·9 |
| C-preferences-003 | Choose interface language | normal | ilovepdf | interface language |  | 0·0·0 |
| C-preferences-004 | Set default view on open | normal | acrobat | default zoom and page layout on open | preview: PDF tab: default zoom | 0·0·0 |
| C-preferences-005 | Tune rendering quality | light | acrobat | rendering quality |  | 0·0·0 |
| C-preferences-006 | Set page display options | light | pdfexpert | page display options |  | 0·0·0 |
| C-preferences-007 | Set units and measuring | light | acrobat | units |  | 0·0·0 |
| C-preferences-008 | Set author name | light | acrobat | author name for comments | preview: author in annotations | 0·0·0 |
| C-preferences-009 | Choose default output location | normal | ilovepdf | output location: ask / next to original / fixed |  | 9·8·9 |
| C-preferences-010 | Set default tool options | light | ilovepdf | default tool options |  | 0·0·0 |
| C-preferences-011 | Make it the default PDF app | normal | pdfexpert | set as default PDF app | stirling: default app prompt | 3·5·9 |
| C-preferences-012 | Startup, updates and network | light | ilovepdf | startup and network |  | 0·0·0 |
| C-preferences-013 | Customize tool shortcuts and list | light | stirling | custom shortcuts |  | 0·0·0 |
| C-preferences-014 | Other preference panels | light | acrobat | other panels |  | 0·0·0 |

### Board · reader (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-reader-001 | Open a PDF in the reader | strong | pdfexpert | double-click opens straight into a reading window with the last position; iLovePDF and Stirling open into a tool flow first | ilovepdf: set-as-default-reader prompt | 2·4·6 |
| C-reader-002 | Continuous vertical scroll | strong | pdfexpert | smooth continuous scroll with no blank pages at speed; Preview equal but fewer view options | preview: scroll position kept per document | 5·6·7 |
| C-reader-003 | Horizontal page scrolling | light | pdfexpert | horizontal scroll is one menu choice in View; Acrobat lacks it |  | 0·0·0 |
| C-reader-004 | Auto-scroll the document | light | acrobat | only Acrobat has auto-scroll |  | 0·0·0 |
| C-reader-005 | Zoom in and out | strong | pdfexpert | ⌘+/⌘−, pinch and toolbar zoom, all centred on the pointer; Preview has the same gestures but coarser steps | preview: zoom steps | 5·6·7 |
| C-reader-006 | Set an exact zoom percentage | normal | acrobat | editable zoom box with presets; PDF Expert has presets only | pdfexpert: preset list | 2·5·9 |
| C-reader-007 | Fit whole page to window | strong | pdfexpert | Fit page is a toolbar choice and ⌘9-style shortcut; Acrobat equal, PDF Expert cleaner |  | 0·0·0 |
| C-reader-008 | Fit page width | strong | pdfexpert | fit width keeps the reading line on resize |  | 7·6·8 |
| C-reader-009 | Zoom to actual size | normal | pdfexpert | actual size ⌘0 |  | 3·5·8 |
| C-reader-010 | Gesture zoom and pan | strong | pdfexpert | trackpad pinch and pan feel native |  | 0·0·0 |
| C-reader-011 | Zoom to a selected area | normal | acrobat | marquee zoom tool; Preview's is hidden behind a gesture |  | 0·0·0 |
| C-reader-012 | Magnifier loupe | light | preview | Preview's loupe is the reference |  | 0·0·0 |
| C-reader-013 | Default zoom and remembered position | normal | stirling | default zoom preference; reopening a file returns to the same page and zoom | stirling: default zoom preference | 0·0·0 |
| C-reader-014 | Single page view | normal | pdfexpert | single page / continuous are two clear choices in View |  | 0·0·0 |
| C-reader-015 | Two-page spread view | normal | pdfexpert | two-page spread with continuous option, as in the evidence capture |  | 0·0·0 |
| C-reader-016 | Cover page alone in spreads | light | pdfexpert | cover page option next to two-page |  | 0·0·0 |
| C-reader-017 | Rotate the view | normal | acrobat | rotate view ⇧⌘+/− without changing the file; PDF Expert rotates the page itself |  | 0·0·0 |
| C-reader-018 | Next and previous page | strong | pdfexpert | arrow keys, space, page up/down and toolbar arrows | preview: space / ⇧space paging | 0·0·0 |
| C-reader-019 | First and last page | normal | pdfexpert | Home/End and menu |  | 0·0·0 |
| C-reader-020 | Go to a page by number | strong | pdfexpert | click the page counter and type a number; labels accepted | acrobat: ⇧⌘N Go to page dialog | 0·0·0 |
| C-reader-021 | Back and forward in view history | normal | pdfexpert | ⌘[ / ⌘] back and forward after following a link |  | 0·0·0 |
| C-reader-022 | Show or hide the side panel | normal | pdfexpert | one button toggles the left panel with Thumbnails/Outline/Bookmarks/Annotations tabs |  | 2·4·8 |
| C-reader-023 | Page thumbnails panel | strong | pdfexpert | thumbnails panel with current page ring and live updates | preview: thumbnail size slider | 5·6·7 |
| C-reader-024 | All pages in a grid | light | preview | contact sheet view (Preview only) |  | 0·0·0 |
| C-reader-025 | Browse the document outline | strong | pdfexpert | outline tab, expandable tree, click to jump, current section followed | acrobat: expand/collapse all | 0·0·0 |
| C-reader-026 | Add and edit outline entries | light | pdfexpert | add outline item from a text selection (context menu) | acrobat: rename/drag to reorder | 0·0·0 |
| C-reader-027 | Bookmark pages for yourself | normal | pdfexpert | bookmark ribbon on the page corner, bookmarks tab |  | 0·0·0 |
| C-reader-028 | Highlights and notes sidebar | normal | preview | Highlights and Notes sidebar lists annotations with their text; PDF Expert's Annotations tab is equivalent | pdfexpert: annotations tab filter by type | 0·0·0 |
| C-reader-029 | Show or hide PDF layers | light | acrobat | Layers panel with visibility toggles |  | 0·0·0 |
| C-reader-030 | View and save attachments | normal | acrobat | Attachments panel: open, save |  | 0·0·0 |
| C-reader-031 | Add an attachment | light | stirling | add attachments tool |  | 0·0·0 |
| C-reader-032 | Find text in the document | strong | pdfexpert | ⌘F search with result count and results grouped by page (evidence 09: 'Found: 108') | preview: results list with context | 3·4·6 |
| C-reader-033 | Next and previous search match | strong | pdfexpert | ⌘G / ⇧⌘G and arrows next to the field |  | 0·0·0 |
| C-reader-034 | Search options | normal | acrobat | whole words, case-sensitive | pdfexpert: precise search toggle | 0·0·0 |
| C-reader-035 | Search results list | normal | preview | results list with context snippets | pdfexpert: grouped by page | 2·4·7 |
| C-reader-036 | Search for the selected text | light | pdfexpert | search from the selection's context menu |  | 0·0·0 |
| C-reader-037 | Advanced and multi-file search | light | acrobat | multi-file search (Acrobat only) |  | 0·0·0 |
| C-reader-038 | Find and replace text | light | acrobat | find and replace in Edit mode |  | 0·0·0 |
| C-reader-039 | Hide toolbars for reading | normal | pdfexpert | hide toolbars and panel for reading |  | 0·0·0 |
| C-reader-040 | Full screen mode | normal | pdfexpert | full screen ⌃⌘F |  | 0·0·0 |
| C-reader-041 | Presentation slideshow | light | preview | slideshow |  | 0·0·0 |
| C-reader-042 | Reflow text and change text size | light | acrobat | Reflow (Acrobat only) |  | 0·0·0 |
| C-reader-043 | Dark and sepia reading themes | normal | pdfexpert | View ▸ Theme Day/Night/Sepia, pages recoloured, app chrome unchanged (evidence 11, 11b) | acrobat: custom page/text colours | 0·0·0 |
| C-reader-044 | Rulers, grid and guides | light | acrobat | rulers and grids |  | 0·0·0 |
| C-reader-045 | Split view | normal | pdfexpert | split view of two places or documents |  | 0·0·0 |
| C-reader-046 | Tabs and windows | normal | pdfexpert | document tabs in one window |  | 4·6·8 |
| C-reader-047 | Switch between open documents | normal | pdfexpert | ⌃Tab between tabs |  | 2·5·8 |
| C-reader-048 | Select and copy text | strong | pdfexpert | drag to select text, ⌘C, context menu with Copy/Highlight/Note |  | 0·0·0 |
| C-reader-049 | Select all text | normal | pdfexpert | ⌘A selects page text |  | 0·0·0 |
| C-reader-050 | Select or snapshot an area | normal | pdfexpert | area selection tool copies an image |  | 0·0·0 |
| C-reader-051 | Undo the last action | normal | acrobat | ⌘Z/⇧⌘Z with the undone action named in the Edit menu |  | 0·0·0 |
| C-reader-052 | View document properties | normal | pdfexpert | Document info panel |  | 0·0·0 |
| C-reader-053 | Measure distances and areas | light | acrobat | measure tool with distance/area/perimeter |  | 0·0·0 |
| C-reader-054 | Set measurement scale | light | acrobat | measurement scale |  | 0·0·0 |
| C-reader-055 | Read aloud | light | preview | Speech via the system |  | 0·0·0 |
| C-reader-056 | Read-aloud voice and speed | light | acrobat | voice and speed |  | 0·0·0 |
| C-reader-057 | Follow links | normal | preview | links clickable, hand cursor | pdfexpert: link preview on hover | 0·0·0 |
| C-reader-058 | Highlight links and form fields | light | pdfexpert | highlight links and fields |  | 0·0·0 |
| C-reader-059 | Look up a word | light | pdfexpert | Look Up from context menu |  | 0·0·0 |
| C-reader-060 | Check spelling | light | preview | spelling (system) |  | 0·0·0 |
| C-reader-061 | Page labels | light | pdfexpert | page labels shown in the counter |  | 0·0·0 |
| C-reader-062 | Select and save images in a PDF | light | ilovepdf | select and save images |  | 0·0·0 |
| C-reader-063 | Unlock a protected PDF to view | normal | stirling | unlock to view |  | 6·6·8 |
| C-reader-064 | Preview non-PDF files and portfolios | light | stirling | non-PDF preview |  | 0·0·0 |

### Board · security (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-security-001 | Open the protect tool | normal | acrobat | Protect tool hub |  | 7·7·9 |
| C-security-002 | Protect a PDF with a password | strong | ilovepdf | set password with confirm, AES-256 | acrobat: compatibility level | 7·7·9 |
| C-security-003 | Change a password | normal | pdfexpert | change password |  | 5·6·9 |
| C-security-004 | Set permissions restrictions | normal | acrobat | permissions: printing, copying, editing | stirling: permission checkboxes | 3·5·9 |
| C-security-005 | Open a password-protected PDF | strong | pdfexpert | password prompt on open |  | 6·6·9 |
| C-security-006 | Remove a password | strong | ilovepdf | unlock tool |  | 8·7·9 |
| C-security-007 | Encrypt with certificates | light | acrobat | certificate encryption |  | 0·0·0 |
| C-security-008 | Security policies and settings exchange | light | acrobat | security policies |  | 0·0·0 |
| C-security-009 | Open the redact tool | normal | acrobat | Redact tool with marking, search and apply | stirling: auto redact | 8·7·8 |
| C-security-010 | Mark an area or text for redaction | strong | acrobat | mark text or area for redaction | pdfexpert: redact by text selection | 6·7·8 |
| C-security-011 | Redact whole pages | normal | acrobat | redact pages by range |  | 5·6·8 |
| C-security-012 | Find text and redact | strong | acrobat | find text and patterns, mark all | stirling: regex | 7·7·8 |
| C-security-013 | Review and remove redaction marks | normal | stirling | review list of marks |  | 7·7·8 |
| C-security-014 | Set redaction appearance | light | acrobat | overlay text and colour |  | 0·0·0 |
| C-security-015 | Apply redactions | strong | acrobat | apply with verification, removes hidden info |  | 8·7·8 |
| C-security-016 | Sanitize a document | normal | acrobat | sanitize: metadata, JS, hidden layers | stirling: options per item | 0·0·0 |
| C-security-017 | Edit document metadata | normal | stirling | edit metadata fields | preview: inspector | 0·0·0 |
| C-security-018 | Get full PDF info report | light | stirling | PDF info report |  | 0·0·0 |

### Board · sign (2026-10-20)

| id | component | weight | main | why | take | Sheaf F·A·P |
|---|---|---|---|---|---|---|
| C-sign-001 | Open the sign tool | strong | pdfexpert | Fill & Sign strip in the reader, signature one click away | acrobat: Fill & Sign tool | 5·6·8 |
| C-sign-002 | Draw a signature | strong | pdfexpert | draw with smoothing, colour and thickness | preview: trackpad signing | 6·6·8 |
| C-sign-003 | Type a signature | strong | stirling | type with several fonts | pdfexpert: type option | 6·6·8 |
| C-sign-004 | Signature from image or camera | normal | preview | camera/image with background removal | pdfexpert: image import | 5·6·8 |
| C-sign-005 | Place a signature on the page | strong | pdfexpert | drag to place, corner handles to resize, several signatures per document | ilovepdf: place on several pages | 5·6·8 |
| C-sign-006 | Add initials | normal | acrobat | initials |  | 0·0·0 |
| C-sign-007 | Repeat a signature on several pages | normal | ilovepdf | repeat on pages |  | 0·0·0 |
| C-sign-008 | Delete a placed signature | normal | stirling | delete placed signature | preview: select + Delete | 5·6·8 |
| C-sign-009 | Save and manage signatures | normal | pdfexpert | saved signatures reused across documents | preview: signatures in Keychain | 0·0·0 |
| C-sign-010 | Add marks to fill a form | normal | acrobat | check, cross, dot marks for forms |  | 0·0·0 |
| C-sign-011 | Request signatures from others | normal | acrobat | request signatures (cloud) | stirling: sign request | 2·4·8 |
| C-sign-012 | Sign with a digital certificate | normal | acrobat | certificate signing with appearance, reason, location, timestamp | ilovepdf: digital signature tool | 6·6·8 |
| C-sign-013 | Manage digital IDs | light | acrobat | digital ID manager |  | 0·0·0 |
| C-sign-014 | Certify a document | light | acrobat | certify |  | 0·0·0 |
| C-sign-015 | Time stamp a document | light | acrobat | timestamp |  | 0·0·0 |
| C-sign-016 | Validate signatures | normal | acrobat | signature panel with validity |  | 0·0·0 |
| C-sign-017 | Inspect signature and certificate details | light | acrobat | signature properties |  | 3·5·8 |
| C-sign-018 | Clear a certificate signature | light | acrobat | clear signature |  | 0·0·0 |

<!-- end board decisions -->

## 2026-10-21 · Automated tests and Sol's review (Nassim's request)
**Decision.** Every change is checked by (1) unit tests of the reader's document thread (`reader_doc::tests`),
(2) `dev/scripts/gui-smoke.sh`, which drives the real app under Xvfb and checks the PDFs it writes, and (3) a
review of broad changes by Sol (Codex). Sol's first review found 11 defects, all fixed the same day: save checked
with `check_save` before a file is replaced; one tab per file (loading tabs included) and Save As refused onto a
file open elsewhere; close-after-save and Tools-after-save wait for a save that includes every edit; edits that
name pages wait while a page edit is in flight, and page edits drop search results, selections and drafts;
refused edits leave the undo stack; form changes re-render every page; searches stop when replaced or the tab
closes; editing the search field drops old results; a plain press on a selected thumbnail keeps the selection for
a drag. **Reason.** Nassim: faster and safer than checking screenshots by hand.

## 2026-10-21 · One test suite, the code map, storage (dev skill v3)
**Decision.** `scripts/test.sh` is the only test entry point (layers lint, unit, gui); README, HANDOFF and CI call
only it. `build-safe.sh` now exits with the build's status (it always exited 0, so a failed build looked green).
The code map is `ARCHITECTURE.md`, linked from the README. `dev/scripts/prune-target.py` keeps only the Cargo
artifacts the suite uses. **Alternatives.** Keep three commands (unit via build-safe, gui-smoke, CI's cargo
calls): rejected, the skill's house rule 1. A format layer: rejected for now, the code base was never
rustfmt-formatted (lines past 100 columns) and reformatting everything would bury the history.
**Would change it.** A decision to adopt rustfmt with a project `rustfmt.toml`.

## 2026-10-21 · Printing through CUPS `lp`
**Decision.** The reader prints through `lp` (CUPS, present on macOS and Linux) from an in-app sheet: printer,
copies, pages, fit or actual size, two-sided. **Alternatives.** macOS's native print panel through
NSPrintOperation (PDF Expert's way, with preview): unverifiable here, the Mac is reserved; GTK's print dialog:
Linux only. **Would change it.** A Mac to verify on: then the native panel on macOS, the sheet elsewhere.

## 2026-10-21 · Two PdfCraft crates vendored and patched for large files (lever: engine speed)
**Decision.** `third_party/pdfcraft-cos` (editing cache 1,024 → 65,536 objects) and `third_party/pdfcraft-engine`
(`mark_saved`: light refresh after an incremental save), used through `[patch]`; every other PdfCraft crate stays
on git at the same rev. Measured on a 1,982-page, 70 MB book (`reader_doc::tests::edit_cost_on_a_big_file`):
annotation 2.13 → 0.19 s, undo 2.0 → 0.19 s, open 8.6 → 4.5 s, ⌘S 4.6 → 1.2 s, Save As 13 → ~1.2 s; peak memory over ten
annotations 370 → 465 MB. **Alternatives.** Fork PdfCraft on GitHub, or a PR upstream: both publish under
Nassim's name, asked before doing (an upstream PR would let both folders go). Patch only Sheaf's side: impossible,
the cost is inside `Session::apply` and `mark_saved`. **Would change it.** Upstream taking the changes.

## 2026-10-21 · Save As writes an incremental update
**Decision.** Save and Save As both write the file as opened plus an appended update. A full rewrite took 13 s on a
large book and PdfCraft refuses it for signed files (Save As of a signed PDF failed). **Alternative.** Full rewrite
(compact, history dropped): that is what Compress does. **Would change it.** Nassim wanting Save As to compact.

## 2026-10-21 · Sol's review of 7a246b0..e5c0fc3: 13 defects, all fixed
Signature resize recreated the kept signature (now PdfCraft's ResizeAnnotation scales the strokes); index-naming
edits refused while another edit is on its way (`edits_in_flight`); cropped image moves pass the full box;
two-page "current page" is the row's first page; a % in the author no longer reaches chrono's format; the cos
cache keeps its byte budget (full pass every 1,024 inserts); the text cache is dropped for every changed page;
printing refuses user-password PDFs with a reason; prune-target resolves a relative target dir from the project;
gui-smoke: the print check deletes only the file it made, "remove all" requires the file to be read, search now
checks its result; CI installs the GUI tools (unverified until the first CI run on main or a PR).

## 2026-10-21 · Deviation from dev-skill policy: two-agent approval with Sol
**What.** Until Codex's weekly quota resets (27/10/2026 06:48 UTC), the second agent for the joint 9/10 is an
independent Opus subagent (read-only review, verdict 'C-xxx: AGREE 9' or 'DISAGREE — reason') instead of Sol.
**Why better here.** Sol's quota stands at 89 %, the stop threshold is 90 % and going past 100 % spends paid credits
without Nassim's word; blocking every 9 for six days would stop the board. **Scope.** Components closed until then.
**Ends when.** The quota resets; Sol then re-reviews the components approved this way.

## 2026-10-22 · Marking a long document: page re-listing and cache accounting fixed in the vendored crates
**What.** Numbering the 1,982-page, 70 MB book took 83 s. perf: pdfcraft-edit re-listed the whole page tree after
each page (`page_list(doc)[i]`, O(n²)); pdfcraft-cos's ObjectReader summed its object cache on every read. Now
`reload(doc, &page)` re-reads one page (inherited attributes kept) and the reader keeps a running total. 12.9 s
(debug build of our crates, opt-level 2 dependencies; open + save alone 3.7 s). **Alternatives.** Batch the per-page
writes into one pass (bigger change upstream). **Would change it.** A release-build profile showing the editor
cache's periodic full pass (~15 %) worth an incremental total too.

## 2026-10-22 · Watermark preview: decrypted one-page copies in temporary folders
**What.** The preview copies the first marked page into a one-page PDF in a private temporary folder (0700),
deleted when the tab closes or the file changes. For a protected input the copy is decrypted. **Why.** Opening a
large file for each change would take seconds; one copy makes each redraw a one-page mark and render. **Risk.** A
crash leaves the copy in the temp folder until the system cleans it. **Would change it.** A startup sweep of
`sheaf-wm-*` folders if crashes prove common.
