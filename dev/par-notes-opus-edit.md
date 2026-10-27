# Parallel batch opus-edit (27/10): notes

Not built or run here (brief rule 1): the orchestrator compiles and runs the suite.

- C-annotate-009 / C-annotate-022: a text box whose appearance draws it in a font of the file's own
  that is not a simple WinAnsi one (a Type0 font, as Acrobat embeds for Japanese, or a simple font
  with its own `/Differences`) keeps that font through a new size, alignment or colour: its lines
  are measured with the font's widths and written in its codes (`appearance::build_encoded`,
  `encoded_text_font`); such a box is no longer refused as "foreign text" nor marked as drawn with
  `?` when its font has every character (test `a_text_box_in_a_type0_or_custom_encoded_font_keeps_it_through_a_new_size_and_colour`,
  in `crates/sheaf-core/tests/engine_patches.rs`).
- C-edit-009: Edit tool, one object picked or more ▸ Order: To front, Forward, Backward, To back
  (Acrobat's Arrange: Bring to Front, Bring Forward, Send Backward, Send to Back), one undo step
  each, the objects staying picked. Drawings, images, paragraphs and added items; each is drawn
  again in a `q … Q` that sets the state it was drawn in (clip, matrix, colours, line style, text
  state), a paragraph's line moves left where they were; one step forward or backward goes past
  the nearest object it overlaps, or moves that object instead when the content there is clipped
  (pdfcraft-edit `restack.rs`; tests `a_drawing_is_brought_to_the_front_and_sent_to_the_back`,
  `a_drawing_steps_past_one_drawn_in_a_clipped_group`, `words_are_brought_in_front_of_a_box`, in
  `crates/sheaf-core/tests/arrange_order.rs`). The Align/Space card now shows for one object too,
  with only the Order row then.
- C-edit-004: with a paragraph open in Edit, U (underline) and • (bulleted list) apply to it, as
  size and family do (Acrobat's Edit PDF format panel; the • is lit when the paragraph opened is a
  list, and turning it off takes the bullets away); More fonts… lists the computer's fonts that may
  be embedded (TrueType/OpenType files of the system's font folders whose name table declares an
  open licence: DejaVu, Liberation, Noto, Ubuntu…; Acrobat's font list), and the paragraph is set in
  the one picked, embedded (`sheaf_core::system_fonts`, `Edit::EditTextBlockInFont`; tests
  `underline_bullets_and_fonts_apply_to_the_paragraph_open`, `a_bulleted_list_is_undone_and_told`,
  `the_computers_open_fonts_are_listed_by_name`,
  `a_paragraph_is_set_in_a_font_of_the_computers_and_underlined`).
- C-annotate-015: Shapes ▸ Loupe: an oval dragged becomes a circle showing the page under it twice
  as large, ringed in the pen's colour; Shapes ▸ Mask: a rectangle or oval dragged leaves that area
  alone and dims the rest of the page (Preview's Loupe and Mask). Both are comments any viewer
  shows (a Circle whose appearance draws a form of the page magnified, a Square over the page with
  an even-odd dim), redrawn by `build_appearance` when moved, resized or recoloured
  (pdfcraft-annot `loupe_mask.rs`; tests `a_loupe_shows_the_page_under_it_magnified`,
  `a_mask_dims_the_page_outside_its_opening`). A mask is picked from the Comments panel, not by a
  click on its page (it covers the page).
- C-edit-024: Shapes ▸ Article (Acrobat's Article tool): boxes dragged one after the other make
  an article's reading order across columns and pages, saved as the catalog's `/Threads` (a ring
  of beads, each page's `/B`) that Acrobat and Preview follow; the articles' boxes show while the
  mode is on (the one being drawn brighter); End article starts the next, Delete article removes
  it (pdfcraft-engine `articles.rs`; test `boxes_are_chained_into_an_article_across_columns_and_pages`).

## Second pass (27/10)

- C-edit-024: with Article on, a click on a box picks it (outlined stronger; new boxes then
  continue its article), a drag from it moves it, a drag from its bottom-right corner resizes it,
  Delete removes that box alone and joins the boxes before and after it (its article's last box
  removes the article) (Acrobat's Article tool; `reader_ui/article_boxes.rs`, engine
  `set_article_box`, `delete_article_box`; test `an_article_box_is_moved_resized_and_deleted_alone`).
- C-edit-004: More fonts… (now last in the format row, so the buttons before it keep their
  places) applies to new text (written as page content in the embedded font, aligned and
  underlined as asked: pdfcraft-edit `text_in_font.rs`) and to text boxes, new or selected (the
  font embedded into the box's appearance, `/DA` naming it, kept through size and colour:
  pdfcraft-annot `text_box_font.rs`) (test `new_text_and_text_boxes_take_a_font_of_the_computers`).
- C-annotate-015: a loupe's magnification is chosen (1.5×, 2×, 3×, 4×: a click on the row's
  button goes to the next, for new loupes and the loupe selected); after any edit of the page's
  content, a loupe whose page changed takes a new copy of it and is redrawn (`refresh_loupes`,
  run by the engine after content edits); a mask is picked by a click in its opening or on its
  border (its opening is its hit box) (tests `a_loupe_follows_its_magnification_and_its_page`,
  `a_masks_opening_is_known`).
- Smoke 210–214 (`3-flows.sh`): Order ▸ To front (by pixel colour), U and • on a paragraph,
  More fonts… on a paragraph (a Type0 font embedded), Shapes ▸ Loupe and Mask, Shapes ▸ Article
  with a box deleted alone. Buttons found by OCR (`at`); 211 clicks U and • by their offset from
  Mono, as step 2c does; 212 clicks the list's first row 40 px under the button.

## Left, and why

- C-edit-004: Acrobat lists every system font; Sheaf lists the openly licensed ones (PdfCraft's
  asset policy refuses others). New text in a computer's font is page content (a paragraph the
  Edit tool changes afterwards), not an added item re-editable as typed; each typed line is one
  line (not rewrapped to the box).
- C-edit-009: inline images (`BI … EI`) and content inside form XObjects are not objects of the
  Edit tool, so they are not arranged. Keyboard shortcuts were not added (⌘[ ⌘] and ⌘⇧[ ⌘⇧] are
  taken by history and tabs).
- C-annotate-015: a loupe's page copy follows content edits, not resource-only changes; its
  circle is resized as other circles are (no handle of its own).
- C-edit-024: a box is moved within its page (not dragged to another page); no live handles are
  drawn on the picked box (its corner works without one); a box is not inserted between two.

## Smoke

- Steps 210–214 written, never run (rule 1): the orchestrator's first run may need an offset
  fixed. The Objects card now shows for one picked object too (top-right of the view): earlier
  steps that pick one object and then click near that corner should be watched.

## For the orchestrator

- No new dependency.
- New pub items: pdfcraft-edit `Restack`, `restack_objects`; pdfcraft-annot `loupe_mask`
  (`add_loupe`, `add_mask`), `encoded_text_font`, `appearance::build_encoded`, `appearance::own_width`;
  pdfcraft-engine `articles` (`Article`, `articles`, `add_article_box`, `delete_article`,
  `Document::articles`), `Edit::RestackObjects`, `Edit::EditTextBlockInFont`, `Edit::AddLoupe`,
  `Edit::AddMask`, `Edit::AddArticleBox`, `Edit::DeleteArticle`, `Edit::SetArticleBox`,
  `Edit::DeleteArticleBox`, `Edit::AddTextInFont`, `Edit::AddTextBoxInFont`,
  `Edit::SetTextBoxFontProgram`, `Edit::SetLoupeMagnify`, `Document::mask_holes`,
  `loupe_magnifications`; pdfcraft-edit `add_text_in_font`; pdfcraft-annot
  `text_box_font::set_text_box_font_program`, `loupe_mask::{set_loupe_magnify, refresh_loupes,
  mask_holes, MAGNIFICATIONS}`; sheaf-core `system_fonts`
  (`SystemFont`, `font_dirs`, `list_fonts`, `installed`).
- New fields: `ReaderUi.text_font`, `shape_special`, `article_thread`, `article_box`,
  `loupe_magnify`; `Snap.articles`; `ReaderState.text-font`, `shape-special`, `loupe-magnify`,
  `selected-loupe`; `ReaderBridge.system-fonts`. No `Prefs` field. A mask's `AnnotEntry.quads`
  is its opening (`reader_doc::snap`).
- The engine now runs `refresh_loupes` after every content-scope edit (`Session` apply, after
  `run_edit`): it returns at once when the document has no loupe.
- Shared files touched (small blocks): `ui/flow-types.slint`, `ui/screens/reader.slint` (the
  marks' colours for kinds 16/17, the card's condition), `reader_ui/mod.rs`,
  `third_party/pdfcraft-engine/src/lib.rs`, `crates/sheaf-core/tests/engine_patches.rs` (appended).

## Fixes after Sol's review (27/10, on dev/round26)

- C-edit-024: deleting an article's last box renumbers the article being drawn (gone: the next
  box starts a new article; an article after it moves down one place) (`article_boxes::after_delete`,
  test `the_article_being_drawn_follows_a_deleted_article`).
- C-edit-009: Forward/Backward take the picked objects as they are (no finding again by kind and
  place) and move them together past the nearest object one of them overlaps; twins of the same
  kind and size at the same place pass each other (test `twin_drawings_at_the_same_place_pass_each_other`).
- C-annotate-022: a text box whose own font sits under a standard resource name (`/Helv` pointing
  at another font) keeps it through a recolour, its non-WinAnsi text in the font's codes
  (`foreign_under_standard_name`; test `a_text_box_whose_own_font_is_named_helv_keeps_it_through_a_new_colour`).
- C-annotate-009: B and I on a text box in a font that is not a standard one pick that font's own
  bold or italic face installed on the computer (`system_fonts::variant`, the box's `/BaseFont`
  now in `AnnotEntry.font_name`, `Document::text_box_base_fonts`); none installed: said
  (test `bold_and_italic_pick_the_fonts_own_faces`).
- C-annotate-015: a moved loupe is drawn again from its new place; a moved mask moves its opening
  (`move_annotation`; test `a_moved_loupe_shows_its_new_place_and_a_moved_mask_its_new_opening`).
- C-edit-004: 1. (numbered list, Acrobat's) beside •, for new text and the paragraph open; the two
  replace each other; a list made again is renumbered (tests `lines_become_a_numbered_list_and_back`,
  `para_bits(10)`). Smoke 2c's + moved to 825 (the new button shifts the row by 34 px).

## Round 27 fixes (Sol's second review)

- C-edit-004: B and I after More fonts… pick the chosen font's own bold/italic face (`system_fonts::face`).
- C-edit-009: after Backward or To back, the picked object (drawn first of its twins) stays picked
  (`reader::picked_again_from`; test `twins_are_picked_again_in_the_order_the_restack_left_them`).
- C-annotate-009: a new callout takes the format row's family and style or a font of the computer's
  (test `a_callout_takes_the_chosen_font`).
- C-annotate-022: a recolour keeps a rich text's mixed runs, only its text colours change
  (`recoloured_css`; test `a_recoloured_rich_text_box_keeps_its_mixed_runs`).
- C-annotate-015: a mask is resized by its opening; its box in the reader is its opening
  (test `a_resized_mask_resizes_its_opening`).
- C-edit-024: Undo or Redo forget an article being drawn that no longer exists
  (`article_boxes::reconcile`; test `an_undone_article_is_no_longer_drawn`).
- Smoke 210–214 run on the real app (display 111): ALL PASS after fixing 210 (To front found in the
  card by OCR) and 212 (the list's first row clicked inside the list, applied with Done). The
  Objects card was too narrow for the Order row ("To back" cut off): now 420 px.
