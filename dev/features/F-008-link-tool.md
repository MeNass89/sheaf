# F-008 · Link tool and the reader's edit group (C-edit-001, C-edit-021, C-annotate-021)

Main app: PDF Expert (Edit ▸ Text, Image, Link, Redact in one mode). Acrobat: Edit a PDF ▸ Link (drag a box,
Link Properties: page view or open a web page; select a link to change or delete it).

## Closing criteria
- **Function.** In the reader, the Link tool (L) makes a link by dragging a box and typing a page number or a web
  address (an e-mail address becomes mailto:); clicking a link with the tool changes where it goes or removes it;
  saved, the file has the /Link annotation. The edit group also opens the document in Redact (saved first).
  Proof: `reader::tests::a_link_goes_to_a_page_number_or_a_web_address`,
  `reader_doc::tests::a_link_is_added_changed_and_removed_and_saved`, gui-smoke 19.
- **Aesthetics.** The tool shows the page's links; the box being linked stays outlined while its target is typed;
  the editor says what to type and what was wrong; the toolbar still fits a 1280 px window (search narrower).
- **Performance.** One edit per link; links read back from the snapshot.

## Journal
- 24/10 21:00: PdfCraft had the link edits (AddLink, SetLink, DeleteLink) and nothing in Sheaf used them. Link tool
  13 (`reader_ui/pointer.rs`: drag → editor kind 8, click on a link → kind 9; `edits.rs` `note_commit`;
  `reader::link_action` parses "3", "p. 4", "example.org", "ada@example.com"); the note editor reused (one line,
  ↩ sets, "Remove link" for an existing one, an error line when the text is neither a page nor an address).
  Redact button (eye-off) in the edit group: `saving::to_redact` (save first, then `flow::open_tool(Redact)`).
  The two buttons pushed Save off a 1280 px toolbar: the search field's share narrowed by 64 px; smoke positions
  of the zoom field updated (ZOOM_X 652 → 716). Evidence: /tmp/o6/edit (edit text active, a paragraph in the
  editor, a new link, an existing link).
- 24/10 23:30, reviewer: C-edit-001 and C-annotate-021 closed 9/9/9; C-edit-021 7/8/9. Its blockers:
  1. Link from selected text: the selection bar's Link (after Strikethrough) asks the target; one box per line,
     one undo step (`tools::link_selection`, a Batch of AddLink).
  2. Links from the text's web addresses: the Link tool's options bar ▸ "Link web addresses" (`Op::LinkUrls`:
     PdfCraft's `find_urls` + `AddLinks`; addresses already under a link are skipped, `reader_doc::unlinked`, so a
     second run adds nothing and says so).
  3. Appearance: the options bar's Border (Acrobat's visible rectangle; invisible by default, as Acrobat), then
     Dashed, the width dots and the colour dots. PdfCraft wrote `/BS` and `/C` but no appearance stream, and
     Sheaf's view (and other readers that draw only appearances) showed no border: patched (`links.rs`
     `write_appearance`). The reader_doc tests were run with `--lib` at first, which runs none of them (they live
     in the app binary): run with `--bin sheaf-app`.
  Tests: `a_visible_link_is_drawn_by_its_appearance` (red without the appearance),
  `web_addresses_in_the_text_are_linked_once`, `an_address_under_a_link_is_not_linked_again`, gui-smoke 19b
  (select "quick brown", Link, "3": saved with one /Dest). Not done: moving or resizing an existing link's box.
