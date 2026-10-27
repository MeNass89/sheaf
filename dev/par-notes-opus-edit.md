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

## Left, and why

- C-edit-004: added text (new text on the page) and text boxes still offer the three standard
  families only: PdfCraft's added text (`AddedText.family`) and text box appearances take a
  standard font; an embedded font there is engine work of its own. Acrobat lists every system
  font; Sheaf lists the openly licensed ones (PdfCraft's asset policy refuses others).
- C-edit-009: inline images (`BI … EI`) and content inside form XObjects are not objects of the
  Edit tool, so they are not arranged. Keyboard shortcuts were not added (⌘[ ⌘] and ⌘⇧[ ⌘⇧] are
  taken by history and tabs).
- C-annotate-015: a loupe shows the page as it was when the loupe was placed (its content copied
  into a form); Preview's magnification can be changed afterwards, Sheaf's is 2× at placing.
- C-edit-024: boxes are not resized or moved after drawing (Acrobat's handles), and a box is not
  picked by clicking it: End/Delete act on the article being drawn, else the last one.

## Smoke

- not automatable here: the Order row, More fonts…, Loupe / Mask / Article buttons — their screen
  places are unknown without running the app (rule 1); the engine behaviour each drives is tested
  above. Suggested smoke steps 210–214 for the orchestrator: Edit ▸ pick a drawing ▸ To front;
  open a paragraph ▸ U and • ▸ ⌘↩; Shapes ▸ Loupe ▸ drag; Shapes ▸ Mask ▸ drag; Shapes ▸ Article ▸
  drag twice.

## For the orchestrator

- No new dependency.
- New pub items: pdfcraft-edit `Restack`, `restack_objects`; pdfcraft-annot `loupe_mask`
  (`add_loupe`, `add_mask`), `encoded_text_font`, `appearance::build_encoded`, `appearance::own_width`;
  pdfcraft-engine `articles` (`Article`, `articles`, `add_article_box`, `delete_article`,
  `Document::articles`), `Edit::RestackObjects`, `Edit::EditTextBlockInFont`, `Edit::AddLoupe`,
  `Edit::AddMask`, `Edit::AddArticleBox`, `Edit::DeleteArticle`; sheaf-core `system_fonts`
  (`SystemFont`, `font_dirs`, `list_fonts`, `installed`).
- New fields: `ReaderUi.text_font`, `shape_special`, `article_thread`; `Snap.articles`;
  `ReaderState.text-font`, `shape-special`; `ReaderBridge.system-fonts`. No `Prefs` field.
- Shared files touched (small blocks): `ui/flow-types.slint`, `ui/screens/reader.slint` (the
  marks' colours for kinds 16/17, the card's condition), `reader_ui/mod.rs`,
  `third_party/pdfcraft-engine/src/lib.rs`, `crates/sheaf-core/tests/engine_patches.rs` (appended).
