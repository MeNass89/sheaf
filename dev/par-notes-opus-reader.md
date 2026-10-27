# Parallel batch opus-reader (27/10) — notes

Not built or run here (brief rule 1): the orchestrator compiles and runs the tests and smoke steps 200–202.

- C-reader-003: More ▸ More tools ▸ Scrolling, split view, hand ▸ Scroll pages across (PDF Expert's View ▸
  Horizontal scrolling): the pages in one strip scrolled sideways, a page fitting the view's height (Fit page,
  kept on resize); two pages and the cover alone stay pairs in the strip, Right to left reads it from its right end
  (manga), the page grid stays a grid. ← → go a page (a pair) and centre it, Space / Page Down move a view's
  width, a mouse's wheel moves along the strip, a zoom keeps the point under the pointer, renders follow the
  horizontal scroll; kept with the document's view (bit 5 of the remembered spread)
  (tests `pages_run_across_in_one_strip_scrolled_sideways`, `a_short_strip_is_centred`,
  `pages_across_are_gone_to_and_rendered_from_the_horizontal_scroll`; smoke 200).
- C-reader-045: … ▸ Split view side by side / one above the other (PDF Expert's View ▸ Split View, Acrobat's
  Window ▸ Split): a second pane on the same document, opened at the place the first shows, scrolled on its
  own; its Sync scroll button (PDF Expert's Sync Scroll) moves both together at the distance they had; Close.
  The panes are the same size, so the pages are laid out alike and rendered once for both (the second pane's
  pages are added to those rendered) (test `a_split_view_shows_both_panes_pages_once`; smoke 201).
- C-reader-010: the hand tool (⇧H, or … ▸ Hand tool; PDF Expert's Pan & Zoom, Acrobat's Hand): a plain drag
  moves the pages, ⌥ + drag up zooms in and down out around the press (Acrobat's Dynamic Zoom); a note at the
  bottom says how to leave it (⇧H, Escape, or picking another tool) (smoke 202).
- Bug found on the way: ↓ ↑ (scroll by) and Page Down / Up put back the horizontal scroll of an earlier zoom or
  jump (the tab still carried it); they now leave the horizontal scroll alone.

## Left, and why

- C-reader-045: two *different* documents side by side (PDF Expert offers it) is not done: each tab is its own
  reader screen stacked in `main.slint`, so it needs the window to place two tabs' screens in halves and route
  clicks and keys to the one clicked. The second pane shows pages and their comments as drawn but takes no
  clicks (tools act in the first pane), and its zoom is the first pane's (PDF Expert's unsynced panes zoom
  apart).
- C-reader-010: the reviewers' gap (a real macOS trackpad session: pinch on page 30+, slow and fast ⌘ + trackpad,
  Magic Mouse) needs a Mac; not doable on this VPS.
- not automatable: horizontal pinch under Xvfb (no gesture events).

## For the orchestrator

- No new dependency.
- New pub items: `viewer::pages_in_view`, `Viewer::{wanted_at, set_horizontal, horizontal, set_left, row_of,
  scroll_x_to}`, `viewer_ui::second_viewport`, `reader_ui::toggle_horizontal` (nav.rs).
- Shared files touched (small blocks): `ui/flow-types.slint` (ReaderState `horizontal`, ReaderBridge
  `toggle-horizontal`, ViewerBridge `second-viewport`), `src/main.rs` (two wirings), `reader_ui/mod.rs`
  (one line in `restore_view`), `reader_ui/model.rs` (one field).
- `Viewer::content_width` now returns a width computed in `relayout` (same value down a column).
