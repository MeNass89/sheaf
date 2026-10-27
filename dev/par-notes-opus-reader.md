
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
