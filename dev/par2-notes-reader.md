# Round 30, batch reader

- C-reader-037: Search ▸ In a folder… reads every PDF of the folder and its subfolders (the cap of 500 files is
  gone); the sheet says how many pages were found in how many PDFs, and when it stops at 2,000 pages found it says
  so and after how many PDFs. Acrobat's Advanced Search ▸ All PDF documents in a folder.
  (`a_folders_search_reads_every_pdf_and_says_when_it_stops`, smoke 311)
- C-reader-064: a text file (.txt, .md, .csv, .tsv, .json, .xml, .log, .ini) or a picture (PNG, JPEG, TIFF, GIF, BMP)
  opened with Sheaf opens in the reader to browse, as a preview PDF written to Sheaf's own temporary folder (JSON
  indented, tabs as spaces, characters Helvetica lacks as `[U+XXXX]`); the original is only read. Office and HTML
  files still open their conversion tool, as Stirling has no preview for them. Stirling's non-PDF viewer.
  (`text_json_and_pictures_become_a_pdf_to_browse`, `what_is_previewed_and_what_is_too_large`,
  `plan_keeps_existing_pdfs_and_reports_the_rest`, smoke 310)
- C-edit-028: Invert on CMYK JPEGs, with Adobe's inverted `/Decode [1 0 1 0 1 0 1 0]` or none, proved end to end
  on a rendered page: each colour becomes the opposite of what PDFium showed. Stirling's Invert.
  (`a_cmyk_jpeg_is_inverted_as_shown_with_or_without_its_decode`; the code was fixed in daf04f9)
- C-edit-010: a paragraph copied with the Edit tool is pasted in its own font, embedded fonts included: the font
  (descriptor, program, widths, Unicode map) is copied into the target file and the text written with it, its
  lines as they broke, as page text the Edit tool edits again. A paragraph whose font cannot write its own text
  again (mixed fonts, no Unicode map) still falls back to the nearest standard family. PDF Expert's ⌘C/⌘V.
  (`a_pasted_paragraph_keeps_its_embedded_font`)
- C-files-017: on macOS, Preferences ▸ Open PDFs with Sheaf by default also writes the six tools as Finder Quick
  Actions (`~/Library/Services/<tool> with Sheaf.workflow`, right-click ▸ Quick Actions and Services) and refreshes
  the Services list; each runs Sheaf with `--tool=<key>` and the files, as the Linux desktop actions and Windows
  verbs do. iLovePDF's context menu. (`each_menu_tool_is_a_quick_action_finder_shows_on_pdfs`)
- C-files-008 (Stirling's local library) and C-organize-024 (Acrobat's Create PDF Portfolio) were already built in
  round 29 (library.rs, sheaf-core portfolio.rs); never graded since the phase 4 baseline. Checked here with their
  tests and smoke 252 and 253.

## Left, and why

- C-files-017: the Quick Action bundles are checked on Linux (both property lists parse with plistlib, the shell
  command keeps each file one argument); Finder showing them needs a Mac to confirm.
- C-files-018: From scanner on macOS needs ImageCaptureCore through Objective-C, which only a Mac can build and
  test; phone scan works through iLovePDF's cloud (network feature, Nassim's decision). Not done.
- C-files-009 (cloud, network and shared files) and C-annotate-037 (shared review): cloud features, out of scope by
  Nassim's decision. Not done.
- not automatable: Finder lists the Quick Actions — needs macOS.

## New dependencies

None.

## C-reader-010 pinch on a real Mac (28/10, Opus, MBP)

Sol rev32 asked for a real macOS trackpad session. Done with real magnify events posted to WindowServer from the
MBP (pyobjc, CGEvent type 29 = gesture, field 110 = 8 zoom, 113 = scale step, 132 = phase 1/2/4), so they take
the trackpad's path: NSEvent magnify → winit PinchGesture → Slint ScaleRotateGestureHandler → zoom_at. On page
80 of 80 (End, then the pointer at 60 % of the window), slow steps (+6 %) then fast ones (+15 %) from 106 % to
800 %: the page point under the pointer stayed page 80, x 0.5040, y 0.2837–0.2838 at every one of ~25 events
(log of zoom_around's anchor, SHEAF_VIEWER_DEBUG, instrumented copy on the MBP only). Found: in one gesture,
fingers past 800 % then closing waited until the scale came back (1738 % asked, nothing shown). Fixed in
reader.slint (the gesture's start follows it past 10–800 %); test pinching_back_past_the_largest_zoom_shrinks_at_once;
on the MBP, the first step back now goes 800 → 720 %.

## BAMA pass: reader (28/10/2026, Opus)

Apple's macOS design rules applied to the reader screens. Anything that moves or resizes a control is macOS only
(`PlatformBridge.mac`): ~350 smoke steps click at fixed points on Linux. Colours, states and bugs: everywhere.

- SmallButton (reader-parts.slint): `on` for toggles and segments (accent tint 85 %, accent text, never a solid
  fill), `primary` kept for committing buttons (Use, Print, Done, Apply, Save, Close, Clear, Replace); hover and
  press only darken; labels weight 400 except primary (600); `enabled` (Print with no printer: grey, dimmed text);
  `tip` (a shortcut under the button). Every toggle/segment of the options bar, sheets, panel, form, prepress and
  production moved from `primary:` to `on:`. Widths are measured in the demibold face on Linux, so nothing moves
  there; macOS takes the label's own width. **All platforms** (widths: macOS only).
- Toolbar: folds the edit group below 1320 px and shows one page-theme button instead of Day/Sepia/Night (Save was
  cut at 1280 px). **macOS only**; Linux unchanged (1180 px, three buttons).
- Toolbar icons: an active tool's icon in the text colour on its pill (it was accent, a second signal); the side
  panel button no longer shows a pill. **All platforms**.
- Page selection (model.rs, reader.slint marks): the object under the pointer is its own kind (19: 1 px, accent at
  60 % transparency); a selection 2 px accent; handles white 9×9, 1 px accent rim, round, a small shadow.
  **All platforms**. Not done: four handles on any single picked object — only paragraphs, images and comments
  can be pulled; handles on objects that cannot would promise a resize that does nothing.
- Bug (pointer.rs): after an image was pulled or dragged, its outline and selection came back at the old place
  and size until the edit landed (the image list is read again only then). The page's image list is now dropped
  at release and read again when the edit lands. **All platforms**.
- Custom colour dot: the stray "+" beside it is gone; until a colour is picked the dot is an empty ring
  (1.5 px, muted) with a plus icon, same size as the others (model.rs sends a transparent colour then).
  **All platforms**.
- Thumbnails: no red frame or red number; a rounded ground behind the page in view (pill grey) and the selected
  pages (accent 80 % transparent), number in the text colour. Sizes unchanged. **All platforms**.
- Popovers: one PopoverFrame (radius 8, hairline border, shadow 12 px, #00000030 light / #00000080 dark) for the
  edit, zoom, More, layout, view, page, tab order, opacity, font, recent searches and mark menus, the palette,
  the options bar and the selection bar; sheets use Theme.scrim. **All platforms** (no move).
- Search panel: the header rows keep their height (vertical-stretch 0), the results follow the options with no
  empty band. **All platforms** (a real bug).
- Page canvas: grey behind the pages when no backdrop is chosen (#E6E6EB light, #1C1C21 dark), page shadow 6 px
  #00000040. **macOS only**.
- Text format bar: hairlines between font | B I U | lists | alignment | size | more fonts. The font chip's width
  (`preferred-width` read short) is fixed on macOS only: on Linux it would move the colours after it.
  **macOS only**.
- Pen widths (options bar, signature sheet): a short line as thick as the stroke, the one in use on a pill
  (StrokeWidth). Same tile sizes. **All platforms**.
- Note/edit card titles: "Edit text", "Link", "Replace with" (existing translated strings); the shortcut is the
  Done button's tip (⌘↩ on macOS, Ctrl+Enter elsewhere). Titles that are the only cue for a key (reply, form
  field, insert) or an instruction (link target, real length) kept. **All platforms**.
- Print sheet: the preview is a white sheet with a shadow on a grey canvas (**all platforms**); the buttons' row
  runs the card's full width at its bottom and the labels are trailing-aligned at one width (**macOS only**).
- Outline rows: selection and hover inset 6 px, rounded. Comment cards: the colour strip inset and rounded inside
  the card's corners. **All platforms**. Redact panel: "Mark whole page" on its own row, the pages field under
  it (**macOS only**; Linux keeps one row, where smoke 18 reads the list below).

## Redesign W3: toolbar, mode bar, rail, docked options bar, inspector (29/10/2026, Fable)

DESIGN.md §5.2, §5.3, §5.5, DESIGN-PLAN W3. Branch wip/w3-reader (from wip/w1-tokens). W1's widgets and
tokens only (ToolbarButton, ModeSegment, RailButton, TipArea, ResizeHandle, Theme, Space, Motion).

- `reader-toolbar.slint` rewritten to §5.2: 44 tall `chrome`, one hairline. Leading group at x 8 (sidebar,
  all pages = Organize, reading view = toolbar hidden, zoom field 72×28 at 132 with its list, − + capsule at
  208/244, page field "1 / 3" 88×28 at 288); the **mode bar** 7×48 centred on the window and never over the
  sidebar (472–808 at 1280: Annotate 496, Edit 544, Fill & Sign 592, Organize 640, Redact 688, Measure 736,
  Tools 784); trailing group ending 8 from the right ("Aa" page-theme popover 864, search 160×28 at 904, AI
  1072, Share 1112, Print 1152, More 1192, inspector 1236). Below 1180 the − + fold into the zoom list, the page
  field hides (⌥⌘G still works), "Aa" folds into More; below 1024 the segments sit on a 40 pitch and the search
  is 120. Undo / Redo / Save left the toolbar (menu bar, ⌘Z ⌘S); Save, Save as… and the page theme are rows of
  More, Custom tools a row of More tools…; Share is a popover (Email, Copy the file, AirDrop on macOS).
- **Modes** (reader.slint `mode`, per tab): 0 Read … 6 Measure, ⌘1–⌘6; a click on the selected segment, or
  Escape with the Select tool and nothing selected, returns to Read. The mode follows the tool (a letter
  typed, a More row: H puts Annotate on, E Edit, S / F Fill & Sign) and the tool follows the mode (leaving a mode
  resets the tool to Select). Organize = the page grid (`spread == 3`, `toggle-grid`), so More ▸ Page grid and
  the "all pages" toolbar button enter and leave it too. A comment picked while reading switches to Annotate.
- `reader-rail.slint` (new): 44 wide, items 36 on a 40 pitch from y 90, Select first, tips to the right.
  Annotate: Highlight, Underline, Strikethrough, Note, Text box, Pen, Eraser, Shapes, Stamp, Link. Edit: Edit
  text, Add image, Link. Fill & Sign: Sign, Prepare form, Detect fields. Organize: Insert blank, Rotate left /
  right, Duplicate, Delete (`danger`, the one tinted icon). Redact: Redact… (the Redact PDF tool), Sanitize.
  Measure: Rulers, Grid, Loupe, Zoom to area (no measuring tool exists yet; these are the aids that read lengths
  off the page). Scrolls (Flickable) when the window is shorter than its items.
- **Options bar docked** (`reader-options.slint` now a `chrome` Rectangle with a hairline under, placed by
  reader.slint above the pages in the canvas column): y 82–118 at 1280 (69 tall for Shapes' two rows, kept),
  content from x 316 (also without the rail: the bar pads itself so its buttons never move). Shown in a mode,
  or for a form's and a kept signature's actions in Read (a reader fills a
  form without entering a mode). Nothing floats over the pages but the selection bar. Two rules keep the page
  still under the pointer: a comment picked in Read stays in Read (no mode switch, so no bar docks), and the
  Edit tool keeps its bar while objects are picked (the floating bar used to go at the pick).
- `reader-inspector.slint` (new): 260 (220–340, W1's handle on its left edge, double click resets), `window`,
  header 36 with the title (Document / the mode) and ×. Read: Document (title, pages, page, zoom, page theme);
  Annotate: Colour, Stroke, Opacity (the same callbacks as the bar); Organize: "n of m" selected; other modes:
  "Select something on the page to see its properties". ⌃⌘I (Ctrl+Alt+I elsewhere) and the trailing button.
  **Deviation from §5.5:** it does not open by itself when a mode is entered — the canvas would narrow by 260 and
  every page click of the smoke would move; it opens on request (closed by default, as the spec's Read state).
- Side panel default 260 (`Theme.sidebar-default`, `reader_ui::PANEL_WIDTH`), as §6.2 asks; W1 had kept 248.
- Not done in this pass, named for the next: the **Tools popover** (⌘K, 320×480 with Favourites / Recent) — the
  Tools segment opens the catalogue page with its search focused, as the wrench did, so `type "compress"; Return`
  still runs a tool on the document (the tool tab); the tool-in-inspector flow with a pinned Run; the sidebar as
  an overlay at 1024; the menu bar reorder (`mac_menu.rs`, main.slint MenuBar). Measure has no measuring tool.
- Icons added: `panel-right.svg`, `ruler.svg` (Lucide). 28 new texts in fr / de / es / it / pt.

Smoke recalibrated (gui-smoke.sh `ZOOM_X=160 ZOOM_MENU_X=190 WRENCH_X=784`): 1-reader 2b' (the signature's page
geometry with the Fill & Sign rail on: 944/612 px a point from 320,134), 2d and 2-documents 20c and 4-flows 210 (the same
Edit-mode geometry), 3-flows 23 (the search field's text read at x 926), 50 (the sixth dot at 664,100, the palette's orange
at 618,135), 54 (the dialog's Cancel read on its own row: the Edit text card has one too), 61 (the zoomed word measured on
the canvas, not in the thumbnails), 4-flows 84 (the Shapes rows from y 80), 352 (custom tools through More ▸ More tools…),
367 (the AI panel's Translate row found by its Classify button: W1's placeholders are too light for OCR), 2c (the ⇧-square
drag starts at x 340: at 260 it sat on the resize handle and widened the panel for every later step), 6c / 6c'0 / 6c''
(the first rectangle 70 lower, the stamp list's rows at 361,136 / 361,304, the Fill row dots at y 100), 6d (two Escapes back
to Read before measuring the gap at x 767), the Notes filter (rows 28 tall since W1: Stamps at y 334) and the reply's bin
(x 232 in the 260 panel), 300 (OCR may read the Fill chip "Fil"), 1-reader 6g (Fit page row at
y 311), the options-bar clicks of 1-reader 2d/2e/9/10/11 and 2-documents 16 and 3-flows 53/58/63 (y 110 → 100,
x + 44: the bar is docked at 82–118 from x 316), 2-documents 18 (Redact: the Redact segment 688,61 then the
rail's second item 282,148), 4-flows 83 (the Pen in the rail at 282,348), `ai_open` (1090,61), `pdfa_reader_tool`
(the Tools segment, no tooltip hunt), every More menu click 1108 → 1210 (its rows at x 1008 and y 102 + 28k are
2 px under the old ones, inside the same rows).

Screenshots /tmp/f5/W3/ (self-snapshots, Xvfb :134–:136; `*-screen.png` are real captures for the popovers):
reader-read / annotate / annotate-highlight-inspector / organize / redact × light, dark × 1280, 1024;
theme-popover, more-menu, tools (the catalogue page the Tools segment opens).
