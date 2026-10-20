# Reference screenshots: PDF Expert, Preview, iLovePDF

Captured 21/10/2026 on the MBP (macOS, 1920x1080 points, Retina 2x). Window-only captures (`screencapture -l <windowid>`), window at {240,40} size {1440,960}, downscaled to 1600 px wide. Menus are captured as their own window, so they are narrower.

Documents: `/tmp/sheaf-ev/study.pdf` (7 pages, not 12 as briefed; no embedded outline) and `/tmp/sheaf-ev/proposal.pdf` (3 pages).

## pdfexpert/ (unpacked build, free tier, not signed in)

| File | What it shows |
|---|---|
| 01-welcome-home.jpg | Home screen: "Unlock 50+ Premium PDF tools / Start 7-day free trial" banner, Open Files drop zone, Tools for You grid (Edit, Annotate, Manage Pages, Other Files to PDF, Sign PDF, PDF Copilot), empty Recents |
| 02a-study-open-copilot-promo.jpg | study.pdf just opened: two-page view, thumbnails sidebar, "Try PDF Copilot for free / LIMITED OFFER" promo popover |
| 02-study-open-reading.png | Reading view, sidebar hidden, two-page spread at 87 %, Fill & Sign tool strip on the left |
| 03-sidebar-thumbnails.jpg | Left panel, Thumbnails tab |
| 04a-sidebar-outline.jpg | Outline tab, empty state ("Right click on the text and select Add Outline Item…") |
| 04b-sidebar-bookmarks.jpg | Bookmarks tab, empty state; bookmark ribbons appear on every page corner |
| 04c-sidebar-annotations-summary.jpg | Annotations tab, empty state |
| 05-annotate-toolbar.jpg | Annotate mode: vertical tool strip (highlight, underline, strikeout, pen, eraser, text box, shapes, note, stamp, signature, area select) |
| 05b-annotate-paywall-dim-overlay.jpg | Highlight tool clicked: tool shows as selected (yellow A), window dims behind the sign-in dialog |
| 05c-annotate-highlight-paywall-dialog.jpg | Sign-in wall triggered by the highlight tool: "Welcome to PDF Expert / Continue with Google / Apple / email / business account" |
| 06a-text-selection-no-highlight.jpg | After the wall is dismissed the tool deselects; dragging gives only a plain blue text selection on page 1 |
| 06b-selection-context-menu.jpg | Right-click on the selection: Copy, Edit Text, Note, Highlight (5 colours), Underline, Squiggly, Strikeout, Add Outline Item, Look Up, AI Summarize/Explain/Translate, Writing Tools |
| 06c-highlight-signin-wall.jpg | Picking yellow Highlight from that menu shows the same sign-in wall; no highlight is created |
| 07-edit-mode.jpg | Edit PDF mode: tool strip (Text, Image, Link, Redact/header) |
| 07b-edit-text-signin-wall.jpg | Clicking the Edit Text tool shows the sign-in wall |
| 08-page-organiser-grid.png | Page thumbnails grid (organiser) with the page-ops strip (insert, insert from file, rotate, copy, extract, split, delete) |
| 09-search-preload.jpg | Search "preload": "Found: 108", results panel grouped by page, hits highlighted in the pages, "Use Precise Search" tip |
| 10a-file-menu.png | File menu (New, Open, Save, Save As, Save as Flattened, Reduce File Size, Export To, Merge Files, Set Password, Translate PDF, Share, Print…) |
| 10b-edit-menu.png | Edit menu (Undo/Redo, clipboard, Page Resize, Insert Blank Page, Rotate Annotation, Align Annotations, Bookmark/Outline, Page Labels, Find, PDF Copilot, Writing Tools…) |
| 11-theme-night.jpg | View > Theme > Night: pages inverted, app UI unchanged |
| 11b-theme-sepia.jpg | View > Theme > Sepia (theme then reset to Day) |
| 12a-save-as-signin-wall.jpg | File > Save As… shows the sign-in wall (File > Export To > Text… does too) |
| 12b-share-signin-wall.jpg | Toolbar Share button shows the sign-in wall |
| 12c-print-dialog.png | File > Print…: the only output dialog reachable without an account (macOS print sheet + PDF Expert scale options) |

## preview/ (macOS Preview)

| File | What it shows |
|---|---|
| 01-study-open-reading.jpg | study.pdf open, sidebar and markup toolbar hidden |
| 02-sidebar-thumbnails.jpg | View > Thumbnails sidebar |
| 03-sidebar-table-of-contents.jpg | View > Table of Contents: shows only "study.pdf" because the file has no outline |
| 04-markup-toolbar.jpg | Markup toolbar shown under the main toolbar |
| 05-highlight-made.jpg | First sentence of 1.1 selected then Tools > Annotate > Highlight Text: orange highlight, title shows "Edited" |
| 06-tools-menu.png | Tools menu open |
| 07-search-preload.jpg | Search "preload": results sidebar per page (16 matches each, 12 on p.7), "Found on 7 pages", Search Rank / Page Order sort |
| 08-contact-sheet.png | View > Contact Sheet: all 7 pages in a grid, highlight visible on p.1 |
| 09-export-dialog.jpg | File > Export… sheet: Format PDF, Quartz Filter, PDF/A, Linearized, Optimize images, JPEG, Permissions; a tag-suggestion popover covers the "Where" row |

## ilovepdf/ (iLovePDF Desktop, free tier, not logged in)

| File | What it shows |
|---|---|
| 01a-home-default-app-prompt.png | First-launch prompt "Make iLovePDF Desktop my default PDF application" (dismissed with Later; Confirm not clicked) + Rosetta warning banner |
| 01b-home-recents.png | Home / Recents: Open file drop zone, Recent Tools (Compress, Merge), Upgrade to Premium card |
| 01c-tools-grid.png | Tools grid by category: Organize, Optimize, Convert to PDF, Convert from PDF |
| 02a-compress-tool-empty.png | Compress PDF tab, empty, options greyed |
| 02b-compress-file-picker.png | "Please choose a file" picker with proposal.pdf selected |
| 03-compress-file-added-options.png | proposal.pdf added (84.17 KB, 3 pages); options panel: Extreme / Recommended (default) / Low, accessibility-tags note |
| 04-options-extreme-selected.png | Extreme compression selected |
| 05a-save-destination-folder-picker.png | Save step: "Please choose a folder / Select destination folder" picker shown when Compress is clicked |
| 05c-compress-result.png | Result: proposal-compressed.pdf 43.26 KB (-48 %), "1 of 2 free tasks left", Continue to… list, Open folder |
| 06-tasks-history.png | Tasks History: today's task plus one from 15/10/2026, Clear All |
| 07-preferences.png | Preferences > General: language, dark mode toggle, default-app reminder, output folder, processing warning, restore purchases |
| 07b-preferences-tools.png | Preferences > Tools: default compression, PDF/A level, watermark defaults, OCR, convert online |
| 07c-preferences-metadata.png | Preferences > Metadata |
| 07d-preferences-security.png | Preferences > Security |
| 07e-preferences-network.png | Preferences > Network |
| 07f-preferences-printer.png | Preferences > Printer |

## States not reached

- **PDF Expert, 6 (highlight made):** blocked. The highlight tool, the context-menu Highlight, Edit Text, Save As, Export To and Share all open a sign-in wall (Google / Apple / email / business). No account was created. Captured: the walls, the text selection and the context menu.
- **PDF Expert, 7 (edit-text toolbar):** only the Edit-mode tool strip is visible; the text-editing toolbar is behind the sign-in wall.
- **PDF Expert, 12 (export/save dialog):** behind the sign-in wall; the Print sheet was captured instead.
- **PDF Expert, 11:** the app has no dark UI mode in View; only page themes Day / Sepia / Night.
- **iLovePDF processing state:** the task finished too fast to catch a progress view.
- **Preview TOC and PDF Expert outline:** both show empty states because study.pdf has no outline.
