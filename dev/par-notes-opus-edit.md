
## Round 28 fixes (Sol's third review; C-annotate-015 and C-edit-024 agreed at 9)

- C-edit-004: More fonts lists every installed font; those PdfCraft may not embed (licence not
  declared open, embedding forbidden, variable font) are dimmed "— not embeddable" and, picked,
  say why (`SystemFont::why_not`; test `the_computers_open_fonts_are_listed_by_name` with Caveat).
- C-edit-009: a moved object's parameters and `gs` operators are set again in the content's order
  (a `5 w` after a `gs` setting 1 point stays 5) (test `a_drawing_brought_to_the_front_keeps_a_width_set_after_its_gs`).
- C-annotate-009: B/I take a `/Helv` resource for Helvetica only when its font is Helvetica
  (`standard_face`; test `an_imported_helv_is_not_taken_for_helvetica`).
- C-annotate-022: a rich text box with runs of its own is recoloured in its drawn appearance (text
  fill colours replaced, runs, fonts, sizes and backgrounds kept), not redrawn from plain text
  (`recolour.rs`; test `a_recoloured_rich_text_box_keeps_its_mixed_runs` checks the appearance).
