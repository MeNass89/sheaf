# Parallel batch opus-reader (27/10) — notes

Not built or run here (brief rule 1): the orchestrator compiles and runs the tests and smoke steps 200–205.

- C-reader-003: More ▸ More tools ▸ Scrolling, split view, hand ▸ Scroll pages across (PDF Expert's View ▸
  Horizontal scrolling): the pages in one strip scrolled sideways, a page fitting the view's height (Fit page,
  kept on resize); two pages and the cover alone stay pairs in the strip, Right to left reads it from its right end
  (manga), the page grid stays a grid. ← → go a page (a pair) and centre it, Space / Page Down move a view's
  width, a mouse's wheel moves along the strip, a zoom keeps the point under the pointer, renders follow the
  horizontal scroll; the pages centred down a view taller than they are; kept with the document's view (bit 5
  of the remembered spread) (tests `pages_run_across_in_one_strip_scrolled_sideways`, `a_short_strip_is_centred`,
  `pages_across_are_centred_down_a_taller_view`,
  `pages_across_are_gone_to_and_rendered_from_the_horizontal_scroll`; smoke 200).
- C-reader-045: … ▸ Split view side by side / one above the other (PDF Expert's View ▸ Split View, Acrobat's
  Window ▸ Split): a second pane, opened on the place the first shows. Each pane is a viewer of its own
  (`crate::split`): its own zoom, place and renders. A press in the other pane makes it the active one and goes on
  to it, so a selection, a link, a comment picked, a drag work there through the same handlers; ⌘+ / ⌘−, the page
  field and the thumbnails then act on it. The other pane follows the active one's edits (its revisions) and page
  changes (laid out again at its own zoom). Its Sync scroll button (PDF Expert's Sync Scroll) moves both together,
  in proportion to their lengths; Close. … ▸ Split with <document> (PDF Expert's split of two documents): the
  other pane shows another open tab's working copy, named at its top, followed as it changes; a click in it brings
  that tab forward with this one in its other pane, each half showing what it showed; the split closes with that
  tab (tests `the_panes_have_their_own_zoom_and_swap_on_a_press`, `the_other_pane_follows_edits_and_page_changes`,
  `a_split_with_another_document_hands_over_and_closes_with_its_tab`; smoke 201, 203, 204).
- C-reader-010: the hand tool (⇧H, or … ▸ Hand tool; PDF Expert's Pan & Zoom, Acrobat's Hand): a plain drag
  moves the pages, ⌥ + drag up zooms in and down out around the press (Acrobat's Dynamic Zoom); a note at the
  bottom says how to leave it (⇧H, Escape, or picking another tool) (smoke 202).
- Bug found on the way: ↓ ↑ (scroll by) and Page Down / Up put back the horizontal scroll of an earlier zoom or
  jump (the tab still carried it); they now leave the horizontal scroll alone.

## Left, and why

- C-reader-045: a click in another document's pane only brings its tab forward (that click is not passed on, the
  next one is). A double-click that starts in the other pane is two single presses (the first swaps the panes).
  The split stays with the tab in view: the document left behind shows alone when its tab is chosen again.
- C-reader-010: the reviewers' gap (a real macOS trackpad session: pinch on page 30+, slow and fast ⌘ + trackpad,
  Magic Mouse) needs a Mac; not doable on this VPS.
- not automatable: horizontal pinch under Xvfb (no gesture events).

## For the orchestrator

- No new dependency.
- New module `src/split.rs` (`mod split;` in main.rs). New pub items: `Viewer::{wanted_at, set_horizontal,
  horizontal, set_height, set_left, row_of, scroll_x_to}`, `ViewerUi::{twin, follow, swap_strip, pane_pages, pane}`,
  `TabState::split`, `reader_ui::toggle_horizontal` (nav.rs). `viewer_ui::request` now queues the split's other pane
  too; renders find their viewer by load id (`update_doc`).
- Shared files touched (small blocks): `ui/flow-types.slint` (ReaderState `horizontal`; ToolTab `split-*`; ReaderBridge
  `toggle-horizontal`, `split-view`, `split-focus`, `split-sync`, `split-with`, `split-names`; ViewerBridge
  `second-viewport`), `src/main.rs` (`mod split;`, wirings), `reader_ui/mod.rs` (one line in `restore_view`, one in
  `update`), `reader_ui/model.rs` (one field), `state.rs` (TabState `split`, one line in `build`).
- `Viewer::content_width` now returns a width computed in `relayout` (same value down a column).

## After Sol's review (27/10)

- C-reader-003: ⌘+ / ⌘−, a typed zoom and Fit keep the page in view deep in a strip (the zoom anchored on the
  strip's start, sx 0, and jumped back toward page 1): `zoom_to` now anchors on the view's top middle at the
  current horizontal scroll (test `a_zoom_deep_in_a_strip_stays_on_the_page_in_view`). In the page grid, Page
  Down / Space and jumps scroll down again: they follow `Viewer::across`, false there, not the kept setting.
- C-reader-045: Sync scroll and zoom (PDF Expert's): on, the other pane also takes the active pane's zoom, as an
  actual size (another document's pages may be another size), on the same point of its page (test
  `sync_scroll_and_zoom_takes_the_active_panes_zoom`; smoke 205). A click in another document's pane brings its
  tab forward and goes on to it at the same point (a link, a comment, a note placed); a drag that starts there
  starts after. The divider drags (a 7 px grip, the share kept from a fifth to four fifths, carried with the
  hand-over; smoke 205).
- C-reader-010: an ⌥-drag zoom with the hand tool ends when the keyboard focus goes, or at the next move once the
  button is no longer held (let go outside the window).
  not automatable: focus taken away mid-drag under Xvfb (no second window to take it while the button is held).
