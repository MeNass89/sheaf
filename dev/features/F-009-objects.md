# F-009 · Select several page objects, move and nudge them (C-edit-007)

Main app: Acrobat (Edit a PDF: select objects, ⇧-click adds, a selection rectangle; drag; arrow keys nudge).
iLovePDF: drag handles, arrow keys nudge.

## Closing criteria
- **Function.** In the Edit tool, ⇧-click picks paragraphs and images together (again to unpick), a drag on empty
  space draws a marquee that picks what it touches, a drag on a picked object moves them all (PdfCraft
  `MoveObjects`, one undo step), ←↑→↓ nudge them 1 point (⇧ 10); Escape lets go. A pressed image is picked too.
  Proof: `reader::tests::objects_are_picked_by_point_by_marquee_and_found_again_after_a_move`, gui-smoke 20.
- **Aesthetics.** Picked objects outlined, the outlines moving with the drag, the content following live.
- **Performance.** The page's objects read with its paragraphs (one call); one edit per move.

## Journal
- 25/10 02:30: the document thread now sends each page's objects as Edit a PDF selects them together
  (`editable_objects`: inventory target and box) with its paragraphs. ⇧ reaches the press as 16 added to the
  click count. After a move the objects are found again where they went in the page read again
  (`reader::picked_again`), so a second nudge names the right ones; a nudge waits for the last one.
  Two paragraphs: drag +73.4 pt, ⇧→ +10 pt, both the same, saved (gui-smoke 20).
  Not done: resizing a paragraph's box (rewrap to a new width); vector drawings are not objects PdfCraft can move.
