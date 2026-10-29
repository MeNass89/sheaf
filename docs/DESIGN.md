# Sheaf design specification

Sheaf becomes a PDF reader first: one document in front of you, the tools around it. PDF Expert is
the reference (captures in `dev/re/evidence/pdfexpert/`); Apple's Mac design rules are the method
(BAMA `mac-design`, `ux-design`, `layout-grid`, `design-system`). This document is the one source
for every screen. Numbers are pixels at 1× (points on macOS); every length is 0, 1, 2 or a
multiple of 4 unless a measurement from the reference says otherwise.

Status: specification, 29/10/2026. The implementation plan is `docs/DESIGN-PLAN.md`. Mocks of the
four key screens are in `docs/design/mocks/`.

Contents

1. Design plan (archetype, information architecture, command map, empty states, feeling)
2. How PDF Expert is built, and the Sheaf mapping
3. Where the tools go
4. Design system (tokens, components, cross-platform rule)
5. Screens, with measurements at 1280×800 and the 1024 / 1600 adaptations
6. Tooltip layer and side-panel width (the three reported bugs)
7. Accessibility, keyboard and focus
8. Glossary

---

## 1. Design plan

### Archetype

A **document-based reader with tabs**, plus a **library** view in the same window and a separate
**Settings** window.

- One window = one tab strip. The first tab is always **Home** (the library). Every other tab is
  a document (a PDF, or a preview PDF of a picture or text file) or a **tool tab** (a batch
  conversion on many files).
- The document tab is the hero. It has a unified toolbar, a left sidebar, a mode rail, the page
  canvas, and a right inspector.
- Settings is its own window (⌘,), never a page in the main window.

### Information architecture

```
Window
├── Home tab (library)
│   ├── Recents            ← default page
│   ├── Favourites
│   ├── Library            (private files, versions)
│   ├── Tasks              (history of runs)
│   ├── Actions            (saved multi-step actions, watched folders)
│   └── All Tools          (reached from "View All" on Recents, or the source list)
├── Document tab ×N
│   ├── Sidebar  : Thumbnails · Outline · Bookmarks · Annotations · Attachments · Layers · Search results
│   ├── Mode rail: tools of the active mode (Annotate · Edit · Fill & Sign · Organize · Redact · Measure)
│   ├── Canvas   : the pages (or the page organiser grid, or Compare, or Split)
│   └── Inspector: properties of the selection, the active tool's options, or a tool's options + Run
└── Tool tab ×N (batch: many files → one run)
    ├── Files area (drop zone, file cards)
    └── Options panel + Run
```

Two levels at most everywhere: the Home source list is one level; the sidebar tabs are one level
with rows inside.

### Command map

Every action lives in the **menu bar** (with its shortcut, when frequent) and in one other place.
"Where" names the first place a person reaches it. Context menu = ⌥ right click on the thing.
Shortcuts are the macOS ones; on Windows and Linux ⌘ reads Ctrl, and ⌥ reads Alt.

| Action | Where | Also | Menu bar | Shortcut |
|---|---|---|---|---|
| New tab (Home) | tab strip + | | File ▸ New Tab | ⌘T |
| New blank PDF, from clipboard, from screenshot, from scanner, from iPhone | Home ▸ Open area ▸ "New" menu | | File ▸ New ▸ … | ⌘N (blank) |
| Open… | Home ▸ Open Files | drop anywhere | File ▸ Open… | ⌘O |
| Open Recent | Home ▸ Recents | | File ▸ Open Recent ▸ | |
| Close tab / window | tab × | | File ▸ Close Tab / Close Window | ⌘W / ⇧⌘W |
| Save, Save As…, Save as Flattened…, Revert | toolbar More | | File | ⌘S, ⇧⌘S |
| Reduce File Size… (Compress) | Tools popover | | File ▸ Reduce File Size… | |
| Export To ▸ Word, Excel, PowerPoint, Images, Text/HTML, PDF/A | Tools popover ▸ Export | | File ▸ Export To ▸ | |
| Export Annotation Summary | sidebar Annotations ▸ … | | File ▸ Export Annotation Summary… | |
| Merge Files… | Home ▸ Tools | reader Tools popover | File ▸ Merge Files… | |
| Set Password… / Remove Password | Tools popover ▸ Protect | | File ▸ Set Password… | |
| Properties… | More | | File ▸ Properties… | ⌘I |
| Print… | toolbar (trailing) | | File ▸ Print… | ⌘P |
| Share | toolbar (trailing) | | File ▸ Share ▸ | |
| Undo / Redo (named) | | | Edit ▸ Undo Highlight | ⌘Z / ⇧⌘Z |
| Cut, Copy, Paste, Delete, Select All | context menu | | Edit | ⌘X ⌘C ⌘V ⌫ ⌘A |
| Copy as Image (area snapshot) | context menu on an area | | Edit ▸ Copy as Image | |
| Find | toolbar search field | | Edit ▸ Find ▸ Find… | ⌘F |
| Find Next / Previous | search panel arrows | | Edit ▸ Find ▸ | ⌘G / ⇧⌘G |
| Find and Replace… (Edit mode) | mode rail (Edit) | | Edit ▸ Find ▸ Replace… | ⌥⌘F |
| Search in Folder… | search field options | | Edit ▸ Find ▸ In Folder… | |
| Check Spelling | Edit mode rail ▸ … | | Edit ▸ Spelling | ⌘: |
| Look Up, Search with Google, Read Aloud | context menu on text | | Edit ▸ Speech | |
| Highlight / Underline / Strikeout / Squiggly | selection bar | context menu | Annotate ▸ | H / U / S / Q |
| Note, Text box, Callout, Pen, Eraser, Shapes, Stamp, Link, Attachment, Date, Check/Cross | mode rail Annotate | | Annotate ▸ | N, T, C, P, X, R, M, L, … (as today) |
| Annotation colour, stroke, opacity, font | inspector (Annotate) | selection bar colours | Annotate ▸ Properties | |
| Delete annotation / Remove all | context menu · sidebar Annotations … | | Annotate ▸ | ⌫ |
| Reply, Status | inspector (comment) | sidebar row context menu | Annotate ▸ | |
| Measure distance / perimeter / area, set scale | mode rail Measure | | Annotate ▸ Measure ▸ | |
| Edit text, Add text, Add image, Replace image, Arrange, Align | mode rail Edit | context menu | Edit PDF ▸ | |
| Add header & footer, page numbers, Bates, watermark, background, overlay | Tools popover ▸ Edit | | Edit PDF ▸ | |
| Links, attachments, bookmarks/outline edit, article threads, page labels | sidebar tabs ▸ + | | Edit PDF ▸ | |
| Fill fields, clear form, highlight fields, flatten | mode rail Fill & Sign | | Forms ▸ | |
| Prepare form: detect, add field types, properties, tab order | Fill & Sign ▸ Prepare | inspector | Forms ▸ Prepare Form | |
| Draw / type / image signature, initials, place, repeat | mode rail Fill & Sign | | Forms ▸ Sign ▸ | |
| Digital signature, certify, time stamp, validate, IDs | Tools popover ▸ Sign | | Forms ▸ Digital Signatures ▸ | |
| Organize: rotate, delete, insert blank, insert file, duplicate, extract, replace, reverse, split here, crop, resize, impose, transitions, labels | Organize mode (grid) rail | sidebar Thumbnails bottom bar | Pages ▸ | ⌘L / ⌘R rotate, ⌫ |
| Redact mark text/area/page, find & redact, appearance, apply, sanitize | Redact mode rail + inspector | | Protect ▸ Redact ▸ | |
| Protect, permissions, certificates, policies, metadata, info report | Tools popover ▸ Protect | | Protect ▸ | |
| Compress, OCR, Repair, Enhance scans, Preflight, Colours/greyscale, Print production, Index | Tools popover ▸ Optimize | | Tools ▸ | |
| Compare… | Tools popover | | Tools ▸ Compare… | |
| Adjust colours / Invert | Tools popover ▸ Edit | | View ▸ Page Theme ▸ Invert | |
| All Tools… (the popover) | toolbar wrench | | Tools ▸ All Tools… | ⌘K |
| Custom tools, Actions, Watch folder, Run action | Home ▸ Actions | Tools popover ▸ Actions | Tools ▸ Actions ▸ | |
| Run JavaScript, Plugins | | | Tools ▸ Developer ▸ | |
| AI: ask, summarize, translate, classify, podcast/slides | toolbar AI | | Tools ▸ AI ▸ | ⇧⌘A |
| Show/Hide Sidebar, Inspector, Toolbar | toolbar toggles | | View | ⌃⌘S, ⌃⌘I, ⌥⌘T |
| Sidebar tab: Thumbnails, Outline, Bookmarks, Annotations, Attachments, Layers | sidebar segmented | | View ▸ Sidebar ▸ | ⌥⌘1…6 |
| Zoom in / out / actual / fit page / fit width / zoom to… / zoom to area / loupe | toolbar zoom popover | | View ▸ Zoom | ⌘+ ⌘− ⌘0 ⌘9 ⌘8 |
| Single page / continuous / two pages / cover alone / horizontal / rotate view / auto-scroll | toolbar layout popover | | View ▸ Page Display ▸ | |
| Page theme: Day / Sepia / Night, Invert, reflow text size | toolbar "Aa" popover | | View ▸ Page Theme ▸ | ⌥⌘D ⌥⌘S ⌥⌘N |
| Reflow, rulers, grid, guides, split view, link/field highlight | More | | View | |
| Hide toolbars, Full screen, Presentation | More | | View | ⌃⌘F, ⇧⌘P (as today) |
| Next / previous / first / last page, go to page, back / forward | toolbar page field | | Go | ⌥↓ ⌥↑ ⌘↑ ⌘↓ ⌥⌘G ⌘[ ⌘] |
| Bookmarks: add page bookmark | sidebar Bookmarks + | | Go ▸ Add Bookmark | ⌘D |
| Tabs: next / previous / merge all windows / open in new window | tab context menu | | Window | ⌃⇥ |
| Settings… | | | Sheaf ▸ Settings… | ⌘, |
| Check for Updates…, Send Feedback…, Help, Shortcuts list | | | Sheaf ▸ / Help ▸ | |

Rules: items that do not apply stay visible and disabled in the menu bar, hidden in context menus.
Context menus show no shortcuts. The menu bar order is Sheaf, File, Edit, View, Go, Annotate,
Edit PDF, Forms, Pages, Protect, Tools, Window, Help (own menus follow the modes, most general
first). Every one of the 331 board components keeps a row above or in its tool; none is dropped.

### Empty states

| Surface | Empty shows | First action |
|---|---|---|
| Home ▸ Recents | the Open area; under "Recent files", a folder glyph (48, tertiary) and "Files you open will appear here" | Open Files |
| Home ▸ Favourites | star glyph, "Star a file or a tool to keep it here" | — (hint only) |
| Home ▸ Library | "Keep private copies and versions of your files here", button "Add to Library…" | Add to Library… |
| Home ▸ Tasks | "Runs of your tools will appear here with their results" | — |
| Home ▸ Actions | "Chain tools into one action", button "New Action…" | New Action… |
| Sidebar ▸ Outline | sketch, "Right click a heading in the page and choose Add Outline Item" | + in the header |
| Sidebar ▸ Bookmarks | "Press ⌘D to bookmark the page you are on" | + |
| Sidebar ▸ Annotations | sketch, "Highlights, notes and drawings will be listed here" | — |
| Sidebar ▸ Attachments | "Files attached to this PDF appear here", + | + |
| Sidebar ▸ Layers | "This PDF has no layers" | — |
| Search results | typed text, "No results for “{text}”" with the Whole Words / Match Case options still shown | — |
| Inspector, nothing selected | "Select something on the page to see its properties" in Annotate/Edit; the mode's tool defaults in other modes | — |
| Tool tab, no files | drop zone "Open Files · or drop them here" | Open Files |
| Document that fails to open | sheet: "Unable to open {name}", reason, Repair… / Cancel | Repair… |

### Feeling

**Calm and sure.** The page is the brightest thing on screen; chrome is quiet grey, one accent,
no coloured tiles in the reader. Motion is short (120–200 ms) and only where it explains a change
(a popover growing from its button, a panel sliding). Words are short and plain. Nothing bounces.

---

## 2. How PDF Expert is built, and the Sheaf mapping

Measured on `02-study-open-reading.png` and friends (1600×1066, 1×).

| PDF Expert | Measure | Sheaf |
|---|---|---|
| Tab strip in the title bar, dark on every theme, tab 440 max, × on the right of the tab, + after | 44 tall | Tab strip 38 tall in the chrome material (light or dark with the window), tab 240 max, × on hover, + after. Home tab is the first tab, pinned, icon only on narrow windows |
| Unified toolbar | 44 tall | 44 tall |
| Leading group: Sidebar, Page grid, Reading view, Zoom "87% ⌄", − / + | icons 20, buttons 36×36, group starts at x 8 | Same: Sidebar (⌃⌘S), Organize (grid), Reading view, Zoom popover, − + |
| Center group: **mode bar** — Annotate · Edit · Convert · Fill & Sign · Organize · Measure · + | 48×44 pill per mode, selected pill `#E7E7EA`, icons 20 | Mode bar: Annotate · Edit · Fill & Sign · Organize · Redact · Measure · **Tools** (wrench, the popover). The mode is a segmented control: one selected at most; clicking the selected one returns to Read |
| Trailing: "Try Full Version" (we have none), AI, Translate, Share, Search field 160×30 | | Trailing: AI, Print, Share, Search field 160×28, Inspector toggle |
| Mode tools in a **vertical rail** between sidebar and content | 50 wide, icons 20, 48 apart, the top item is "pop out" | Mode rail 44 wide, items 36×36 on a 40 pitch, the first is the mode's selection arrow |
| Sidebar: segmented tabs (Bookmarks, Outline, Annotations, Thumbnails) at top, section title "Thumbnails" below, content | 266 wide; tabs 32 tall, 16 margin; title 15 pt semibold | Sidebar 260 default, 200–480 with a handle; segmented tabs 28 tall; title row 36 tall with the tab's actions on the right (+, …) |
| Thumbnails: 186 wide, page number under, selected = 2 px accent frame + accent number badge | | Thumbnails fit the width minus 32; number under; selected = 2 px accent frame + accent badge |
| Selection context menu: Copy · Edit Text · Note · Highlight (colour dots in the row) · Underline · Squiggly · Strikeout · Add Outline Item · Look Up · Search · AI (Summarize, Explain, Translate) · Services | 310 wide, rows 28 | Same groups in the same order; colour dots inside the Highlight row |
| Page organiser: full-canvas grid, 196-wide pages, 56 gaps, number under, rail (add page, add file, rotate, duplicate, insert, extract, delete red) | | Organize mode: same grid, rail with the same seven plus crop, replace, reverse, split here; multi-select, drag to reorder |
| Search: results in a right column 266 wide, "Found: 108", grouped by page with "16 items", yellow mark in context; the sidebar stays | | Search results in the **inspector column** (260) when the field has text; the sidebar stays |
| Night theme: canvas and pages dark; chrome unchanged | | Page theme is separate from the app appearance (as today) |
| Welcome: left source list (Home, Tools, Recents, Favourites; Account, Settings, Support at bottom), content: title, Open Files area (dashed, blue tint), "Tools for You" row of six tiles, Recents, Favourites | sidebar 240; area 976×208; tiles 156×132 | Home: source list 220; Open area 100 tall full width; "Tools for you" six tiles 152×108; Recent files grid; "Favourites" below. Settings not in the list (⌘,) |
| Print: a sheet with a page preview on the left and a grouped form on the right | 866×650 | Same layout, 864×640 |

What we keep of Sheaf that PDF Expert lacks and iLovePDF has: the batch **tool tab** (many
files, one run) and the **Actions** page. They stay, restyled, behind Home, not in front of the
reader.

---

## 3. Where the tools go

Sheaf has 30 catalogue tools in 6 categories (`sheaf-core/src/tools.rs`: Organize, Optimize,
Convert to PDF, Convert from PDF, Edit, Security) plus plugins and custom tools, and 331 board
components behind them. Today they are a wall of coloured cards on Home and on a Tools page, and
each run opens a tool tab. That is the iLovePDF feel. The reader must own them.

Decision, three places, by what the tool acts on:

1. **A mode** when the tool is interactive on the page: Annotate, Edit (text, images, objects,
   find & replace, spelling), Fill & Sign (forms, signatures), Organize (the page grid), Redact,
   Measure. These are PDF Expert's modes plus Redact and Measure. Each mode = a rail of tools on
   the left and an inspector of options on the right. Sheaf's current options bar (the horizontal
   strip of colours and widths under the toolbar) disappears into the inspector; the floating
   selection bar stays.

2. **The Tools popover** (toolbar wrench, ⌘K, Tools ▸ All Tools…) for every tool that transforms
   the open document in one run: Compress, OCR, Repair, Protect, Unlock, Watermark, Page numbers
   / header & footer / Bates, Adjust colours, Crop, PDF to Word / Excel / PowerPoint / Images /
   Text / PDF/A, Split, Extract, Compare…, Sign digitally, Sanitize, Preflight, Index, plus
   plugins and custom tools. It is a searchable list, 320×480, grouped by category, with
   Favourites and Recent on top. Choosing one opens the tool in the **inspector column** with its
   options and one **Run** button at the bottom; the canvas shows the live preview where there is
   one (watermark, crop, page numbers). The result replaces the document in the tab (Undo is
   "Undo Compress"), or opens a Save sheet when the output is not a PDF. The iLovePDF cards never
   appear inside the reader.

3. **Home ▸ All Tools** and the **tool tab** for batch runs on many files (Merge, Compress ×N,
   JPG to PDF, Word to PDF…, Compare of two files chosen from disk). The tool tab keeps its files
   area and options panel. From the reader, a tool that needs more files (Merge, Compare)
   opens a tool tab with the current document already in it.

Why: a reader is judged by what it does to the page in front of you; PDF Expert puts every
such thing one click from the page and hides nothing behind a store page. The batch tools are a
different job (many files, no page) and get their own, quieter place. Nothing is lost: every
tool is in the popover or on Home, and in the menu bar.

---

## 4. Design system

### Tokens

All in `ui/theme.slint`. Views use tokens, never raw values. One file per group is not possible
in Slint without many imports, so the groups below are sections of `Theme`.

**Colour roles** (light / dark). On macOS, `accent` is the system accent read at start (as today);
elsewhere the default is Sheaf blue `#1F6FEB` / `#58A6FF` — calm, not the current red. The accent
picker in Settings keeps the six choices; red moves to second place.

| Role | Light | Dark | Use |
|---|---|---|---|
| `window` | #FFFFFF | #1E1E1E | Home content, inspector, sheets |
| `chrome` | #F6F6F6 | #2B2B2B | tab strip, toolbar, sidebar, mode rail (one material, the "control layer") |
| `chrome-inactive` | #F0F0F0 | #262626 | chrome when the window is not key |
| `canvas` | #E6E6EB | #1C1C21 | behind the pages (kept) |
| `page-shadow` | #00000040 | #00000066 | 6 px blur under pages |
| `hairline` | #D9D9DE | #3A3A3F | 1 px separators, panel edges |
| `fill-1` | #0000000A | #FFFFFF0F | row hover |
| `fill-2` | #00000014 | #FFFFFF1A | pressed, selected segment, mode pill |
| `fill-3` | #0000001F | #FFFFFF29 | toolbar pill of an active tool |
| `field` | #FFFFFF | #1A1A1A | text fields, search field; border `hairline` |
| `text` | #1D1D1F | #F2F2F2 | primary |
| `text-2` | #6E6E73 | #A1A1A6 | secondary |
| `text-3` | #9A9AA0 | #6E6E73 | tertiary, placeholders |
| `text-disabled` | #C4C4C8 | #4A4A4F | |
| `accent` | system / #1F6FEB | system / #58A6FF | selection, focus ring, primary button, links |
| `accent-text` | accent darker 20 % | accent brighter 15 % | accent-coloured text |
| `on-accent` | per accent (white, or #1D1D1F on yellow/mint) | same | text on an accent fill |
| `selection` | accent 25 % | accent 35 % | text selection on the page |
| `info`, `success`, `warning`, `danger` | #0A84FF, #1D9D58, #D97706, #D70015 | #409CFF, #30D158, #F5A623, #FF453A | always with an icon or a word |
| `scrim` | #00000040 | #00000080 | behind sheets |
| `tooltip` | #F2F2F2 at 96 % | #3A3A3A at 96 % | tooltip body; text `text` |
| `toast` | #2B2B2B | #EBEBF0 | bottom toast; text `on-toast` |

Materials: Slint has no blur. `chrome` is opaque; the control layer is told apart from content by
tone (chrome is 4 % darker than `window` in light, 5 % lighter in dark) and by the hairline between
them. Popovers get `window` with the `popover` shadow. Nothing else fakes glass.

**Type scale** (system font: SF on macOS, Segoe UI on Windows, the GTK font on Linux; the fallback
is Inter if bundled). Sizes in px, leading in px, weight.

| Token | Size / leading | Weight | Use |
|---|---|---|---|
| `fs-large-title` | 26 / 32 | 400 | Home page title ("Recents") |
| `fs-title-1` | 22 / 26 | 400 | sheet titles |
| `fs-title-2` | 17 / 22 | 600 | Home section headers ("Recent files") |
| `fs-title-3` | 15 / 20 | 600 | sidebar section title ("Thumbnails"), inspector section headers |
| `fs-headline` | 13 / 16 | 600 | card titles, dialog headings |
| `fs-body` | 13 / 16 | 400 | everything else: rows, buttons, fields, menus, tabs |
| `fs-callout` | 12 / 15 | 400 | hints in inspector, tool descriptions |
| `fs-subhead` | 11 / 14 | 400 | file metadata, tooltips, page number badges |
| `fs-caption` | 10 / 13 | 400 | the floor; thumbnail numbers in a dense grid |

No light weights. Numbers in columns are tabular (`font-features: tnum`).

**Spacing** (4 px grid): `xs 4`, `sm 8`, `md 12`, `lg 16`, `xl 20`, `xxl 24`, `section 32`.
Window and page margins 20. Related controls 8 apart; groups 20; sections 32. Cards pad 16.

**Radii** (concentric: inner = outer − padding): `r-xs 4` chips, tooltips, keycaps; `r-sm 6`
buttons, fields, segments; `r-row 6` row hover fills; `r-md 10` cards, popovers; `r-lg 12`
sheets; `r-xl 16` the Open area. Mode pills in the toolbar: 8.

**Shadows**: `popover` 0 4 16 #00000026 (light) / #00000080 (dark) + 1 px `hairline` border;
`sheet` 0 12 40 #00000040; `page` 0 2 6 `page-shadow`; `toast` 0 4 12 #00000033.

**Motion** (every animation through these; all off under Reduce Motion, replaced by a 0 ms fade):
`hover 80 ms`, `quick 120 ms ease-out` (press, segment move, tooltip in), `ui 160 ms ease-in-out`
(sidebar / inspector slide, popover scale 0.98→1 + fade), `appear 200 ms` (sheet rises 8 px +
fade), `dismiss 120 ms`. Search results, tab switches and the page organiser do not animate.
Tooltip out: 60 ms.

**Icons**: Lucide (bundled, kept), 1.5 px stroke, 16 px in rows, menus and the inspector; 18 px in
the toolbar, rail and sidebar tabs; 20 px in the Home source list. Monochrome, `text` or `text-2`;
never tinted except the destructive Delete in the organiser rail (`danger`).

### Component inventory

Sizes are width × height; states are rest / hover / pressed / selected (on) / disabled / focused.
Focus ring: 2 px `accent` outside the shape, 2 px offset, keyboard only.

| Component | Size | Rest | Hover | Pressed | On | Disabled |
|---|---|---|---|---|---|---|
| **ToolbarButton** (icon) | 36×36, icon 18, radius 8 | transparent | `fill-1` | `fill-2` | `fill-3` pill, icon `text` | icon `text-disabled` |
| **ModeSegment** (one item of the mode bar) | 48×36, icon 18, radius 8 | transparent | `fill-1` | `fill-2` | `fill-3`, icon `text` (never accent: one signal) | |
| **ToolbarText** (zoom "87% ⌄") | 72×28, radius 6 | `fill-1` | `fill-2` | `fill-3` | — | |
| **SearchField** | 160×28 (grows to 240 when focused), radius 6, `field` + `hairline` | magnifier 14 `text-3`, placeholder "Search" `text-3` | | | focus: 2 px accent ring | |
| **RailButton** (mode rail) | 36×36, icon 18, radius 8 | transparent | `fill-1` | `fill-2` | `fill-3` pill | |
| **SidebarTab** (segmented, 4–6 items) | each ≥ 36×28, icon 16; track `fill-1` radius 6; thumb `window` radius 5 with shadow 0 1 2 #0000001F | | `fill-1` | | thumb | |
| **SidebarRow** (outline, bookmarks, attachments, layers) | height 28, text 13, inset 8 both sides, hover/selection fill radius 6 inset 6 from the panel edge, disclosure 12 px at depth×16 | | `fill-1` | | `fill-2` (keyboard focus: accent fill, `on-accent` text) | |
| **ThumbnailCell** | width = sidebar − 32, page keeps ratio, number under 11 px centred, cell gap 12 | page shadow | | | 2 px accent frame outside, badge 22×18 accent radius 4 `on-accent` | |
| **CommentCard** (sidebar Annotations) | full width − 16, pad 10, radius 6, 3 px colour strip inset 6 radius 2 | `window` + hairline | `fill-1` | | accent border | |
| **InspectorSection** | header 28 (title `fs-title-3` 15/600 at x 16, disclosure 12 right at 16), body pad 16, rows 28 tall with label left / control right aligned at 16 | hairline under each section | | | | |
| **Button, secondary** | min 72×28, pad 12, radius 6, `fill-2` border none | | `fill-3` | darker | | text `text-disabled` |
| **Button, primary** | min 72×28, radius 6, `accent`, text `on-accent` 13/600 | | accent-hover | accent-pressed | | `fill-2` + `text-disabled` |
| **Button, destructive** | as secondary, text `danger` | | | | | |
| **Button, large** (Open Files) | 120×32, radius 16 (capsule), primary | | | | | |
| **TextField** | height 28, pad 8, radius 6, `field` + `hairline` | | | | focus ring | |
| **Combo / popup button** | height 28, pad 8 + chevron 12 right, radius 6, `fill-2` | | `fill-3` | | | |
| **Checkbox** | 14×14 radius 3, label 13 at 8 | | | | accent fill, white check | |
| **Radio** | 14 circle | | | | accent ring, dot | |
| **Switch** (Settings) | 26×15 | | | | accent | |
| **Segment** (inspector choices) | height 24, pad 10 | track `fill-1` | | | thumb `window` | |
| **Chip** (filter, category) | height 22, pad 8, radius 11, `fill-1` | | `fill-2` | | accent 15 % + accent-text | |
| **Swatch** (colour) | 18 circle, 1 px inner ring #00000014 | | scale 1.1 | | 2 px `text` ring at 2 px offset | |
| **Popover** | min 200, max 360 wide; radius 10; `window`; `popover` shadow; arrow none (Mac 26 popovers show no arrow in toolbars) ; pad 8; rows 28; opens 4 px under its button | | | | | |
| **Menu (context)** | 240–320 wide, radius 8, rows 28, pad 4, separators 1 px `hairline` at 4 margin; colour dots 16 in a row at 8 gaps | | row: accent fill + `on-accent` | | | text `text-disabled` |
| **Sheet** | 360 / 480 / 640 / 864 wide (print), radius 12, `window`, `sheet` shadow, centred, 8 px under the toolbar on a `scrim`; title `fs-title-1` at 20; buttons row bottom right, Cancel then primary, 8 apart | | | | | |
| **Dialog (alert)** | 360 wide, icon 48 centred, title `fs-headline`, message `fs-body`, buttons right | | | | | |
| **Toast** | min 200×36, radius 8, bottom centre 20 above the edge; text 13 `on-toast`; optional "Undo" in accent | | | | | |
| **Tooltip** | pad 4×8, radius 4, `tooltip` bg, 1 px `hairline`, text 11/14 `text`, shortcut in `text-2` after two spaces; max width 280, wraps | | | | | |
| **FileCard** (Home recents) | 152×180: thumb 96×124 at top 16, name 13 one line middle-truncated, meta 11 `text-2`; radius 10; `window` + hairline | | `fill-1` | | accent 2 px | |
| **ToolTile** (Home) | 152×108: icon 24 `text` on a 40×40 `fill-1` radius 10 tile, label 13 under at 12 | `fill-1` bg, radius 10 | `fill-2` | | | |
| **DropArea** (Home) | full width × 100, radius 16, dashed 1.5 px accent 50 %, accent 6 % fill; button "Open Files" capsule centred; "or drop files here" 12 `text-2` under | drag-over: accent 12 % fill, solid border | | | | |
| **TabItem** | 240 max, min 120, height 38, text 13, icon 14 at 12, × 14 at right on hover; selected: `window` with 1 px hairline sides; others: `chrome` | | `fill-1` | | | |
| **Scrollbar** | the platform's (Slint std) overlay, never restyled | | | | | |

### Cross-platform rule

One design on all three: same metrics, same tokens, same icons. Only true conventions differ:

- Traffic lights (macOS, left, 78 px reserved in the tab strip) vs window buttons (Windows/Linux
  right, provided by the OS decorations: Sheaf draws nothing there).
- Menu bar: native on macOS (app menu with Settings…, Quit); in-window menu bar on Windows and
  Linux, 28 tall, under the OS title, above the tab strip. Shortcut glyphs ⌘⌥⇧ vs Ctrl/Alt/Shift.
- The accent follows the system only on macOS; elsewhere the Settings choice (default blue).
- Continuity features (iPhone, Scanner on Mac) show only on macOS.
- Fonts: the platform's UI font. Nothing else differs; the smoke must click the same places on
  Linux as on macOS.

---

## 5. Screens

Reference window 1280×800. Columns (x from the left): sidebar 0–260, mode rail 260–304 (when a
mode is on), canvas to the inspector, inspector 1020–1280 (when open). Rows: tab strip 0–38,
toolbar 38–82, options bar 82–118 (only in a mode), content 82–800 (118–800 in a mode).

Adaptation: at **1024** the sidebar defaults closed (opens over the canvas as a 260 overlay with a
shadow when toggled; it stays docked once the window is ≥ 1120), the inspector docks at 240, the
search field shrinks to 120 and expands on focus, the mode bar keeps all seven items (7×48 = 336).
At **1600** the sidebar opens at 280, the inspector at 280, the canvas gets the rest; nothing else
grows.

### 5.1 Home (library)

```
┌──────────────────────────────────────────────────────────────────────────────┐ 0
│ ● ● ●  [⌂ Home]  [study.pdf ×] [report.pdf ×] [+]                            │ tab strip 38
├──────────┬───────────────────────────────────────────────────────────────────┤ 38
│          │  Recents                                        [Customize]       │ title row 72–104
│ ⏱ Recents│ ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐    │
│ ★ Favour.│ │              [ Open Files ]  [New ⌄]                        │    │ open area 120–220
│ ▤ Library│ │              or drop any files here                         │    │
│ ☰ Tasks  │ └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘    │
│ ⚙ Actions│  Tools for you                                       View All     │ 252–276
│          │  [Edit] [Annotate] [Organize] [Convert] [Sign] [Compress]         │ tiles 288–396
│          │  Recent files                [Search recents…] [Newest ⌄]        │ 428–452
│          │  [card][card][card][card][card][card]                              │ cards 464–644
│          │  Favourites                                                       │
│ ? Help   │  …                                                                │
└──────────┴───────────────────────────────────────────────────────────────────┘ 800
```

- Source list 220 wide, `chrome`, hairline right. Rows 28 tall, icon 16 at x 16, label at x 40,
  selected row `fill-2` radius 6 inset 8; first row at y 54 (16 under the strip). "Help" at the
  bottom (y 756). No Settings row (⌘,). Collapses to icons (52 wide) below 1024, with tooltips.
- Content margin 32 left/right (x 252–1248), top 24. Title `fs-large-title` at y 72; "Customize"
  secondary button right-aligned (lets people hide sections and pick the six tiles).
- Open area: 996×100, `r-xl`, dashed accent at 50 %, 6 % accent fill. Capsule "Open Files"
  120×32 centred at y 160; "New ⌄" popup (blank, from clipboard, from screenshot, from scanner,
  from iPhone) 8 right of it; the line "or drop any files here" 12 `text-2` under. Drag over the
  whole window lights the area.
- "Tools for you": header `fs-title-2` at y 252; "View All" accent text right (opens All Tools).
  Six ToolTiles 152×108 at 16 gaps from x 252. Default six: Edit, Annotate, Organize Pages,
  Convert (to PDF), Sign, Compress; Customize swaps them. Clicking a tile with no document open
  opens the Open dialog and then the reader in that mode; with files dropped on it, the tool
  runs.
- "Recent files": header at y 428 with a 200×28 search field and a "Newest ⌄" popup right
  (Newest, Name, Size; Clear Recents in the popup's last group). FileCards 152×180 in a grid of
  16 gaps, 6 per row at 1280 (5 at 1024, 8 at 1600). Context menu: Open, Open in New Window,
  Show in Finder, Add to Favourites, Add to Library, Remove from Recents.
- "Favourites": same cards and tiles mixed, files first.
- Library, Tasks, Actions pages keep their current content, laid out on the same 32 margin with
  the same title row; their coloured chrome goes; tool colours become the monochrome icon on a
  `fill-1` tile.
- **All Tools** page: title "All Tools", search field 240 right of the title, chips row (All ·
  Organize · Optimize · Convert · Edit · Protect · Plugins · Custom), then the grid of ToolTiles
  152×108 by category with `fs-title-3` headers. Favourites first when any. Each tile's context
  menu: Add to Favourites, Add to Tools for you. Opening a tile opens a tool tab (batch) — the
  same tool runs on one document from the reader's popover.

### 5.2 Reader: chrome

Toolbar 44 tall, `chrome`, hairline under. Item x positions at 1280 with the sidebar open (260):

| x | Item | Over column | Tooltip |
|---|---|---|---|
| 8 | Sidebar toggle (36) | sidebar | Show or hide the sidebar (⌃⌘S) |
| 48 | Organize grid (36) | sidebar | Show all pages (⌘⇧O) |
| 88 | Reading view (36) | sidebar | Read with the toolbars hidden (⌃⌘R) |
| 132 | Zoom "87% ⌄" (72×28) | canvas | Zoom (⌘0 fit) |
| 208 / 244 | − / + (36 each, one capsule) | canvas | Zoom out (⌘−) / Zoom in (⌘+) |
| 288 | Page field "4 / 7" (88×28) | canvas | Go to a page (⌥⌘G) |
| centred on the canvas | Mode bar 7×48 = 336, items Annotate · Edit · Fill & Sign · Organize · Redact · Measure · Tools | canvas | each "Annotate (⌘1)"… "All tools (⌘K)" |
| 864 | Page theme "Aa" (36) | inspector | Page theme and text size |
| 904 | Search field 160×28 | inspector | Find in the document (⌘F) |
| 1072 | AI (36) | inspector | Ask AI about this document (⇧⌘A) |
| 1112 | Share (36) | inspector | Share this document |
| 1152 | Print (36) | inspector | Print… (⌘P) |
| 1192 | More ⋯ (36) | inspector | More |
| 1236 | Inspector toggle (36) | inspector | Show or hide the inspector (⌃⌘I) |

The mode bar is centred on the window; it never crosses the sidebar edge: when the sidebar is
open at 1280 it sits at x 472–808. Below 1180 wide the − / + buttons fold into the zoom popover
and the page field becomes "4 / 7" text in the status strip; below 1024 the mode bar shows icons
only with a 40 pitch (280 wide). Nothing is dropped silently: whatever folds is in More.

Status strip: none. The page number lives in the toolbar field; the organiser shows its count in
the inspector header. No controls at the bottom edge except the sidebar's page actions bar
(which is in the sidebar column, not the window edge, and has its tooltips flipped above —
§6).

Window title (macOS): the document name, under 15 characters truncated in the middle; the tab
shows the full name.

### 5.3 Reader: modes

Read (no mode): no rail; the inspector shows document info (title, pages, size, security, fonts
count) and is closed by default. Click a mode again, or Escape with nothing selected, returns to
Read.

Mode rail 44 wide between the sidebar and the canvas, `chrome`, hairline right; items 36×36 on a
40 pitch starting at y 90 (8 under the toolbar). Its first item is always the **Select** arrow
(V). Tool defaults and the selection's properties are in the inspector.

| Mode (shortcut) | Rail items, top to bottom | Inspector sections |
|---|---|---|
| Annotate (⌘1) | Select · Highlight · Underline · Strikeout · Squiggly · Note · Text box · Callout · Pen · Eraser · Shapes ⌄ (rect, oval, line, arrow, polygon, polyline, cloud, loupe, mask) · Stamp ⌄ · Signature · Link · Attachment · Date | Colour (6 swatches + custom), Opacity (slider), Stroke (3 widths), Font (family, size, B/I/U) for text, Author; for a selected comment: text, replies, status, "Delete" |
| Edit (⌘2) | Select · Edit text · Add text · Add image · Link · Find & Replace · Spelling · Arrange ⌄ (front/back, align, distribute) | Font family, size, colour, style, alignment; image: replace, rotate, flip, remove background; object: x, y, w, h, rotation |
| Fill & Sign (⌘3) | Select · Fill (hand) · Signature · Initials · Check · Cross · Dot · Line · Text (free) · Prepare form ⌄ (detect, text, choice, button, special) · Clear form | Field properties (name, required, format, validate, calculate, tab order); Signatures list (saved, "Repeat on pages…"); Digital signature ("Sign with certificate…", validate) |
| Organize (⌘4) | Select · Insert blank · Insert file · Rotate left · Rotate right · Duplicate · Extract · Replace · Crop · Resize · Reverse · Split here · Delete (danger) | Selection count, page labels, transitions, "Impose…" (N-up, booklet), undo list |
| Redact (⌘5) | Select · Mark text · Mark area · Mark pages · Find & mark · Review marks · Apply (primary) | Appearance (fill colour, overlay text), Sanitize options, marks list |
| Measure (⌘6) | Select · Distance · Perimeter · Area · Set scale | Scale, units, precision, snap |
| Tools (⌘K) | — opens the popover (not a mode) | the chosen tool's options and **Run** |

The rail scrolls with the mouse wheel when the window is shorter than its items (800 tall holds
17 items at a 40 pitch); it never clips without a scroll.

**Options bar (docked, never floating).** When a mode is on, a secondary bar 36 tall docks under
the unified toolbar across the canvas column (y 82–118, `chrome`, hairline under), as PDF
Expert's annotate bar. It holds the active tool's quick options in one row from x 12: colour
swatches, stroke widths, opacity, and for text (Edit text, Text box, Callout, free Text) the
text-format controls: font popup 140, size field 56, B / I / U, lists, alignment, colour, "More
fonts…". The current floating text-format bar and the floating options strip are replaced by
this bar. Rule: **no mode or option toolbar ever covers the document**; the only floating thing
over a page is the selection bar on selected text, and it flips to stay off the selection.
The canvas starts at y 118 in a mode, y 82 in Read. The inspector holds the full properties;
the options bar holds the four or five a person changes while drawing.

**Page themes.** Day, Sepia and Night change the **page** only; the canvas around the pages stays
the neutral `canvas` colour of the app appearance (light `#E6E6EB`, dark `#1C1C21`), the chrome
never tints. Chosen from the toolbar's "Aa" popover (left of the search field at 1280: Day ·
Sepia · Night as three 64×48 page swatches, then Invert colours, then the reflow text size
slider) and from View ▸ Page Theme ▸ (⌥⌘D / ⌥⌘S / ⌥⌘N). Sepia = cream paper `#F6EFE2`, ink
`#3B3127`, photos untouched; Night = paper `#1A1A1A`, ink `#E6E6E6`, photos untouched (the
rendering is another worker's job; this is the contract). The choice is per document and
remembered; the default is in Settings ▸ Appearance.

Canvas: `canvas` colour, pages centred with 20 gaps, `page` shadow. The selection bar (floating,
when text is selected) stays as today, restyled with `Popover` tokens: Copy · Highlight (colour
dots) · Underline · Strikeout · Note · Link · Look up · AI, 36 tall, 8 above the selection and
flipped below at the top edge.

### 5.4 Sidebar

260 wide (handle 200–480, §6), `chrome`, hairline right.

```
y 82   ┌────────────────────────────┐
       │ [▤][☰][★][✎][📎][◫]        │ segmented tabs 90–118 (margin 16 top/sides)
       │ Thumbnails            ⋯   │ title row 126–162 (15/600 at x 16; actions right at 16)
       │ ┌────────┐                 │
       │ │  page  │                 │ cells from y 170
       │ └────────┘                 │
       │     4                      │
       │ …                          │
       │ ↺ ↻ + ⧉ ⤓ # 🗑           │ page actions bar 756–800 (only on Thumbnails)
       └────────────────────────────┘
```

- Tabs: Thumbnails, Outline, Bookmarks, Annotations, Attachments, Layers (⌥⌘1…6). Six items of
  36×28 = 228 + 16 margins = 260. When the sidebar is narrower than 260 the tabs shrink to 28
  wide (icons only, already the case) down to 200 = 6×28 + 32.
- Thumbnails: cells `width − 32`, 12 gaps, number badge 22×18 under each; the page in view
  has a `fill-1` ground, selected pages a 2 px accent frame + accent badge. Multi-select with
  ⌘ / ⇧; drag to reorder. Context menu = the Organize rail items. The bottom bar holds Rotate
  left, Rotate right, Insert blank, Duplicate, Extract…, Number pages…, Delete, 36×36 on a 36
  pitch from x 8; at a sidebar width under 268 (7×36 + 16) items from the right fold into a ⋯
  menu, Delete last to fold (the first clipped icon was the trash: never again).
- Outline: rows 28, disclosure, depth × 16; + in the title row adds an item for the current
  page; row hover shows Rename and Delete icons; context menu: Add Child, Rename, Delete, Go to.
- Bookmarks: rows 28 with the page number right-aligned `text-2`; + bookmarks the current page.
- Annotations: filter chips row (All · Highlights · Notes · Drawings · Mine) at 8 under the
  title; CommentCards grouped by page with a 28-tall page header "Page 3 · 4"; ⋯ in the title
  row: Summary of comments…, Export (XFDF/FDF)…, Import…, Remove all.
- Attachments: rows 28 with size right; + attaches a file; row actions Save a copy, Rename,
  Delete.
- Layers: rows 28 with a checkbox.
- Search results replace the sidebar? No: they go to the **inspector** column (PDF Expert puts
  them on the right); the sidebar keeps what it showed.

### 5.5 Inspector (right column)

260 wide (240 at 1024, 280 at 1600), `window`, hairline left. Header row 36 tall: title
`fs-title-3` at x 16 (the mode, the tool, "Search", or "Document"), a close × 28×28 at the right
for tools and search. Body scrolls; InspectorSections as listed in §5.3. Rows: label `text` at
x 16, control right-aligned ending at x 244; sliders 120 wide; swatches 18 at 8 gaps. The last
section of a tool is its **Run** row: a primary button full-width-minus-32 at the bottom
(pinned), with "Runs on this computer. Nothing is uploaded." 11 `text-2` under it.

Toggle: ⌃⌘I, the toolbar's trailing button; it opens by itself when a mode is entered or a tool
is chosen and closes when both are left, unless the person pinned it (toggled it by hand).

### 5.6 Search

⌘F or the field: typing shows results live in the inspector column titled "Search" with the count
"108 results" in the header's right, then groups "Page 6 · 16" and rows of 2-line context (13,
the match marked `warning` 30 % behind), 8 pad; ↑ ↓ and ⌘G / ⇧⌘G move the current result (accent
fill row + page scroll); Escape clears and closes. Options popup on the field's magnifier:
Match Case, Whole Words, Search in Folder…, Recent searches (removable one by one, Clear). No
results: "No results for “preload”" with the options still shown. The field grows to 240 when
focused (it takes space from the mode bar's centring, never from the trailing group).

### 5.7 Page organiser (Organize mode)

The canvas becomes a grid: cells `thumb 196 wide`, 56 gaps horizontal, 40 vertical, number
under, left margin 40; 4 columns at 1280 with the sidebar closed (the mode closes the sidebar's
Thumbnails tab since it shows the same thing; it reopens on exit). Selection 2 px accent frame +
badge; rubber-band and ⌘ / ⇧ selection; drag with a 2 px accent insertion bar; drop files from
Finder inserts them. Rail per §5.3; Delete at the bottom of the rail in `danger`. Inspector
"Pages": "3 of 7 selected", page labels, transitions, Impose…, Split… . Zoom slider in the
inspector sets the thumb size 120–320.

### 5.8 Settings window

Own window, 520 wide, title follows the tab, tabs as toolbar icons (Slint draws the icon tab bar
52 tall: icon 24 over an 11 px label, selected `fill-2` radius 6). One grouped form per tab:
sections as cards (`window` on a `chrome` page, radius 10, hairline), rows 28, label left as a
statement, control right. Applies at once; no buttons. Height fits the tab (max 640, then
scrolls). Reopens on the last tab.

| Tab | Rows (today's preferences, regrouped) |
|---|---|
| General | Open at launch (Home / last documents); Language; Check for updates automatically; Open PDFs with Sheaf by default (+ Quick Actions on macOS); Default output folder; Ask where to save each result; Notify when a task finishes |
| Appearance | Appearance (System / Light / Dark — kept because Nassim asked); Accent (system first on macOS, then 6); Page background; Page theme default (Day / Sepia / Night); Smooth text / line art / images; Display colour profile |
| Reading | Default zoom and layout; Remember position; Scroll direction; Page labels as printed; Units; Highlight links and fields; Read-aloud voice and speed; Author name |
| Tools | Default options per tool (a list, Edit…); Tools for you (the six); Custom tools; Plugins (on/off); Watched folders; Network for online conversion |
| AI | Provider, key (masked), model; what is sent; Spaces |
| Shortcuts | the list (action, shortcut, record) with Reset |

Advanced items (data folder, reset, debug, JavaScript console) go at the bottom of General under
"Data"; destructive buttons last with a confirmation.

### 5.9 Sheets

All centred, `r-lg`, `sheet` shadow on a `scrim`, Escape cancels, Return runs the primary.

- **Print** (864×640): preview column 320 (white page on `canvas`, "Page 1 of 7" capsule under,
  ← → to page); form column: Printer, Presets, Copies, Pages (All / Current / Range from–to /
  Selection), Paper size, Orientation, then a "Sheaf" disclosure group (Scale to fit / Scale %,
  Comments and forms, Booklet, N-up). Bottom row: ? · "PDF ⌄" left; Cancel · Print right.
- **Export / Convert** (480): format popup, format options (OCR, layout for Word; quality,
  DPI for images; level for PDF/A), pages, "Where" (folder + name); Cancel · Export.
- **Protect** (480): Open password (field + strength bar), Permissions password, permissions
  checkboxes, encryption popup; Cancel · Protect. Certificates: a second tab "Certificates".
- **Sign with certificate** (480): digital ID popup + Manage…, reason, location, appearance,
  time stamp; Cancel · Sign.
- **Save changes?** (dialog 360): "Save changes to study.pdf?" with Don't Save · Cancel · Save.
- **Password** (dialog 360): field + Unlock.
- **Unable to open** (dialog 360): reason, Repair… · Cancel.

### 5.10 Toasts

Bottom centre, 20 above the window edge, `toast`, 36 tall, text 13, up to 4 s, an "Undo" accent
word when the action can be undone ("Saved study.pdf", "3 pages deleted · Undo", "Compressed to
1.2 MB (−64 %) · Undo"). Never the only place an error shows: errors go in a dialog or inline.
Announced to the screen reader.

---

## 6. The three reported bugs, designed

### 6.1 Tooltip layer

Today each button draws its own tip inside itself, so a sibling painted later (a divider, the
side panel edge, a dropdown) covers it, the panel's clip cuts it, and a tip at the bottom bar
hangs under the window. Design:

- One **`TooltipLayer`** element at the root of `AppWindow`, last child, above every screen,
  popover and sheet. One **`TooltipBridge`** global: `anchor` (x, y, w, h in window
  coordinates), `text`, `shortcut`, `shown`.
- Any hoverable control calls `TooltipBridge.hover(anchor, text)` on `has-hover` true and
  `TooltipBridge.leave()` on false. The anchor is `map-to-window` of the control's rect. No
  control draws a tip itself.
- Timing: show after **600 ms** of rest; while a tip is shown, moving to another anchor within
  **300 ms** shows the new one at once (warm); hide **60 ms** after leaving; a press or a key
  hides at once and arms a 600 ms delay again; the tip never shows while a mouse button is down
  or while a popover opened from the same anchor is open.
- Placement, in window coordinates (W×H), tip size (w×h) measured from its wrapped text (max
  width 280): x = anchor centre − w/2, y = anchor bottom + **6**. If y + h > H − 8 → y = anchor
  top − 6 − h (flip above: the sidebar's page actions bar, the Run button, any bottom anchor).
  Clamp x into [8, W − w − 8]. If the anchor is a vertical rail item (the mode rail, the organiser
  rail) the tip sits to the right: x = anchor right + 6, y centred, flipped to the left when it
  would overflow. Never over its anchor.
- Look: §4 Tooltip. Text "Rotate left" then two spaces then "⌘L" in `text-2`. Fade in 120 ms,
  out 60 ms; no motion under Reduce Motion.
- Keyboard focus shows the same tip after 600 ms (accessibility).

### 6.2 Side-panel width and resize handle

- Width tokens: `sidebar-min 200`, `sidebar-default 260`, `sidebar-max 480`, and never more
  than 45 % of the window. The inspector: 220 / 260 / 340.
- A **resize handle**: an 8-wide invisible hit area centred on the hairline (4 each side), cursor
  `col-resize`, the hairline turns `accent` while hovered or dragged; double-click resets to the
  default. Width saved per window, restored at launch.
- Content never clips: the segmented tabs shrink to icons (28 each) down to 200; the title row
  keeps its ⋯ at the right edge inside a 16 margin; the Annotations filter chips scroll
  horizontally; the "By page ⌄" sort popup has `min-width 88` and truncates its label in the
  middle before the chevron, never clipping the chevron; the page actions bar folds into ⋯ from
  the right (§5.4). When the window itself is narrower than sidebar + rail + 480 canvas, the
  sidebar becomes an overlay (§5 adaptation).

### 6.3 Tooltips at the window edge

Covered by the clamp and flip rules in 6.1, applied to every anchor in the app: toolbar (top:
below), rails (right), sidebar page actions and inspector Run (bottom: above), the trailing
toolbar group (clamped to W − 8).

---

## 7. Accessibility, keyboard, focus

- Every icon-only control has an accessible label equal to its tooltip text without the shortcut.
  Slint: `accessible-role`, `accessible-label`, `accessible-description` (the shortcut).
- Focus order: tab strip → toolbar (leading, mode bar, trailing) → sidebar tabs → sidebar
  content → rail → canvas → inspector. Tab moves between groups; arrows move inside a group
  (segmented controls, rails, lists, grids). Focus ring 2 px accent, keyboard only.
- Nothing is focused when a window, sheet or popover opens, except the search field (⌘F), the
  page field (⌥⌘G), the Go-to and password dialogs, and the Tools popover (its search field).
- Return = default button; Escape = close the topmost popover/sheet, else clear the selection,
  else leave the mode; Delete removes the selection (with Undo); Space = pan / preview; arrows
  move the selection by 1 px, ⇧ by 10.
- Every destructive one-click action (Delete page, Remove all comments, Clear form) has an Undo
  toast and Edit ▸ Undo; nothing asks "Are you sure?" except Apply redactions and Reset settings.
- Contrast: `text` on `window` ≥ 12:1, `text-2` ≥ 4.6:1, `text-3` is for placeholders only;
  `on-accent` passes 4.5:1 on every accent (dark text on yellow and mint).
- Reduce Motion: all `Motion` durations 0, fades only. Reduce Transparency: no change (nothing
  is translucent except the tooltip at 96 %).
- Text never under 10 px; tooltips 11; the UI follows the system text size on macOS where Slint
  allows (default scale factor).
- Right-to-left: the layout mirrors (sidebar right, rail right of it, inspector left), as today's
  `PlatformBridge.rtl`.

---

## 8. Glossary

| Term | Means | Not |
|---|---|---|
| Home | the library tab: Recents, Favourites, Library, Tasks, Actions, All Tools | dashboard, start page |
| Document | a PDF open in a tab | file (on disk), paper |
| Mode | Annotate, Edit, Fill & Sign, Organize, Redact, Measure: a rail of tools + an inspector | tool, panel |
| Tool | one run that transforms a document (Compress…) or many files (Merge) | feature, action |
| Tools popover | the searchable list of tools in the reader (⌘K) | catalog, command palette |
| Tool tab | a batch run on several files | task |
| Action | a saved chain of tools | workflow, automation |
| Sidebar | the left column: Thumbnails, Outline, Bookmarks, Annotations, Attachments, Layers | side panel, navigator |
| Inspector | the right column: properties, options, search results | properties panel, options bar |
| Rail | the vertical strip of the active mode's tools | toolbar, palette |
| Run | the primary button of a tool | Apply, Go, Convert |
| Settings | the ⌘, window | Preferences |
| Comment | any annotation listed in the Annotations tab | markup, note (a Note is one kind) |
