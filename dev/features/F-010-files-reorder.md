# F-010 · Reorder a tool's input files by drag (C-files-015)

Main app: iLovePDF (merge, JPG to PDF: cards dragged into a new order, a gap opens where the card goes).
Stirling PDF: drag-sort of the file list.

## Closing criteria
- **Function.** In any tool that takes several files, a file card pressed and moved more than 6 px is dragged;
  released, it lands before the slot under the pointer (after the last card past the end), the order is kept
  for the run. A click without movement still opens the card; hover buttons (remove, move) still work.
  Proof: `flow::tests::a_dropped_card_lands_before_the_slot_it_is_dropped_on`, gui-smoke 21 (a.png dragged past
  c.png: cards read b c a).
- **Aesthetics.** The dragged card dims to 35 %, a ghost label with its name follows the pointer, an accent bar
  (3 px) shows where it will land; hover buttons are hidden while dragging.
- **Performance.** The drag is UI only (Slint properties); one reorder and refresh on release.

## Journal
- 25/10: drag reorder built (`flow::place_file`, `dropped_at`; `file-card.slint` drag-to/drag-end;
  `tool-flow.slint` drop slot, bar and ghost). Manual check: three PNGs (blue a, orange b, green c) in JPG to
  PDF, a dragged past c: the bar shows after c, the ghost reads "a.png"; the PDF made has pages orange, green,
  blue (b c a). Became gui-smoke 21.
