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

## Redesign W4: sidebar, search results, page organiser (29/10/2026, Opus)

DESIGN.md §5.4, §5.6, §5.7, §6.2; DESIGN-PLAN W4. Branch wip/w4-sidebar (from wip/w1-tokens). W1's tokens and
widgets only (SidebarTabs, ToolbarButton, Chip, Checkbox, Button, SortCombo, MenuRow, PopoverFrame, TipArea).

- `ui/screens/reader-panel.slint`, rewritten: SidebarTabs 28 tall at 16 (Thumbnails, Outline, **Bookmarks**,
  Annotations, Attachments, Layers; always six, empty tabs show their empty state), a title row 36 (15/600 at 16, the
  tab's one action 28×28 at the right: + for Outline, Bookmarks, Attachments; ⋯ for Thumbnails, Annotations, Layers),
  a hairline, the content. `panel` keeps its codes (0 Thumbnails, 1 Outline, 2 Annotations, 4 Attachments, 5 Layers)
  plus 6 Bookmarks; 3 (search) is gone.
  - Thumbnails `width − 32` (reader_ui::reader_thumb), 8 above the first, number badge 18 tall 6 under each, 12
    apart (THUMB_TOP/BELOW/GAP, `Viewer::thumb_top/below/gap`; the tools' strip keeps its rhythm). The page in view
    on a `fill-1` ground, selected = 2 px accent frame 3 px out + accent badge. Insertion bar 2 px accent, the
    carried capsule accent.
  - Page actions bar: 52 tall (8 above and below), ToolbarButtons 36 on a 36 pitch from x 8 (centres y 774):
    Rotate left, Rotate right, Insert blank, Duplicate, Extract…, Number pages…, Delete. Under 260 wide the bar
    folds into ⋯: Number pages and Extract first (248: Delete stays at 170, ⋯ at 206), then Delete and
    Duplicate (200: RL RR Insert Duplicate ⋯). The ⋯ menu opens above the bar.
  - Strips over the bar (Number pages, Transitions, Replace's pages, Extract) on Theme tokens with Buttons; their
    first line elides and the last row is fixed, so the smoke clicks fixed places.
  - Outline rows 28 (fill inset 6, radius 6, disclosure 12 at depth × 16, page right `text-2`, hover Rename /
    Delete / ⋯ at 20×20); drag pitch 28. Empty: "Click + to add the page you are on…".
  - Bookmarks (new, PDF Expert's page bookmarks): `ReaderUi::bookmarks`, `nav::bookmark_op` (0 toggle the page in
    view, 1 go, 2 remove), kept in the preferences beside the file (`Prefs::page_bookmarks`, `home::page_bookmarks`),
    never written into the PDF. Each row is titled by the outline heading it falls under, else "Page n".
  - Annotations: filter chips that scroll sideways (Flickable), "n comments" + "By page ⌄" (SortCombo 88–120, the
    chevron never clipped), CommentCards (window + hairline, radius 6, pad 10, 3 px strip inset 6) grouped under
    "Page 3 · 4" (`ReaderAnnotRow::group`, model.rs) when sorted by page. Hide/Show all, Summary, Export, Import,
    Remove all moved to the title's ⋯.
  - Attachments rows 28 (clip, name, size right; hover Save a copy…, Rename, Delete); Layers rows 28 with a Checkbox,
    Show all / Hide all / Reset in ⋯.
- `ui/screens/reader-search.slint` (new): `SearchResults`, the inspector column's Search content: header 36 ("Search",
  "140 results", options, ×), groups "Page 6 · 16" (`trf("Page {} · {}")`), 2-line rows (the match bold in the
  `warning` amber: Slint's styled text has no background tint), the current row on the accent, no-results view with
  the words and a hint, Replace / Replace All pinned at the bottom. Options popup: Match Case, Whole Words, Search in
  Folder…, Recent Searches (each removable, `search_forget`), Clear. `show-options()` for the field's magnifier (W3).
  Searching no longer takes the sidebar over (search.rs).
- **Seam with W3**: until W3's inspector lands, reader.slint hosts `SearchResults` as a 260-wide column (240 under
  1120) at the right of the pages, shown while the search field has run (`search-count != ""`). W3 moves that one
  element into its inspector; the smoke already clicks at x ≥ 1020. The organiser's marks are one element in the
  pages' scroll content (`OrganiserMarks`).
- `ui/screens/reader-organize.slint` + viewer/mod.rs `relayout_grid`: the page grid (spread 3) is the organiser:
  196-wide pages at 100 % (the zoom is the thumb size: 120–320 = 61–163 %), 56 between columns, rows 40 apart under
  the number (8 under the page, 18 tall), 40 margin, left-aligned. Numbers under every page, selected ones framed with
  an accent badge, a 2 px accent insertion bar in the gap's middle (`ReaderState::grid-cells / grid-bar`, replacing
  the old kind-12 rings). Turning the grid on closes the sidebar when it showed Thumbnails; it reopens on exit.
- viewer_ui `resize`: down a column too, a width change keeps the page read at the top (the search column opening
  and closing re-fitted the pages and slid from page 3 to page 2: smoke step 4). The resize test now asserts it.
- RTL: every row of the sidebar is a direction-aware FlexboxLayout (platform test `right_to_left_screens…`).
- Smoke recalibrated (measured on the self-snapshots, OCR where there is a word): 1-reader 3 (bar at y 774: Duplicate
  134, Delete 170, Insert 98; drag 35 px a step: thumbnails are taller), 6c-notes (Annotations 142,106; ⋯ 224,146 ▸
  Hide all 60,178, ▸ `at Remove`; chips dragged slowly to Stamps, first card 120,290, `at Reply`, status list by
  `span Reply`, `at Accepted`), 6c-outline (71,106; + 224,146; rename 120,183); 3-flows 23 (options 1226,102, "fox"
  1100,292, count in the column's header), 25 / 52 (Attachments 177,106; Find 120,187; rows from 219, 28 apart;
  rename before delete: the Find field goes at three files), 30 / 44 (⋯ 206,774 ▸ Extract… 250,706; checkboxes
  23,654 / 686; Extract 196,718), 31 (thumbnails 216 → 316), 32 (142,106), 35 (⋯ ▸ Number pages… 250,734; style list
  120,645 then 40,670; Number 196,715), 38 / 104 (`first_x … 0`: the grid closes the Thumbnails sidebar), 51
  (71,106; + 224,146; rows 183 / 211, ⋯ 228; drag 211 → 170), 68 (`at All` in the column; field 36 above), 87
  (strip's last row y 718), 108 (Layers 212,106; ⋯ ▸ `at Show`), 86 (Outline 71,106; row 183, ⋯ 228), 311 (options ▸
  `at Folder`), 2-documents 20e (page 2's thumbnail at 124,560), 4-flows 230 (`at Fill 1030x40+0+90`: the RTL
  sidebar's tab icons on that row made tesseract read "Fil"). gui-smoke.sh `at` / `span`: with the Thumbnails tab
  shown (`thumbs_side`: the bar's hairline at y 748, left or right to left), a lookup wider than the sidebar takes a
  word in the thumbnail strip only when the area holds no other (the fixtures' 28 pt text is readable in a 216-wide
  thumbnail; a menu over the strip still answers).
- Not done here (other streams' files): ⌥⌘1…6 and Go ▸ Add Bookmark ⌘D (menu bar / reader keys, W3), the thumb
  size slider (inspector, W3: set the grid's zoom), the Organize rail, "n of 7 selected" (W3's inspector),
  rubber-band selection in the grid. At 480 one letter-size thumbnail (448 × 580) fills the column, so "two numbers
  readable" from the plan cannot hold with `width − 32`.
