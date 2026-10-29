# Sheaf design implementation plan

The spec is `docs/DESIGN.md`. This plan cuts it into five workstreams with disjoint file
ownership so they can run in parallel worktrees, lists what each must show on a screenshot to be
done, and names the GUI smoke steps each one breaks so the stream recalibrates them in the same
unit of work. No stream touches another's files; shared needs go through W1, which lands first.

Conventions for every stream: tokens only (no raw colours, radii, durations outside
`theme.slint`); every icon-only control has `accessible-label` and a tooltip through
`TooltipBridge`; light and dark snapshots at 1280×800 and 1024×768 before the PR; the one test
suite (`bash scripts/test.sh`) green, the smoke steps the stream owns recalibrated by measuring
the new screenshots (or switched to `at`/`span` OCR lookups when the target has a word).

## Order and dependencies

```
W1 tokens + widgets + tooltip layer + panel handle      (first; everyone imports it)
 ├── W2 Home / library                                   (after W1 merges)
 ├── W3 reader chrome: toolbar, modes, options bar, inspector, Tools popover
 ├── W4 sidebar, search results, page organiser          (after W1; W3 and W4 share no file)
 └── W5 Settings window, sheets, dialogs, toasts         (after W1)
```

W2–W5 are independent of each other. W3 and W4 both change the reader screen but own different
files: W3 owns `reader.slint`'s layout (columns, options bar, inspector) and the toolbar; W4 owns
the sidebar's content and the organiser grid. The seam is `ReaderBridge.panel-width` (W1 adds
`sidebar-min/max` and the handle) and the column rects W3 exposes as `in` properties.

Estimated size: W1 1 day, W2 1 day, W3 3 days, W4 2 days, W5 2 days. Smoke recalibration is
inside each estimate.

## W1 — Tokens, widgets, tooltip layer, side-panel width

**Owns:** `ui/theme.slint`, `ui/widgets/*.slint` (all), `ui/widgets/tooltip.slint` (new),
`ui/screens/reader-parts.slint` (ToolButton, SmallButton, PopoverFrame, Divider),
`src/platform_ui.rs` (system accent, reduce-motion flag), `src/icons.rs`.
In `ui/main.slint` only the last child `TooltipLayer {}` and the `TooltipBridge` import (two
lines; W2–W5 do not edit those lines).

**Does:**

1. Rewrite `Theme` to the token table of DESIGN §4: colour roles (`window`, `chrome`, `canvas`,
   `fill-1..3`, `hairline`, `text`, `text-2`, `text-3`, `accent`, `accent-text`, `on-accent`,
   status, `tooltip`, `toast`), type scale (`fs-large-title`… `fs-caption`), `Space`
   (4…32), radii, shadows, `Motion` durations (all 0 under reduce motion). Keep the old names as
   aliases for one release so W2–W5 can migrate screen by screen; delete the aliases in W1's
   last commit after the others merge.
2. Accent default: system on macOS, blue `#1F6FEB` elsewhere; the swatch list reordered (system,
   blue, red, green, purple, orange, graphite).
3. Widgets to the component inventory (DESIGN §4): `ToolbarButton`, `ModeSegment`, `RailButton`,
   `SidebarTab`, `Button` (secondary / primary / destructive / large), `TextField`,
   `PopupButton`, `Checkbox`, `Radio`, `Switch`, `Segment`, `Chip`, `Swatch`, `Popover`,
   `ContextMenu`, `Sheet`, `Toast`, `FileCard`, `ToolTile`, `DropArea`, `TabItem`. Sizes and
   states as the table. Every one has hover, pressed, focus-ring (keyboard only), disabled.
4. **Tooltip layer** (`ui/widgets/tooltip.slint`): `TooltipBridge` global (`hover(rect, text,
   shortcut)`, `leave()`, `cancel()`), `TooltipLayer` component drawn last in `AppWindow`.
   Timing 600 / 300 warm / 60 ms; placement below, flip above when it would pass `H − 8`, right
   of rail anchors with a left flip, clamp x to `[8, W − w − 8]`, max width 280, wrap. Every
   `tip:` property in the codebase is rerouted: the widget calls the bridge and draws nothing.
   Keyboard focus shows the tip too.
5. **Side-panel width**: `Theme.sidebar-min 200`, `sidebar-default 260`, `sidebar-max 480`
   (and ≤ 45 % of the window); `inspector-min 220 / 260 / 340`. A `ResizeHandle` widget (8 wide
   hit area, `col-resize` cursor, hairline turns accent on hover/drag, double-click resets),
   used by W3 for both columns. Width persisted per window (`src/prefs.rs` key, W1 adds it).
6. Tab strip (`widgets/tab-bar.slint`): 38 tall `chrome`, tabs max 240 min 120, Home pinned
   first with its icon, × on hover, selected tab `window` with hairline sides.

**Screenshot acceptance:**
- A gallery screen (`SHEAF_GALLERY=1`, debug builds) shows every widget in every state, light
  and dark, with an 8 px grid overlay; every edge sits on the grid.
- Hovering the sidebar's trash icon (bottom bar) shows its tip **above** the icon, fully inside
  the window; hovering the last toolbar button shows a tip clamped 8 px from the right edge;
  hovering a rail item shows the tip to its right; no tip is ever covered by a divider, a panel
  edge or a popover.
- The sidebar dragged to 150 stops at 200 with every tab icon and the ⋯ visible; dragged to 600
  stops at 480; double-click on the handle returns 260.

**Smoke impact:** `1-reader.sh` step 6i (a link's dark tooltip at the pointer: the tip now comes
from the layer; the OCR crop may move by the new placement), `3-flows.sh` step 31 (the side
panel's edge at (247,400) dragged 100 px: the handle is now centred on x 260, the width after
the drag is 360 not 347). Tab positions: steps 8, 12, 13 (`click 230 21`, `ntabs`, tab drag
`FX0`): the tab strip height stays 38 and the first document tab starts after the 120-wide Home
tab, so `230 21` still hits it; verify. Everything that clicks `click 50 21` / `click 40 21`
(Home tab) stays.

## W2 — Home / library

**Owns:** `ui/screens/recents.slint`, `tools.slint`, `catalog.slint`, `library.slint`,
`history.slint`, `actions.slint`, `portfolio.slint`, `result.slint`, `tool-flow.slint`,
`options.slint`, `options-parts.slint`, `processing.slint`; `src/home.rs`, `library_ui.rs`,
`actions_ui.rs`, `portfolio_ui.rs`, `flow.rs`, `state.rs` (Page enum: Tools → AllTools; the
tool tab stays). In `main.slint`: the `Sidebar` component (source list) and the `Page` switch.

**Does:**

1. Source list 220 wide per DESIGN §5.1: Recents, Favourites, Library, Tasks, Actions; Help at
   the bottom; no Preferences row (W5 owns the window; the menu bar item stays). Collapses to
   52 under 1024.
2. Recents page: title row, DropArea with capsule "Open Files" and the "New ⌄" popup (blank,
   clipboard, screenshot, scanner, iPhone), "Tools for you" six ToolTiles + "View All",
   "Recent files" with search + sort popup + FileCards 152×180, "Favourites". Customize sheet
   (which sections, which six tiles).
3. All Tools page: title, search, category chips, ToolTile grid with `fs-title-3` headers,
   favourites first, plugins and custom tools as categories. Coloured square icons gone:
   monochrome Lucide glyph on a `fill-1` tile.
4. Tool tab (batch): files area on `canvas`, options panel 292 → restyled with W1 widgets, the
   Run button primary at the panel's bottom; result screen on the same tokens. No layout
   change beyond tokens and the options panel's widgets (the reader-side single-document path is
   W3's; the batch tab is kept as the home of multi-file runs).
5. Library, Tasks, Actions pages on the 32 margin with the same title row and tokens.

**Screenshot acceptance:**
- Home at 1280×800 light and dark matches `docs/design/mocks/home.png` within the tolerance of
  real content: source list 220, title at y 72, DropArea 996×100 at y 120, six tiles at y 288,
  recents header at y 428, cards at y 464; no coloured tile anywhere.
- All Tools shows every catalogue tool, plugins and custom tools; search "cmprs" finds
  Compress.
- A tool tab with three files and its options panel; the Run button is the only accent fill.

**Smoke impact:** `2-documents-and-tools.sh` step 9 (`click 50 21`, then `at Preferences` in
the source list: Preferences is no longer a row — switch to `key ctrl+comma`, W5's window), step
10 (`click 90 112` = Tools row: now All Tools is reached through "View All" or the source list
order changes; recalibrate with `at Tools`/`at "View All"`), steps 11, 15, 16 (tool cards from
outside: positions of the first card change: the grid starts at x 252, tiles 152 wide at 16
gaps), step 21–21e, 22, 26, 27, 39, 40, 44, 66 (tool tab files area and option panel clicks
`click 1140 755` Run — the Run button is now at the panel's bottom: measure), 45/45b/81/90/100
(Home ▸ New ⌄ popup instead of a row of buttons: `at New` then the item), 09c/09d recent tools
(`click 233 318`, `click 233 266`: tiles move to y 288 and x 252). Step 36 (Preferences ▸ Accent
from Home's menu) moves to W5.

## W3 — Reader chrome: toolbar, mode bar, rail, options bar, inspector, Tools popover

**Owns:** `ui/screens/reader.slint` (layout only: columns, rows, handles), `reader-toolbar.slint`,
`reader-options.slint` (becomes the docked options bar), `reader-rail.slint` (new),
`reader-inspector.slint` (new), `reader-tools.slint` (new: the Tools popover and the tool-in-
inspector flow), `edit-panel.slint`, `sign-panel.slint`, `redact-panel.slint`, `reader-form.slint`,
`reader-ink-preview.slint`, `reader-prepress.slint`, `reader-production.slint`, `ai-panel.slint`;
`src/reader_ui/{mod,tools,pointer,edits,sign,prepare,forms}.rs`, `edit_ui.rs`, `sign_ui.rs`,
`ai_ui.rs`, `wm_preview.rs`; `src/mac_menu.rs` and the `MenuBar` block of `main.slint` (the
new menu order: Sheaf, File, Edit, View, Go, Annotate, Edit PDF, Forms, Pages, Protect, Tools,
Window, Help).

**Does:**

1. Toolbar 44 per DESIGN §5.2: leading (sidebar, organize grid, reading view, zoom popover,
   − +, page field), centred mode bar (Annotate · Edit · Fill & Sign · Organize · Redact ·
   Measure · Tools), trailing (Aa page theme popover, search field, AI, Share, Print, More,
   inspector toggle). Folding at 1180 and 1024 as specified; nothing dropped: More holds what
   folded. Active tool = `fill-3` pill, icon `text`, never accent.
2. Mode state: one mode or Read; ⌘1–⌘6; clicking the selected mode or Escape (no selection)
   returns to Read. Entering a mode opens the rail and the options bar and, unless pinned, the
   inspector.
3. Mode rail 44 wide (`reader-rail.slint`): items per mode as DESIGN §5.3, Select first, Delete
   last in `danger` for Organize, scrolls when short.
4. **Options bar** docked 36 tall under the toolbar across the canvas column: the current
   floating options strip and the floating text-format bar move here. Nothing floats over the
   page except the selection bar (which flips off the selection).
5. **Inspector** 260 (`reader-inspector.slint`): header 36 with title and ×, InspectorSections
   per mode; document info in Read; the search results (W4 supplies the rows component; W3 hosts
   it); a chosen tool's options and pinned **Run** row with the "Runs on this computer" line.
   The resize handle from W1 on its left edge.
6. **Tools popover** (⌘K, wrench): 320×480, search field focused, Favourites and Recent on top,
   then categories; choosing opens the tool in the inspector; tools that need more files
   (Merge, Compare) open a tool tab with the document in it. Every catalogue tool, plugin and
   custom tool is listed; the menu bar Tools menu mirrors it.
7. Page theme popover "Aa": Day / Sepia / Night swatches, Invert, reflow text size; the canvas
   stays neutral (the tint itself is another worker's change in `viewer/`; W3 only chooses).
8. Menu bar reordered to the command map; every toolbar and rail item has a menu item with its
   shortcut; context menus lose their shortcuts.

**Screenshot acceptance:**
- Reader in Annotate at 1280×800 matches `docs/design/mocks/reader-annotate.png`: toolbar 44,
  mode bar centred at x 472–808 with the sidebar open, rail 44 at x 260, options bar 36 at y 82,
  inspector 260 on the right with Colour / Opacity / Stroke sections, pages on `canvas` from y
  118; no options bar or format bar over the page text.
- Read mode: no rail, no options bar, canvas from y 82, inspector closed.
- Tools popover open with "comp" typed shows Compress first; choosing it shows the Compress
  options in the inspector with Run pinned at the bottom; running shows the toast "Compressed
  to … · Undo".
- 1024×768: mode bar icons at a 40 pitch, − + folded, sidebar overlay.

**Smoke impact (the biggest):** every step that clicks the toolbar by coordinate:
`gui-smoke.sh` `ZOOM_X 750 / ZOOM_MENU_X 778` (zoom field now at x 132–204: set `ZOOM_X=168`,
menu at the same place), `WRENCH_X 1222` at y 61 (the Tools mode segment, centred: at 1280 with
the sidebar open it is at x 784; use `at`-free fixed `WRENCH_X=784` and keep the popover's search
focus so `type "compress"; Return` still works), `click 413 61` Redact (step 18/18b: now the
Redact mode segment at x 688), More menu items (`1127,61` then `1008,y` rows: More moves to x
1192, its rows to the new popover width: steps 24, 33, 35c, 35d, 38, 63, 64, 92, 94, 96, 97,
105, 106, 109, 110, 111, 200–204; many of these actions move out of More into modes or the
Tools popover — rewrite each step to its new entry: e.g. 38/104 Page grid → `key ctrl+4`,
53 Prepare form → `key ctrl+3` then the rail item, 97 Loupe → Annotate rail Shapes ⌄), the
colour palette and width clicks of steps 2b, 6c, 6c', 6c'', 41, 42, 50, 84, 213, 214 (the
options bar is now docked at y 82–118: palette swatches start at x 316 (260 + 44 + 12) on a
26 pitch), text-format bar clicks of 2c, 2e, 211, 212 (`click 322 110`, `355 110`: now in the
docked bar at y 100), the selection bar's items (step 19b `⌘K` link, 46 Replace text 7th item:
the bar keeps its order; positions follow the selection, unchanged), `click 1140 755` Run (steps
10, 18b, 20d, 47, 66: now the inspector's Run at (1150, 772)), keyboard letters for tools (H, N,
S, E, R, M, L, X, F, Z: unchanged, they still select the tool and switch the mode), ⌥⌘2 two
pages (6d, 33: unchanged shortcut). Steps 220 (menu bar order) and 221 (shortcuts list)
recalibrate to the new menu order.

## W4 — Sidebar, search results, page organiser

**Owns:** `ui/screens/reader-panel.slint` (sidebar content), `reader-search.slint` (new: the
results rows hosted by W3's inspector), `reader-organize.slint` (new: the grid), the thumbnails
and `PageCardView` in `widgets/file-card.slint` are W1's — W4 uses them; `src/reader_ui/
{pages,search,nav,folder_search,layers}.rs`, `reader.rs` (outline), `viewer_ui.rs` for the
organiser's thumbnails.

**Does:**

1. Sidebar per DESIGN §5.4: segmented tabs 28 tall at a 16 margin (Thumbnails, Outline,
   Bookmarks, Annotations, Attachments, Layers; ⌥⌘1…6), title row 36 with the tab's actions,
   content; the bottom page-actions bar 36 pitch from x 8 folding into ⋯ from the right under
   268 wide (Delete folds last). Rows 28 with the inset hover fill; thumbnails `width − 32`
   with the badge; CommentCards with the inset colour strip; filter chips that scroll.
2. Search results component: count in the header, page groups, 2-line context rows with the
   match marked, current result accent, options popup (Match Case, Whole Words, In Folder…,
   recent searches). No-results view with the query.
3. Page organiser (Organize mode canvas): 196-wide cells, 56 / 40 gaps, 40 margin, selection,
   rubber band, drag with an insertion bar, file drop, thumb size 120–320 from the inspector's
   slider; it closes the Thumbnails tab while on and restores it after.
4. The "By page ⌄" sort popup `min-width 88` with middle truncation; never a clipped chevron.

**Screenshot acceptance:**
- Sidebar at 200: six tab icons visible, title ⋯ inside the margin, bottom bar shows Rotate
  left/right, Insert, Duplicate + ⋯ (Extract, Number pages, Delete inside), nothing clipped.
- Sidebar at 480: thumbnails 448 wide, two numbers readable.
- Search "preload" in the sample: inspector titled Search, "108 results", "Page 6 · 16" groups,
  the current row accent, the page scrolled to it.
- Organize mode at 1280×800 with the sidebar closed: 4 columns, page 4 selected with the frame
  and badge, Delete red at the rail's bottom, "1 of 7 selected" in the inspector.

**Smoke impact:** `1-reader.sh` step 3 (Pages panel ops: the bottom bar at y 778 → items now at
y 774 on a 36 pitch from x 8: Rotate left 26, Rotate right 62, Insert 98, Duplicate 134, Extract
170, Number 206, Delete 242 — at the default 260 the bar holds all seven; steps 30 `156,778`
Extract and 35 `188,778` Number pages recalibrate), step 4 (search results were in the side
panel: now in the inspector on the right — the result row click moves to x ≥ 1020; use
`at "Page 3"` in `260x700+1020+82`), `3-flows.sh` 23 (recent searches popup from the field's
magnifier at x 904), 25 (Files panel tab `211,106` → Attachments is the 5th tab at x 16 + 4×36 +
18 = 178, y 104), 35b/82/87/95 (thumbnail right-click: thumbnails are now `width − 32` wide, the
first at y 170), 108 (Layers tab, 6th: x 214), 86 (bookmark ⋯), `4-flows.sh` 104 (page grid
selection by colour: cells 196 wide at a 252 pitch from x 40 with the sidebar closed), 85
(Organize odd/even: now in the organiser's inspector).

## W5 — Settings window, sheets, dialogs, toasts

**Owns:** `ui/settings.slint`, `ui/screens/preferences*.slint` → `settings-window.slint` (new)
with one file per tab (`settings-general.slint`, `-appearance`, `-reading`, `-tools`, `-ai`,
`-shortcuts`), `reader-sheets.slint` (print, export, protect, sign, properties), `dialogs.slint`,
`certificate-encrypt.slint`, `security-policy.slint`, `signature-manager.slint`,
`spelling-dialog.slint`, `ocr-review.slint`, `feedback.slint`, `updates.slint`,
`javascript.slint`; `src/settings.rs`, `settings_ui.rs`, `prefs.rs` (except W1's width key),
`print.rs`, `reader_ui/{print_sheet,properties,saving}.rs`, `feedback_ui.rs`, `updates_ui.rs`,
`plugins_ui.rs`, `platform.rs` (shortcut recorder), `windows.rs` (the second window).

**Does:**

1. Settings as its **own window** (⌘,; Sheaf ▸ Settings… on macOS, Edit ▸ Settings… elsewhere),
   520 wide, icon tabs General / Appearance / Reading / Tools / AI / Shortcuts, grouped form
   cards, rows 28, labels as statements, applies at once, fits its content (max 640 then
   scrolls), reopens on the last tab. Rows regrouped per DESIGN §5.8; Advanced items under
   General ▸ Data with confirmations. The Preferences page in Home is removed (W2 removes the
   row; W5 removes the screen).
2. Sheets on the `Sheet` widget: Print 864×640 with the preview column, Export/Convert 480,
   Protect 480 with a Certificates tab, Sign with certificate 480, Properties; buttons bottom
   right, Cancel then primary; Escape/Return.
3. Dialogs 360: Save changes?, Password, Unable to open (Repair…), confirmations for Apply
   redactions and Reset.
4. Toasts: bottom centre, 36 tall, Undo word, 4 s, announced; errors never only in a toast.

**Screenshot acceptance:**
- Settings ▸ General light and dark: 520 wide, no focus ring at open, trailing values aligned at
  one x, no empty band under the last card, no scroll bar.
- Settings ▸ Appearance: six accent swatches with the system one first on macOS; appearance
  System/Light/Dark as radios.
- Print sheet matches the proportions of PDF Expert's capture: preview 320 left, form right,
  "PDF ⌄" bottom left, Cancel · Print bottom right.
- A toast "3 pages deleted · Undo" at the bottom centre over the canvas; ⌘Z also undoes.

**Smoke impact:** `2-documents-and-tools.sh` step 9 (Preferences ▸ Identity ▸ Your name: now
Settings ▸ Reading ▸ Author name: `key ctrl+comma`, click the Reading tab by `at Reading` in the
settings window, then `at Author`), 11 (damaged file: dialog 360 centred: Repair… button
position), `3-flows.sh` 28 (Open new documents at Actual size → Settings ▸ Reading), 36 (accent
blue → Settings ▸ Appearance second swatch), 62/80/98 (Document Properties sheet layout),
6b/301 (Print sheet: Return still prints; defaults OCR crop moves), 27 (Protect with
restrictions: sheet 480 from the Tools popover), 54/29 (Save changes? dialog buttons: Don't
Save · Cancel · Save at the new positions), `4-flows.sh` 99 (default app row → Settings ▸
General), 103 (save failure dialog), 227 (dialog after change). `gui-smoke.sh` `pref_find`
helper: point it at the settings window's geometry (`xdotool search --name Settings`).

## Cross-stream checks before merge

- One screenshot of each: Home, reader Read, reader Annotate, Organize, Settings General, Print
  sheet, light and dark, 1280×800 and 1024×768, attached to the PR; every edge on the 4 px
  grid (overlay the gallery grid).
- Every board component (331) reachable: a script walks the command map of DESIGN §1 and asserts
  each menu item exists (`dev/scripts/smoke` step 220 extended).
- Tooltip audit: a debug flag lists every hoverable control without a tooltip; the list is empty.
- No `Theme.mac ?` layout conditions; the smoke clicks the same places on Linux and macOS.
