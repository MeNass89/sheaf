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
- 25/10 13:30, reviewer 8/8/9 on 4822c52. Taken:
  - Resize: one picked paragraph shows four corners; pulling a side rewraps it to the new width (left side: width
    and place; `EditTextBlock` with `BlockStyle::width`/`offset`, live preview, at least 24 pt); the box shows the
    new width and about the height of the same text; picked again after. gui-smoke 20b (pulled 200 px: "here"
    falls below "Alpha" in the saved file).
  - Old outlines of picked objects hidden while they move; their outlines follow the offset of the last preview
    sent (`live_offset`), not the pointer ahead of the page; several picked get one box around them; the text
    format row is hidden while objects are picked (an image's own bar stays).
  Left: vector drawings cannot be picked (PdfCraft's object inventory: added items, paragraphs, images/forms).
- 25/10 15:00, drawings: PdfCraft patched (`third_party/pdfcraft-edit/src/paths.rs`): each painted path (its
  construction operators to its painting operator; a clipping path or an ended one is none) is an object,
  `ObjectKind::Path`, its box under the current transform; `move_objects` wraps it in `q cm … Q`. Past 4,000
  paths on a page none is offered (the inventory's limit, picking stays usable). In Sheaf a single picked drawing
  is dragged directly. MICI: the green title band picked by ⇧-click and dragged moves alone (its white title stays,
  as in Acrobat). Tests: `drawings_are_objects_that_move_alone` (sheaf-core edit_text: the square and the line
  listed, not the clip; the square moved, the line and the word not; qpdf clean), gui-smoke 20c (rendered from
  the saved file: the square lower, the word in place).
