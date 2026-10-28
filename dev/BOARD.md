# Parity board

Inspirations: Adobe Acrobat (Reader + Pro tools) (), PDF Expert (Readdle) (), iLovePDF Desktop (2.3.0), Stirling-PDF (), macOS Preview ()
331 components · at parity (≥9): 311 · closed 311, open 20
Rendered by `dev/scripts/board.py render` from `dev/board/components.json`. Do not edit by hand.

## Queue (next first)
| # | id | weight | component | main | F · A · P → overall | next |
|---|---|---|---|---|---|---|
| 1 | C-platform-012 | strong | Native desktop builds | ours | 7 · 7 · 9 → **7** | notarisation out of scope (DECISIONS 28/10); regrade the native builds without it |
| 2 | C-optimize-006 | strong | OCR a scanned PDF | ilovepdf | 9 · 9 · 8 → **8** | speed on big scans; deskew |
| 3 | C-reader-010 | strong | Gesture zoom and pan | pdfexpert | 8 · 9 · 9 → **8** | pinch zoom and drag-pan (hand tool / space-drag) |
| 4 | C-files-009 | normal | Cloud, network and shared files | acrobat | 0 · 0 · 0 → **0** | out of local-first scope (needs Nassim) |
| 5 | C-platform-001 | normal | Check for updates | ilovepdf | 0 · 0 · 0 → **0** | light; release channel decision is Nassim's |
| 6 | C-platform-014 | normal | Native menu bar and shortcuts | ours | 2 · 4 · 9 → **2** | menu bar and shortcuts (File/Edit/View/Go/Tools) |
| 7 | C-sign-011 | normal | Request signatures from others | acrobat | 2 · 4 · 8 → **2** | out of local-first scope (cloud); light |
| 8 | C-convert-006 | normal | Create PDF from scanner or screenshot | acrobat | 7 · 9 · 9 → **7** | light |
| 9 | C-preferences-003 | normal | Choose interface language | ilovepdf | 8 · 8 · 8 → **8** | localisation (after core) |
| 10 | C-sign-004 | normal | Signature from image or camera | preview | 8 · 9 · 9 → **8** | background removal |
| 11 | C-annotate-037 | light | Send for shared review | acrobat | 0 · 0 · 0 → **0** | out of local-first scope; light |
| 12 | C-forms-016 | light | Distribute and submit forms | acrobat | 0 · 0 · 0 → **0** | light |
| 13 | C-platform-008 | light | Sign in with an account | ilovepdf | 0 · 0 · 0 → **0** | Sheaf has no account by design |
| 14 | C-sign-015 | light | Time stamp a document | acrobat | 0 · 0 · 0 → **0** | engine timestamp_document |
| 15 | C-organize-011 | light | Insert pages from clipboard or scanner | preview | 6 · 9 · 9 → **6** | light |
| 16 | C-files-018 | light | Scan or capture into a PDF | ilovepdf | 7 · 9 · 9 → **7** | light |
| 17 | C-platform-007 | light | OS automation of PDF actions | preview | 8 · 7 · 8 → **7** | CLI and MCP cover scripting |
| 18 | C-ai-005 | light | Generate podcast or presentation | acrobat | 9 · 8 · 9 → **8** | light |
| 19 | C-ai-006 | light | Group documents in AI spaces | acrobat | 8 · 8 · 8 → **8** | light |
| 20 | C-ai-007 | light | Classify documents with AI | stirling | 9 · 8 · 9 → **8** | light |

## By domain
| domain | components | avg overall | at ≥9 |
|---|---|---|---|
| ai | 7 | 8.6 | 4 |
| annotate | 38 | 8.8 | 37 |
| batch | 9 | 9.0 | 9 |
| compare | 7 | 9.0 | 7 |
| convert | 20 | 8.9 | 19 |
| edit | 29 | 9.0 | 29 |
| files | 19 | 8.4 | 17 |
| forms | 17 | 8.5 | 16 |
| optimize | 15 | 8.9 | 14 |
| organize | 27 | 8.9 | 26 |
| output | 15 | 9.0 | 15 |
| platform | 14 | 6.9 | 9 |
| preferences | 14 | 8.9 | 13 |
| reader | 64 | 9.0 | 63 |
| security | 18 | 9.0 | 18 |
| sign | 18 | 8.1 | 15 |

## Closed
| id | component | overall | last proof |
|---|---|---|---|
| C-ai-001 | Open the AI assistant | 9 | Joint 9: Opus ai (9/9/9, round 33 notes) and Sol AGREE 9 (rev34ai, 28/10); landed 995534f |
| C-ai-002 | Ask AI about the document | 9 | Joint 9: Opus ai (9/9/9, round 33 notes) and Sol AGREE 9 (rev34ai, 28/10); landed 995534f |
| C-ai-003 | Summarize with AI | 9 | Joint 9: Opus ai (9/9/9, round 33 notes) and Sol AGREE 9 (rev34ai, 28/10); landed 995534f |
| C-ai-004 | Translate with AI | 9 | Joint 9: Opus ai (9/9/9, round 33 notes) and Sol AGREE 9 (rev34ai, 28/10); landed 995534f |
| C-annotate-001 | Show annotation toolbar | 9 | Joint 9: Opus (F-014 annotation gaps) and Sol AGREE 9 (re-review b19-annot, 26/10) |
| C-annotate-002 | Highlight selected text | 9 | Highlight: 1,982-page 70 MB book (reader_doc::tests::edit_cost_on_a_big_file): engine 0.19 s per edit (was 2.1 s), drawn at once provisionally; gui-smoke saves and checks the file |
| C-annotate-003 | Underline selected text | 9 | Underline: 1,982-page 70 MB book (reader_doc::tests::edit_cost_on_a_big_file): engine 0.19 s per edit (was 2.1 s), drawn at once provisionally; gui-smoke saves and checks the file |
| C-annotate-004 | Strike through selected text | 9 | Strikethrough: 1,982-page 70 MB book (reader_doc::tests::edit_cost_on_a_big_file): engine 0.19 s per edit (was 2.1 s), drawn at once provisionally; gui-smoke saves and checks the file |
| C-annotate-005 | Propose text insertion or replacement | 9 | Joint 9: Opus (Sol b16b-round12 25/10) and Sol AGREE 9 (re-review b16b-round12, 25/10) |
| C-annotate-006 | Add sticky note | 9 | N, click, type: Note with popup; a click on it opens its text to read and edit (SetAnnotationContents), Notes panel follows (dev/re/evidence/F-002/06, dev/re/evidence/F-002/36) |
| C-annotate-007 | Add text box | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b23-round20a, 26/10) |
| C-annotate-008 | Add callout or speech bubble | 9 | Joint 9: Opus (Round 14) and Sol AGREE 9 (re-review b17-round14, 26/10) |
| C-annotate-009 | Change text annotation font | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-annotate-010 | Draw freehand ink | 9 | Pen with 5 colours and 3 widths, strokes smoothed (Chaikin ×2) as PDF Expert's pen (dev/re/evidence/F-002/37) |
| C-annotate-011 | Erase ink strokes | 9 | Eraser (X, toolbar): drag across drawings, rubbed out along the path (PdfCraft EraseInk; a stroke split or removed), three sizes, one undo step, signatures left alone; gui-smoke erases a pen stroke entirely |
| C-annotate-012 | Draw rectangle or oval | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-annotate-013 | Draw line or arrow | 9 | Joint 9: Opus (round 11, 8d1346d) and Sol AGREE 9 (re-review b15c-round11, 25/10) |
| C-annotate-014 | Draw polygon, polyline or cloud | 9 | Joint 9: Opus (round 8, 17ac802) and Sol AGREE 9 (re-review b14-round8, 25/10) |
| C-annotate-015 | Add loupe or mask | 9 | Joint 9: Sol AGREE 9 (review-opus-edit, 27/10) and Opus edit regraded 9 after smoke 213/214 passed on the round 27 app |
| C-annotate-016 | Place a stamp | 9 | Stamp tool (M): Acrobat's 22 stamps (standard, sign here, dynamic) from a list in the options bar, placed with a click, 160 × 42 pt; moved and resized like other comments; gui-smoke places Approved and a dated Received |
| C-annotate-017 | Create a custom stamp | 9 | Joint 9: Opus (Sol b16a-round12 25/10) and Sol AGREE 9 (re-review b16a-round12, 25/10) |
| C-annotate-018 | Add check, cross or symbol | 9 | Stamp tool list starts with Acrobat's Fill & Sign marks (✓ ✗ • —), placed with a click at 18 pt; gui-smoke places a checkmark |
| C-annotate-019 | Add date stamp | 9 | Joint 9: Opus (F-014 annotation gaps) and Sol AGREE 9 (re-review b19-annot, 26/10) |
| C-annotate-020 | Attach file or audio comment | 9 | Joint 9: Opus (round 8, 17ac802) and Sol AGREE 9 (re-review b14-round8, 25/10) |
| C-annotate-021 | Add link annotation | 9 | reviewer 24/10 on d613100: drag a box, page/URL/mailto target, saved /Link; reader_doc and gui-smoke 19 |
| C-annotate-022 | Change annotation colour | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-annotate-023 | Set stroke, fill and opacity | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27a-round22, 26/10) |
| C-annotate-024 | Edit annotation properties and defaults | 9 | Each tool keeps its colour, width, opacity between sessions; new tools start from defaults; the selection bar's highlight uses the highlighter's colour. Joint 9/10: Sol agreed in round 3 (gui-smoke: Shapes style in the preferences) |
| C-annotate-025 | Delete an annotation | 9 | Click selects (blue outline), Delete or the panel's trash removes, ⌘Z restores (dev/re/evidence/F-002/15) |
| C-annotate-026 | Undo, redo, cut and paste annotations | 9 | Joint 9: Opus (round 25) and Sol AGREE 9 (re-review b30a-round25, 27/10) |
| C-annotate-027 | Rotate an annotation | 9 | Joint 9: Opus (round 25) and Sol AGREE 9 (re-review b30a-round25, 27/10) |
| C-annotate-028 | Align and snap annotations | 9 | Joint 9: Opus (round 25) and Sol AGREE 9 (re-review b30a-round25, 27/10) |
| C-annotate-029 | Remove all annotations | 9 | Notes panel trash: Remove all comments in one step, ⌘Z brings them back (gui-smoke checks the saved file both ways) |
| C-annotate-030 | List comments in a panel | 9 | Notes panel: kind, page, text, then 'author · date' as the file records them (PDF Expert's comment list); click jumps, delete button. Author/date row checked by screenshot (not automatable: reading text off the panel); author written into new annotations checked by gui-smoke |
| C-annotate-031 | Sort and filter comments | 9 | Joint 9: Opus (round 5, 0a37446: Checked filter beside the status filter, reader::tests) and Sol AGREE 9 (re-review b12, 25/10) |
| C-annotate-032 | Hide or show all comments | 9 | Notes panel Hide all / Show all: pages render without comments (form fields still drawn), hidden comments not selectable; gui-smoke samples a stamp hidden and shown |
| C-annotate-033 | Reply to a comment | 9 | Joint 9: Opus (round 6, eca24df) and Sol AGREE 9 (re-review b13a-round6, 25/10) |
| C-annotate-034 | Set comment review status | 9 | Selected comment in Notes: status list (No status, Accepted, Rejected, Cancelled, Completed) as Acrobat's Set Status; shown after the author and date; saved as a status reply (reader_doc::tests::a_review_status_shows_on_its_comment; gui-smoke saves Accepted) |
| C-annotate-035 | Export or summarise comments | 9 | Notes panel: Summary of comments (PDF) and Export comments to XFDF or FDF, as Acrobat (reader_doc::tests::comments_are_exported_and_imported, a_summary_of_comments_is_written). Dialogs not automatable here (portal) |
| C-annotate-036 | Import comments | 9 | Notes panel: Import comments from XFDF or FDF (one undo step; every page renders again); unit test exports from one copy and imports into another |
| C-annotate-038 | Measure distance, perimeter, area | 9 | Joint 9: Opus (Round 15) and Sol AGREE 9 (re-review b18b-round15, 26/10) |
| C-batch-001 | Process many files in one run | 9 | Joint 9: Opus (round 6–7 and earlier, eca24df/70417de) and Sol AGREE 9 (re-review b13c-round6, 25/10) |
| C-batch-002 | Build a multi-step action | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35C, 27/10) |
| C-batch-003 | Run a saved action | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35C, 27/10) |
| C-batch-004 | Import or export an action | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35C, 27/10) |
| C-batch-005 | Use suggested actions | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35C, 27/10) |
| C-batch-006 | Create a custom tool | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35C, 27/10) |
| C-batch-007 | Chain another tool from results | 9 | Continue to… on the result screen chains any tool with the outputs, as iLovePDF; dev/re/evidence/F-001/sheaf-compress-result.png beside dev/re/evidence/ilovepdf/05c-compress-result.png |
| C-batch-008 | Automate a watched folder | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35C, 27/10) |
| C-batch-009 | Minimize or cancel a running task | 9 | Joint 9: Opus (round 6–7 and earlier, eca24df/70417de) and Sol AGREE 9 (re-review b13c-round6, 25/10) |
| C-compare-001 | Choose two documents to compare | 9 | Joint 9: Opus (round 6–7 and earlier, eca24df/70417de) and Sol AGREE 9 (re-review b13c-round6, 25/10) |
| C-compare-002 | Swap the compared files | 9 | Swap the compared files in one click; Joint 9/10: toolbox audit (interim reviewer) against iLovePDF's QML, agreed |
| C-compare-003 | Compare text content | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b23-round20a, 26/10) |
| C-compare-004 | Compare visually by overlay | 9 | Joint 9: Opus (Round 15) and Sol AGREE 9 (re-review b18a-round15, 26/10) |
| C-compare-005 | View documents side by side, synced | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b23-round20a, 26/10) |
| C-compare-006 | Browse differences and summary | 9 | Joint 9: Opus (round 11, 8d1346d) and Sol AGREE 9 (re-review b15c-round11, 25/10) |
| C-compare-007 | Download a change report | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27a-round22, 26/10) |
| C-convert-001 | Open the conversion hub | 9 | Joint 9: Opus (round 6–7 and earlier, eca24df/70417de) and Sol AGREE 9 (re-review b13c-round6, 25/10) |
| C-convert-002 | Convert Office documents to PDF | 9 | Joint 9: Sol (author, batch sol-*, pass 4 grade 9/9/9) and Opus (counterpart review AGREE 9, /tmp/f5/par/review-sol-*-2.txt); landed e83625e, suite ALL GREEN 27/10 |
| C-convert-003 | Convert images to PDF | 9 | iLovePDF's options: Fit / A4 / US Letter, orientation and margin for paper, Merge all images into one PDF (off: a PDF next to each image); PNG JPEG WebP TIF/TIFF BMP GIF; images opened from outside open the tool; JPEGs embedded as they are, EXIF rotation applied. Tests: images (Letter landscape, tif+gif), run, dest, open_with, route_drop; gui-smoke 16. Reviewer (Opus) agreed 9/9/9. |
| C-convert-004 | Create PDF from clipboard | 9 | Joint 9: Opus (round 23) and Sol AGREE 9 (re-review b28a-round23, 26/10) |
| C-convert-005 | Convert a web page or HTML to PDF | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35B, 27/10) |
| C-convert-007 | Convert other formats to PDF | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35B, 27/10) |
| C-convert-008 | Create an empty document | 9 | Joint 9: Opus (rounds 9–10, 8d1346d) and Sol AGREE 9 (re-review b15a-round9-10, 25/10) |
| C-convert-009 | Convert PDF to Word | 9 | Joint 9: Opus (round 6–7, eca24df/70417de) and Sol AGREE 9 (re-review b13b-round6, 25/10) |
| C-convert-010 | Convert PDF to Excel | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35B, 27/10) |
| C-convert-011 | Convert PDF to PowerPoint | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35B, 27/10) |
| C-convert-012 | Export pages as images | 9 | Joint 9: Opus (Round 15) and Sol AGREE 9 (re-review b18a-round15, 26/10) |
| C-convert-013 | Extract embedded images | 9 | Joint 9: Opus (Round 15) and Sol AGREE 9 (re-review b18a-round15, 26/10) |
| C-convert-014 | Export PDF as text or RTF | 9 | Joint 9: Opus (Round 15) and Sol AGREE 9 (re-review b18b-round15, 26/10) |
| C-convert-015 | Export PDF as HTML or XML | 9 | Joint 9: Opus (Round 15) and Sol AGREE 9 (re-review b18b-round15, 26/10) |
| C-convert-016 | Export to PostScript or ebook | 9 | Joint 9: Opus (round 23) and Sol AGREE 9 (re-review b28a-round23, 26/10) |
| C-convert-017 | Set export options | 9 | Joint 9: Opus (round 6–7 and earlier, eca24df/70417de) and Sol AGREE 9 (re-review b13c-round6, 25/10) |
| C-convert-018 | Convert PDF to PDF/A | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35B, 27/10) |
| C-convert-019 | Save as PDF/X or PDF/E | 9 | Joint 9: Opus (round 35, 2472a17) and Sol AGREE 9 (rev37a, 28/10) |
| C-convert-020 | Choose online or offline conversion | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35B, 27/10) |
| C-edit-001 | Enter edit mode | 9 | reviewer 24/10 on d613100: edit group = Edit text, Add image, Link, Redact…, Sign (PDF Expert's strip); fresh captures; fits 1280 px |
| C-edit-002 | Edit existing text in place | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-edit-003 | Add a new text box | 9 | E on empty space adds text with the format row (Sans/Serif/Mono, bold, 9–24 pt, 5 ink colours); gui-smoke checks the text (pdftotext) and the font (pdffonts: Times-Bold) |
| C-edit-004 | Format text font, size and style | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-edit-005 | Change text colour | 9 | Joint 9: Opus (F-014 annotation gaps) and Sol AGREE 9 (re-review b19-annot, 26/10) |
| C-edit-006 | Add an image to a page | 9 | Edit tool: select a page image, corners resize it live keeping proportions (the picture itself follows, one preview in flight), drag moves it live, bar rotates/flips/replaces/deletes, Delete key; Add image and ⌘V paste place it (80 % cap each way) selected; WebP/TIFF/BMP/GIF. gui-smoke 14 on the saved file: resize ratio, flip (red right), rotate right (red down), Delete takes only the selected of two, ⌘V 120×60 then Delete. Reviewer (Opus) agreed 9/9/9 (dev: /tmp/rv/edit006-live.png). |
| C-edit-007 | Move, resize and select page objects | 9 | reviewer 25/10 on 07c9d1c: multi-pick (⇧-click, marquee), drawings (PdfCraft patch) found by hover and picked by press, drag together, nudge, paragraph resize with matching box; gui-smoke 20/20b/20c |
| C-edit-008 | Replace, rotate or edit an image | 9 | Joint 9: Opus (round 8, 17ac802) and Sol AGREE 9 (re-review b14-round8, 25/10) |
| C-edit-009 | Align and arrange objects | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-edit-010 | Cut, copy, paste and delete objects | 9 | Joint 9: Opus (round 34) and Sol AGREE 9 (re-review rev36a, 28/10) |
| C-edit-011 | Undo and redo edits | 9 | Undo/redo ⌘Z ⇧⌘Z: 0.19 s on the 1,982-page book (was 2.0 s); gui-smoke: undo then redo restores a note, undo brings a deleted page back |
| C-edit-012 | Find and replace text | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b23-round20a, 26/10) |
| C-edit-013 | Spell check | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-edit-014 | Save edited PDF | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-edit-015 | Add header and footer | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b23-round20a, 26/10) |
| C-edit-016 | Add page numbers | 9 | Font section (families in their own look, size pt, B/I/U), presets + RRGGBB with swatch, batch range clamped per file; tests/numbers.rs (font, underline, custom colour, clamp, redaction of a font name); 1,982-page book numbered in 12.9 s (was 83 s: O(n²) page re-listing in pdfcraft-edit, per-read cache sum in pdfcraft-cos); joint review round 3 |
| C-edit-017 | Add Bates numbering | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27a-round22, 26/10) |
| C-edit-018 | Add a watermark | 9 | Joint 9: Opus (round 6, eca24df) and Sol AGREE 9 (re-review b13a-round6, 25/10) |
| C-edit-019 | Add or remove a page background | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b23-round20a, 26/10) |
| C-edit-020 | Overlay PDFs | 9 | Joint 9: Opus (round 24) and Sol AGREE 9 (re-review b29a-round24, 27/10) |
| C-edit-021 | Add and edit links | 9 | 6edfb12: Sol AGREE 9 (round 2 25/10); F-013 round 2 |
| C-edit-022 | Manage file attachments | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27a-round22, 26/10) |
| C-edit-023 | Edit bookmarks and table of contents | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27b-round22, 26/10) |
| C-edit-024 | Create article threads | 9 | Joint 9: Sol AGREE 9 (review-opus-edit, 27/10) and Opus edit regraded 9 after smoke 213/214 passed on the round 27 app |
| C-edit-025 | Edit page labels | 9 | Joint 9: Opus (Sol b16c-round13 25/10) and Sol AGREE 9 (re-review b16c-round13, 25/10) |
| C-edit-026 | Insert a blank page while editing | 9 | Pages panel: Insert blank page after the selection; gui-smoke checks an empty page 2 in the saved file and that ⌘Z removes it |
| C-edit-027 | Restrict editing | 9 | Joint 9: Opus (round 6–7 and earlier, eca24df/70417de) and Sol AGREE 9 (re-review b13c-round6, 25/10) |
| C-edit-028 | Adjust or invert colours | 9 | Joint 9: Opus (round 34) and Sol AGREE 9 (re-review rev36a, 28/10) |
| C-edit-029 | Remove images or image background | 9 | Joint 9: Opus (round 25) and Sol AGREE 9 (re-review b30a-round25, 27/10) |
| C-files-001 | Open a PDF from a file picker | 9 | Home ▸ Open file: system dialog (GTK portal on Linux) → the PDF opens in a reader tab (dev/re/evidence/F-002/29, dev/re/evidence/F-002/30) |
| C-files-002 | Drag and drop files to open | 9 | Joint 9: Opus (round 6, eca24df) and Sol AGREE 9 (re-review b13a-round6, 25/10) |
| C-files-003 | Home screen | 9 | Home as iLovePDF's: drop zone with Open file, Recent tools (latest runs' tools, wrapping), Recent files with thumbnails, remove and clear. Joint 9/10: independent Opus reviewer (interim for Sol, see DECISIONS) agreed in round 3 |
| C-files-004 | Reopen a recent file | 9 | Recents with thumbnails reopen a PDF in the reader at the page it was left on (777, dev/re/evidence/F-002/34) |
| C-files-005 | Manage the recent files list | 9 | Recents: × on hover removes one, Clear recents removes all, Tasks History untouched (hidden-recents.json; unit tests). Joint 9/10: Sol built it and set on-screen validation as its condition; Claude reviewed, validated on screen and fixed (removal checked on screen) |
| C-files-006 | Star or pin favourite files | 9 | Joint 9: Opus (Round 17) and Sol AGREE 9 (re-review b20-round17, 26/10) |
| C-files-007 | Search, sort and select listed files | 9 | Joint 9: Opus (Round 17) and Sol AGREE 9 (re-review b20-round17, 26/10) |
| C-files-008 | Local file library | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35C, 27/10) |
| C-files-010 | Documents in tabs or windows | 9 | Tabs drag to reorder (drop line), right-click Close Other/Left/Right, 1px dividers as iLovePDF; only the bar order moves so tab pages keep options and scroll. gui-smoke 13: drag reorders, dragged document keeps Page 4, Close Other Tabs leaves one. Reviewer (Opus) agreed 9/9/9. |
| C-files-011 | Close documents, tabs and windows | 9 | Joint 9: Opus (Round 15) and Sol AGREE 9 (re-review b18a-round15, 26/10) |
| C-files-012 | Browse and open tools | 9 | Tools grid by category with colour tiles, collapsible, sort, search, favourites, recent tools. Joint 9/10: independent visual audit (interim reviewer) against PDF Expert / iLovePDF captures, from gui-smoke screenshots, agreed (vs ilovepdf/01c) |
| C-files-013 | Search tools | 9 | Tools search: ranked (name start, name, description word, letters in order: 'cmprs' finds Compress), results as a list, ↑↓ with the chosen row kept in view, ↩ opens, Escape clears; focused on Tools, + and ⌘F (sheaf_core tests; gui-smoke: 'pdf to', ↓, ↩ opens PDF to Word without clicking the field). Joint 9/10: independent Opus reviewer (interim for Sol, see DECISIONS) agreed in round 3 |
| C-files-014 | Favourite and recent tools | 9 | Star any tool (Tools and Home cards; prefs favourite_tools), Favourites first on Tools; Home's Recent tools from the latest runs. gui-smoke stars on Tools and unstars on Home. Joint 9/10: interim Opus reviewer agreed (round 4) |
| C-files-015 | Manage a tool's input files | 9 | 4e08ff3: Sol AGREE 9 (round 4 25/10); F-013 round 4 |
| C-files-016 | Unlock password-protected input | 9 | Joint 9: Opus (round 6, eca24df) and Sol AGREE 9 (re-review b13a-round6, 25/10) |
| C-files-017 | System integration and default app | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35C, 27/10) |
| C-files-019 | Unreadable file feedback | 9 | files::open_error one wording reader+tools, unit test, gui-smoke damaged PDF refused, app stays open; joint review (commit 43f1fff) |
| C-forms-001 | Fill form fields | 9 | Text fields edited on the field (↩ fills, Tab/⇧Tab to the next text field, check boxes untouched), check boxes, radios, combo lists; values saved and read back with qpdf (dev/re/evidence/F-004/01–04) |
| C-forms-002 | Clear a form | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-forms-003 | Required field warnings | 9 | Joint 9: Opus (Sol b16b-round12 25/10) and Sol AGREE 9 (re-review b16b-round12, 25/10) |
| C-forms-004 | Auto-complete form entries | 9 | Joint 9: Opus (round 23) and Sol AGREE 9 (re-review b28b-round23, 26/10) |
| C-forms-005 | Highlight fillable fields | 9 | 41b1756: Sol AGREE 9 (round 3 25/10); F-013 round 3 |
| C-forms-006 | Enter form editing mode | 9 | Joint 9: Opus (Sol b16a-round12 25/10) and Sol AGREE 9 (re-review b16a-round12, 25/10) |
| C-forms-007 | Auto-detect form fields | 9 | Joint 9: Opus (round 23) and Sol AGREE 9 (re-review b28b-round23, 26/10) |
| C-forms-008 | Add a text field | 9 | Joint 9: Opus (rounds 9–10, 8d1346d) and Sol AGREE 9 (re-review b15a-round9-10, 25/10) |
| C-forms-009 | Add choice fields | 9 | Joint 9: Opus (rounds 9–10, 8d1346d) and Sol AGREE 9 (re-review b15a-round9-10, 25/10) |
| C-forms-010 | Add a button field | 9 | Joint 9: Opus (Sol b16b-round12 25/10) and Sol AGREE 9 (re-review b16b-round12, 25/10) |
| C-forms-011 | Add special fields | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35C, 27/10) |
| C-forms-012 | Edit field properties | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b23-round20a, 26/10) |
| C-forms-013 | Format, validate and calculate fields | 9 | Joint 9: Opus (round 24) and Sol AGREE 9 (re-review b29a-round24, 27/10) |
| C-forms-014 | Move, duplicate and arrange fields | 9 | Joint 9: Opus (round 23) and Sol AGREE 9 (re-review b28b-round23, 26/10) |
| C-forms-015 | Import and export form data | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27b-round22, 26/10) |
| C-forms-017 | Flatten form fields | 9 | Flatten bakes field values into the page: 0 fields left, 'Jane Doe' in the page text (pdftotext) |
| C-optimize-001 | Compress a PDF (levels) | 9 | Joint 9: Opus (round 6–7, eca24df/70417de) and Sol AGREE 9 (re-review b13b-round6, 25/10) |
| C-optimize-002 | See compression savings | 9 | state::compress_note test; joint review (commit 43f1fff) |
| C-optimize-003 | Compress to a target size | 9 | Joint 9: Opus (Built earlier, first joint review (9d585a0)) and Sol AGREE 9 (re-review b25-built, 26/10) |
| C-optimize-004 | Batch compress files | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27b-round22, 26/10) |
| C-optimize-005 | Advanced optimization | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35B, 27/10) |
| C-optimize-007 | Choose OCR language and level | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35B, 27/10) |
| C-optimize-008 | OCR multiple files | 9 | OCR takes several files, per-file progress and cancel (jobs.rs each()); Joint 9/10: toolbox audit (interim reviewer) against iLovePDF's QML, agreed |
| C-optimize-009 | Correct recognized text | 9 | Joint 9: Sol (author, batch sol-*, pass 4 grade 9/9/9) and Opus (counterpart review AGREE 9, /tmp/f5/par/review-sol-*-2.txt); landed e83625e, suite ALL GREEN 27/10 |
| C-optimize-010 | Enhance scanned pages | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27b-round22, 26/10) |
| C-optimize-011 | Repair a damaged PDF | 9 | Joint 9: Opus (round 11, 8d1346d) and Sol AGREE 9 (re-review b15c-round11, 25/10) |
| C-optimize-012 | Run preflight and output preview | 9 | Joint 9: Opus (round 34) and Sol AGREE 9 (re-review rev36a, 28/10) |
| C-optimize-013 | Convert colours or grayscale | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35B, 27/10) |
| C-optimize-014 | Prepare for print production | 9 | Joint 9: Opus (round 35, 2472a17) and Sol AGREE 9 (rev37a, 28/10) |
| C-optimize-015 | Create a full-text index | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35B, 27/10) |
| C-organize-001 | Open page organizer grid | 9 | Joint 9: Opus (round 24) and Sol AGREE 9 (re-review b29a-round24, 27/10) |
| C-organize-002 | Reorder pages by dragging thumbnails | 9 | Drag thumbnails (multi-select) with a drop line, a page-count badge, auto-scroll at the strip's edges; one undo step; gui-smoke reorders and checks the saved order. Joint 9/10: interim reviewer agreed in round 3 (screenshots 02b, 02e, 03b) |
| C-organize-003 | Rotate a page left or right | 9 | Rotate left/right from the strip on the selection or the page shown; thumbnail and page follow (dev/re/evidence/F-003/01) |
| C-organize-004 | Rotate pages by angle and range | 9 | Joint 9: Opus (round 11, 8d1346d) and Sol AGREE 9 (re-review b15c-round11, 25/10) |
| C-organize-005 | Auto-rotate pages by orientation | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35B, 27/10) |
| C-organize-006 | Flip pages horizontally or vertically | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b23-round20a, 26/10) |
| C-organize-007 | Delete pages | 9 | Delete the selected pages (strip, refused for every page), undoable; 6 → 5 pages checked with qpdf |
| C-organize-008 | Remove blank pages automatically | 9 | Joint 9: Opus (round 11, 8d1346d) and Sol AGREE 9 (re-review b15c-round11, 25/10) |
| C-organize-009 | Insert a blank page | 9 | Insert a blank page of the same size after the selection; 4 → 5 pages checked with qpdf |
| C-organize-010 | Insert pages from another file | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b23-round20a, 26/10) |
| C-organize-012 | Replace pages from another PDF | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27b-round22, 26/10) |
| C-organize-013 | Copy or duplicate pages | 9 | Joint 9: Opus (round 10, 8d1346d) and Sol AGREE 9 (re-review b15b-fixes, 25/10) |
| C-organize-014 | Extract pages to a new PDF | 9 | Joint 9: Opus (round 10, 8d1346d) and Sol AGREE 9 (re-review b15b-fixes, 25/10) |
| C-organize-015 | Split by page ranges or split points | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-organize-016 | Split by page count, size or bookmarks | 9 | Joint 9: Opus (round 11, 8d1346d) and Sol AGREE 9 (re-review b15c-round11, 25/10) |
| C-organize-017 | Split a page into parts | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b24-round20b, 26/10) |
| C-organize-018 | Crop pages | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b24-round20b, 26/10) |
| C-organize-019 | Resize or scale pages | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27b-round22, 26/10) |
| C-organize-020 | Reverse or reshuffle page order | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b24-round20b, 26/10) |
| C-organize-021 | Merge files into one PDF | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-organize-022 | Arrange files and pages before merging | 9 | Joint 9: Opus (round 24) and Sol AGREE 9 (re-review b29a-round24, 27/10) |
| C-organize-023 | Name the merged output file | 9 | dest::tests::typed_output_names_are_checked; inline error + disabled action (options.slint); joint 9 with interim reviewer 22/10 (round 1) |
| C-organize-024 | Create a PDF portfolio | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35C, 27/10) |
| C-organize-025 | Impose pages: N-up, booklet, one long page | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b24-round20b, 26/10) |
| C-organize-026 | Set page transitions and page properties | 9 | Joint 9: Opus (round 25) and Sol AGREE 9 (re-review b30b-round25, 27/10) |
| C-organize-027 | Undo and redo page edits | 9 | Page edits (duplicate, delete, rotate, move) are undo steps in the reader's Pages panel; gui-smoke: undo brings the deleted page back, undo again removes the duplicate |
| C-output-001 | Save the document | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-output-002 | Save as a new file | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27c-round22, 26/10) |
| C-output-003 | Choose the output folder | 9 | Joint 9: Opus (round 6–7 and earlier, eca24df/70417de) and Sol AGREE 9 (re-review b13c-round6, 25/10) |
| C-output-004 | Run progress and cancel | 9 | Joint 9: Opus (Round 15) and Sol AGREE 9 (re-review b18b-round15, 26/10) |
| C-output-005 | Results screen | 9 | 6edfb12: Sol AGREE 9 (round 2 25/10); F-013 round 2 |
| C-output-006 | Show the output in its folder | 9 | Open folder on the result screen and per-output file/folder buttons, as iLovePDF; dev/re/evidence/F-001/sheaf-compress-result.png beside dev/re/evidence/ilovepdf/05c-compress-result.png |
| C-output-007 | Print a document | 9 | Joint 9: Opus (Round 14) and Sol AGREE 9 (re-review b17-round14, 26/10) |
| C-output-008 | Print layout options | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35B, 27/10) |
| C-output-009 | Share a document | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35B, 27/10) |
| C-output-010 | Email as attachment | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27c-round22, 26/10) |
| C-output-011 | Export or mail flattened | 9 | Joint 9: Opus (round 24) and Sol AGREE 9 (re-review b29b-round24, 27/10) |
| C-output-012 | Tasks history | 9 | Joint 9: Opus (Round 14) and Sol AGREE 9 (re-review b17-round14, 26/10) |
| C-output-013 | Notify when a task finishes | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27c-round22, 26/10) |
| C-output-014 | Revert, versions and undo history | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b24-round20b, 26/10) |
| C-output-015 | Rename or trash the document | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27c-round22, 26/10) |
| C-platform-002 | Onboarding and help guides | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-platform-003 | Send feedback or report issue | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-platform-004 | Accessibility check and RTL | 9 | Joint 9: Opus (round 32) and Sol AGREE 9 (re-review rev32, 27/10) |
| C-platform-005 | Run JavaScript | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-platform-006 | Manage plugins | 9 | Joint 9: Opus (round 34) and Sol AGREE 9 (re-review rev36a, 28/10) |
| C-platform-009 | Command-line interface | 9 | Help on every option and value (args::help_tests every_option_and_value_has_help, page_number_help_reads_as_written); {page}/{pages} placeholders; joint review |
| C-platform-010 | MCP server for AI agents | 9 | Joint 9: Opus (round 6–7 and earlier, eca24df/70417de) and Sol AGREE 9 (re-review b13c-round6, 25/10) |
| C-platform-011 | Open files from the system | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27c-round22, 26/10) |
| C-platform-013 | Work offline without account | 9 | Stale-grade audit 22/10 (independent reviewer): no network library at run time (reqwest only a Windows build dep of winprint); outgoing URLs only opened by the user (main.rs, reader_ui/pointer.rs) |
| C-preferences-001 | Open preferences | 9 | ⌘, from anywhere (Home, tool tabs, documents, their fields) opens Preferences, leaving full screen and a presentation; Home sidebar entry. gui-smoke: from a tool tab and a document. Joint 9/10: independent Opus reviewer (interim for Sol, see DECISIONS) agreed in round 3 |
| C-preferences-002 | Choose theme and colours | 9 | Joint 9: Opus (round 6, eca24df) and Sol AGREE 9 (re-review b13a-round6, 25/10) |
| C-preferences-004 | Set default view on open | 9 | Joint 9: Opus (round 24) and Sol AGREE 9 (re-review b29b-round24, 27/10) |
| C-preferences-005 | Tune rendering quality | 9 | Joint 9: Opus (round 34) and Sol AGREE 9 (re-review rev36a, 28/10) |
| C-preferences-006 | Set page display options | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-preferences-007 | Set units and measuring | 9 | Joint 9: Opus (round 25) and Sol AGREE 9 (re-review b30b-round25, 27/10) |
| C-preferences-008 | Set author name | 9 | Joint 9: Opus (round 6–7, eca24df/70417de) and Sol AGREE 9 (re-review b13b-round6, 25/10) |
| C-preferences-009 | Choose default output location | 9 | Preferences screenshot beside iLovePDF 07 (cleaner, more choices); dest.rs test; smoke Compress next to original; joint review |
| C-preferences-010 | Set default tool options | 9 | Joint 9: Opus (round 32) and Sol AGREE 9 (re-review rev32, 27/10) |
| C-preferences-011 | Make it the default PDF app | 9 | Joint 9: Opus (round 24) and Sol AGREE 9 (re-review b29b-round24, 27/10) |
| C-preferences-012 | Startup, updates and network | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-preferences-013 | Customize tool shortcuts and list | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-preferences-014 | Other preference panels | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-reader-001 | Open a PDF in the reader | 9 | PDFs open in document tabs from Open, drop, recents and the system; pages in 1.3 s on 1,982 pages; password, unsaved prompt. Joint 9/10: independent visual audit (interim reviewer) against PDF Expert / iLovePDF captures, from gui-smoke screenshots, agreed |
| C-reader-002 | Continuous vertical scroll | 9 | Continuous scroll; keyboard: arrows (row by row with two pages), Space / ⇧Space, PgUp/PgDn, Home/End (gui-smoke reads the page in view after each); 1,982-page book scrolls with renders on demand |
| C-reader-003 | Horizontal page scrolling | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-reader-004 | Auto-scroll the document | 9 | Joint 9: Opus (Built earlier, first joint review (9d585a0)) and Sol AGREE 9 (re-review b25-built, 26/10) |
| C-reader-005 | Zoom in and out | 9 | Zoom 25–400 % in actual-size steps like PDF Expert, ⌘+/⌘−, ⌘-wheel, toolbar (dev/re/evidence/F-002/26); pages re-render 150 ms after the zoom settles |
| C-reader-006 | Set an exact zoom percentage | 9 | Zoom field in the toolbar: type a percentage (↩), or pick 50–400 %, Fit width, Fit page from its list, as PDF Expert's zoom menu; actual-size percentages (reader::tests::zoom_typed_by_the_user; gui-smoke types 200 then picks Fit page) |
| C-reader-007 | Fit whole page to window | 9 | 6edfb12: Sol AGREE 9 (round 2 25/10); F-013 round 2 |
| C-reader-008 | Fit page width | 9 | ⌘9 / toolbar fit width (168 %), kept when the window or panel changes size (dev/re/evidence/F-002/26, dev/re/evidence/F-002/28) |
| C-reader-009 | Zoom to actual size | 9 | ⌘0 zoom to 100 % actual size, PDF Expert's shortcut (dev/re/evidence/F-002/26) |
| C-reader-011 | Zoom to a selected area | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b24-round20b, 26/10) |
| C-reader-012 | Magnifier loupe | 9 | Joint 9: Opus (round 24) and Sol AGREE 9 (re-review b29b-round24, 27/10) |
| C-reader-013 | Default zoom and remembered position | 9 | Joint 9: Opus (rounds 9–10, 8d1346d) and Sol AGREE 9 (re-review b15a-round9-10, 25/10) |
| C-reader-014 | Single page view | 9 | Joint 9: Opus (round 23) and Sol AGREE 9 (re-review b28c-round23, 26/10) |
| C-reader-015 | Two-page spread view | 9 | Joint 9: Opus (round 6, eca24df) and Sol AGREE 9 (re-review b13a-round6, 25/10) |
| C-reader-016 | Cover page alone in spreads | 9 | Two pages with the cover alone in the right column, then pairs (Acrobat's and PDF Expert's book mode); unit-tested layout, gui-smoke cycles through it |
| C-reader-017 | Rotate the view | 9 | Joint 9: Opus (round 24) and Sol AGREE 9 (re-review b29b-round24, 27/10) |
| C-reader-018 | Next and previous page | 9 | ← → page, PgUp/PgDn/space screen, scroll by arrows (GUI 21/10) |
| C-reader-019 | First and last page | 9 | Home/End and ⌘↑/⌘↓ to first/last page (End → 1982/1982, dev/re/evidence/F-002/25) |
| C-reader-020 | Go to a page by number | 9 | Page field: number or label ↵ jumps (420), field returns to the page shown; history recorded (dev/re/evidence/F-002/22) |
| C-reader-021 | Back and forward in view history | 9 | Joint 9: Opus (Sol b16c-round13 25/10) and Sol AGREE 9 (re-review b16c-round13, 25/10) |
| C-reader-022 | Show or hide the side panel | 9 | Joint 9: Opus (round 6, eca24df) and Sol AGREE 9 (re-review b13a-round6, 25/10) |
| C-reader-023 | Page thumbnails panel | 9 | Pages tab: 150 px thumbnails like PDF Expert's, page number, current ringed, click jumps, lazy renders (dev/re/evidence/F-002/11, dev/re/evidence/F-002/18) |
| C-reader-024 | All pages in a grid | 9 | Joint 9: Opus (round 8, 17ac802) and Sol AGREE 9 (re-review b14-round8, 25/10) |
| C-reader-025 | Browse the document outline | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b24-round20b, 26/10) |
| C-reader-026 | Add and edit outline entries | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27c-round22, 26/10) |
| C-reader-027 | Bookmark pages for yourself | 9 | Bookmark the page in view with + Bookmark; stored in the PDF's outline, so other readers see it; Outline panel: + Bookmark (the page in view, in page order), rename in place (double-click or pencil; ↩ / Escape), delete (with what is under it); PdfCraft Add/Rename/DeleteBookmark (reader_doc::tests::bookmarks_are_added_renamed_and_deleted; gui-smoke adds and renames one) |
| C-reader-028 | Highlights and notes sidebar | 9 | Joint 9: Opus (round 10, 8d1346d) and Sol AGREE 9 (re-review b15b-fixes, 25/10) |
| C-reader-029 | Show or hide PDF layers | 9 | Joint 9: Opus (round 24) and Sol AGREE 9 (re-review b29b-round24, 27/10) |
| C-reader-030 | View and save attachments | 9 | Joint 9: Opus (rounds 9–10, 8d1346d) and Sol AGREE 9 (re-review b15a-round9-10, 25/10) |
| C-reader-031 | Add an attachment | 9 | Files panel ▸ Attach…: a file attached to the page in view (Acrobat's Attach File comment), listed at once; unit-tested |
| C-reader-032 | Find text in the document | 9 | Joint 9: Opus (Sol b16c-round13 25/10) and Sol AGREE 9 (re-review b16c-round13, 25/10) |
| C-reader-033 | Next and previous search match | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-reader-034 | Search options | 9 | Aa / Whole words toggles re-run the search (dev/re/evidence/F-002/12) |
| C-reader-035 | Search results list | 9 | 41b1756: Sol AGREE 9 (round 3 25/10); F-013 round 3 |
| C-reader-036 | Search for the selected text | 9 | Selection bar Search and ⇧⌘E: the selection searched, results listed; shows the toolbar again if hidden; bar kept inside the view. Joint 9/10: Claude built it, Sol agreed after its second review round (gui-smoke: page 4's text searched from page 1) |
| C-reader-037 | Advanced and multi-file search | 9 | Joint 9: Opus (round 32) and Sol AGREE 9 (re-review rev32, 27/10) |
| C-reader-038 | Find and replace text | 9 | Joint 9: Opus (round 23) and Sol AGREE 9 (re-review b28d-round23, 26/10) |
| C-reader-039 | Hide toolbars for reading | 9 | ⇧T or More ▸ Hide toolbar: toolbar and panel hidden for reading, kept hidden through full screen; Escape restores. Joint 9/10: Sol agreed (round 5); gui-smoke checks hide, restore, and full screen keeping it hidden |
| C-reader-040 | Full screen mode | 9 | ⌃⌘F and More ▸ Full screen; Escape leaves it and keeps a hidden toolbar hidden. Joint 9/10: Sol agreed (round 4). Window state itself not checkable without a window manager here |
| C-reader-041 | Presentation slideshow | 9 | Presentation (⇧⌘P, More menu): full screen, no toolbar, one page at a time fitted; Space, arrows and Page keys turn whole pages; Escape ends and restores the view and two-page mode. Joint 9/10: Sol agreed (round 7); gui-smoke: Space turns the page while presenting |
| C-reader-042 | Reflow text and change text size | 9 | Joint 9: Opus (round 23) and Sol AGREE 9 (re-review b28d-round23, 26/10) |
| C-reader-043 | Dark and sepia reading themes | 9 | Day / Sepia / Night on pages only, remembered, Night keeps hues (yellow highlight, green headings) (dev/re/evidence/F-002/08, dev/re/evidence/F-002/18); test themes_map_white_* |
| C-reader-044 | Rulers, grid and guides | 9 | Joint 9: Opus (round 25) and Sol AGREE 9 (re-review b30c-round25, 27/10) |
| C-reader-045 | Split view | 9 | Joint 9: Opus (round 32) and Sol AGREE 9 (re-review rev32, 27/10) |
| C-reader-046 | Tabs and windows | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27d-round22, 26/10) |
| C-reader-047 | Switch between open documents | 9 | ⇧⌘[ / ⇧⌘] previous / next tab (PDF Expert's shortcut), wraps around |
| C-reader-048 | Select and copy text | 9 | Drag selects in reading order (also over a highlight), double-click word, ⌘A page, ⌘C → clipboard verified with xclip (dev/re/evidence/F-002/03, dev/re/evidence/F-002/19); test a_drag_selects_* |
| C-reader-049 | Select all text | 9 | ⌘A selects the page's text (PDF Expert: same), ⌘C copies; gui-smoke checks the clipboard |
| C-reader-050 | Select or snapshot an area | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27d-round22, 26/10) |
| C-reader-051 | Undo the last action | 9 | ⌘Z / toolbar undo for every reader edit (markup, notes, pages, forms); gui-smoke undo/redo checks; 0.19 s on 1,982 pages |
| C-reader-052 | View document properties | 9 | 4e08ff3: Sol AGREE 9 (round 4 25/10); F-013 round 4 |
| C-reader-053 | Measure distances and areas | 9 | Joint 9: Opus (round 10, 8d1346d) and Sol AGREE 9 (re-review b15b-fixes, 25/10) |
| C-reader-054 | Set measurement scale | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27d-round22, 26/10) |
| C-reader-055 | Read aloud | 9 | Joint 9: Opus (round 25) and Sol AGREE 9 (re-review b30c-round25, 27/10) |
| C-reader-056 | Read-aloud voice and speed | 9 | Joint 9: Opus (round 23) and Sol AGREE 9 (re-review b28d-round23, 26/10) |
| C-reader-057 | Follow links | 9 | Internal and web links followed, ⌘[ ⌘] history; hovering shows where a link goes, cleared on scroll and kept in view. Joint 9/10: Sol agreed in its third review round (gui-smoke: tooltip shown, then gone after scrolling) |
| C-reader-058 | Highlight links and form fields | 9 | More ▸ Highlight links (⇧⌘L) tints every link; fillable fields tinted in Select mode, as PDF Expert. Joint 9/10: Sol agreed (round 4); gui-smoke samples the tint |
| C-reader-059 | Look up a word | 9 | Joint 9: Opus (round 23) and Sol AGREE 9 (re-review b28d-round23, 26/10) |
| C-reader-060 | Check spelling | 9 | Joint 9: Opus (round 34) and Sol AGREE 9 (re-review rev36a, 28/10) |
| C-reader-061 | Page labels | 9 | Joint 9: Opus (round 6, eca24df) and Sol AGREE 9 (re-review b13a-round6, 25/10) |
| C-reader-062 | Select and save images in a PDF | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27d-round22, 26/10) |
| C-reader-063 | Unlock a protected PDF to view | 9 | 4e08ff3: Sol AGREE 9 (round 4 25/10); F-013 round 4 |
| C-reader-064 | Preview non-PDF files and portfolios | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35A, 27/10) |
| C-security-001 | Open the protect tool | 9 | Joint 9: Opus (round 11, 8d1346d) and Sol AGREE 9 (re-review b15c-round11, 25/10) |
| C-security-002 | Protect a PDF with a password | 9 | Protect: strength meter (weak/fair/strong from length and character classes, unit-tested) and show/hide on every password field. Joint 9/10: Sol built it and set on-screen validation as its condition; Claude reviewed, validated on screen and fixed (eye-off icon was a bare slash) |
| C-security-003 | Change a password | 9 | Joint 9: Opus (round 6–7, eca24df/70417de) and Sol AGREE 9 (re-review b13b-round6, 25/10) |
| C-security-004 | Set permissions restrictions | 9 | 4e08ff3: Sol AGREE 9 (round 4 25/10); F-013 round 4 |
| C-security-005 | Open a password-protected PDF | 9 | A protected PDF asks for its password in the reader; a wrong one says so, the right one opens it (dev/re/evidence/F-002/35) |
| C-security-006 | Remove a password | 9 | Unlock: password field with reveal, wrong-password feedback, batch; Joint 9/10: toolbox audit (interim reviewer) against iLovePDF's QML, agreed |
| C-security-007 | Encrypt with certificates | 9 | Joint 9: Opus (round 34) and Sol AGREE 9 (re-review rev36a, 28/10) |
| C-security-008 | Security policies and settings exchange | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35C, 27/10) |
| C-security-009 | Open the redact tool | 9 | Joint 9: Opus (round 6–7 and earlier, eca24df/70417de) and Sol AGREE 9 (re-review b13c-round6, 25/10) |
| C-security-010 | Mark an area or text for redaction | 9 | reviewer 24/10 on 42fcabb: live selection, caret ends, word/line clicks, result counts text passages; output scrubbed (fiche); gui-smoke 18 |
| C-security-011 | Redact whole pages | 9 | Joint 9: Opus (round 6–7, eca24df/70417de) and Sol AGREE 9 (re-review b13b-round6, 25/10) |
| C-security-012 | Find text and redact | 9 | 85666fe: Sol AGREE 9 (round 1 25/10, /tmp/f5/solrv/b8-round1.out); see dev/features/F-011..F-013 |
| C-security-013 | Review and remove redaction marks | 9 | Joint 9: Opus (Round 19) and Sol AGREE 9 (re-review b22-round19, 26/10) |
| C-security-014 | Set redaction appearance | 9 | Joint 9: Opus (round 24) and Sol AGREE 9 (re-review b29b-round24, 27/10) |
| C-security-015 | Apply redactions | 9 | Joint 9: Opus (Round 15) and Sol AGREE 9 (re-review b18a-round15, 26/10) |
| C-security-016 | Sanitize a document | 9 | Joint 9: Opus (Round 15) and Sol AGREE 9 (re-review b18a-round15, 26/10) |
| C-security-017 | Edit document metadata | 9 | Joint 9: Opus (Round 15) and Sol AGREE 9 (re-review b18b-round15, 26/10) |
| C-security-018 | Get full PDF info report | 9 | Joint 9: Opus (round 22) and Sol AGREE 9 (re-review b27d-round22, 26/10) |
| C-sign-001 | Open the sign tool | 9 | Sign tool (S) in the reader toolbar, PDF Expert's Fill & Sign place; gui-smoke.sh 'a drawn signature is placed and saved' PASS |
| C-sign-002 | Draw a signature | 9 | Pad with 3 inks and 3 widths shown while drawing, smoothed strokes; up to 4 kept signatures with their own pen, picked or forgotten from thumbnails in their ink; gui-smoke checks ink and width in the saved file. Joint 9/10: interim reviewer agreed in round 3 (screenshots 02b, 02e, 03b) |
| C-sign-003 | Type a signature | 9 | Four handwriting fonts (Dancing Script, Great Vibes, Satisfy, Caveat; sheaf-core/src/script.rs, Cyrillic/Greek fallback), live 2×2 previews on the pad, kept per signature, Sign tool and CLI --script-font; script::tests, tests/sign.rs a_typed_name_takes_its_handwriting_font + same ink visual/digital for every font, history compat test, gui-smoke typed signature in Great Vibes; Stirling offers no script fonts; joint review 2 rounds |
| C-sign-005 | Place a signature on the page | 9 | Select a placed signature, drag its corner handle: scales from its top-left with proportions kept (pull projected on the diagonal); several placements per run with the kept signature. gui-smoke checks the saved Ink Rect; dev/re/evidence/F-002/41-signature-resized.png |
| C-sign-006 | Add initials | 9 | Joint 9: Opus (Built earlier, first joint review (9d585a0)) and Sol AGREE 9 (re-review b25-built, 26/10) |
| C-sign-007 | Repeat a signature on several pages | 9 | Joint 9: Opus (round 32) and Sol AGREE 9 (re-review rev32, 27/10) |
| C-sign-008 | Delete a placed signature | 9 | Click a placed signature, Delete (or the Notes panel's delete): gone from the saved file (gui-smoke) |
| C-sign-009 | Save and manage signatures | 9 | Joint 9: Sol (author, batch sol-*, pass 4 grade 9/9/9) and Opus (counterpart review AGREE 9, /tmp/f5/par/review-sol-*-2.txt); landed e83625e, suite ALL GREEN 27/10 |
| C-sign-010 | Add marks to fill a form | 9 | 41b1756: Sol AGREE 9 (round 3 25/10); F-013 round 3 |
| C-sign-012 | Sign with a digital certificate | 9 | Joint 9: Sol (author, batch sol-*, pass 4 grade 9/9/9) and Opus (counterpart review AGREE 9, /tmp/f5/par/review-sol-*-2.txt); landed e83625e, suite ALL GREEN 27/10 |
| C-sign-013 | Manage digital IDs | 9 | Joint 9: Opus (round 34) and Sol AGREE 9 (re-review rev36a, 28/10) |
| C-sign-014 | Certify a document | 9 | Joint 9: Opus (round 24) and Sol AGREE 9 (re-review b29b-round24, 27/10) |
| C-sign-016 | Validate signatures | 9 | Joint 9: Opus (rounds 28-33) and Sol AGREE 9 (re-review rev35C, 27/10) |
| C-sign-017 | Inspect signature and certificate details | 9 | Joint 9: Opus (Rounds 20–21 (9d585a0)) and Sol AGREE 9 (re-review b24-round20b, 26/10) |
| C-sign-018 | Clear a certificate signature | 9 | Joint 9: Opus (round 23) and Sol AGREE 9 (re-review b28d-round23, 26/10) |
