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

## Page themes: cream Sepia, pictures kept, Night inverts only black and white (29/10/2026, Opus)

Nassim, 29/10: Sepia ("crème") looked like a dull veil; Night is right but must not flip images, nor colours,
"only black and white basically". PDF Expert's Night keeps photos (smart invert); its Sepia tints paper and text.

- Sepia (reader.rs): paper #F8F1E3 (Apple Books' cream, was 244,236,216), ink #4A3B2A (was 91,70,54); contrast
  9.4:1 (test ≥ 7:1). `backdrop(Sepia)` is `None`: the canvas around the pages is the reader's own (or the
  backdrop chosen in Preferences), no more beige 226,216,192. Colours keep their hue as before.
- Night: grey pixels (paper, black and grey text, lines) are inverted as before (paper 30,30,32, ink
  222,222,220); a coloured pixel keeps its own colour. The mix is by chroma (`max - min`): below 24 inverted,
  from 96 kept, smoothstep between, so anti-aliased edges of coloured strokes have no step. Known limit:
  black text under a yellow highlight turns light on the kept yellow (the highlight is kept, its text is grey).
- Pictures: `render_bytes` and the new `render_page_themed` read the page's image objects (top level and inside
  Form XObjects, their matrices applied) under the pdfium lock, map them to render pixels with pdfium's own
  `points_to_pixels` for the same config, and `tint_except` leaves those rectangles as drawn
  (`reader::kept_pictures`). One-bit images (stencil masks, black-and-white scans) are ink: themed.
  A scan is themed whole: when the kept pictures add up to ≥ 90 % of the page (`SCAN_COVER`), nothing is
  kept, as PDF Expert inverts scanned text pages; a full-page photo is themed too then. The picture's bounding
  box is kept, not its clip path: a photo clipped to a circle keeps its corners' paper un-themed.
  Pixels: `points_to_pixels` rounds to whole pixels and left the photo's top and right edges a pixel short (a
  light hairline at night, Greenberg p. 122); the exact map is built from `pixels_to_points` of three bitmap
  corners, inverted, and rounded outwards as pdfium draws the picture over every pixel it touches.
- Speed (debug build, Greenberg's Handbook, 1,982 pages, 1,200 px wide, best of 5): reading the pictures costs
  0.06–0.12 ms a page (0 to 2 images); `tint_except` with a rectangle 100–105 ms against 110–113 ms for the
  whole-page tint (debug, unoptimised sheaf-app; the tint loop was already this cost). Day 90–125 ms, Night
  200–257 ms per page including opening the 1,982-page file each time, before and after alike.
- Tests: reader.rs `sepia_is_a_light_cream_…`, `night_inverts_black_and_white_and_keeps_colours`,
  `pictures_keep_their_pixels_…`, `a_scan_is_themed_whole_…`; renders.rs `themes_keep_photos_and_theme_ink_and_scans`
  (a PDF with a photo, the photo inside a form, a 1-bit mask, and a full-page scan). No GUI smoke step asserted
  theme colours. **All platforms**.
