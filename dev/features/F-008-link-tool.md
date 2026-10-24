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
