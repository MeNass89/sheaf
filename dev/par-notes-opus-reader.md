
## Round 31 (Sol's round 30 review)

- C-reader-037: In a folder… lists the PDFs it could not read, each with why (protected by a password, damaged),
  under "Not searched", and counts only the PDFs read ("4 pages found in 1 PDF. 601 PDFs could not be searched")
  (tests `a_folders_search_lists_a_protected_pdf_as_not_searched`, `a_folders_search_reads_every_pdf_and_says_when_it_stops`;
  smoke 311).
- C-reader-045: a double-click whose first click lands in another document's pane: the tab comes forward at the
  first release, the second click is made a double-click there (`reader_ui::pointer::HandedClick`; test
  `a_click_after_a_hand_over_at_the_same_point_is_its_second`; smoke 207b, which copies '' without the fix).
- C-reader-010: the pinch through the reader's own gesture handler, in Slint's testing backend
  (`crates/sheaf-app/tests/pinch.rs`): the zoom follows the fingers' scale around their point, and over another
  document's pane that tab comes forward and zooms. Still not a real macOS trackpad.

## Round 28 (Sol's third review)

- C-reader-003: pages across and right to left: Space, Page Down and a mouse's wheel go on to later pages, leftward
  (they added to the horizontal scroll, so at page 1, on the right end, they stayed there) (`Viewer::across_step`;
  test `a_step_across_goes_forward_in_reading_order`).
- C-reader-045: a press in another document's pane goes on to that document with its drag: the tab comes forward
  at the press, the drag's points follow the pointer from there, the release ends it (a selection, a comment
  dragged); it was a click at once (smoke 206).
- C-reader-010: a pinch, or ⌘ / Ctrl + wheel, over another document's pane brings its tab forward and zooms that
  document around the fingers or the pointer (smoke 207 for the wheel).
  not automatable: a pinch under Xvfb (no gesture events).
