# F-015 · Parallel batches (rounds 26–27, 27/10/2026)

Six workers (three Opus, three Sol), each on a batch of related components in its own worktree, paired for joint grades (Nassim, 27/10): opus-reader ↔ sol-convert, opus-edit ↔ sol-ocr-sign, opus-platform ↔ sol-batch. Merged in rounds 26 and 27; the GUI smoke runs in three shards at once (dev/scripts/gui-parallel.sh). Each batch's own notes follow, as its author left them (tests, limits, grades), then the reviews' verdicts are in the board's history.

## Batch opus-reader


Not built or run here (brief rule 1): the orchestrator compiles and runs the tests and smoke steps 200–205.

- C-reader-003: More ▸ More tools ▸ Scrolling, split view, hand ▸ Scroll pages across (PDF Expert's View ▸
  Horizontal scrolling): the pages in one strip scrolled sideways, a page fitting the view's height (Fit page,
  kept on resize); two pages and the cover alone stay pairs in the strip, Right to left reads it from its right end
  (manga), the page grid stays a grid. ← → go a page (a pair) and centre it, Space / Page Down move a view's
  width, a mouse's wheel moves along the strip, a zoom keeps the point under the pointer, renders follow the
  horizontal scroll; the pages centred down a view taller than they are; kept with the document's view (bit 5
  of the remembered spread) (tests `pages_run_across_in_one_strip_scrolled_sideways`, `a_short_strip_is_centred`,
  `pages_across_are_centred_down_a_taller_view`,
  `pages_across_are_gone_to_and_rendered_from_the_horizontal_scroll`; smoke 200).
- C-reader-045: … ▸ Split view side by side / one above the other (PDF Expert's View ▸ Split View, Acrobat's
  Window ▸ Split): a second pane, opened on the place the first shows. Each pane is a viewer of its own
  (`crate::split`): its own zoom, place and renders. A press in the other pane makes it the active one and goes on
  to it, so a selection, a link, a comment picked, a drag work there through the same handlers; ⌘+ / ⌘−, the page
  field and the thumbnails then act on it. The other pane follows the active one's edits (its revisions) and page
  changes (laid out again at its own zoom). Its Sync scroll button (PDF Expert's Sync Scroll) moves both together,
  in proportion to their lengths; Close. … ▸ Split with <document> (PDF Expert's split of two documents): the
  other pane shows another open tab's working copy, named at its top, followed as it changes; a click in it brings
  that tab forward with this one in its other pane, each half showing what it showed; the split closes with that
  tab (tests `the_panes_have_their_own_zoom_and_swap_on_a_press`, `the_other_pane_follows_edits_and_page_changes`,
  `a_split_with_another_document_hands_over_and_closes_with_its_tab`; smoke 201, 203, 204).
- C-reader-010: the hand tool (⇧H, or … ▸ Hand tool; PDF Expert's Pan & Zoom, Acrobat's Hand): a plain drag
  moves the pages, ⌥ + drag up zooms in and down out around the press (Acrobat's Dynamic Zoom); a note at the
  bottom says how to leave it (⇧H, Escape, or picking another tool) (smoke 202).
- Bug found on the way: ↓ ↑ (scroll by) and Page Down / Up put back the horizontal scroll of an earlier zoom or
  jump (the tab still carried it); they now leave the horizontal scroll alone.

## Left, and why

- C-reader-045: a click in another document's pane only brings its tab forward (that click is not passed on, the
  next one is). A double-click that starts in the other pane is two single presses (the first swaps the panes).
  The split stays with the tab in view: the document left behind shows alone when its tab is chosen again.
- C-reader-010: the reviewers' gap (a real macOS trackpad session: pinch on page 30+, slow and fast ⌘ + trackpad,
  Magic Mouse) needs a Mac; not doable on this VPS.
- not automatable: horizontal pinch under Xvfb (no gesture events).

## For the orchestrator

- No new dependency.
- New module `src/split.rs` (`mod split;` in main.rs). New pub items: `Viewer::{wanted_at, set_horizontal,
  horizontal, set_height, set_left, row_of, scroll_x_to}`, `ViewerUi::{twin, follow, swap_strip, pane_pages, pane}`,
  `TabState::split`, `reader_ui::toggle_horizontal` (nav.rs). `viewer_ui::request` now queues the split's other pane
  too; renders find their viewer by load id (`update_doc`).
- Shared files touched (small blocks): `ui/flow-types.slint` (ReaderState `horizontal`; ToolTab `split-*`; ReaderBridge
  `toggle-horizontal`, `split-view`, `split-focus`, `split-sync`, `split-with`, `split-names`; ViewerBridge
  `second-viewport`), `src/main.rs` (`mod split;`, wirings), `reader_ui/mod.rs` (one line in `restore_view`, one in
  `update`), `reader_ui/model.rs` (one field), `state.rs` (TabState `split`, one line in `build`).
- `Viewer::content_width` now returns a width computed in `relayout` (same value down a column).

## After Sol's review (27/10)

- C-reader-003: ⌘+ / ⌘−, a typed zoom and Fit keep the page in view deep in a strip (the zoom anchored on the
  strip's start, sx 0, and jumped back toward page 1): `zoom_to` now anchors on the view's top middle at the
  current horizontal scroll (test `a_zoom_deep_in_a_strip_stays_on_the_page_in_view`). In the page grid, Page
  Down / Space and jumps scroll down again: they follow `Viewer::across`, false there, not the kept setting.
- C-reader-045: Sync scroll and zoom (PDF Expert's): on, the other pane also takes the active pane's zoom, as an
  actual size (another document's pages may be another size), on the same point of its page (test
  `sync_scroll_and_zoom_takes_the_active_panes_zoom`; smoke 205). A click in another document's pane brings its
  tab forward and goes on to it at the same point (a link, a comment, a note placed); a drag that starts there
  starts after. The divider drags (a 7 px grip, the share kept from a fifth to four fifths, carried with the
  hand-over; smoke 205).
- C-reader-010: an ⌥-drag zoom with the hand tool ends when the keyboard focus goes, or at the next move once the
  button is no longer held (let go outside the window).
  not automatable: focus taken away mid-drag under Xvfb (no second window to take it while the button is held).

## Round 27 (Sol's second review; smoke run on round 27's app)

- Bug, found running smoke 200 in the real app: More ▸ More tools ▸ Scrolling, split view, hand › never opened. The
  merge that wrapped More tools' list in a ScrollView left layout-menu inside the more-tools popup, which closed it
  with itself; pages across, split view and the hand entry were unreachable from the menu (reader-toolbar.slint, one
  closing brace moved). Steps 200, 201, 203–205 fail until the app is rebuilt with it.
- C-reader-003: a window or side panel resized deep in a strip keeps the page in view (ViewerUi::resize; test
  `a_resize_deep_in_a_strip_keeps_the_page_in_view`).
- C-reader-045: ⌘ / Ctrl + wheel over the other pane makes it the active one and zooms it around the pointer; a drag
  there with the middle button, Space or the hand moves that pane; the hand with ⌥ zooms it.
- C-reader-010: a pinch that starts over the other pane zooms that pane; every pinch's centre is taken in its own
  pane (the right or lower pane's offset was missing).
- Smoke (edited in round 27's tree, dev/scripts/smoke/4-flows.sh): `layout_item` clicks the submenu entry by its
  place (OCR missed it under the hover), and the steps read colours with `c8` (8-bit: `colour_at` reads this
  ImageMagick's 16-bit output as #RRRRGG, so green showed as #2020A0).

## Batch opus-edit


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

## Batch opus-platform


## What a user now gets

- C-edit-013, C-reader-060: check spelling while typing, offline (Acrobat's Check Spelling in comments and
  form fields, Preview's Check Spelling While Typing). A note, text box, reply, form text field, edited
  paragraph or in-place text box being typed is checked 250 ms after typing pauses, on a worker thread; a
  card under the editor shows the text with every misspelt word underlined in the theme's red, then up to
  four of them, each with up to three corrections (a click puts it in the text), Ignore (this session) and
  Learn (kept in the preferences). The word still being typed is left alone until a space or a mark ends it.
  Addresses, paths, words with digits, all capitals (PDF) and mixed case (iPhone) are not checked, as Word's
  defaults. Dictionary: the system's Hunspell `.aff`/`.dic` for the system's language (then American, then
  British English, then any), searched in `SHEAF_DICTIONARIES`, `<data>/Sheaf/dictionaries`,
  `/usr/share/hunspell`, myspell, LibreOffice's extensions (Linux, Windows, macOS), `/Library/Spelling`;
  else a word list (`/usr/share/dict/words`, one-edit suggestions); else Preferences ▸ Spelling says what to
  install. Edit ▸ Check Spelling While Typing and Preferences ▸ Spelling turn it off; Preferences shows the
  dictionary in use and Forget the learned words. (tests `spell::tests::*` in `sheaf-core/src/spell.rs`:
  `words_skip_addresses_acronyms_and_numbers`, `misspelt_skips_the_word_being_typed_and_learned_words`,
  `word_list_suggestions_are_one_edit_away`, `replace_word_follows_the_text_when_it_moved`,
  `dictionaries_are_found_by_language_then_english`, `latin1_dictionaries_are_decoded`,
  `a_hunspell_dictionary_checks_and_suggests`, `the_system_english_dictionary_when_installed`;
  `platform::tests::misspelt_words_are_underlined_and_the_rest_shown_as_typed`; smoke step 222)
- C-edit-013 (second pass): Edit ▸ Check Spelling… (F7; Acrobat's Check Spelling in comments and form
  fields): a dialog goes word by word through every comment with text (notes, text boxes, markups' notes, in
  page order; the page shown follows) then every text field the user can fill. For each misspelt word: where
  it is ("Comment on page 3", "Form field “Name”"), its sentence with the word underlined in red, up to five
  corrections (a click puts one in Change to, which can be typed), and Change (the comment or field gets the
  corrected text, one undo step each), Ignore (this word until Sheaf quits), Learn (kept), Next (left as it
  is). At the end: how many texts were checked and words changed; no dictionary: what to install.
  (`reader_ui/spelling.rs`, `platform_ui` `Pass`; tests `spell::tests::the_check_pass_goes_word_by_word_through_the_texts`;
  smoke step 227)
- C-edit-013, C-reader-060 (third pass, Sol's review): Check Spelling also goes through each comment's
  replies ("Reply on page 2"). The pass scans every text once, on a worker thread, when it starts
  (`sheaf_core::spell::CheckPass`); Next, Ignore, Learn and Change only move through that list (a Change
  shifts what follows in the same text), and a word's corrections are looked up off the UI thread. Check
  Grammar With Spelling (Preview's; Edit menu and Preferences ▸ Spelling, on by default), light and
  offline: a word written twice ("the the"; not "had had", "that that"), a small letter after a full stop,
  question or exclamation mark (not after "e.g.", "Dr.", an initial or "…"), two spaces or more between
  words, and "a"/"an" against the next word's sound ("an hour", "a unit" are right). English rules; other
  languages get the repeated words and spaces. Grammar findings show in the typing card ("Repeated word",
  its fix, Ignore) and in the Check Spelling dialog. Not covered: agreement, tense, punctuation beyond
  those. (tests `spell::tests::the_light_grammar_pass`, `the_check_pass_goes_word_by_word_through_the_texts`,
  `reader_ui::spelling::tests::a_replys_text_is_checked_without_its_author`)
- C-platform-014 (third pass, Sol's review): Edit ▸ Undo and Redo now act on what has the keyboard, as
  Cut/Copy/Paste do: a focused text field undoes its own typing, otherwise the reader's pages undo the
  PDF's last edit. They are enabled everywhere (Preferences, Home). Undo, Redo, Cut, Copy, Paste and Select
  All show their usual shortcuts in the menu (Ctrl+Z, Ctrl+Shift+Z, Ctrl+X/C/V/A; ⌘ on a Mac): the menu bar
  takes those keys first and sends them on to the focused field or the pages (`platform_ui::edit_key`, its
  shortcuts off while it does; right-hand Ctrl/Shift so a left Ctrl being held stays held). (tests
  `shortcuts_change_conflict_and_reset`, `shortcut_rows_are_searched`, `rebound_commands_stop_their_other_keys`;
  smoke step 228)
- Fourth pass (Sol's review): C-edit-013, a reply is checked even when its comment has no text (a highlight
  replied to; `replies_are_checked_even_when_their_comment_has_no_text`). C-reader-060, the grammar's
  English rules (a/an, a capital after a full stop) apply only with an English dictionary; another
  language gets repeated words and spaces (`the_light_grammar_pass`: "Il a une idée"). C-platform-014,
  Shift, Alt or Super still held when Edit ▸ Undo and the like send their keys on (Ctrl+Shift+U given to
  Undo) are let go for the key and pressed again after (the window's ModifiersChanged, main.rs).
  C-preferences-013, Reset on a command refuses keys a tool launcher has taken by now
  (`reset_refuses_keys_a_tool_has_taken`).
- C-platform-014: a menu bar, File · Edit · View · Go · Tools · Window · Help (Acrobat's and Preview's
  menus; Slint's `MenuBar`: drawn in the window on Linux and Windows, the system menu on macOS). Every
  command shows its shortcut; a command that does not apply where the user is (Save on Home) is disabled, so
  its keys go on to the field or page that has the keyboard. File: New Blank PDF, Open, Open Recent (the
  recents), Save, Save As, Print, Document Properties, Close Tab, Quit (unsaved documents asked about, as the
  close button). Edit: Undo, Redo, Select All, Find, Find Next/Previous, Check Spelling While Typing,
  Preferences. View: zooms and fits, side panel, page layout, rotate view, Full Screen (F11; ⌃⌘F on a Mac),
  Present. Go: pages, Go to Page, Back, Forward. Tools: All Tools, the reader's Annotate and Edit PDF tools,
  the starred tools. Window: tabs, Home. Help: Keyboard Shortcuts, Customize Shortcuts. Hidden in full
  screen. Second pass: Edit ▸ Cut, Copy, Paste and Select All act on what has the keyboard: they send ⌘X
  ⌘C ⌘V ⌘A (Ctrl on Linux and Windows) to the window once the menu has closed, so a focused text field
  cuts, copies, pastes or selects its own text and the reader's pages their selection, picked objects or
  selected comment, exactly as the keys do (`platform_ui::edit_key`). (tests
  `platform::tests::the_menu_bar_reads_each_command_at_its_index` (each MenuItem in
  main.slint reads its own COMMANDS index; every default is a valid, unique shortcut); smoke steps 220, 226)
- C-preferences-013: Preferences ▸ Keyboard shortcuts (Stirling's Hotkeys card): search the commands or keys,
  click a shortcut and press the new keys (Esc cancels, Backspace removes it), Reset one, Reset all. Refused
  with the reason: keys another command has ("Ctrl+S is already File ▸ Save"), a key without ⌘/Ctrl or Alt
  (it would stop typing; function keys alone are fine), the text field's own ⌘C ⌘V ⌘X ⌘A ⌘Z ⌘Y and
  arrows. The menu bar takes the new keys at once (while keys are recorded it has none, so they reach the
  recorder). Preferences ▸ Interface ▸ Hide tools that can't run here (Stirling's): Word, PowerPoint, Excel and
  HTML to PDF without LibreOffice, OCR without its models, left off the Tools page and its search; the page
  says which and why. Second pass: the reader's one-letter tool keys (V Select, H Highlight … F Prepare
  Form, Z Marquee Zoom) are listed there too ("Tools ▸ Highlight (on the pages)") and changed the same way
  (one letter or digit alone; refused when another tool has it); the reader's key handler asks
  `PlatformBridge.tool-for-key`, and the toolbar's tips, the More menu and the ? help sheet show the user's
  letters and keys. A command given new keys stops answering its old ones: every key the reader or Home
  handled besides the menu bar (⌘= for Zoom In, ⇧⌘] for Next Tab, ⌘Z on the pages for Undo…) is guarded by
  `!PlatformBridge.rebound[i]`. Undo, Redo and Select All show "Ctrl+Z on the pages" while they keep them.
  (tests `tool_letters_change_and_conflict`, `rebound_commands_stop_their_other_keys` (each guard's index
  is its command's); smoke step 225) (tests `platform::tests::a_pressed_key_becomes_shortcut_parts`,
  `shortcuts_that_would_stop_typing_are_refused`, `shortcuts_change_conflict_and_reset`,
  `shortcut_rows_are_searched`, `tools_that_cannot_run_here`; `history.rs` prefs literal; smoke step 221)
- C-preferences-013 (third pass, Sol's review): every tool of the Tools page (Merge PDF, Split PDF,
  Compress… all of `sheaf_core::tools::catalogue`) can be given keys that open it, as Stirling's tool
  hotkeys: rows "Tools page ▸ Merge PDF" in Preferences ▸ Keyboard shortcuts, the same recording, rules
  and conflict check as the commands (both ways: a command cannot take a tool's keys, nor a tool a
  command's), Reset removes them. Tools ▸ Open Tool lists every tool shown on the Tools page with its keys.
  (tests `every_tool_can_get_keys_that_open_it`, `shortcut_rows_are_searched`; history.rs literal; smoke 229)
- C-platform-004: Preferences ▸ Interface ▸ Direction (iLovePDF's right-to-left interface): as the system's
  language (Arabic, Hebrew, Persian, Urdu… right to left), Left to right, Right to left. Right to left:
  Home's sidebar goes to the window's right edge, its entries mirrored (icon right, text against it,
  selection bar on the left); the tab bar starts at the right (Home at the right edge, tabs going left,
  each tab's icon on its right and × on its left, a drag to the left moves a tab along, the drop line and
  keep-in-view scrolling mirrored); the reader's toolbar groups and the buttons in each are reversed; the
  side panel sits right of the pages, its border and drag edge on its left (dragged leftwards it widens).
  The rows mirrored are FlexboxLayouts with `flex-direction: row-reverse` (Slint 1.18's HorizontalLayout has
  no direction). (test `platform::tests::right_to_left_follows_the_language_unless_chosen`; smoke steps 223,
  224)
- C-platform-007: `sheaf bookmark <pdf> --title <t> --page <n> -o <out>` adds a bookmark opening a page after
  the last top-level one (UTF-16 titles when not ASCII); the MCP server offers it as a tool (generated from
  clap), `sheaf info` lists it. This closes Sol's "no command adds a bookmark" (Preview's App Intents,
  Acrobat's `bookmarkRoot.createChild`). (tests `crates/sheaf-core/tests/bookmarks.rs`
  `bookmarks_are_added_after_the_last_one_at_their_pages`, `a_bookmark_needs_a_page_of_the_document_and_a_title`;
  `crates/sheaf-cli/tests/cli.rs` `bookmark_adds_one_that_info_lists`; `every_tool_schema_parses` now 29 tools)

## Left, and why

- C-edit-013: the underline while typing is in the card under the editor, not inside the field: Slint's
  TextInput/TextEdit cannot style part of their own text. The pass does not check replies (they are listed
  inside their comment as "author: text") nor the page's own text (Acrobat checks only editable text too).
- C-reader-060: grammar (Preview's Spelling and Grammar) is not checked: no offline grammar checker that is
  pure Rust and small.
- C-platform-014: the reader's one-letter tool keys are not shown as menu shortcuts (Tools ▸ Annotate lists the
  tools without them): a menu bar shortcut is taken before any text field, so a bare letter would stop
  typing; Preferences lists them instead. The menu bar is not mirrored right to left (Slint draws it left
  to right). Cut, Copy, Paste and Select All keep ⌘X ⌘C ⌘V ⌘A whatever keys the user adds: Slint's text
  fields answer those keys themselves.
- C-preferences-013: "API output handling" (Stirling) is a server setting with no meaning offline. The
  reader's other plain keys (arrows, Space, Delete, ⇧T, ?) are not customizable.
- C-platform-004 (third pass, Sol's review): right to left also mirrors the reader's tool options bar
  (its options reversed, the bar at the top right of the pages), Preferences (every row reversed, labels
  and section titles right-aligned) and the dialogs (password, unsaved changes, close a running task,
  launch a file, Check Spelling: text right-aligned, buttons from the left). The rows were turned into
  direction-aware FlexboxLayouts by a brace-aware script (a row's default stretch kept, a child's `y`
  nudge turned into `cross-axis-self-alignment: center`; scripts kept in /tmp/f5/opus-scripts, not in the
  repo). Smoke step 230.
- C-platform-004 (fourth pass, Sol's review): right to left also mirrors the side panel (its tabs
  Pages/Outline/Notes/Files/Layers, every row, the outline's indentation, the hover buttons and page
  labels, the notes and attachments rows) and the tool pages (the file area and the options panel swap
  sides, the + and sort buttons move to the left, the options' rows and labels, the result page). Rows
  became direction-aware flexboxes as before; placed items have `x: rtl ? parent.width - (x) -
  self.width : x` (a width written from `self.x` rewritten from the same x, so they do not depend on
  each other); popups, centred, mouse- or drag-placed items are left as they are (scripts
  /tmp/f5/opus-scripts/mirror.py, mirror_x.py, flexify.py).
- C-platform-004, what Slint 1.18 blocks or what is left: (1) Slint has no window-wide layout direction, so
  each row is mirrored by hand (done: Home sidebar, tab bar, reader toolbar, side panel with its tabs and
  rows, tool options bar, tool pages, result page, Preferences, the dialogs; left: the reader's sheets
  (print, properties, signature), Home's Recents, Tools and History pages).
  (2) Text alignment: `Text`'s default `horizontal-alignment` is `start`, which follows each string's own
  script (an Arabic string sits right, an English one left); Sheaf has no translations, so labels are set
  to `right` explicitly where mirrored. (3) The `MenuBar` element has no direction: Slint draws it with its
  own MenuBarImpl (std-widgets common/menus.slint), a row of entries from the left with no direction or
  alignment property, so the menus cannot start at the right; reversing their order alone would need
  every Menu written twice behind `if PlatformBridge.rtl` and would still be left-aligned (on macOS the
  system menu bar follows the system's language). (4) Icons are not flipped (the Side panel icon still
  points left): Slint has no mirrored image drawing short of a second icon file. (5) A tool page's file
  cards keep their left-to-right order: their drag-and-drop slots are computed from the pointer in
  left-to-right columns (tool-flow.slint `drop-col`).
- C-platform-007: Shortcuts / AppleScript actions need macOS (App Intents, a Mac build): not done, as the
  brief says. Linux and Windows have no system automation of that kind; the CLI and the MCP server are it.

## Not automatable

- not automatable: the menu bar as the macOS system menu — needs a Mac build.
- not automatable: the menu bar drawn on Windows — needs a Windows build (Slint draws it as on Linux).

## For the orchestrator

- New dependency: `spellbook = "0.3"` (pure Rust Hunspell, helix-editor; workspace `Cargo.toml` and
  `crates/sheaf-core/Cargo.toml`). Its API as used: `spellbook::Dictionary::new(&aff, &dic) -> Result<_, impl
  Display>`, `.check(&str) -> bool`, `.suggest(&str, &mut Vec<String>)`; the `Dictionary` held in a
  `static Mutex` (needs `Send`).
- New modules: `reader_ui/spelling.rs` (`mod spelling;` + `pub use` in reader_ui/mod.rs), `sheaf_core::spell`, `sheaf_core::bookmarks`; `sheaf_app::platform` (lib); `platform_ui` (bin,
  `mod platform_ui;` in main.rs, `platform_ui::install(&ui)` after `wire_reader`); `home::update_platform`.
- New Prefs field `platform: sheaf_app::platform::PlatformPrefs` (added to `tests/history.rs`'s literal, `tool_keys` included).
- New Slint files: `ui/screens/spelling-dialog.slint`, `ui/platform.slint` (`PlatformBridge`, `SpellIssue`, `ShortcutRow`, exported from
  main.slint by a separate `export { … }` line), `ui/widgets/spell-strip.slint`,
  `ui/screens/preferences-platform.slint`. `main.slint` imports the sidebar's `MenuItem` as `SideMenuItem`
  (the name is the menu bar's own element there).
- The GUI smoke hides the menu bar (debug builds with `SHEAF_READER_DEBUG`, unless `SHEAF_TEST_MENU_BAR=1`):
  the earlier steps click at places measured without it; its shortcuts stay active, so their `key ctrl+…`
  now go through the menu bar to the same actions. Step 220 shows it. Steps 221 and 223 put the platform
  preferences back (`platform_reset`).
- Step 222 needs an English Hunspell dictionary (`hunspell-en-us`, installed on the VM) or
  `/usr/share/dict/words`.
- `crates/sheaf-cli/tests/cli.rs` `every_tool_schema_parses`: 28 → 29 tools (another batch adding a command
  changes the same number).

## Author's grades (third pass, 27/10, not built)

- C-edit-013: f9 a8 p9 — while typing (spell card) and Edit ▸ Check Spelling over comments, replies and form fields, scanned once off the UI thread (`the_check_pass_goes_word_by_word_through_the_texts`, `a_replys_text_is_checked_without_its_author`, smoke 222/227); the underline is in a card under the field (Slint limit).
- C-reader-060: f8 a8 p9 — spelling plus a light offline grammar pass (`the_light_grammar_pass`); no full grammar model.
- C-platform-014: f9 a8 p9 — menu bar, Undo/Redo/Cut/Copy/Paste/Select All on the focused field or the PDF, default labels (smoke 220/226/228); macOS/Windows unseen.
- C-preferences-013: f9 a8 p9 — commands, tool letters and every Tools page tool customisable, conflicts both ways (`every_tool_can_get_keys_that_open_it`, smoke 221/225/229).
- C-platform-004: f8 a7 p9 — Home, tab bar, reader toolbar, panel, options bar, Preferences, dialogs mirrored (smoke 223/224/230); reader sheets, tool pages and the menu bar still left to right.
- C-platform-007: f8 a7 p9 — `sheaf bookmark` (CLI, MCP); Shortcuts/AppleScript need a Mac.

## Batch sol-convert


## Mandate (Tatouages)

- Work only in `par/sol-convert`; six components from `/tmp/f5/par/sol-convert.json`.
- The common brief forbids builds, compiler, tests, app, browser and Codex execution. Tests below
  are written for the orchestrator, never reported as passing.
- No network features. Do not choose LibreOffice bundling on Nassim's behalf (HANDOFF).

## Components — pass 1

- C-convert-002 (partial): Office conversion can discover a packaged LibreOffice runtime beside
  Sheaf, ahead of a system installation (iLovePDF's offline Office conversion; test
  `a_packaged_office_runtime_is_found_without_a_system_installation`). `$SHEAF_SOFFICE` still wins.
  The runtime is **not bundled by this change**: distributing it, licence files, platform package
  integration and real Office corpus fidelity remain. Inspected `office.rs`, Office cache in
  `files.rs`, `tests/office.rs`, `dev/HANDOFF.md` and iLovePDF's `WordToPdfTool.qml`. The handoff
  explicitly reserves the distribution decision to Nassim. No dependency added.

- C-optimize-014 (partial): More ▸ More tools ▸ Fix page hairlines (0.25 pt) raises strokes in the
  selected pages' own content, or the current page (Acrobat's Fix Hairlines). It measures the
  width at painting time through the current transform, honours saved graphics state, ExtGState
  widths and UserUnit, and restores the width after painting. Add black crop marks (Acrobat's Add
  Printer Marks) enlarges the sheet, preserves effective trim/bleed/art boxes, clips the original
  content to its previous crop, and adds eight black crop marks outside the bleed. Repeated clicks
  add no duplicate marks. Each command is undoable. Tests
  `page_hairlines_are_measured_after_transforms_and_undone_in_one_step`,
  `page_hairline_repairs_honour_user_units_and_nonuniform_scaling`,
  `crop_marks_keep_inherited_page_boxes_and_do_not_invent_bleed` in `tests/engine_patches.rs`.
  Left: hairlines inside grouped Form artwork, patterns, text strokes and annotation appearances;
  configurable widths, general page-box editor, registration marks/colour bars, trapping and
  flattening preview. Inspected Acrobat `PrintProduction_R_RHP.aapp`, PdfCraft organize boxes,
  content parser, page-stream splice helpers, and engine edit/permission/undo handling.
  New public engine edits `FixPageHairlines`, `AddCropMarks`; public modules
  `pdfcraft_edit::prepress`, `pdfcraft_engine::prepress`. No dependency added.
  not automatable: verify both menu commands, page selection and undo in both themes — app
  execution forbidden in this worker.

- C-optimize-012 (partial): More ▸ More tools ▸ Print preflight report… saves Acrobat's Preflight
  style findings from the document **as edited**, on the document thread: effective image ppi,
  media/crop/trim/bleed boxes, bleed margins, output intents, unembedded fonts and structural
  PDF/A-2b issues. The report records source, check time, profile and its thresholds, and lists
  checks not performed. An existing report is kept. Test
  `print_preflight_reports_low_resolution_and_keeps_the_document_unchanged` exercises the engine
  check through sheaf-core (engine crate tests are excluded). Left: ink separations/output preview,
  editable profiles, automatic fixups and a full audit trail. Inspected PdfCraft `printcheck.rs`
  and Acrobat `PrintProduction_R_RHP.aapp`. No dependency added.
  not automatable: More tools ▸ Print preflight report… opens Save, writes and announces the report
  from an unsaved document — GUI execution forbidden in this worker.

- C-convert-005 (partial): HTML to PDF offers Original layout, A4 and Letter, portrait/landscape,
  and none/small/big margins (iLovePDF's HTML page settings). Explicit layouts insert print CSS
  after existing styles and preserve relative local resources with a file base. The output bypasses
  the default-layout preview cache. Markdown and mail use the same layout through their existing
  HTML converter (mail attachments kept); PostScript explicitly asks for Original layout.
  Tests `html_print_settings_keep_resources_and_override_the_page_rule`,
  `html_print_settings_reject_invalid_margins_and_existing_bases`,
  `html_layout_finds_tags_instead_of_comments_scripts_or_headers`,
  `custom_html_layout_explains_unsupported_encodings_and_postscript`,
  `html_layout_reaches_the_job_without_reusing_the_default_conversion`,
  `local_html_prints_on_the_chosen_paper_with_its_relative_picture`,
  `markdown_and_mail_use_the_chosen_html_paper_and_keep_attachments` (both require LibreOffice).
  Left: URL capture and screen mode; URL capture is a network feature forbidden by the batch brief.
  Files with an explicit `<base>` require Original layout; non-UTF-8 HTML requires Original layout.
  Inspected iLovePDF `HtmlToPdfTool.qml` and the existing Office pipeline. No dependency added.
  not automatable: visually check the HTML controls in both themes — app execution forbidden in this worker.

- C-convert-018 (**not implemented**): PDF/A remains 2b/3b. Inspected the core `pdfa.rs`, its
  integration tests, the app's `PdfToPdfA` job and level cards, the actual pinned
  `crates/preflight/src/lib.rs` (Level, verify and convert) and `xmp.rs`, plus iLovePDF's
  `PdfToPdfATool.qml` and Acrobat's `Standards_R_RHP.aapp`. The pinned engine implements only
  A2b/A3b. PDF/A-1b needs PDF 1.4 restrictions and transparency handling; a/u levels require real
  tagging/Unicode checks and repairs. A new label or XMP part number would not implement those
  standards. No converter or test was added for these absent levels. Ghostscript is installed on
  this worker but veraPDF is not; neither was executed. No claim of full standards validation.

- C-convert-019 (**not implemented**): no Save as PDF/X or PDF/E was added. Inspected Acrobat's
  `Standards_R_RHP.aapp` and `PrintProduction_R_RHP.aapp`, engine `catalog.rs`/`commands.rs`, and the
  pinned preflight crate. PDF/X is only a planned catalogue item in PdfCraft; there is no converter
  to expose, and the existing print-readiness report explicitly excludes PDF/X, overprint, spot
  colours and flattening. Real PDF/X needs a chosen press profile, output intent and version-specific
  colour/transparency/font rules; PDF/E needs a separate standards implementation. This batch does
  not supply them. No test added because no PDF/X or PDF/E conversion was implemented.

## Integration — pass 1

- No new dependency, no Cargo manifest or lockfile change.
- `Job::Office` now has `html: Option<sheaf_core::html::Options>`; its constructor in
  `crates/sheaf-app/tests/jobs.rs` was updated. Other workers adding Office jobs must supply it.
- Public core modules added: `office_runtime`, `html`, `preflight`. Engine edits added:
  `FixPageHairlines`, `AddCropMarks`; new public `prepress` modules in edit and engine.
- Reader More tools appends indices 20–22. Reconcile indices with any other worker appending to
  that array. Slint callbacks and main.rs wiring are in small adjacent blocks.
- New engine/edit behaviour is exercised from `crates/sheaf-core/tests/engine_patches.rs`, not
  from the excluded vendored crate tests.
- All named tests are **written, not run**. The HTML integration test requires LibreOffice and
  skips when absent; integration tests using rendering need the existing PDFium setup. The
  orchestrator must compile and run the suite and perform the GUI checks above. In particular,
  confirm LibreOffice honours HTML print CSS on the supported platforms.
- Static checks performed: `git diff --check`; read API signatures from the actual pinned source;
  searched all Office job constructors; inspected undo/permissions/page-box refresh paths. The
  Impeccable static detector returned `[]` for the changed Slint targets; no rendered UI was judged.
- No board grades changed. None of the six components is claimed at parity or 9/10.

## Polaroid — pass 1 handoff (superseded below)

- Status: worker implementation finished; ready for the orchestrator's compilation and review.
- Delivered: four partial implementations and their tests; two standards components inspected
  without an implementation. Constraints and all remaining gaps are recorded above.
- Verification limit: source inspection only. No cargo, rustc, clippy, tests, app, Chrome, Xvfb or
  codex executed. The remaining runtime checks belong to the orchestrator under the common brief.
- Next action: merge this branch with the other batches, reconcile shared Slint menu indices and
  callback blocks, then compile and run the actual tests and inspect the GUI.
- Problem-solving control: mandate **partial** (all six examined; four changed); evidence **yes**
  (code, tests, commit history); external dependency **yes** for Office packaging/network capture,
  **no** for absent standards implementations (not delivered, not disguised as an external block);
  independent work **partial** (printing/conversion subsets delivered; advanced gaps listed).

## Changed files

- App Rust: `crates/sheaf-app/src/jobs.rs`, `main.rs`, `run.rs`, `reader_doc/mod.rs`,
  `reader_ui/mod.rs`, `reader_ui/prepress.rs`; app integration test `crates/sheaf-app/tests/jobs.rs`.
- Slint: `crates/sheaf-app/ui/flow-types.slint`, `screens/options.slint`,
  `screens/reader-toolbar.slint`.
- Core: `crates/sheaf-core/src/lib.rs`, `office.rs`, `office_runtime.rs`, `html.rs`, `preflight.rs`;
  tests `crates/sheaf-core/tests/office.rs`, `preflight.rs`, `engine_patches.rs`.
- Vendored code: `third_party/pdfcraft-edit/src/lib.rs`, `prepress.rs`;
  `third_party/pdfcraft-engine/src/lib.rs`, `prepress.rs`.
- Notes: `dev/par-notes-sol-convert.md`.

## Pass 2 — C-optimize-012

- C-optimize-012: More ▸ More tools ▸ Print preflight… now opens a Slint sheet in the reader's
  house style (Acrobat's Preflight). Close-up / large-format presets and custom image/bleed
  thresholds change the actual checks, not just a report label. Results include source, time,
  the measured image/box/font/intent findings, and checks not performed. Page findings jump to
  their page. Save report keeps exactly the inspected snapshot, using a free name atomically.
  Test `print_preflight_reports_low_resolution_and_keeps_the_document_unchanged` now contrasts
  real findings under two profiles; `preflight_profiles_reject_thresholds_that_would_hide_failures`
  rejects invalid comparisons; `preflight_rows_keep_every_condition_and_split_long_findings`
  preserves all diagnostic words across rows of at most four lines plus a title.
- Left: ink separations, output simulation, editable fixup profiles, automatic corrections and a
  durable history of successive runs. Existing unembedded fonts cannot be repaired by this check.
- New engine API: `printcheck::Profile`, `Document::print_readiness_with`. `Op::PrintPreflight`
  now checks and replies with a report instead of writing a file; export writes the saved snapshot.
- No dependency. Tests written, not executed. Static inspection corrected calls to the engine's
  private inspector in its existing tests too. Reference inspected: Acrobat's real archive path
  `aapp/PrintProduction_R_RHP.aapp` (the input's prefixed path is not an archive member).
- not automatable: inspect light/dark sheet, presets, invalid values, page navigation, report Save,
  and an unsaved edit before rerun — worker app/build/test execution forbidden by rule 1.
- LIGNES : max 5 (preflight finding, one title + four lines) · cases > 6 : aucune.
- Provisional author grade: f6 a7 p7; source-only evidence, no parity claim. Independent counterpart
  review belongs after orchestrator merge/build/full green suite.

## Pass 2 — C-optimize-014

- C-optimize-014: More tools ▸ Fix hairlines / page boxes… and Add printer marks… open a proper
  Print production sheet (Acrobat's Print Production). The user chooses selected/current or all
  pages, types the minimum stroke (0.01–10 pt), or sets/resets Crop/Trim/Bleed/Art margins relative
  to each MediaBox in unrotated PDF units. Artwork is not moved. Crop marks remain black, outside
  existing bleed, with the sheet enlarged. Every action uses the permission/undo document path.
  The sheet explicitly says grouped art, text and comments are not hairline-repaired, and boxes
  should be set before adding marks. No new hidden CLI-only operation.
- Tests `print_boxes_preserve_margin_order_and_reject_bad_numbers` and
  `print_box_margins_use_each_media_origin_and_failed_changes_leave_it_intact`; existing stroke,
  UserUnit and crop-mark tests remain. Written only. No dependency or new engine edit.
- Left: Form/pattern/text/appearance hairlines, registration marks, colour bars, configurable mark
  dimensions, trapping and transparency flattening preview. Existing marks must be undone before
  changing trim/bleed then re-added; this pass does not reposition previously added marks.
- Preflight rows are now prepared once when its report arrives and shared with Slint (rather than
  rewrapping a large report on every reader model update).
- not automatable: light/dark production sheet, invalid stroke/margins, selection/all scope,
  changed boxes and one-step undo — runtime forbidden by rule 1.
- LIGNES : max 4 (Set page boxes / Crop marks, title + three lines) · cases > 6 : aucune.
- Provisional author grade: f5 a7 p7; source-only, not rendered or executed, no parity claim.

## Pass 2 — C-convert-002

- C-convert-002: Tools ▸ Excel to PDF now offers saved print layout or one whole sheet per PDF page
  (iLovePDF's spreadsheet conversion layout). Saved layout keeps the original cached conversion;
  whole sheets use Calc's real `SinglePageSheets` export and bypass that cache. The app explicitly
  says hidden sheets are included and saved paper/print areas are ignored. Batch semantics,
  isolated temporary Office profile, timeout and atomic free output name are preserved.
- Primary evidence: installed LibreOffice `share/registry/main.xcd`'s `SinglePageSheets`, and
  [LibreOffice PDF CLI parameters](https://help.libreoffice.org/latest/en-US/text/shared/guide/pdf_params.html).
  Test `whole_sheet_export_keeps_outside_print_area_and_hidden_sheet_content` builds a real XLSX
  with a print area and hidden sheet, checks both layouts' text/pages, source preservation and
  qpdf cleanliness. `whole_sheet_export_does_not_apply_calc_settings_to_a_word_file` rejects a
  wrong source type. `whole_sheet_layout_is_only_used_for_spreadsheets_and_bypasses_html` checks
  the app job route. Written, not run; integration requires LibreOffice and PDFium.
- Left: sheet-by-name selection and editing cell print ranges inside Sheaf; a real Word/PPT fidelity
  corpus and font substitutions. The previous pass's packaged-runtime discovery is present, but
  bundling/distributing LibreOffice remains Nassim's reserved decision (HANDOFF), not implemented.
- Integration: `Job::Office.whole_sheets: bool` added; constructors updated. New public core
  `office::spreadsheet_to_pdf`. No dependency or preference field added. Whole-sheet layout may
  require a second conversion after the ordinary card preview; the default still uses its cache.
- not automatable: Excel tool's layout choice in both themes and compare wide/hidden sheets on
  output — app execution forbidden by rule 1.
- LIGNES : max 3 (whole-sheet layout radio card and its information) · cases > 6 : aucune.
- Provisional author grade: f8 a8 p8; unexecuted source and tests, no parity claim.

## Pass 2 — C-convert-005

- C-convert-005: the existing HTML-to-PDF app controls now also work on legacy encodings and
  ordinary explicit base addresses (iLovePDF's local HTML page settings). UTF BOMs take priority;
  declared HTML encodings are decoded without replacement, with Windows-1252 as the legacy
  fallback. The temporary copy declares UTF-8, including when no original declaration existed.
  Relative bases resolve against the source file; their target and escaped queries are kept.
  Original layout still uses the original converter/cache. The app explains local-only support.
- Source scanner ignores comments and script/style content; the attribute parser handles quoted
  and unquoted values. Numeric HTML entities in a base address still ask for Original layout;
  they are not silently resolved to a different resource. Malformed/unsupported encodings also
  give an actionable Original layout error.
- Tests `html_encodings_keep_accents_and_normalize_the_declaration`,
  `html_base_uses_the_original_file_and_keeps_target_and_escaped_queries`, and real-LO
  `legacy_html_with_its_own_relative_base_keeps_its_text_and_local_picture`; existing paper tests
  remain, and `html_print_settings_reject_invalid_margins_and_existing_bases` was renamed to
  `html_print_settings_reject_invalid_margins` because ordinary bases now work. All unexecuted.
- New direct dependency: `encoding_rs = "0.8"` in workspace and core manifests. Pure Rust, already
  present transitively in Cargo.lock and the worker's registry. The direct sheaf-core lock edge
  was added by hand to that existing locked package; no version resolution, cargo or lock
  generator was executed.
- Left: browser viewport/screen media, JavaScript rendering and one-long-page mode. Network URL
  capture remains deliberately out of scope. Inspected iLovePDF's actual archive member
  `code/rcc/iLovePDF/tools/HtmlToPdfTool.qml` (screen size and One long page controls) and the
  installed encoding_rs API. LibreOffice is not a browser rendering engine.
- not automatable: inspect local-only copy and custom paper output with accented text, a relative
  base and pictures in both themes — app/test execution forbidden by rule 1.
- LIGNES : max 3 (local-files information) · cases > 6 : aucune.
- Provisional author grade: f7 a8 p8; source-only, no full browser parity claim.

## Pass 2 — C-convert-018

- C-convert-018 (partial): PDF to PDF/A now refuses to publish a copy with unresolved built-in
  checks by default. The Slint options expose Keep a copy even if checks fail; opt in to keep the
  previous salvage behaviour and its clause/message report. The choice survives Run again.
  A failed strict conversion deletes its temporary file and preserves existing output files.
  New `pdfa::to_pdfa_with_policy`; existing `to_pdfa` keeps its public/CLI compatibility behaviour.
- Tests `strict_archival_output_keeps_existing_files_and_publishes_no_failed_copy`, expanded
  `pdfa_job`, and `options_come_back_as_they_were_without_their_passwords`. Written, not run.
  App job fixtures explicitly select their old permissive policy; normal app jobs default strict.
- Inspected again: pinned preflight `Level`, `verify`, `convert`; core `pdfa.rs` and its tests;
  iLovePDF's `PdfToPdfATool.qml` (Allow Downgrade); local Ghostscript's `PDFA_def.ps` prefix.
  The new choice controls unresolved issues, **not** iLovePDF's valid lower-conformance downgrade.
  Levels remain 2b/3b. PDF/A-1b and a/u conversion/validation are still **not implemented**.
  Ghostscript could provide a separate 1b conversion route, but needs profile/runtime discovery,
  execution safeguards and an independent standards validator; none was added or claimed run.
- Integration: `Job::PdfA.allow_incomplete` and `ToolOptions.pdfa_allow_incomplete` added, with job
  constructors, Slint restore/export and task-history serialization updated. No new preference
  or dependency for this component. The app explicitly calls its check structural, not full ISO.
- not automatable: strict failure, permissive partial result, batch mixed failures, Run again's
  option restored and light/dark options — runtime forbidden by rule 1.
- LIGNES : max 3 (archival-check information) · cases > 6 : aucune.
- Provisional author grade: f8 a8 p8; standard-level gap remains, source-only, no parity claim.

## Pass 2 — C-convert-019

- C-convert-019 (partial, replaces pass 1's no implementation): Print production now offers
  Save as PDF/X-3:2002, an offline Ghostscript conversion of the whole current working document.
  The operator chooses a CMYK/Gray **output** ICC profile and names the press condition. Profile
  header/class/channels/size are checked. The source document and its encryption/undo state are
  untouched; a temporary copy is converted, with bounded log tails and a 120-second child timeout.
  An existing destination is kept through atomic_write. Output is explicitly unprotected.
- The actual written file must have PDF 1.3, PDF/X-3:2002 metadata, GTS_PDFX output intent with
  the right channel count, and no unembedded fonts in the engine check before publication.
  These are limited structural checks, **not full ISO conformance validation**; the Slint panel
  and export completion say so. PDF/E, PDF/X-1 and PDF/X-4 are **not implemented**.
- Primary evidence: Acrobat `aapp/Standards_R_RHP.aapp`, engine catalog/commands, installed
  Ghostscript `PDFX_def.ps`, ICC header of the installed FOGRA39L profile, and
  [Ghostscript's documented PDF/X route](https://ghostscript.readthedocs.io/en/gs10.05.1/VectorDevices.html#creating-a-pdf-x-document).
  A full PDF/X validator was not found (`command -v verapdf` returned no path; veraPDF alone
  would not supply general PDF/X/PDF/E verification anyway). No Ghostscript executable was run.
- Tests `press_profiles_reject_rgb_and_postscript_strings_cannot_escape`,
  `pdfx3_uses_the_chosen_press_profile_and_preserves_the_working_document`,
  `a_converter_that_claims_success_without_pdfx_and_a_timeout_publish_nothing`. All written,
  not run. The real export test requires Ghostscript and a real CMYK profile (environment
  `SHEAF_TEST_PRESS_PROFILE`, else the known Linux FOGRA39L profile), plus PDFium/poppler/qpdf.
- Public core module `press`, APIs `Options`, `validate`, `find_ghostscript`, `to_pdfx3`,
  `to_pdfx3_with`. Reader `Op::ExportPdfX` and `Reply::PdfXExported` handle the document thread.
  No new menu index, no crate dependency. External runtime: Ghostscript from PATH or
  SHEAF_GHOSTSCRIPT; macOS Homebrew's normal paths also detected. Ghostscript is not bundled.
- not automatable: choose ICC/condition, export unsaved changes, wrong/RGB ICC, missing runtime,
  existing destination, password/permission cases and both themes — rule 1 forbids execution.
- LIGNES : max 4 (PDF/X section, title + three lines) · cases > 6 : aucune.
- Provisional author grade: f4 a7 p6; genuine conversion code, but unexecuted and no standards
  validator, PDF/E, or full Acrobat parity. Counterpart must review after the green merged suite.

## Pass 2 — final static integration audit

- ReaderUi::send already increments in_flight. Removed the extra manual increments from the
  new preflight/PDF-X paths; their replies now balance exactly one send count. Do not copy the
  older accessibility caller's extra increment into these paths.
- Preflight page links also recognize structural-check `- page N:` and bleed `- [warn] Page N`
  diagnostics. Expanded the row test for those forms and for a document title containing "page".
- Added only the direct encoding_rs edge to sheaf-core in Cargo.lock, using the already locked
  package/version/checksum. No cargo invocation, dependency resolver or compiler was run.
- not automatable: preflight and PDF/X on a previously clean document must leave it clean after
  completion, with Undo/close state restored — runtime execution forbidden by rule 1.

## Polaroid — pass 2 handoff, current

- Status: source implementation delivered for all six components; all six remain **partial**.
  Six component commits were pushed individually, then the request-count/dependency correction.
  No component was closed or board-graded. No counterpart review has happened; it belongs after
  the orchestrator merges, builds and runs the full suite green.
- Done: Slint preflight with custom profiles and page links; print-production sheet with stroke,
  scope and page-box controls; whole-sheet Excel option; HTML encoding/base preservation;
  strict/permissive archival publication choice; ICC-guided offline PDF/X-3 export.
- Not done: LibreOffice distribution and fidelity corpus; browser/URL/long-page HTML; extra PDF/A
  levels and full ISO validation; PDF/E and other PDF/X versions; separations, output simulation,
  full fixup/audit profiles, nested artwork hairlines, trapping and flattening preview.
- Runtime dependencies: existing LibreOffice; optional Ghostscript for PDF/X-3. Core adds the small
  pure-Rust encoding_rs direct dependency already present in the lock. No network service added.
- Verification actually performed: live source/API inspection, all relevant Office/PDF-A job
  constructors searched, static send/reply count audit, git diff --check, final Impeccable static
  detector `[]`, local and remote branch heads compared. No cargo, rustc, clippy, tests, app,
  Chrome, Xvfb or codex executed. No render was visually judged.
- New tests and amended tests are named under each component above. The orchestrator must execute
  them; LibreOffice, Ghostscript, the real press ICC and the existing PDFium/poppler/qpdf setup
  are needed for their corresponding integration checks. Optional-runtime skips are not passes
  proving conversion parity.
- First action: integrate this branch, reconcile reader menu indices 20–22 with other workers,
  compile Slint/Rust and run the real suite. Then inspect both sheets and options in both themes,
  source cleanliness after inspections, unsaved-document export, permissions and cancellation.
  New callbacks are wired in main.rs; no extra menu index beyond pass 1 was allocated.
- Integration fields: Job::Office.whole_sheets; Job::PdfA.allow_incomplete;
  ToolOptions.pdfa_allow_incomplete (including restore and saved history); ReaderState's
  production/preflight/press fields. No new Prefs field. New engine API printcheck::Profile and
  Document::print_readiness_with. Core press exports and the renamed PrintPreflight operation
  are described above.
- Control: mandate **partial**, with independent source work delivered on all six; evidence **yes**
  for code/static checks, **not yet** for runtime; dependencies **yes**, named above, while the
  omitted standards/advanced engines are unfinished work rather than a claimed external block;
  independent work **yes within this pass**, with all remaining feature gaps retained explicitly.

## Pass 2 changed files (complete)

- Cargo.toml, Cargo.lock; crates/sheaf-core/Cargo.toml.
- crates/sheaf-app/src/jobs.rs, main.rs, run.rs, saved_options.rs, reader_doc/mod.rs,
  reader_ui/mod.rs, reader_ui/model.rs, reader_ui/prepress.rs.
- crates/sheaf-app/tests/jobs.rs.
- crates/sheaf-app/ui/flow-types.slint, screens/options.slint, screens/reader-toolbar.slint,
  screens/reader.slint, screens/reader-prepress.slint, screens/reader-production.slint.
- crates/sheaf-core/src/lib.rs, html.rs, html_source.rs, office.rs, pdfa.rs, preflight.rs, press.rs.
- crates/sheaf-core/tests/engine_patches.rs, office.rs, pdfa.rs, preflight.rs, press.rs.
- third_party/pdfcraft-engine/src/printcheck.rs; dev/par-notes-sol-convert.md.

## Author grades — provisional, source-only

C-convert-002: f8 a8 p8 — whole_sheet_export_keeps_outside_print_area_and_hidden_sheet_content,
crates/sheaf-core/tests/office.rs:276; saved print areas vs whole/hidden sheets, source preserved;
UI crates/sheaf-app/ui/screens/options.slint:641. Bundling/fidelity/sheet selection still missing.

C-convert-005: f7 a8 p8 — legacy_html_with_its_own_relative_base_keeps_its_text_and_local_picture,
crates/sheaf-core/tests/office.rs:312; legacy text and a real relative local picture under custom
paper settings. Screen media/URL/long page remain absent.

C-convert-018: f8 a8 p8 — strict_archival_output_keeps_existing_files_and_publishes_no_failed_copy,
crates/sheaf-core/tests/pdfa.rs:99; failed structural checks prevent publication unless opted in;
UI crates/sheaf-app/ui/screens/options.slint:1099. Extra levels and full validation still missing.

C-convert-019: f4 a7 p6 — pdfx3_uses_the_chosen_press_profile_and_preserves_the_working_document,
crates/sheaf-core/tests/press.rs:8; ICC-guided PDF/X-3 conversion, marker/font checks and source kept;
UI crates/sheaf-app/ui/screens/reader-production.slint:75. PDF/E and full validation not delivered.

C-optimize-012: f6 a7 p7 — print_preflight_reports_low_resolution_and_keeps_the_document_unchanged,
crates/sheaf-core/tests/preflight.rs:5; changing real profile thresholds changes findings without
editing; UI crates/sheaf-app/ui/screens/reader-prepress.slint:8. No separations or automatic fixes.

C-optimize-014: f5 a7 p7 — print_box_margins_use_each_media_origin_and_failed_changes_leave_it_intact,
crates/sheaf-core/tests/engine_patches.rs:117; media-relative boxes, failure atomicity and undo;
UI crates/sheaf-app/ui/screens/reader-production.slint:8. Advanced print-production gaps above.

All cited tests are WRITTEN, NOT RUN. These numbers are not joint grades or closure requests.
LIGNES : max 5 (preflight finding: title + four lines) · cases > 6 : aucune.

## Pass 3 — C-convert-018

- C-convert-018: restored the previous copy-plus-issues default, including old task history
  without the new policy field (iLovePDF's permissive conversion choice). Explicit strict mode
  still prevents publication and survives Run again. UI now starts checked; `pdfa_job` checks
  permissive then strict, and `archival_history_keeps_the_old_copy_and_issues_default_but_honours_strict`
  covers migration and strict round-trip. Tests written, not run (brief rule 1).
- Left: PDF/A-1b and a/u conversion, valid level downgrade, full ISO validator. The pinned Rust
  preflight supports 2b/3b only; no relabelled output or unsupported certification added.
- No dependency. not automatable: fresh PDF/A screen has Keep a copy checked; Run again restores
  an explicitly unchecked strict choice — worker cannot launch the application.
- Grade: f7 a8 p8, partial; fixes the regression, does not close standard-level parity.

## Pass 3 — C-convert-002

- C-convert-002: Excel to PDF has an explicit Built-in data report, entirely Rust (no runtime
  discovery, child process or network). XLS/XLSX/XLSB/ODS data pages keep cell addresses, stored
  values and accents; long values wrap and paginate. Visible sheets by default, numbered sheet
  ranges select hidden sheets deliberately and preserve order. Existing outputs and sources
  stay intact. The app permits this mode without LibreOffice and bypasses the preview cache.
- Tests `native_data_report_selects_sheets_preserves_cells_and_needs_no_office_runtime`,
  `native_data_report_wraps_long_values_and_refuses_missing_glyphs_atomically`,
  `native_spreadsheet_choice_keeps_sheet_selection_and_bypasses_the_office_cache`. None skips
  for missing LibreOffice; written, not run. Uses lopdf assertions without external PDF tools.
- Dependency: existing pure-Rust `calamine` moved from dev to regular core dependencies; already
  workspace-declared and locked, no new package/version or lock change. New public module
  `sheaf_core::spreadsheet`; `Job::Office.data_sheets: Option<String>` added and constructors
  updated. No preference field.
- Left: faithful Office import still requires LibreOffice (unbundled); Word/PowerPoint options,
  sheet selection in the faithful converter, charts/styles/formula recalculation in the native
  report. Native mode reports raw dates and supports Western European text only; other glyphs
  fail clearly with Saved print layout guidance, without publishing a damaged PDF. Not parity.
- not automatable: without LibreOffice select data report, choose sheets, Run, inspect both themes
  and confirm Run again restores layout/ranges — app execution forbidden.
- Grade: f8 a8 p8 (partial). LIGNES : max 5 (data-report information) · cases > 6 : aucune.

## Pass 3 — C-optimize-014

- C-optimize-014: Fix Hairlines now descends into nested Form XObjects (Acrobat's grouped
  artwork). Each repaired placement receives a private Form/resource copy, accounting for
  inherited line width, each Form Matrix, placement transforms and UserUnit. Shared Forms on
  unselected pages stay intact. Width is restored after painting, preserving inherited state.
  Cycles/depth >16, unreadable operators and excessive work fail through the existing atomic
  engine edit path; the document and undo history are not partially changed.
- Tests `grouped_hairlines_use_private_forms_per_scale_and_leave_unselected_pages_unchanged`
  checks two scales, nested Forms, untouched originals/unselected content and undo;
  `cyclic_grouped_artwork_fails_without_publishing_partial_hairline_repairs` checks atomic
  refusal. Existing page-stroke tests continue to cover ExtGState, UserUnit and saved state.
  All written, not executed. No dependency or new public API.
- Left: hairlines in tiling patterns, Type 3 glyphs, text and annotation appearances; configurable
  registration/colour/page marks, trapping, flattening preview. Crop marks remain fixed black
  0.25 pt. These are not implemented, and this partial pass does not claim Acrobat parity.
- not automatable: both-theme production sheet, grouped-art repair on a selected page and undo —
  rule 1 forbids app execution. Grade: f6 a7 p7, provisional source evidence only.
- LIGNES : max 3 (hairline description, title + two lines) · cases > 6 : aucune.

## Pass 3 — C-optimize-012

- C-optimize-012: Print preflight now inventories nested resource declarations for spot colours
  (Separation/DeviceN), enabled overprint, alpha, soft masks, blend modes and transparency groups
  in Rust. Findings identify each page and explicitly include potentially unused resources;
  the report does not equate this inventory with painted ink simulation. Cyclic references are
  visited once and depth/work limits produce an incomplete-inventory warning rather than a pass.
- Reports identify Close-up print / Large format presets by their actual thresholds, keeping
  custom thresholds named custom. The results sheet links directly to the existing undoable
  hairline/page-box fixups, rather than requiring the reader to rediscover More tools.
- Test `preflight_inventories_nested_press_resources_without_claiming_ink_simulation` covers
  nested, indirect, unused spot/overprint/transparency resources, isolation of an unaffected
  page, named profiles, stated simulation limits and no source mutation. Written, not run.
- Left: actual separations/total area coverage/overprint preview, profile-specific automatic
  fixups, ISO PDF/X validation and durable successive-run audit history. Opening production
  controls is not a claim of automatic fixes for every finding. No dependency/new public API.
- not automatable: results-to-fixups link and both-theme sheet layout — rule 1 forbids the app.
- Grade: f6 a7 p7, partial. Existing report row splitter still limits each block to title + four
  40-character lines. LIGNES : max 5 (preflight finding) · cases > 6 : aucune.

## Pass 3 — C-convert-019

- C-convert-019: PDF/X captures the current unsaved document on its owner thread, then performs
  the external conversion on a separate worker (Acrobat's background export). `PdfXStarted`
  releases the reader operation count; later edits/save/undo remain available. The original
  snapshot, source protection and undo state stay independent of subsequent edits. Spawn errors,
  panics and conversion failures still return a final result and clear the press busy flag.
- Rust now compares the **actual embedded ICC bytes** with the chosen profile, in addition to
  PDF version/X markers/channel count/fonts. Encrypted output, declared transparency and an
  incomplete resource inventory are rejected before publication. These are conservative local
  checks, not ISO certification. No external tool is needed to run these structural checks.
- Tests `press_snapshot_keeps_unsaved_changes_separate_from_later_edits` and
  `written_press_checks_reject_substituted_icc_and_transparency_without_ghostscript` have no
  Ghostscript/ICC-install skip. Synthetic profile bytes are explicitly only a checker fixture;
  they do not prove a usable colour profile. Existing real-GS integration remains necessary.
- New public core `press::Snapshot` with `capture`/`export`; app `Reply::PdfXStarted` added.
  Reply callbacks are mutex-serialized so the new worker requires no stronger Sync API from
  callers. No dependency. Written tests only; worker does not run code under brief rule 1.
- Left: actual conversion still needs Ghostscript and a real user-supplied press ICC. No bundled
  FOGRA/GRACoL, native colour/transparency converter, PDF/X-1a/X-4, PDF/E or full ISO validator.
  These missing engines are not substituted with labels or metadata-only conversion.
- not automatable: start a long PDF/X conversion; edit/save/undo while it runs, close the tab,
  reopen production to see Converting, and inspect final/error toasts in both themes — runtime
  forbidden. Cancellation remains the existing bounded 120-second subprocess timeout.
- Grade: f4 a7 p7, partial. LIGNES : max 4 (press requirements) · cases > 6 : aucune.

## Pass 3 — C-convert-005

- C-convert-005: local HTML on explicit A4/Letter layouts can Stack pages into one long page.
  LibreOffice still prints the HTML; Sheaf's Rust stacks those paper pages as Form XObjects,
  retaining vector/searchable content, resources, transparency groups, URI links, positioned
  link rectangles/QuadPoints and internal/named/bookmark destinations. Tall outputs use UserUnit
  instead of clipping at PDF's 14,400 default-unit dimension limit. Source/output collision
  handling stays atomic. The choice is saved/restored with Run again and bypasses preview cache.
- This is explicitly **paper stacking**, preserving print breaks/margins. It does not claim
  iLovePDF's browser layout or continuous browser content. Unsupported rotated/UserUnit/tagged/
  form layouts and non-link annotations fail clearly with Turn off Stack guidance; no partial
  result is published. Limits: 1000 pages, 32 MB decoded per paper page, 1,000,000 pt tall.
- Tests `long_local_output_keeps_vector_content_link_rectangles_and_internal_destinations`,
  `very_long_local_output_scales_page_units_and_rejects_unsupported_rotation_atomically` are
  pure Rust and never skip for missing LibreOffice. Expanded
  `html_layout_reaches_the_job_without_reusing_the_default_conversion` and saved-options test
  cover dispatch/restoration. Written only, no execution permitted by rule 1.
- Integration: new `html::Options.long_page: bool` and `ToolOptions.html_long_page`; every known
  literal/restore/export/history site updated. No preference or dependency added.
- Left: URL capture remains **out under rule 3 (network)** and needs Nassim's decision.
  Browser viewport width, screen media, JavaScript and real browser-continuous layout remain
  unimplemented; LibreOffice is a print engine. No URL-first parity claim or joint-9 request.
- not automatable: check both themes, long HTML with pictures and internal/URI links in multiple
  viewers, very tall output and Run again — app/test execution forbidden.
- Grade: f7 a8 p8, partial. LIGNES : max 4 (long-page information) · cases > 6 : aucune.

### Pass 3 static follow-up — C-optimize-014

- Source API inspection found lopdf's `get_page_content` appends a stream separator newline;
  corrected the unselected-page test's expected bytes to include it. Hairline page streams now
  decode explicitly with a 32 MB total page bound rather than the image helper's silent empty
  fallback on decode errors; corrupt content cannot masquerade as a fully inspected page.
  The existing atomic-edit refusal test covers rollback; no test was executed.

### Pass 3 static follow-up — C-convert-005

- Long-page UserUnit also scales link border widths/radii/dashes, including the implicit default
  border, so very tall output does not thicken link outlines. The very-long-page Rust test now
  checks the physical border width as well as the physical page size. Written, not executed.

### Pass 3 test follow-up — C-optimize-014

- `unreadable_page_streams_cannot_be_silently_skipped_by_hairline_repair` explicitly covers a
  corrupt Flate stream after a repairable stroke: the edit must fail without changing the
  working snapshot. Written, not run; no external program or optional-runtime skip.

## Polaroid — pass 3 handoff (current)

- Project Sheaf, batch sol-convert, branch `par/sol-convert`, status **partial / ready for
  orchestrator integration**. All six reviewed components received changes and per-component
  commits pushed immediately. No component is claimed closed or at joint 9.
- Closed subgaps: PDF/A's regressed default; native spreadsheet data pages without LibreOffice
  with sheet selection; nested Form hairlines; press-resource inventory and report-to-fixups
  navigation; PDF/X document-thread blocking and weak ICC/transparency checks; local printed
  HTML long-page stacking. These are source implementations with written tests, not runtime proof.
- Remaining deciding gaps: faithful Office runtime/bundling and full fidelity; network URL capture
  (explicitly excluded), browser screen/continuous layout; extra PDF/A levels and real downgrade;
  PDF/X/E engines/levels, ICC distribution and ISO validation; actual output preview/ink coverage,
  fixup profiles/audit history; pattern/Type 3/annotation hairlines, richer marks/traps/flattening.
- No cargo, rustc, clippy, test suite, app, Chrome, Xvfb or codex executed. Verification performed:
  pinned Rust API/source inspection, all known changed constructors/callbacks searched,
  `git diff 4d5e12b..HEAD --check`, Impeccable static detector `[]`, clean worktree and actual
  `git ls-remote` head equality after feature pushes. These do not prove compilation or behaviour.
- No new package/version or lockfile change. Existing pure-Rust `calamine` is now a regular core
  dependency (formerly dev-only). New public `spreadsheet::to_pdf`, `press::Snapshot::capture`
  and `Snapshot::export`; new `Job::Office.data_sheets`, `html::Options.long_page`,
  `ToolOptions.html_long_page`, and `Reply::PdfXStarted`. No Prefs field or reader menu index added.
- Integrator: reconcile small shared-file blocks, build Slint/Rust, run full suite and all named
  tests; then manually check both themes, absent-LibreOffice native mode, saved options,
  PDF/X editing during export, and long-page links/UserUnit in actual viewers. Real LO/GS/ICC
  tests still require those runtimes; their skips are not passes. Validate whether Ghostscript
  preserves the chosen ICC bytes exactly on all supported versions; mismatch must publish nothing.
- Next authorized action is the orchestrator's merge/build/test and independent Opus re-review.
  Author grades below remain provisional and source-only, replacing earlier pass grades.

### Pass 3 complete changed-file list

- `crates/sheaf-app/src/jobs.rs`
- `crates/sheaf-app/src/reader_doc/mod.rs`
- `crates/sheaf-app/src/reader_ui/mod.rs`
- `crates/sheaf-app/src/reader_ui/prepress.rs`
- `crates/sheaf-app/src/run.rs`
- `crates/sheaf-app/src/saved_options.rs`
- `crates/sheaf-app/tests/jobs.rs`
- `crates/sheaf-app/ui/flow-types.slint`
- `crates/sheaf-app/ui/screens/options.slint`
- `crates/sheaf-app/ui/screens/reader-prepress.slint`
- `crates/sheaf-app/ui/screens/reader-production.slint`
- `crates/sheaf-core/Cargo.toml`
- `crates/sheaf-core/src/html.rs`
- `crates/sheaf-core/src/html_long_page.rs`
- `crates/sheaf-core/src/lib.rs`
- `crates/sheaf-core/src/preflight.rs`
- `crates/sheaf-core/src/press.rs`
- `crates/sheaf-core/src/spreadsheet.rs`
- `crates/sheaf-core/tests/engine_patches.rs`
- `crates/sheaf-core/tests/office.rs`
- `crates/sheaf-core/tests/preflight.rs`
- `dev/par-notes-sol-convert.md`
- `third_party/pdfcraft-edit/src/prepress.rs`
- `third_party/pdfcraft-engine/src/print_resources.rs`
- `third_party/pdfcraft-engine/src/printcheck.rs`

### Pass 3 author grades — not joint closure

C-convert-002: f8 a8 p8 — `native_data_report_selects_sheets_preserves_cells_and_needs_no_office_runtime`,
`crates/sheaf-core/src/spreadsheet.rs:100`: visible/explicit sheets, addresses, values and source kept;
faithful Office import/runtime gap stays open.

C-convert-005: f7 a8 p8 — `long_local_output_keeps_vector_content_link_rectangles_and_internal_destinations`,
`crates/sheaf-core/src/html_long_page.rs:202`: preserved vector forms and link/destination geometry;
URL/browser-continuous rendering stays absent.

C-convert-018: f7 a8 p8 — `archival_history_keeps_the_old_copy_and_issues_default_but_honours_strict`,
`crates/sheaf-app/src/saved_options.rs:64`, UI `screens/options.slint:331`: default/history regression
repaired while strict opt-in survives; extra conformance levels stay absent.

C-convert-019: f4 a7 p7 — `press_snapshot_keeps_unsaved_changes_separate_from_later_edits`,
`crates/sheaf-core/src/press.rs:224`, plus `written_press_checks_reject_substituted_icc_and_transparency_without_ghostscript:241`:
independent snapshot and stronger local checks; conversion still external and only X-3.

C-optimize-012: f6 a7 p7 — `preflight_inventories_nested_press_resources_without_claiming_ink_simulation`,
`crates/sheaf-core/tests/preflight.rs:62`: page-scoped declarations, preset labels, unchanged source;
output/separations simulation and audit/fixup-profile gaps stay open.

C-optimize-014: f6 a7 p7 — `grouped_hairlines_use_private_forms_per_scale_and_leave_unselected_pages_unchanged`,
`crates/sheaf-core/tests/engine_patches.rs:183`: private scaled nested Forms, unselected content and
undo; cyclic/corrupt refusal tests at 239/259. Patterns/marks/trapping/flattening gaps stay open.

All cited tests are WRITTEN, NOT RUN. No aesthetic render or performance benchmark was performed.
LIGNES : max 5 (preflight finding / data-report information) · cases > 6 : aucune.

## Pass 4 — review-sol-convert-2 (active)

- C-convert-002: Returning to Sheaf after installing/removing LibreOffice now refreshes existing
  Office/HTML tabs once on window focus (iLovePDF's offline conversion). The built-in Excel report
  remains available without LibreOffice. Test WRITTEN, NOT RUN:
  `office_runtime_refresh_enables_existing_tabs_after_install_and_disables_after_removal`.
  No bundling or new dependency; corpus fidelity still needs the orchestrator's real runtime.
  not automatable: open a Word tab without LibreOffice, install it, focus Sheaf, verify Run becomes
  available without a new tab — worker forbidden to launch the app.

Polaroid: Pass 4 targets the five DISAGREEs plus the accepted Office refresh nit; source-only
verification. No network features; no compiler, test, app or Codex invocation. Next: PDF/A levels.

- C-convert-018 (partial): PDF/A-1b is selectable in GUI/history and CLI `--level 1b`, written
  natively in Rust after the existing baseline repairs. Classic PDF 1.4 output, part-1 XMP,
  explicit checks for transparency (including indirect alpha), layers, attachments and newer
  stream/font features. Strict mode publishes nothing on failure; permissive mode reports the
  unresolved issues. Test WRITTEN, NOT RUN:
  `native_pdfa1_writes_pdf14_and_checks_transparency_before_publication`; `pdfa_job` extended.
  Left: accessible a/u levels require semantic tagging/Unicode validation; automatic downgrade
  needs a reported actual output level and explicit policy. Neither is simulated by relabeling.
  Native 1b is a conservative structural subset, not a full ISO validator or flattening engine.
  No dependency. New public variant `PdfALevel::A1b`; orchestrator must update exhaustive matches
  in concurrent branches if any. New private core module `pdfa1.rs`.
  not automatable: check new level, history, strict failure and report in both themes — no app runs.

- C-convert-019 (partial): Acrobat's Save as PDF/X now offers X-1a:2001, X-3:2002 and X-4.
  Ghostscript gets the explicit variant and PDF version; Sheaf checks exact variant markers,
  X-1a conformance, exact ICC bytes, embedded fonts, forbidden colour spaces (X-1a), and
  transparency (allowed only for X-4). An old converter's wrong output is refused, never relabeled.
  Named FOGRA39 / GRACoL / SWOP buttons find validated CMYK profiles already installed in bounded
  local directories (`SHEAF_ICC_DIR` supported); absent profiles get a recovery message and the
  manual ICC picker. No profile is fabricated or downloaded. PDF/E remains unavailable: no engine
  or validator for its engineering-specific restrictions. No new dependency.
  Tests WRITTEN, NOT RUN:
  `press_levels_require_the_requested_markers_and_allow_transparency_only_in_x4`,
  `named_press_profiles_require_a_real_matching_installed_cmyk_profile`; existing snapshot,
  wrong-output, timeout, ICC-substitution and transparency tests updated/preserved.
  Public additions: `press::Level`, `Preset`, `installed_profile`; `Options.level` is required.
  Existing `to_pdfx3[_with]` names retained for compatibility but now honor `Options.level`.
  Runtime reference checked: https://ghostscript.readthedocs.io/en/latest/VectorDevices.html
  (`-dPDFX=1/3/4`; its older "Creating a PDF/X" warning contradicts the updated option section).
  Installed 10.02.1 sample only demonstrates X-3; real-runtime X-1a/X-4 corpus validation remains
  mandatory, and unsupported runtimes must publish nothing. Profile files are not bundled.
  not automatable: both themes, three levels, absent profile/runtime recovery, background snapshot
  — app execution is forbidden. Source reference: Acrobat's `PrintProduction_R_RHP.aapp` and
  `Standards_R_RHP.aapp` read directly from the archive.

- C-optimize-014 (partial): Acrobat's Fix Hairlines now traverses selected tiling patterns and
  Type 3 charprocs as well as Forms. Each pattern selection/text show gets private resources;
  glyph scale includes FontMatrix, text size, horizontal scale, text matrix and page transform.
  The source font's encoding, advances and originals for other pages stay untouched. Depth,
  operator and decode bounds also cover patterns/glyphs; damaged content is refused.
  Add Printer Marks offers crop / all / registration / CMYK bars / page information and a physical
  mark width. Registration uses Separation All, including spot plates. Information includes page
  number/count and trim dimensions in a shared embedded Inter font. Extra marks get 30 pt outside
  bleed; crop-only keeps 18 pt. Existing trim/bleed/art and original cropping remain; repeat calls
  do not duplicate a mark type; each operation is undoable. No external program or dependency.
  Tests WRITTEN, NOT RUN:
  `pattern_and_type3_hairlines_are_private_per_use_and_leave_shared_pages_unchanged`,
  `printer_marks_include_all_plate_registration_process_bars_and_embedded_page_info`.
  Left: trap presets (no trapping engine), flattener preview (no compositing comparison/render
  path); UI explicitly says so. Ordinary-font stroked text and annotation appearances remain.
  Public engine addition: `Edit::AddPrinterMarks` and `prepress::add_printer_marks`.
  not automatable: both themes, scope, five mark choices/width, repeated use and undo, Type 3
  glyph positions/advances and nested pattern matrices — no app/renderer allowed in this worker.

- C-optimize-012 (partial): Acrobat-style named Digital press / PDF/X-1a readiness / PDF/X-4
  readiness profiles now run target rules together, beyond the existing resolution/bleed thresholds.
  They check embedded fonts, explicit trim, a real embedded CMYK/Gray press ICC output intent,
  encryption and incomplete resource inventory. X-1a additionally flags transparency and conservative
  unsupported colour declarations. The dated report retains the target and exact thresholds, failure
  and warning counts. Resource measurements are computed once per report. No document edits.
  Test WRITTEN, NOT RUN: `named_preflight_profiles_check_target_rules_together_without_editing`.
  Left: on-page ink toggles, total area coverage, overprint simulation and full PDF/X certification.
  These require actual compositing/separation rendering; a resource inventory or RGB-to-CMYK guess
  would not prove the ink on the page, so no imitation preview was added. Existing page-linked
  inventory and undoable fix links remain. No dependency. Public `preflight::Preset` and
  `report_with_preset`; `Op::PrintPreflight` now includes the preset. New private core module
  `preflight_profiles.rs`; `press::profile_channels` / `non_device_colour` are crate-visible only.
  not automatable: select each target, alter thresholds, inspect page links and save the dated
  snapshot in both themes — app execution forbidden.

- C-convert-005 (partial, material gap addressed): Local HTML now has optional Chromium screen
  layout at 800 / 1280 / 1920 CSS px and a genuine continuous-page option. Rust prepares local
  resources/encoding/base, disables meta refresh, bounds the source, measures at the chosen width,
  invokes the isolated local browser with bounded pipes/timeouts, verifies a nonempty PDF and (for
  continuous mode) exactly one page of the measured width/height, then atomically publishes it.
  Screen-media rules are frozen before printing (grid/flex stay screen layouts), print-only rules
  are disabled; paper breaks are suppressed rather than stacked. Margins occur only at the page
  edges. A failed measurement, extra pages or wrong dimensions publishes nothing, with recovery
  instructions. Source JavaScript is off. CSP allows local/data images/fonts and local CSS only.
  No URL capture, downloading, cloud or other network feature; browser background networking and
  DNS resolution disabled. Chromium/Chrome/Edge is discovered locally, or `SHEAF_CHROMIUM` supplies
  the binary. If absent, the error offers installation or explicit LibreOffice paper layout; it
  never silently falls back. Native Rust retains Office paper/stacked output for the old mode.
  Saved options keep width and long-page choice; screen mode does not require LibreOffice.
  Tests WRITTEN, NOT RUN:
  `screen_html_freezes_media_and_uses_one_measured_page_without_network_resources`,
  `browser_long_page_checks_reject_pagination_and_wrong_dimensions`, extended HTML job/history,
  `real_local_screen_grid_has_no_paper_breaks_when_a_browser_is_explicitly_supplied` (opt-in via
  SHEAF_TEST_CHROMIUM; a skip is not a pass). Worker never launched any browser.
  Left: a bundled/native browser-class CSS layout engine is absent; source scripts are disabled;
  continuous output is conservatively limited to 200 inches and 100k DOM elements. Actual local
  CSS/font/link fidelity and process behavior require the orchestrator's real-browser corpus.
  No dependency (existing getrandom/serde_json/tempfile only). New public `html::Options.screen_width`
  and `ToolOptions.html_screen_width`; new private `html_browser.rs`.
  Browser reference read: https://developer.chrome.com/docs/automation-and-testing/headless .
  not automatable: both themes, widths, absent Chromium/absent LibreOffice, saved history, relative
  CSS/images/fonts, grid/flex, screen-vs-print CSS and one continuous page — no app/Chrome allowed.

- C-convert-018 source-review follow-up: lopdf preserves an input's xref-stream choice even after
  changing the PDF header. Native 1b now explicitly selects a classic xref table and removes old
  object/xref containers and trailer offsets. Verification inspects the actual written input bytes,
  not a fresh engine serialization. The native 1b regression also asserts the xref-table type.
  Still source-only: no compiler or test was run.

- C-convert-018 final static audit: xref type uses pattern matching (upstream enum has no
  PartialEq); part-1 identification reuses the engine's RDF parser, including attribute-form XMP,
  now covered in the native 1b regression. Indirect filter names and a newer catalog version are
  checked too. These are source/API checks, not executed test results.

## Pass 4 handoff — source work complete; integration pending

Tatouages unchanged: one batch/worktree; no compiler/build/test/app/Chrome/Codex invocation;
network features excluded; no copied competitor code and no new dependency.

Polaroid — Sheaf / sol-convert / pass 4: active pending orchestrator integration. All six components
have a feature commit pushed to `origin/par/sol-convert`; additional native PDF/A source-audit fixes
were pushed separately. The starting merged tree was `ce2a6dc` (the worktree had already fast-forwarded
from pass 3 to `origin/wip/round27` before this pass's first commit). Review input remains
`/tmp/f5/par/review-sol-convert-2.txt`. No component is declared jointly closed by this source pass.

Fait: code, regression tests and notes authored/re-read; per-component commits pushed. Static
`git diff ce2a6dc..HEAD --check` clean; Impeccable detector returned `[]` for the four changed Slint
files. Source APIs checked against the actual cached PdfCraft/lopdf revision (including the xref
writer and enum). These checks prove neither compilation, rendered aesthetics nor runtime speed.

Pas fait: every new/modified test is WRITTEN, NOT RUN. All six components need orchestrator build,
full suite and counterpart review. Runtime-dependent checks must distinguish SKIP from PASS.
Specifically: real Office fidelity, all three Ghostscript PDF/X variants with actual ICC files,
real Chromium flex/grid/screen CSS and one-page output, native 1b against an independent validator,
Type 3 advances/pattern matrix renders, mark placement/undo and both Slint themes.

Prochain pas: merge the small shared-file blocks, build Slint/Rust once in the orchestrator, run the
named regressions/full suite, enable `SHEAF_TEST_CHROMIUM` for the opt-in browser corpus, then request
Opus's read-only review. Outside runtimes/profiles are not bundled. Any unavailable runtime must
fail clearly; there is no network/download fallback.

Integrator interface changes (no Cargo.toml/lockfile change):
- `PdfALevel::A1b`, CLI `CliPdfA::A1b`; update any concurrent exhaustive matches.
- `press::Options.level` required; `press::Level`, `Preset`, `installed_profile` added.
- `html::Options.screen_width: Option<u32>` required; `ToolOptions.html_screen_width` saved.
- `preflight::Preset`, `report_with_preset`; reader `Op::PrintPreflight` includes `preset`.
- `Edit::AddPrinterMarks`, engine `prepress::add_printer_marks` added; label/permission/apply arms
  included. Existing `AddCropMarks` retained.
- Slint `preflight-check`, `press-export`, `production-marks` have additional arguments;
  `press-preset` added. Rust handlers and wiring changed together. No new Prefs field.

### Pass 4 touched files (relative to ce2a6dc)

- `crates/sheaf-app/src/main.rs`
- `crates/sheaf-app/src/reader_doc/mod.rs`
- `crates/sheaf-app/src/reader_ui/prepress.rs`
- `crates/sheaf-app/src/run.rs`
- `crates/sheaf-app/src/saved_options.rs`
- `crates/sheaf-app/src/state.rs`
- `crates/sheaf-app/ui/flow-types.slint`
- `crates/sheaf-app/ui/screens/options.slint`
- `crates/sheaf-app/ui/screens/reader-prepress.slint`
- `crates/sheaf-app/ui/screens/reader-production.slint`
- `crates/sheaf-cli/src/args.rs`
- `crates/sheaf-cli/src/main.rs`
- `crates/sheaf-core/src/html.rs`
- `crates/sheaf-core/src/html_browser.rs`
- `crates/sheaf-core/src/html_source.rs`
- `crates/sheaf-core/src/pdfa.rs`
- `crates/sheaf-core/src/pdfa1.rs`
- `crates/sheaf-core/src/preflight.rs`
- `crates/sheaf-core/src/preflight_profiles.rs`
- `crates/sheaf-core/src/press.rs`
- `crates/sheaf-core/tests/engine_patches.rs`
- `crates/sheaf-core/tests/office.rs`
- `crates/sheaf-core/tests/pdfa.rs`
- `crates/sheaf-core/tests/preflight.rs`
- `crates/sheaf-core/tests/press.rs`
- `dev/par-notes-sol-convert.md`
- `third_party/pdfcraft-edit/src/prepress.rs`
- `third_party/pdfcraft-engine/src/lib.rs`
- `third_party/pdfcraft-engine/src/prepress.rs`

### Pass 4 provisional author grades — source evidence, not joint closure

C-convert-002: f9 a9 p9 — counterpart's existing AGREE 9 preserved; refresh nit addressed by
`office_runtime_refresh_enables_existing_tabs_after_install_and_disables_after_removal`,
`crates/sheaf-app/src/state.rs:761` (written). Native Excel / original Office conversion unchanged.

C-convert-005: f8 a8 p7 — `screen_html_freezes_media_and_uses_one_measured_page_without_network_resources`
and `browser_long_page_checks_reject_pagination_and_wrong_dimensions`,
`crates/sheaf-core/src/html_browser.rs:187/204` (written). Actual browser-grid corpus opt-in at 221;
no browser-class native engine or source-script execution; real browser behavior unverified.

C-convert-018: f8 a8 p8 — `native_pdfa1_writes_pdf14_and_checks_transparency_before_publication`,
`crates/sheaf-core/tests/pdfa.rs:121` (written): part 1, classic xref, attribute XMP and strict refusal.
a/u levels and automatic downgrade remain absent; structural checking is not full ISO validation.

C-convert-019: f7 a8 p7 — `press_levels_require_the_requested_markers_and_allow_transparency_only_in_x4`,
`named_press_profiles_require_a_real_matching_installed_cmyk_profile`,
`crates/sheaf-core/src/press.rs:300/322` (written). X-1a/X-4 runtime fidelity still unverified;
PDF/E unsupported and named profiles require real installed ICC files.

C-optimize-012: f7 a8 p7 — `named_preflight_profiles_check_target_rules_together_without_editing`,
`crates/sheaf-core/tests/preflight.rs:96` (written). Named profiles add real grouped target checks;
ink toggles, coverage and overprint simulation require a separation/compositing renderer, absent.

C-optimize-014: f8 a8 p7 — `pattern_and_type3_hairlines_are_private_per_use_and_leave_shared_pages_unchanged`,
`printer_marks_include_all_plate_registration_process_bars_and_embedded_page_info`,
`crates/sheaf-core/tests/engine_patches.rs:536/601` (written). Trap presets/flattener preview remain
absent (no trapping/compositing path); ordinary-font stroked text/comments remain outside repair.

Contrôle: mandat oui (six components addressed; unresolved gaps explicitly retained); preuves oui
(source/diff/remote evidence, no invented runtime proof); dépendances oui (runtime/profile checks
identified, native-engine gaps stated as implementation gaps); parties indépendantes oui (the
selected meaningful fixes and their regression coverage are authored and pushed).

LIGNES : max 5 (preflight finding) · cases > 6 : aucune.

## Batch sol-ocr-sign


## Tatouages

- Work only in `par/sol-ocr-sign`. No compiler, build, lint, test, app, browser or codex runs (parallel brief rule 1). The orchestrator owns compilation and runtime verification.
- No network features or new native dependencies. References are inspected from archives without extraction.

## Component notes

- C-sign-009: kept signatures and initials leave `prefs.json` for the system credential store
  (Preview's Keychain; Linux Secret Service via `secret-tool`, macOS `security`, Windows DPAPI via
  PowerShell). Old profiles migrate after the store accepts them; a locked store never falls back
  to plaintext or overwrites a saved secret, and new signatures remain available for the session.
  A failed save is shown in the reader. Tests `signatures_and_initials_leave_preferences_and_round_trip_as_a_secret`,
  `an_unreadable_store_is_not_erased_when_preferences_are_saved`.
  Existing preferences round-trip fixture now tests nonsensitive settings; the new unit test owns
  signature serialization. `not automatable: save, restart, reuse and delete a mark with each OS
  credential store, including locked-store behaviour — requires logged-in desktop/keyring access`.
  Left: platform integration must be run; no runtime parity claim from this unbuilt worker.

- C-optimize-006 (partial): the Latin recognizer now prepacks its matrix weights once when loaded,
  reusing them for every line instead of packing on each inference. RTen's installed source
  (`ModelOptions::prepack_weights`) documents this inference optimization. Test
  `prepacked_recognition_keeps_the_words_and_their_placement` compares packed and unpacked results.
  New public item: `Ocr::load_with_prepacked_weights` for accuracy/performance comparisons.
  Left: release timing on the MICI scan against the recorded 15.5 s/page and Tesseract's
  11.75 s/page; no claim that this closes the performance grade without the orchestrator's run.

- C-optimize-007 (partial): the existing OCR language selector offers installed Chinese,
  Japanese, Korean (horizontal and vertical), Arabic, Hebrew, Cyrillic, Greek, Hindi, Thai and
  Vietnamese packs through offline Tesseract, as iLovePDF's `LanguageOcrModel.qml` does.
  The same engine job preserves skip-text, duplicate filtering, page geometry and cancellation.
  Tests `local_ocr_keeps_cjk_words_lines_and_vertical_geometry`,
  `malformed_local_ocr_fails_instead_of_losing_a_page_silently`; locale regression updated to
  account for an installed Japanese pack. New engine public items `Line`, `Word`, `Turn`
  reexports and `OcrJob::run_with`.
  Left: Tesseract executable and language packs are optional external runtime requirements,
  not bundled; scripts absent locally are not offered. This does not reach iLovePDF's roughly
  100-language bundled/downloadable coverage. No downloads or network code added.
  `not automatable: select an installed CJK language in the OCR panel and inspect selection
  against a real scan — needs the orchestrator's app and matching local language pack`.

- C-optimize-015: a set of PDFs gets a reusable, versioned full-text catalog (Acrobat's
  Catalog / Add search index), built by `sheaf index a.pdf b.pdf -o catalog.json`, searched by
  `sheaf search-index catalog.json "phrase"`. Rebuild by rerunning Index to a fresh output;
  delete or exchange the JSON as an ordinary file. Unicode phrases cross line ends; changed,
  replaced or removed sources invalidate searches. Folder Search uses a `.sheaf-index.json`
  beside the chosen folder when available and falls back to parsing changed/new PDFs.
  Protected files are refused to avoid leaking their text. Test
  `a_saved_catalog_finds_unicode_phrases_and_refuses_changed_sources`.
  Existing workspace dependencies serde and serde_json now also linked by sheaf-core (no new
  version/package). Left: a dedicated GUI catalog manager; no Acrobat `.pdx` interchange.

- C-security-008: reusable named security policies are saved/exported as versioned JSON,
  imported with strict validation, and applied to PDFs with passwords supplied separately
  (Acrobat's Security Policies). CLI `save-security-policy NAME --no-copying … -o policy.json`
  and `apply-security-policy FILE --policy policy.json --user-password … -o protected.pdf`.
  Permissions and the encrypt-metadata choice survive exchange; unknown security fields and
  versions are refused. No passwords saved in policies. Tests
  `policies_exchange_permissions_and_metadata_but_never_credentials`,
  `a_policy_with_unknown_security_fields_or_versions_is_refused`,
  `an_imported_policy_applies_restrictions_and_leaves_metadata_clear`.
  New public core module `security_policy` and `security::protect_with_settings`.
  Left: dedicated GUI policy manager and Acrobat policy-file compatibility.

- C-sign-012 (partial): Sheaf's visible signing and invisible certification APIs now accept
  an explicit offline `TimestampAuthority` and invoke the engine's `sign_with_timestamp`
  (Acrobat's signature timestamp / PAdES B-T). Invalid/mismatched responses fail before writing;
  no silent fallback to an untimestamped signature. Tests
  `a_local_authority_timestamps_visible_and_invisible_certificate_signatures`,
  `an_invalid_timestamp_response_never_writes_an_untimestamped_signature` use a local authority.
  New public core items `sign::sign_with_timestamp`, `sign::certify_with_timestamp`.
  Left: TSA server selection and transport remain absent: brief rule 3 explicitly prohibits
  network features. A local adapter does not confer trust on a self-signed TSA. No GUI claim.

- C-optimize-009 (partial): `sheaf review-ocr FILE` lists hidden OCR runs with their page
  boxes and possible decoding artifacts; `correct-ocr FILE --page N --line N --expected OLD
  --text NEW -o fixed.pdf` corrects the hidden text, or `--not-text` removes it (Acrobat's
  Correct Recognized Text / This is not text). Visible text and stale review rows are refused.
  Tests `corrected_and_rejected_ocr_runs_change_search_text_but_never_the_scan`,
  `ocr_correction_refuses_visible_text_and_stale_review_rows` check scan pixels and searchable text.
  New public items core `ocr_review::{ReviewLine, review, correct}`, `TextLine::invisible`.
  Left: GUI suspect highlighting, per-suspect Accept/Next and retained model confidence are
  absent. This is an accessible CLI correction workflow, not Acrobat GUI parity.

- C-security-007 (not implemented): inspected Acrobat's `PPKLite.Primary.txt` entries
  Encrypt with Certificate / Encrypt all contents / Except metadata / Only attachments,
  core `security.rs`, PdfCraft COS `security::encrypt_dict`, the crypt crate's
  `SecurityHandler::open`, lopdf's `encryption.rs` and the signing crate's `cms.rs`.
  The reader's crypt handler explicitly rejects every `/Filter` except `/Standard`; its
  EncryptDict has no recipient envelopes, and the current CMS code supports signatures,
  not EnvelopedData. A writer alone would make documents Sheaf cannot reopen. Recipient CMS
  envelopes, `/Adobe.PubSec` file-key derivation, private-key unlocking and the recipient UI
  remain to implement together; no misleading certificate-encryption switch or output added.
  The independent except-metadata setting was implemented for password policies, not claimed
  as certificate encryption. No test added for an implementation that does not exist.

## Integration checks written (not run)

- Secure-storage smoke helpers now read Secret Service rather than plaintext preferences;
  smoke 240 asserts marks are absent from JSON and present in the system store. Linux GUI
  smoke now requires `secret-tool` and an unlocked Secret Service session. No fallback.
- `an_offline_ocr_adapter_keeps_skips_cancellation_and_unicode_placement` tests the new engine
  adapter from sheaf-core, without any model or Tesseract binary.
- `a_catalog_build_reads_real_pdfs_and_reuses_only_unchanged_pages` and
  `protected_sources_and_cancelled_catalog_builds_are_refused` exercise real PDF extraction,
  deduplication, persistence, cancellation and the protected-text guard.
- All new CLI arguments have help text, as required by `every_option_and_value_has_help`.
- Signature-store cache avoids OS queries on unrelated saves, and refuses writes when marks
  could not be loaded at startup (including a keyring unlocked later in the same session).

## Worker handoff / Polaroid — 27/10/2026

- Delivered: implementation for C-sign-009, C-optimize-015 and C-security-008, with GUI/runtime
  limits above; partial C-optimize-006, C-optimize-007, C-optimize-009 and C-sign-012;
  C-security-007 explicitly not implemented. No board grades changed or 9/10 closure claimed.
- Code proof: commits `de004b3` through `3177f2c` pushed to `origin/par/sol-ocr-sign`.
  The final documentation commit follows these. Branch started from `d501e13`.
- Checks actually performed: source/archive inspection, `git diff --check`, and
  `bash -n dev/scripts/smoke/1-reader.sh dev/scripts/smoke/3-flows.sh` succeeded.
  No cargo, rustc, clippy, test suite, app, Chrome, Xvfb or codex invocation.
- Compile handoff: no new Rust package/version. sheaf-core now directly depends on the
  workspace's existing serde/serde_json; the orchestrator's first Cargo invocation will update
  those direct dependency edges in Cargo.lock (not regenerated by this worker).
- Runtime handoff: secure signatures need Linux `secret-tool` + an unlocked Secret Service;
  macOS uses built-in `security`; Windows uses built-in PowerShell/DPAPI. Extra OCR scripts
  require optional local Tesseract + the matching traineddata packs. No downloads implemented.
- Required next action: orchestrator merges, compiles/lints, runs unit/integration and GUI smoke,
  measures release OCR time against the MICI baseline, and reports failures back. macOS and
  Windows credential adapters still need live platform verification. No macOS-only build work.
- Additional storage tests: `an_unloaded_profile_cannot_clear_a_store_that_later_becomes_available`,
  `failed_migration_preserves_an_existing_legacy_signature_without_writing_new_plaintext`.
  Credential writes are read back before plaintext legacy data is removed. A failed migration
  leaves the original JSON unchanged and never writes the newly selected mark in plaintext.
- Additional locale/CLI tests: `chinese_locales_choose_the_installed_script`,
  `ocr_correction_requires_one_action_and_positive_review_indexes`,
  `catalog_and_security_policy_commands_are_accessible_from_the_cli`.
- Control: mandate covered for every assigned component (including explicit nonimplementation);
  evidence linked to source/tests/commits; remaining execution belongs to the orchestrator under
  rule 1; independent permitted changes completed. Runtime success remains unverified.

## Touched files

- `ARCHITECTURE.md`
- `crates/sheaf-app/src/home.rs`
- `crates/sheaf-app/src/lib.rs`
- `crates/sheaf-app/src/prefs.rs`
- `crates/sheaf-app/src/reader_ui/folder_search.rs`
- `crates/sheaf-app/src/signature_store.rs`
- `crates/sheaf-app/tests/history.rs`
- `crates/sheaf-cli/src/args.rs`
- `crates/sheaf-cli/src/main.rs`
- `crates/sheaf-core/Cargo.toml`
- `crates/sheaf-core/src/lib.rs`
- `crates/sheaf-core/src/ocr.rs`
- `crates/sheaf-core/src/ocr_local.rs`
- `crates/sheaf-core/src/ocr_review.rs`
- `crates/sheaf-core/src/search_index.rs`
- `crates/sheaf-core/src/security.rs`
- `crates/sheaf-core/src/security_policy.rs`
- `crates/sheaf-core/src/sign.rs`
- `crates/sheaf-core/tests/engine_patches.rs`
- `crates/sheaf-core/tests/ocr.rs`
- `crates/sheaf-core/tests/ocr_review.rs`
- `crates/sheaf-core/tests/search_index.rs`
- `crates/sheaf-core/tests/security.rs`
- `crates/sheaf-core/tests/sign.rs`
- `dev/par-notes-sol-ocr-sign.md`
- `dev/scripts/smoke/1-reader.sh`
- `dev/scripts/smoke/3-flows.sh`
- `third_party/pdfcraft-edit/src/text.rs`
- `third_party/pdfcraft-engine/src/ocr.rs`
- `third_party/pdfcraft-ocr/src/latin.rs`
- `third_party/pdfcraft-ocr/src/lib.rs`

## Second pass — C-optimize-009

- C-optimize-009: More → More tools → Correct recognized text opens a reader panel beside
  the scan (Acrobat's Correct Recognized Text). Previous, Accept and next, Correct and next,
  and This is not text operate on the live working document, with a highlighted run and page
  navigation. Corrections use the reader's undo/redo/save pipeline; stale text and visible runs
  are checked on the document thread. Other document changes refresh the review.
  Tests written: `live_ocr_review_sees_unsaved_corrections_and_undo_and_refuses_missing_pages`,
  `review_completion_and_empty_layer_are_distinct`; existing pixel-preservation tests still apply.
  New public core helpers: `ocr_review::review_document`, `ocr_review::correction_edit`.
  No new dependency. Not run (brief rule 1).
- `not automatable: More tools → Correct recognized text on an OCR scan; check the highlighted
  run, Accept/Previous, correct, reject, undo/redo, save/reopen and empty/finished states at
  small and large window sizes — app execution forbidden to this worker; orchestrator must run`.
- Left: recognition confidence and suspect-only filtering are absent; decoding artifacts are
  labelled as such. Panel rendering, keyboard traversal and large-document timing unverified.
- Proposed grade: C-optimize-009: f8 a7 p7 — live correction/undo test above, panel in
  `ui/screens/ocr-review.slint`, thread validation in `reader_doc/mod.rs`. No joint 9 claim.

## Second pass — C-optimize-015

- C-optimize-015: More → More tools → Search catalog opens a dedicated manager (Acrobat's
  Catalog). Add/remove saved PDFs, build/save a JSON catalog, open/import an existing catalog,
  search phrases, and open a result at its page, including an already-open PDF. Source paths
  distinguish equal file names. Rebuild saves to a new file; source changes invalidate old hits.
  Build progress, cancellation, missing/changed sources, errors and no results have visible states.
  Work runs off the UI thread; closing drops late replies. Folder Search still reuses a catalog
  saved as `.sheaf-index.json` in its folder.
- Tests written: `catalog_manager_can_reopen_sources_and_rebuild_after_a_removal`,
  `closing_a_catalog_cancels_work_and_reopening_cannot_accept_its_reply`,
  `catalog_selection_deduplicates_sources_in_order`. New public helper `Catalog::sources`.
  No new dependency; tests not run under rule 1.
- `not automatable: More tools → Search catalog; add two PDFs, remove/readd one, build, search,
  open a result, reopen the catalog, cancel a build, then change a source and search again —
  app execution forbidden to this worker; orchestrator must check small/large windows too`.
- Left: no Acrobat PDX interchange, folder picker/recursive catalog creation, or incremental
  updates; rebuild rereads sources. Catalog search cancellation discards the reply but does not
  interrupt an in-progress file fingerprint. Large catalogs and visual fit remain unmeasured.
- Proposed grade: C-optimize-015: f8 a7 p7 — manager/rebuild tests above,
  `ui/screens/catalog.slint`, `reader_ui/catalog.rs`. No joint 9 claim.

- Catalog output follows the actual path returned by the atomic writer: an existing name is
  preserved and the new catalog receives a numbered name. The rebuild test checks both files.

## Second pass — C-security-008

- C-security-008: More → More tools → Security policies opens a dedicated manager (Acrobat's
  Security Policies): create, name, edit, save and delete reusable policies; import JSON for
  review, export it, then apply the selected saved settings to a protected copy of the open PDF.
  Permission and metadata checkboxes use the existing house controls. Applying asks for confirmed
  open/owner passwords; restrictions require a distinct owner password. Unsaved reader changes
  and read-only documents are refused. Encryption runs off the UI thread; status shows the
  actual output path or error. Success and switching policies clear the password controls.
- A local `security-policies/` library stores only typed policy settings, no credentials.
  Unknown/corrupt entries are reported rather than silently overwritten. Same-name Save updates
  that policy; a changed name saves another policy, leaving the original available for deletion.
- Tests written: `a_policy_library_survives_edits_import_export_and_deletion_without_passwords`,
  `a_corrupt_policy_is_reported_without_overwriting_the_library`,
  `applying_a_policy_requires_confirmed_passwords_and_a_distinct_owner_for_restrictions`.
  Existing `an_imported_policy_applies_restrictions_and_leaves_metadata_clear` covers encrypted
  output. New public app module `security_policies`; no new dependency. Nothing executed.
- `not automatable: Security policies → create/save/edit, import/export, delete; apply with bad
  confirmations and a distinct owner password, reopen output and inspect permissions/metadata;
  verify fields clear after success and policy switch — rule 1 forbids this worker's app runs`.
- Left: Acrobat's certified/encrypted .acrobatsecuritysettings exchange, certificate policies,
  trust/signature preferences exchange and live visual/platform verification. This is password
  policy management; certificate encryption is not claimed.
- Proposed grade: C-security-008: f7 a7 p8 — library and password-validation tests above,
  `ui/screens/security-policy.slint`, `reader_ui/security_policy.rs`. No joint 9 claim.

## Second pass — C-optimize-007

- C-optimize-007: all installed Tesseract traineddata packs are now offered, including packs
  outside the initial script list. Known languages retain readable names; unlisted/custom packs
  show their exact installed pack name. Orientation/equation data and unsafe pack names are
  excluded. Nothing downloaded or bundled. The OCR panel's language list scrolls separately.
- Every checked language now reaches recognition (previously only the first was used).
  Built-in Latin languages share the multilingual Latin model; mixed scripts use a single
  Tesseract call with all selected installed packs (e.g. `eng+jpn`). Missing Latin packs in a
  mixed selection are reported before OCR starts. Horizontal/vertical packs must be run in
  separate jobs; they cannot silently assign one orientation to the other's words.
  CLI accepts `--language en+ja`. Local Tesseract can also run when bundled models are missing.
- Tests written/updated: `every_installed_language_pack_is_offered_but_orientation_data_is_not`,
  `mixed_scripts_use_all_selected_packs_and_refuse_missing_or_conflicting_packs`,
  `ocr_job_uses_every_selected_language`. No new dependency or public item.
- `not automatable: OCR panel → select English + installed Japanese, run a mixed scan; verify
  searchable text in both scripts, missing-pack feedback, vertical-only geometry and scrolling
  through 100 installed languages — app/OCR execution forbidden by rule 1`.
- Left: no pack download/bundling; names of unknown packs are their identifiers. Available
  packs are discovered once at process startup, so local installation requires an app restart.
- Proposed grade: C-optimize-007: f8 a8 p8 — selection/pack tests above, `ocr_local.rs`,
  `run.rs::ocr_options`, `ui/screens/options.slint`. Live multilingual accuracy unverified.

## Second pass — C-optimize-006

- C-optimize-006: OCR options now offer Use installed Tesseract, mirrored by CLI
  `ocr --local-engine`. Latin scans can use the already-supported local engine without loading
  the bundled recognizer; page skipping, duplicate filtering, progress and output geometry use
  the same engine job. It is an explicit alternative for big scans, not an unmeasured speed claim.
  The selected engine survives Run again/history; absent local packs fail with a clear message.
- Inspected the profile in F-006 and DECISIONS 24/10: 89% of inference time was Conv, detection
  0.8s vs recognition 18.3s; native ONNX Runtime was deliberately deferred. Adding more unrelated
  detection tweaks cannot demonstrate closing this gap. No new native library or int8 model added.
- Tests written/updated: `local_ocr_is_explicit_for_latin_and_automatic_for_extra_scripts`,
  `local_ocr_engine_and_mixed_languages_are_explicit_cli_options`,
  `ocr_job_uses_every_selected_language`,
  `options_come_back_as_they_were_without_their_passwords`.
  Public `OcrOptions` gains `use_local_engine: bool` (default false); no dependency added.
- `not automatable: time the same MICI scan in release with bundled recognition and with Use
  installed Tesseract/fra, compare text accuracy and word placement, measure cold and warm RSS —
  no builds, OCR or app execution allowed by rule 1; orchestrator owns this benchmark`.
- Left: native bundled recognition's 15.5s/page vs 11.75s/page baseline remains unmeasured after
  prepacking. Optional Tesseract is a user path, not bundled-engine performance parity.
- Proposed grade: C-optimize-006: f9 a8 p8 — prior OCR integration/placement tests plus engine
  selection tests above; UI in `options.slint`. No performance 9 without the release benchmark.

## Second pass — C-sign-009

- C-sign-009: More → More tools → Manage signatures opens a full manager (PDF Expert's
  Edit Signature). Preview saved marks, choose the active one, reorder Up/Down, delete,
  edit a typed name/font/ink/width or a saved drawing, and create a new saved mark (four max,
  matching the existing library). Saved drawing points are converted back into pad coordinates.
  Reordering selects the first mark, consistently with the existing reader tray.
- Updates persist through the same system credential store. A failed manager save/delete restores
  the previous visible list and reports the error. Unsaved editor changes stay local to the sheet;
  the existing one-time-use checkbox remains in Fill & Sign. Initials keep their existing editor.
- Tests written: `signature_edits_preserve_position_and_reject_a_stale_selection`,
  `saved_drawing_can_be_edited_without_changing_its_normalized_points`.
  Existing credential migration, failure and plaintext-exclusion tests still apply. No new deps.
- `not automatable: create/edit/reorder/select/delete saved marks, close/restart and reuse;
  repeat with a locked/missing keyring and ensure failed deletion remains visible — requires
  desktop credential stores and app execution prohibited to this worker by rule 1`.
- Left: platform credential adapters and manager rendering are still unverified on real desktops.
  The existing four-mark limit remains. No cloud/shared signatures (outside this batch).
- Proposed grade: C-sign-009: f8 a8 p8 — mutation/drawing tests above, `signature_manager.rs`,
  `ui/screens/signature-manager.slint`; secure-store tests from pass 1. No joint 9 before runtime.

- More tools now scrolls within the reader's available height, so these added entries remain
  reachable at the app's 600px minimum height. Runtime menu scrolling is part of GUI handoff.

## Second pass — C-optimize-009 follow-up

- The correction panel now reserves space in the reader rather than covering the scan; it
  temporarily closes the left sidebar and restores it on closing. Failed corrections keep
  the current run instead of advancing past an unresolved error.
- Added Possible decoding errors only, retaining each run's original page/line target. This
  supersedes the earlier second-pass note that suspect filtering was absent. It is explicitly
  a decoding-artifact filter, not recognition confidence. Test written:
  `suspect_filter_preserves_the_original_page_and_line_targets`.
- GUI handoff additionally checks sidebar restoration, scan fit beside the panel, filtering,
  failed-correction position and Correct/Next within the filtered set. No execution here.

- OCR review hides its old highlight while refreshing and bounds-checks page navigation, so
  deleting/reordering pages cannot render an obsolete review target. GUI handoff includes
  removing the reviewed page while the panel is open and confirming the refresh is safe.

- Signature preview slots now preserve the saved list's cardinality even for an unreadable
  legacy mark; Edit/Delete cannot shift onto the following entry because its thumbnail failed.
  Test written: `unrenderable_saved_signatures_keep_their_library_positions`.

## Second pass — C-sign-012 (no further implementation)

- Re-inspected core `sign.rs::sign_inner`/`certify_inner`, engine
  `Session::sign_with_timestamp`/`timestamp_document`, the sign crate's
  `TimestampAuthority::timestamp(request)`, and both local-authority integration tests.
  Visible and invisible signing already route the supplied authority through the engine, and
  an invalid response prevents writing. Reason, location and visible certificate appearance
  keep their existing GUI. No new CLI-only signature capability was added in pass 1 or pass 2.
- The outstanding reviewer gap is a timestamp server. It remains excluded by the user's
  no-network instruction. A preselected response file is not a substitute: RFC 3161 responses
  must match the newly signed bytes' request. A self-signed local clock is not a trusted server.
  No server field, fabricated certified time, or misleading file-picker switch was added.
- No code change or new test for this component in pass 2. Existing written tests:
  `a_local_authority_timestamps_visible_and_invisible_certificate_signatures`,
  `an_invalid_timestamp_response_never_writes_an_untimestamped_signature` (not executed here).
- Proposed grade: C-sign-012: f8 a8 p8 — core `sign.rs` and tests above; the real server gap is
  still open. No joint 9 or runtime validation claimed.

## Second pass — C-security-007 (not implemented)

- Re-inspected core `security.rs::protect_with_settings`, COS `security::encrypt_dict`,
  the locked PdfCraft crypt source `crates/crypt/src/lib.rs::{EncryptDict,SecurityHandler::open}`,
  and sign source `crates/sign/src/cms.rs` at the Cargo.lock revision d3a91c5. Acrobat's
  `aapp/Protect_R_RHP.aapp` and `strings/PPKLite.Primary.txt` were read directly from its archive.
- Verified: the crypt handler still rejects every non-Standard /Filter; the parsed encryption
  dictionary has no recipient envelopes. The CMS implementation is SignedData for signatures,
  not recipient EnvelopedData. The app exposes digital IDs for signing, with no private-key
  recipient-unlock path. Password policy encryption cannot stand in for these contracts.
- No certificate-encryption implementation was added. A workable next unit must add CMS
  recipient envelopes plus Adobe.PubSec key derivation and private-key opening, then integrate
  writer, reader and recipient controls together. An offline OpenSSL envelope writer alone
  would not make the reader open the resulting PDF. No output or nonfunctional GUI switch added.
  This is unfinished implementation, not a claim that offline certificate encryption is impossible.
- Tests: none added for an absent feature. Next verification: recipient A opens, unrelated B
  fails, multiple recipients open, metadata/attachment encryption modes agree in an independent
  reader, and Sheaf reopens its own output. The orchestrator must run any eventual tests.
- Proposed grade: C-security-007: f0 a0 p0 — absence confirmed at crypt
  `SecurityHandler::open` and COS `encrypt_dict`; no supported certificate-encryption path.

- The OCR review controls scroll at short window heights; Close and correction actions cannot
  be stranded below the 600px app minimum. This remains a written GUI smoke check, not a render.

## Second-pass handoff / Polaroid — 27/10/2026

- Project: Sheaf, batch sol-ocr-sign, second pass. Branch `par/sol-ocr-sign`; base `10bfcf6`.
  Status: implementation handoff active; compilation/runtime and counterpart review pending.
- Six components advanced: C-optimize-006/007/009/015, C-sign-009, C-security-008. Four dedicated
  GUI surfaces now expose the previously CLI-only workflows plus signature management. They are
  reached from the reader's More → More tools. Every component's remaining gap is stated above.
- C-sign-012: inspected again, no second-pass implementation; network TSA is explicitly excluded.
  C-security-007: inspected again, no certificate-encryption implementation. Its missing reader,
  envelope and private-key contracts remain work, not a claimed external blocker or impossibility.
- Checks performed: repeated `git diff --check` on each component and the complete second-pass
  diff; static source/API inspection. Impeccable's static detector returned `[]` for the Slint
  paths; that does not establish rendered layout or accessibility. No tests or runtime executed.
  No cargo, rustc, clippy, app, Chrome, Xvfb or codex invocation in this pass.
- Every component commit was pushed immediately. Final documentation follows `9c7ec96`.
  No new dependency in this pass. Pass 1's existing serde/serde_json direct edges in sheaf-core
  still require the orchestrator's Cargo.lock integration if not already done elsewhere.
- Public API changes in this pass: core `ocr_review::{review_document,correction_edit}`,
  `Catalog::sources`, `OcrOptions::use_local_engine`; app `security_policies` module.
  No Prefs field added, so no additional history Prefs literal update is needed.
- Runtime requirements remain optional local Tesseract + selected packs, Linux Secret Service
  and secret-tool for saved signatures, macOS security/Keychain, Windows PowerShell/DPAPI.
- Next action: merge, build/lint, run unit/integration + the GUI checks specified per component;
  benchmark OCR in release on the MICI baseline; send failures back. Only after the full suite is
  green does counterpart opus-edit review read-only for the joint grades. No board grade changed.
- Mandate control: six independent implementable components advanced, every CLI addition has an
  app control, no forbidden execution/network feature added; proofs name source and written tests;
  unimplemented/untested work stays explicit. The two remaining gaps are retained, not relabelled
  as finished. This pass is ready for integration, not a claim of end-to-end parity.

## Second-pass proposed grades (all tests are written, not run)

- C-optimize-006: f9 a8 p8 — `crates/sheaf-core/src/ocr.rs:278`,
  `local_ocr_is_explicit_for_latin_and_automatic_for_extra_scripts`: deliberate engine selection;
  release speed still needs the recorded benchmark.
- C-optimize-007: f8 a8 p8 — `crates/sheaf-core/src/ocr_local.rs:131`,
  `every_installed_language_pack_is_offered_but_orientation_data_is_not` and
  `mixed_scripts_use_all_selected_packs_and_refuse_missing_or_conflicting_packs`: pack coverage
  and all selected scripts; installation/accuracy remain unverified.
- C-optimize-009: f8 a7 p7 — `crates/sheaf-core/tests/ocr_review.rs:40`,
  `live_ocr_review_sees_unsaved_corrections_and_undo_and_refuses_missing_pages`: working-document
  edits and undo; reader panel/filter exist, rendering and large-document speed unverified.
- C-optimize-015: f8 a7 p7 — `crates/sheaf-core/tests/search_index.rs:29`,
  `catalog_manager_can_reopen_sources_and_rebuild_after_a_removal`: source management and
  nondestructive rebuild; GUI and large-catalog timings still need execution.
- C-sign-009: f8 a8 p8 — `crates/sheaf-app/src/reader_ui/signature_manager.rs:98`,
  `signature_edits_preserve_position_and_reject_a_stale_selection`: edits/order and stale guards;
  credential adapters and manager rendering still need live platform checks.
- C-sign-012: f8 a8 p8 — `crates/sheaf-core/tests/sign.rs:473`,
  `a_local_authority_timestamps_visible_and_invisible_certificate_signatures`: local adapter;
  server remains absent by instruction, so no closure.
- C-security-007: f0 a0 p0 — locked PdfCraft `crates/crypt/src/lib.rs:148` at d3a91c5 and
  `third_party/pdfcraft-cos/src/security.rs:50`: Standard-only reader, no recipient envelopes;
  no implementation or test claimed.
- C-security-008: f7 a7 p8 — `crates/sheaf-app/src/security_policies.rs:52`,
  `a_policy_library_survives_edits_import_export_and_deletion_without_passwords`: reusable
  settings and exchange; Acrobat security-settings/certificate-policy parity still absent.

## Second-pass touched files

- `ARCHITECTURE.md`
- `crates/sheaf-app/src/home.rs`
- `crates/sheaf-app/src/lib.rs`
- `crates/sheaf-app/src/main.rs`
- `crates/sheaf-app/src/reader_doc/mod.rs`
- `crates/sheaf-app/src/reader_ui/catalog.rs`
- `crates/sheaf-app/src/reader_ui/mod.rs`
- `crates/sheaf-app/src/reader_ui/model.rs`
- `crates/sheaf-app/src/reader_ui/ocr_review.rs`
- `crates/sheaf-app/src/reader_ui/security_policy.rs`
- `crates/sheaf-app/src/reader_ui/sign.rs`
- `crates/sheaf-app/src/reader_ui/signature_manager.rs`
- `crates/sheaf-app/src/run.rs`
- `crates/sheaf-app/src/saved_options.rs`
- `crates/sheaf-app/src/security_policies.rs`
- `crates/sheaf-app/ui/flow-types.slint`
- `crates/sheaf-app/ui/screens/catalog.slint`
- `crates/sheaf-app/ui/screens/ocr-review.slint`
- `crates/sheaf-app/ui/screens/options.slint`
- `crates/sheaf-app/ui/screens/reader-toolbar.slint`
- `crates/sheaf-app/ui/screens/reader.slint`
- `crates/sheaf-app/ui/screens/security-policy.slint`
- `crates/sheaf-app/ui/screens/signature-manager.slint`
- `crates/sheaf-app/ui/widgets/panel-button.slint`
- `crates/sheaf-cli/src/args.rs`
- `crates/sheaf-cli/src/main.rs`
- `crates/sheaf-core/src/ocr.rs`
- `crates/sheaf-core/src/ocr_local.rs`
- `crates/sheaf-core/src/ocr_review.rs`
- `crates/sheaf-core/src/search_index.rs`
- `crates/sheaf-core/tests/ocr_review.rs`
- `crates/sheaf-core/tests/search_index.rs`
- `dev/par-notes-sol-ocr-sign.md`

## Third pass — C-optimize-015

- C-optimize-015: phrase search uses an in-memory trigram index and cached normalized pages,
  including CJK substrings and punctuation; no source-byte hashing or whole-vocabulary scan
  per query. Source freshness uses size + modification time. Changed/missing PDFs are excluded
  individually and the manager reports their count; `validate` remains an explicit byte audit.
- Build accepts recursive local folders (symlink directories excluded). `index --update OLD`
  and rebuilding in the manager reuse unchanged pages, add/replace changed sources and purge
  omitted sources. Failures/cancellation keep the old catalog. No new dependencies.
- Tests written (not run): `indexed_substrings_and_incremental_reuse_do_not_require_a_readable_pdf_body`,
  `incremental_folder_catalog_reuses_unchanged_sources_and_purges_removed_ones`, updated
  `a_saved_catalog_finds_unicode_phrases_and_excludes_changed_sources` and real PDF integration.
- Left: no PDX interchange; GUI recursive folder picker absent (CLI accepts directories).
  Metadata-preserving edits require explicit `validate`; no release benchmark under rule 1.
  `not automatable: reopen/build a catalog, change/delete one source, search the others, then
  rebuild and inspect stale-source feedback — app execution forbidden to this worker`.
- Proposed grade: C-optimize-015: f8 a7 p8 — tests above; actual large-catalog timing unverified.

## Third pass — C-sign-009

- C-sign-009: new saves use Rust ChaCha20-Poly1305 authenticated local storage, without
  secret-tool/security/PowerShell writes or read-back subprocesses on the UI thread. Each
  save has a fresh nonce; the separate random per-profile key and ciphertext are owner-only
  on Unix, atomically replaced. No 64K-character Windows payload limit. Legacy system-vault
  profiles migrate after successful reads; unreadable legacy data is never overwritten.
- This supersedes the earlier mandatory-keyring design. The key is local and protects against
  other local accounts (Windows inherits profile ACLs); it does not claim Keychain-equivalent
  isolation from the signed-in account. Legacy migration may still invoke its old vault once
  at startup. Future saves and normal local loads need no outside program or unlock prompt.
- New dependency: workspace + sheaf-app `chacha20poly1305 = 0.10`, alloc only (pure Rust,
  orchestrator must fetch/update lockfile); existing workspace getrandom linked by sheaf-app.
- Tests written, not run: `local_signatures_survive_restart_without_external_tools_or_size_caps`
  (>64K drawn data, restart, ciphertext and Unix permissions),
  `damaged_or_missing_signature_keys_never_clear_a_saved_library`; previous migration guards.
  Smoke helpers now decrypt the local file using Python cryptography (smoke-only requirement).
- `not automatable: create/reorder/edit/delete/once-only marks, restart with no secret-tool,
  reopen and reuse on Linux/Windows/macOS — app/platform execution forbidden by rule 1`.
- Proposed grade: C-sign-009: f8 a8 p8 — persistence tests above; no live platform verification.

## Third pass — C-optimize-009

- C-optimize-009: recognition confidence now survives the built-in Latin and Tesseract paths,
  placement, PDF save/reopen and reader review. Each new OCR word has its own marked-content
  scope, preventing same-baseline words from merging into one correction target. Suspects now
  include words below 85% confidence, not just decoding artifacts. Existing external/older
  layers explicitly show unknown confidence. Corrections set the reviewed word's score to 1;
  undo restores it. The scan and neighboring words remain untouched.
- The reader's document thread caches review pages: targeted corrections only reread their
  page; arbitrary edits and undo/redo invalidate the snapshot. Accept/Previous/filter do not
  extract PDF text again. UI labels now say OCR suspects; the position shows confidence.
- Tests written, not run: `word_confidence_survives_save_and_corrections_leave_neighboring_words_and_scan_intact`,
  `local_ocr_preserves_low_confidence_and_refuses_invalid_scores`, updated CJK TSV test and
  reader review-state tests. No dependency. Public fields added: OCR `Word::confidence`,
  `PlacedWord::confidence`, `TextLine::ocr_confidence`, `ReviewLine::confidence`; new core
  `ocr_review::ReviewCache`. Callers constructing PlacedWord literals must set confidence.
- Left: old/external OCR layers may still group words in line runs and lack confidence; Sheaf
  does not invent it. New word grouping may affect OCR text-export line boundaries and must
  be checked by orchestrator integration tests. Large-file/GUI timing remains unmeasured.
  `not automatable: OCR a scan, filter suspects, correct one word, accept/next, undo, save/reopen;
  check confidence labels and highlight — running the app is forbidden by rule 1`.
- Proposed grade: C-optimize-009: f8 a8 p8 — confidence/neighbor/pixel/undo test above; no joint 9.

## Third pass — C-security-008

- C-security-008: policies now retain high/low print resolution, changes allowed (none,
  page assembly, forms, comments+forms, all checked actions), and AES-128/AES-256 compatibility.
  These choices are editable in the existing GUI and CLI (`--print-quality`, `--changes`,
  `--algorithm`), survive the local library/import/export, and reach actual PDF permission
  bits and crypt filters. Existing booleans can further restrict, never broaden, the choices.
  Restricted output gets a distinct generated owner password if none is supplied by CLI;
  the GUI requires a separate confirmed owner password. PDF headers meet the AES minimum.
- Old v1 policies default to their original settings; unknown algorithms/options are refused.
  No new dependency. New core public types `ProtectionOptions`, `PrintQuality`, `ChangeLevel`,
  `EncryptionAlgorithm`; function `security::protect_with_options`; public policy `validate`.
  Slint policy-save callback has three appended choice indexes; model gets policy-choices.
- Tests written (not run): `policy_print_quality_change_levels_and_aes_compatibility_reach_the_encrypted_pdf`,
  `every_changes_allowed_level_only_removes_the_requested_pdf_permissions`,
  `old_policies_keep_defaults_and_unknown_compatibility_is_refused`,
  `policy_choice_indexes_map_to_exact_print_change_and_algorithm_settings`,
  `policy_compatibility_choices_and_incremental_folder_index_are_accessible`, plus expanded
  policy-library round-trip test (ensures saving an imported advanced policy cannot reset it).
- Left: no certificate policies, stored-password option, Acrobat proprietary settings exchange
  or trust-settings exchange. Passwords are still asked per apply; AES only, no legacy RC4.
  `not automatable: create/import advanced policy, save/edit/export/reopen, inspect print quality,
  changes and compatibility on protected output — app and tests forbidden by rule 1`.
- Proposed grade: C-security-008: f8 a7 p8 — PDF-output/round-trip tests above, unrun.

## Third pass — C-optimize-006

- C-optimize-006: fixed the review's inference risk by annotating the progress closure's
  `usize` arguments. Added ignored release benchmark
  `release_ocr_mici_benchmark_reports_cold_load_and_warm_page_time`: the same supplied MICI
  scan, bundled engine only, prepacked/unpacked variants loaded sequentially, cold model load
  recorded, one warm-up and three timed runs each, warm median per page, word/placement
  equivalence, explicit failure above the recorded 11.75 s/page target (overridable for host).
- Orchestrator must set `SHEAF_OCR_BENCH_PDF` to the original MICI scan and explicitly run the
  ignored test in release mode on an idle benchmark host. This worker did NOT execute it.
  No compiler/test/app invoked and no performance result fabricated. No new dependency.
- Left: the actual 15.5 vs 11.75 s/page gap is still UNMEASURED after prepacking; a test harness
  is not a measured optimization or parity proof. Further inference work depends on that run.
- Proposed grade unchanged from counterpart: C-optimize-006: f9 a8 p7 — existing accuracy
  coverage and new benchmark protocol; the speed gap remains open.

## Third pass — C-sign-012

- C-sign-012 remains partial: no app/CLI TSA transport. The caller-supplied offline API alone
  is not user-facing certified timestamping, as the counterpart correctly points out. A
  network TSA client/server selection is explicitly excluded by this batch's rule 3; none added.
- Fixed the timestamp fixture: separate TSA identity/key, critical timeStamping-only EKU and
  digitalSignature key usage, with assertions that it is not the document signer's certificate.
  Test `a_local_authority_timestamps_visible_and_invisible_certificate_signatures` is still
  not run. This proves only the intended offline adapter contract once executed; no trust
  or certified-clock claim. Existing invalid-response/no-output test remains.
- Proposed grade: C-sign-012: f7 a8 p8 — offline API and corrected fixture; user-facing
  certified timestamping remains open pending authorized TSA transport. No dependencies.

## Third pass — C-optimize-007 (gap not closed)

- Rechecked `scripts/fetch-models.sh` and `third_party/pdfcraft-ocr/src/lib.rs`: shipped asset
  declarations are text detection, unaccented recognition and the accented Latin model/dictionary.
  There is no Japanese/Chinese/Korean model manifest or asset in this worktree (`vendor/models`
  is absent here). The current non-Latin route still requires installed Tesseract + traineddata.
- No extra CJK label, unverifiable model adapter or download was added to pretend that a stock
  user can recognize Japanese. A real native route needs a pinned licensed compatible model,
  dictionary/token decoding, packaging and real multilingual accuracy/geometry fixtures.
  Those assets and runtime validation are not available in this worker's permitted environment.
  Network features stay out; no automatic language downloads introduced.
- Existing selection/TSV/vertical tests remain; this pass additionally retained TSV confidence
  under C-optimize-009. No separate implementation or new dependency for stock language coverage.
- Proposed grade unchanged: C-optimize-007: f8 a8 p8 —
  `mixed_scripts_use_all_selected_packs_and_refuse_missing_or_conflicting_packs`,
  `local_ocr_keeps_cjk_words_lines_and_vertical_geometry`; these are parsing/selection proof,
  not proof of bundled Japanese recognition. The counterpart's stock-install gap stays open.

## Third pass — C-security-007 (not implemented)

- Rechecked the locked crypt reader: `crates/crypt/src/lib.rs:147` rejects non-Standard
  filters; the local COS EncryptDict route still has no recipient-envelope read/write contract.
  Existing signing CMS primitives do not provide PDF public-key encryption by themselves.
- Certificate encryption remains absent. Closing it requires CMS EnvelopedData recipient
  envelopes, Adobe.PubSec key derivation/crypt filters, recipient private-key unlocking and
  reader support together. An output-only path would create a PDF Sheaf cannot reopen; none
  added. This is a substantial unimplemented native Rust feature, not a network dependency.
  No new API, dependency, test or certificate-encryption UI claimed.
- Proposed grade: C-security-007: f0 a0 p0 — no implementation. The original gap stays open.

- C-sign-009 follow-up: independent mark storage is also loaded when prefs.json is absent or
  corrupt; resetting unrelated preferences cannot hide saved signatures. Added unrun tests
  `kept_marks_load_even_when_unrelated_preferences_are_missing` and
  `changing_ciphertext_is_detected_before_replacing_any_kept_marks` (AEAD tamper refusal,
  no empty-library overwrite). Grade remains f8 a8 p8 until platform/runtime checks.

- C-optimize-015 follow-up: CLI catalog searches also report changed/missing exclusions;
  stale sources no longer masquerade as a complete no-match response. Existing stale-source
  search tests cover exclusion; this only wires the already-tested count into CLI notes.

## Third-pass handoff / Polaroid — 27/10/2026

- Project: Sheaf, sol-ocr-sign, branch `par/sol-ocr-sign`, third pass. Status: handed off for
  orchestrator compilation/runtime review; no component declared jointly closed.
- Implemented this pass: catalog search/index updates, reliable encrypted local marks,
  retained OCR word confidence + correction-page cache, detailed password-policy choices.
  Narrow fixes: OCR closure typing + ignored release benchmark, distinct TSA test identity.
- Unclosed: bundled OCR speed (not measured), stock non-Latin models (not shipped), older OCR
  line granularity/unknown confidence, GUI/platform execution, certified timestamp transport
  (network excluded), certificate encryption (unimplemented), certificate/proprietary policies.
- Code/evidence commits `018b9b5` through `ac49aaa` pushed after each component/follow-up.
  Documentation-only receipts for C-optimize-007 and C-security-007 explicitly record no closure.
- Actually checked: aggregate `git diff 5353386..HEAD --check`, shell syntax with `bash -n` for
  both touched smoke scripts, source/API inspection at locked revisions, Slint static detector
  for the two edited sheets (both returned `[]`). No build, lint, tests, app, browser or codex run.
- Compilation handoff: fetch new pure-Rust workspace/sheaf-app dependency chacha20poly1305 0.10
  (alloc only) and update Cargo.lock; link the existing getrandom workspace entry to sheaf-app.
  `PlacedWord` literals now require `confidence`; all local occurrences were updated. Also
  new `Word::confidence`, `TextLine::ocr_confidence`, `ReviewLine::confidence`, `ReviewCache`,
  Catalog `update`/`stale_sources`/`collect_inputs`, advanced protection enums/options and
  `protect_with_options`. Policy-save Slint callback appends three integer choice indexes.
- Runtime handoff: new marks need no outside save program; retain both signatures.secure and
  signatures.key on backup/move. Legacy vault migration can still need the old platform tool.
  The smoke environment needs Python cryptography for ciphertext inspection, not the app.
  Tesseract + local traineddata remain optional for extra scripts. No network features added.
- Next: orchestrator merges, fetches dependency, compiles/lints and runs the written tests and
  GUI flows; run the ignored release OCR benchmark on the original MICI scan, then counterpart
  reviews. No timing, rendering, cross-platform or test success is asserted by this worker.
- Delivery control: all eight component gaps explicitly dispositioned; source/test evidence
  linked below/above. Remaining feature work is distinguished from forbidden runtime checks.
  No external authorization requested or inferred. This is source delivery, not proven parity.

### Third-pass changed files (complete)

- `ARCHITECTURE.md`
- `Cargo.toml`
- `crates/sheaf-app/Cargo.toml`
- `crates/sheaf-app/src/main.rs`
- `crates/sheaf-app/src/prefs.rs`
- `crates/sheaf-app/src/reader_doc/mod.rs`
- `crates/sheaf-app/src/reader_ui/catalog.rs`
- `crates/sheaf-app/src/reader_ui/model.rs`
- `crates/sheaf-app/src/reader_ui/ocr_review.rs`
- `crates/sheaf-app/src/reader_ui/security_policy.rs`
- `crates/sheaf-app/src/security_policies.rs`
- `crates/sheaf-app/src/signature_store.rs`
- `crates/sheaf-app/ui/flow-types.slint`
- `crates/sheaf-app/ui/screens/ocr-review.slint`
- `crates/sheaf-app/ui/screens/security-policy.slint`
- `crates/sheaf-cli/src/args.rs`
- `crates/sheaf-cli/src/main.rs`
- `crates/sheaf-core/src/ocr.rs`
- `crates/sheaf-core/src/ocr_local.rs`
- `crates/sheaf-core/src/ocr_review.rs`
- `crates/sheaf-core/src/search_index.rs`
- `crates/sheaf-core/src/security.rs`
- `crates/sheaf-core/src/security_policy.rs`
- `crates/sheaf-core/tests/ocr.rs`
- `crates/sheaf-core/tests/ocr_review.rs`
- `crates/sheaf-core/tests/search_index.rs`
- `crates/sheaf-core/tests/security.rs`
- `crates/sheaf-core/tests/sign.rs`
- `dev/par-notes-sol-ocr-sign.md`
- `dev/scripts/smoke/1-reader.sh`
- `dev/scripts/smoke/3-flows.sh`
- `third_party/pdfcraft-edit/src/text.rs`
- `third_party/pdfcraft-ocr/src/lib.rs`

### Third-pass proposed grades (tests written, not executed)

C-optimize-006: f9 a8 p7 — `tests/ocr.rs:260` release benchmark protocol; speed gap unmeasured.
C-optimize-007: f8 a8 p8 — `ocr_local.rs` mixed-script/TSV tests; non-Latin packs still external.
C-optimize-009: f8 a8 p8 — `tests/ocr_review.rs:60` confidence/save/neighbor/pixels/undo contract.
C-optimize-015: f8 a7 p8 — `search_index.rs:244`, `tests/search_index.rs:48` memory search/reuse/purge.
C-sign-009: f8 a8 p8 — `signature_store.rs:247` restart/>64K/no-tool contract; platform checks pending.
C-sign-012: f7 a8 p8 — `tests/sign.rs:476` distinct TSA fixture; no user-facing TSA transport.
C-security-007: f0 a0 p0 — no implementation; Standard-only reader still blocks PubSec reopening.
C-security-008: f8 a7 p8 — `tests/security.rs:222` exact PDF permissions/AES compatibility contract.

## Fourth pass — C-optimize-015

- C-optimize-015: Acrobat's Advanced Search options now reach catalog searches in the reader
  and CLI: whole words, match case, all/any words, bounded Boolean AND/OR/NOT with grouping and
  quoted phrases, unordered proximity (10 intervening words in the UI, configurable in CLI),
  and explicitly English Snowball stemming. Inverted word/stem/trigram postings prune page
  candidates; Boolean negation never complements an approximate posting set. The manager's
  Add folder includes subfolders through the existing recursive incremental builder.
- Tests written: `catalog_advanced_queries_cover_case_words_boolean_stems_and_proximity`,
  `catalog_refuses_invalid_or_excessive_boolean_queries_and_conflicting_options`,
  `catalog_search_controls_keep_mode_and_independent_options`.
- New small pure-Rust offline dependency: `rust-stemmers = 1.2` in workspace and sheaf-core.
  Orchestrator must fetch/update Cargo.lock. New public items `SearchMode`, `SearchOptions`,
  `Catalog::search_with_options`. Old catalog files and `Catalog::search` retain phrase defaults.
- `not automatable: Catalog → Add folder, build, run each search mode and checkbox, malformed
  Boolean query, open hit and cancel; check narrow/wide layouts — rule 1 forbids app execution`.
- Left: PDX interchange and non-English stemming; render, memory/time and compilation unverified.
  No network code. Proposed grade f9 a8 p8 (static evidence only; no joint closure claim).

## Fourth pass — C-security-008

- C-security-008: Acrobat's Save passwords with the policy is available beside Apply. After
  a protected copy succeeds, the optional local credentials are encrypted/authenticated in
  `policy-passwords.secure` using the profile's existing random `signatures.key`. Neither
  settings JSON nor exports receive passwords. Selecting the saved policy and leaving fields
  empty reuses its passwords; Forget saved passwords and deleting the policy remove them.
  Credential binding includes the entire typed policy, so changed settings require new values.
  A damaged vault/missing key refuses replacement; writes across document tabs are serialized.
- Tests written: `remembered_policy_passwords_survive_restart_but_never_export_or_follow_changed_settings`,
  `corrupt_policy_passwords_are_never_replaced_and_forgetting_preserves_other_policies`,
  `missing_policy_password_key_refuses_new_saves_without_destroying_existing_credentials`,
  `a_saved_policy_applies_without_retyping_passwords_and_edits_require_new_credentials`.
- No new dependency. New public app items `security_policies::{Passwords, saved_passwords,
  remember_passwords, forget_passwords}`. Policy Apply bridge gains one bool; ReaderTab gains
  policy-remembered. Three existing signature-store file/key helpers become crate-visible;
  signature persistence behaviour is unchanged. Unix temp files remain owner-only.
- `not automatable: save a policy, apply with Save passwords checked, restart, select and apply
  with blank fields, forget/delete, export and inspect no passwords, damage the key and retry —
  rule 1 prohibits this worker's app execution; orchestrator must also verify the panel fit`.
- Left: certificate policies depend on C-security-007; Acrobat settings-file interoperability,
  runtime and visual verification remain. Network features remain out. Proposed f9 a8 p8 for
  password policies; f8 a8 p8 overall while certificate policies are unavailable.

## Fourth pass — C-security-007 (partial implementation)

- C-security-007: `sheaf encrypt-certificates FILE --recipient alice.cer bob.pem --scope
  all|except-metadata|attachments -o encrypted.pdf` now writes Adobe.PubSec S5 AES-256 PDFs
  with CMS RSA key-transport envelopes. Include the sender as a recipient to retain access.
  `unlock-certificate FILE --identity alice.p12 --id-password … -o copy.pdf` uses the authorized
  private key in Rust and produces a copy the existing Sheaf reader can open. No OpenSSL,
  Ghostscript, keyring command, or network is needed at runtime. X.509 DER/PEM imports reuse
  the existing parser; signing-only, non-RSA, short-key, expired and unknown-critical-extension
  certificates are refused. Full saves avoid obsolete object streams and never overwrite.
- Tests written: `certificate_encryption_reopens_for_each_recipient_and_preserves_every_content_scope`
  (two recipients, outsider denial/no output, all three scopes, text/metadata/attachment roundtrip,
  engine reopen) and `certificate_encryption_refuses_wrong_usage_invalid_inputs_and_does_not_overwrite`.
  Test identities and real signed X.509 encryption certificates are made in Rust, with no
  subprocess fixture dependency. CLI test `offline_certificate_commands_require_recipients_identity_and_explicit_scope`.
- New public core module `certificate_security::{Scope, encrypt, unlock}`. Existing transitive
  pure-Rust `rsa = 0.10.0-rc.18` (encoding/std) and `pdfcraft-cos` are now direct dependencies
  in workspace/sheaf-core; getrandom gains `sys_rng`. No new native dependency or new version
  of an existing package. Orchestrator updates the dependency edges in Cargo.lock.
- Format reference (ideas translated, no copied implementation): pyHanko's public-key handler,
  `https://github.com/MatthiasValvekens/pyHanko/blob/master/pkgs/pyhanko/src/pyhanko/pdf_utils/crypt/pubkey.py`
  and permissions.py: SHA-256(seed + encoded envelopes + optional metadata marker), big-endian
  permission word, one CMS string for an attachment-only nondefault crypt filter. CMS is
  encoded through the existing DER writer and AES uses lopdf's existing Rust crypt filter.
- Left: CLI only, all recipients have full permissions, no certificate policy library or direct
  private-key prompt in the reader. Opening an authorized copy is explicit, not transparent.
  Legacy S3/S4/RC4/AES-128, EC key agreement, external encrypted object streams, named stream
  Crypt overrides and nondefault filter names are refused clearly. Independent Acrobat/
  pyHanko interoperability, visual fit, compilation and runtime tests are still unverified.
  Proposed f5 a2 p7; this moves the absent component forward, without a joint 9 claim.

## Fourth pass — C-optimize-006 (partial)

- C-optimize-006: the built-in Rust path now prepacks detection weights as well as recognition
  weights, skips loading the unused English recognizer whenever Paddle's model is present,
  and allows a complete detection/Paddle install without that unused file. Line preprocessing
  caches horizontal bilinear coordinates once per line, fills contiguous BGR channel planes
  directly and validates malformed buffers before accessing pixels. Scaling arithmetic and
  recognition geometry are preserved; this is less work, not a claimed measured speedup.
- Tests written: `paddle_preprocessing_matches_reference_bilinear_pixels_and_rejects_invalid_images`
  compares every float bit with the prior preprocessing at borders and fractional crops;
  `paddle_installations_do_not_require_the_unused_english_recognition_file` checks complete/
  incomplete data; `prepacked_recognition_keeps_the_words_and_their_placement` now also checks
  real recognition without an English model. New public OCR items `RecognitionInput` and
  `recognition_input`, reexported by engine::ocr for host integration tests. No new dependency.
- Left: the original release MICI benchmark and comparison with 11.75 s/page remain **not run**.
  Rule 1 expressly prohibits this worker from running it. Only an idle host running the ignored
  `release_ocr_mici_benchmark_reports_cold_load_and_warm_page_time` can close the speed gap.
  No model assets were present in this worktree during source inspection. Proposed f9 a8 p7.

- Catalog static follow-up: normalization-only empty queries are rejected before matching;
  combining marks remain part of whole words instead of being treated as punctuation.
  The invalid-query regression covers a dehyphenated empty input.

- Certificate static follow-up: CMS recipient serials must be INTEGERs. The usage regression
  now builds an actually signed signing-only certificate, rather than mutating parsed fields.

- Catalog CLI proximity uses u32, so the existing generated MCP schema advertises an integer
  (rather than the string fallback for usize). The CLI proximity parsing regression covers it.

## Fourth pass — unchanged accepted components and remaining language gap

- C-optimize-009: unchanged. Counterpart `review-sol-ocr-sign-2.txt` accepted 9 after the
  orchestrator's round-27 build/unit pass. Proof remains
  `word_confidence_survives_save_and_corrections_leave_neighboring_words_and_scan_intact`.
- C-sign-009: behaviour unchanged; three file/key helpers are crate-visible for policy secrets.
  Counterpart accepted 9. Proof `local_signatures_survive_restart_without_external_tools_or_size_caps`.
- C-sign-012: unchanged. Counterpart accepted the offline feature at 9; TSA transport and
  online revocation stay outside this batch, not silently counted as implemented. Proof
  `a_local_authority_timestamps_visible_and_invisible_certificate_signatures`.
- C-optimize-007: **gap not closed, no language coverage added in this pass**. A stock install
  still recognizes Latin with Sheaf's own Rust inference; other scripts require installed
  Tesseract and matching traineddata. Source inspection confirmed only the Latin model and
  dictionary in `scripts/fetch-models.sh`; no bundled native non-Latin weights/dictionaries or
  matching accuracy fixtures are available here. Wrapping the same external program again
  would not close this gap. Native multilingual coverage needs compatible trained assets plus
  real scan/geometry/accuracy validation; no language is advertised without an engine able to
  read it, and no downloader or online feature was added. Existing tests
  `every_installed_language_pack_is_offered_but_orientation_data_is_not` and
  `mixed_scripts_use_all_selected_packs_and_refuse_missing_or_conflicting_packs` remain the proof
  of the explicit optional local fallback. Proposed grade remains f8 a8 p8.

## Fourth-pass Polaroid / orchestrator report

- Project: Sheaf / sol-ocr-sign / fourth pass. Status: source delivery complete, integration
  pending. Branch `par/sol-ocr-sign`; worktree `/tmp/f5/w-sol-ocr-sign` resolves to
  `/home/nassim/f5/w-sol-ocr-sign`. This pass started from `ce2a6dc` (the already merged round-27
  work), not the older `1262028` remote tip. Only this pass's commits are listed below.
- Commits pushed after each component/follow-up: `bb7e6e9` catalog, `e942def` remembered policy
  passwords, `4e808bf` certificate encryption, `dc22ee8` OCR work reduction, `174f8f4` / `5369224`
  catalog Unicode/empty-query regressions, `ed508cc` certificate parsing/real key-usage fixture.
  A final CLI-schema/documentation commit follows. No unrelated worker changes authored.
- Checks actually performed: source/primary-reference inspection, `git diff --check` for the
  entire pass, live branch/remote status reads; Impeccable's static detector returned `[]` for
  the two changed Slint sheets. **No cargo, rustc, clippy, tests, app, Chrome, Xvfb or codex ran.**
  The detector is not a render, and written tests are not passed tests.
- Compile handoff: fetch `rust-stemmers 1.2`; link existing transitive rsa 0.10.0-rc.18 with
  encoding/std and pdfcraft-cos directly into sheaf-core; enable getrandom sys_rng. Update
  Cargo.lock in the orchestrator. No native package/version added. New public items and the
  one-bool policy Apply bridge change are recorded in the component entries above.
- Not done: non-Latin native assets, release speed measurement, certificate UI/direct-reader
  credential plumbing/per-recipient restrictions/certificate policies, legacy recipient modes,
  independent encrypted-PDF interoperability, cross-platform and rendered UI verification.
- Next: orchestrator integrates and runs its required full compile/lint/unit/GUI checks, the
  new certificate round-trip and policy-vault tests, and the ignored release MICI benchmark.
  Then Opus rereviews the changed components. No board closure or fresh joint 9 claimed.
- Control: mandate covered yes (every gap dispositioned); evidence yes (source/tests/commits);
  dependencies explicit yes; permitted source work delivered yes. Runtime results remain
  unknown under the explicit execution ban. Certificate work is partial implementation, not
  an externally blocked claim that the absent UI has already been delivered.

### Fourth-pass proposed grades

C-optimize-006: f9 a8 p7 — `tests/engine_patches.rs:536` pixel equivalence and
`tests/ocr.rs:264` release protocol; measured speed still outstanding.
C-optimize-007: f8 a8 p8 — `src/ocr_local.rs:133` installed-pack filtering; stock native
non-Latin coverage unchanged.
C-optimize-009: f9 a9 p9 — `tests/ocr_review.rs:60` confidence/correction/scan preservation;
previous counterpart AGREE 9, no new behaviour.
C-optimize-015: f8 a8 p8 — `src/search_index.rs:294` advanced query cases and reload;
folder selection `reader_ui/catalog.rs:101`; fresh build/UI/performance verification pending.
C-sign-009: f9 a9 p9 — `src/signature_store.rs:247` restart/no-program/>64K;
previous counterpart AGREE 9, storage behaviour unchanged.
C-sign-012: f9 a9 p9 — `tests/sign.rs:476` independent offline TSA identity;
previous counterpart AGREE 9, network excluded.
C-security-007: f5 a2 p7 — `tests/certificate_security.rs:42` two recipients/three scopes/
outsider refusal/engine reopen; CLI only and interoperability unverified.
C-security-008: f8 a8 p8 — `security_policies/passwords.rs:99`,
`reader_ui/security_policy.rs:167` local encrypted remember/reuse/forget;
certificate policies and fresh runtime/UI verification remain.

### Fourth-pass files touched

- `Cargo.toml`
- `crates/sheaf-app/src/main.rs`
- `crates/sheaf-app/src/reader_ui/catalog.rs`
- `crates/sheaf-app/src/reader_ui/model.rs`
- `crates/sheaf-app/src/reader_ui/security_policy.rs`
- `crates/sheaf-app/src/security_policies.rs`
- `crates/sheaf-app/src/security_policies/passwords.rs`
- `crates/sheaf-app/src/signature_store.rs`
- `crates/sheaf-app/ui/flow-types.slint`
- `crates/sheaf-app/ui/screens/catalog.slint`
- `crates/sheaf-app/ui/screens/security-policy.slint`
- `crates/sheaf-cli/src/args.rs`
- `crates/sheaf-cli/src/main.rs`
- `crates/sheaf-core/Cargo.toml`
- `crates/sheaf-core/src/certificate_security.rs`
- `crates/sheaf-core/src/certificate_security/envelope.rs`
- `crates/sheaf-core/src/certificate_security/objects.rs`
- `crates/sheaf-core/src/lib.rs`
- `crates/sheaf-core/src/search_index.rs`
- `crates/sheaf-core/src/search_index/query.rs`
- `crates/sheaf-core/tests/certificate_security.rs`
- `crates/sheaf-core/tests/engine_patches.rs`
- `crates/sheaf-core/tests/ocr.rs`
- `dev/par-notes-sol-ocr-sign.md`
- `third_party/pdfcraft-engine/src/ocr.rs`
- `third_party/pdfcraft-ocr/src/latin.rs`
- `third_party/pdfcraft-ocr/src/lib.rs`

## Batch sol-batch


## Tatouages

- Worktree `/tmp/f5/w-sol-batch`, branch `par/sol-batch`; only this batch's files.
- Brief rule 1: no cargo, rustc, clippy, tests, app, browser or codex execution. The orchestrator
  owns compilation, the suite and visual verification. No score of 9 is claimed here.
- Pass 3 input: `/tmp/f5/par/review-sol-batch.txt`, Opus DISAGREE for all nine. Close evidenced gaps without claiming unrun parity.

## Polaroid

Sheaf sol-batch pass 3, 27/10/2026, active hand-off on `par/sol-batch` in
`/tmp/f5/w-sol-batch`. Mandate: close Opus's evidenced gaps in
`/tmp/f5/par/review-sol-batch.txt`, valuable native work first, one pushed commit per component.
All nine now have a third implementation pass and pushed component commits, plus narrowly scoped
static follow-ups. None is untouched; none is declared at joint parity. The Pass 3 entries and
final grades below supersede earlier baselines and Left notes for the same behavior.

Tatouages remain: no cargo/rustc/clippy/test/app/browser/codex execution; no network features;
no outside conversion processes added. Pure Rust tools and explicit limitations are used.
Observed proof is source inspection, git diff --check, bash -n and the static UI detector only.
Tests are written, not executed; layouts are unrendered; runtime/performance remain unverified.
The app zip dependency's missing lock entry is now corrected manually against already locked
zip 4.6.1 packages; no new dependency was added in this pass.

Next action belongs to the orchestrator: merge, compile, run the suite and new tests, render the
four surfaces at minimum size/light/dark, then request opus-platform's read-only rereview.
Send compile errors or remaining DISAGREE evidence back here; no component closes until joint 9
on the green merged tree. Code committed/pushed is the hand-off result, not proof that it runs.

## Components

- C-batch-002: Home ▸ Actions creates and edits ordered steps, text arguments, instructions and
  pauses (Acrobat's Action Wizard), saved independently of suggested actions. Tests
  `actions::tests::action_round_trip_keeps_order_arguments_instructions_and_prompts`,
  `actions::tests::unreadable_saved_actions_are_not_overwritten`.
- C-batch-003: Run on PDFs… executes each saved step in order, off the UI thread, with file/step
  progress, instruction pauses, Stop, per-file errors and a count of successful outputs. Copy,
  rename (edit Name then Save), Delete and duplicate-safe names manage the saved list (Acrobat's
  Manage Actions). A script or edit failure stops that file instead of claiming success. Tests
  `actions::tests::saved_action_runs_in_order_and_never_replaces_originals_or_outputs`,
  `actions::tests::cancelled_action_does_not_publish_a_file`,
  `an_action_stops_on_a_script_error_before_later_steps` in core `engine_patches.rs`.
  Left: password entry for encrypted inputs; per-file output links; editing prompted settings
  while paused (settings are edited before Run). Engine cancellation is between steps.
- C-batch-004: import a JSON file or paste its contents; export the action as portable JSON
  including arguments, prompts and instructions (Stirling's Import automation / Export).
  Invalid versions/tools/empty actions are refused before touching saved actions; name conflicts
  get a fresh name. Tests `actions::tests::import_rejects_unknown_tools_versions_and_empty_actions`,
  `actions::tests::action_round_trip_keeps_order_arguments_instructions_and_prompts`.
  Left: drag-and-drop and translating Stirling/Acrobat's proprietary action formats; currently
  the portable format is Sheaf JSON only, rather than claiming silent compatibility.
- C-batch-005: five suggested actions can be browsed and copied: Prepare for Distribution,
  Optimize Scanned Documents, Add Page Numbers, Mark as Confidential and Sanitize Documents
  (Acrobat's Suggested actions, Stirling's presets). Saving a suggestion creates a user copy;
  the suggestion stays unchanged. Test
  `actions::tests::suggested_actions_are_valid_offline_templates_and_survive_copy_edits`.
  Online action discovery is deliberately excluded by brief rule 3.
- C-batch-006: New custom tool builds a named command set; edit, reorder, copy, rename, delete,
  import/export it, or launch any configured command independently with its Run button
  (Acrobat's Custom Tools). Test `actions::tests::suggested_copies_and_custom_tools_are_independent`
  covers copy independence, custom kind persistence, edits and deletion.
  Left: pinning custom sets to the reader toolbar; the picker currently offers the twelve
  Action Wizard commands exposed by the engine, not every toolbox command.
- C-batch-008: Watch folder… automatically runs the draft action on stable PDFs, once per
  size/mtime revision, into a separate folder (Stirling's Folder Scanning). Optional title-based
  naming strips path characters and reserved device names. Stop ends the watch. Partial uploads,
  subfolders and symlinks are skipped; failures stay visible and retry on file changes. Tests
  `watch_folder::tests::watched_pdfs_wait_until_stable_run_once_and_retry_after_change`,
  `watch_folder::tests::watched_folders_cannot_feed_their_outputs_back_to_the_input`.
  Left: watches run while this app process stays open, without restart recovery, recursive
  folders or a background daemon. Prompted actions are refused for unattended watching.
- C-organize-024: Actions ▸ Create portfolio… bundles selected files verbatim into a PDF with a
  readable cover, `/Collection` details view, schema and attachment-pane fallback (Acrobat's PDF
  Portfolio; Stirling's cover). Duplicate basenames keep both originals, and an existing output
  is never replaced. The portfolio opens in the reader, where Attachments can open/save its items.
  Tests `a_portfolio_keeps_original_files_and_duplicate_basenames_instead_of_merging`,
  `a_missing_portfolio_input_does_not_publish_a_partial_bundle` in core `tests/portfolio.rs`.
  Left: a dedicated portfolio visual browser; Sheaf currently exposes the files in Attachments.
- C-files-008: Home ▸ Library keeps private file copies, deletes them without touching originals,
  exports current/older versions, unzips archives into library items and restores an older version
  as a new one (Stirling's local file manager). Open uses a working copy; the reader's Save hook
  records a new immutable version. New version… also accepts external edits. ZIP preflight
  rejects escaping paths/symlinks and bounds expansion; partial imports report successes/errors.
  Tests `library::tests::library_versions_keep_private_copies_and_restore_without_losing_history`,
  `library::tests::unzip_imports_nested_files_and_rejects_escaping_paths_before_any_import`.
  Left: ZIP folders are flattened into file entries; an app already showing Library needs Refresh
  after reader saves; working-copy cleanup/recovery remain to polish. Deleting requires an explicit in-app confirmation.
  New dependency: workspace/app `zip = 4.6.1`, default features off, `deflate` only (pure Rust,
  version already present in Cargo.lock). Orchestrator must update the lockfile's app dependency list.
- C-platform-005: reader ▸ More ▸ More tools ▸ JavaScript console… edits and runs scripts against
  the current unsaved document (Acrobat's JavaScript Console). Console output, completion values,
  alerts and errors appear; field changes refresh the reader and undo as one step. Execution is
  on the document thread with the engine's existing loop/recursion bounds. Test
  `reader_doc::tests::forms_and_files::javascript_console_prints_errors_and_changes_fields_as_one_undo_step`.
  Left: viewer requests (print, navigate, Save As, focus, external URL/submit) are listed in the
  console rather than dispatched; document-script management is not added. The ⌘J shortcut opens the console.

## GUI checks for the orchestrator

- Smoke 250: active-document JavaScript console opens with Ctrl+J and runs into its output pane.
- Smoke 251: a suggestion exports, then imports as an independent named action (canonical JSON
  produced by the UI's serializer, no invented fixture).
- Smoke 252: Library adds a private copy and exports it.
- Smoke 253: Create portfolio keeps the original and writes a PDF Collection.
- These steps are written, not run. The orchestrator must check their coordinates/OCR against
  the compiled app; all new screens inherit Sheaf's existing theme and native widget controls.
- not automatable: watched-folder chooser flow with the existing debug dialog hook — it supplies
  the same answer synchronously to both folder panels; watching requires two distinct folders.
  The polling rules have deterministic unit coverage; manually choose separate folders, drop a
  PDF, confirm one output, change the input, confirm a second output, then Stop.
- not automatable: dark/light appearance, keyboard traversal, resizing and pause-dialog focus —
  this worker is explicitly forbidden to run/render the GUI; orchestrator visual verification
  remains required, rather than a claimed visual pass.
- Additional watched-folder integration test:
  `watch_folder::tests::watched_action_uses_the_document_title_and_keeps_existing_outputs` runs
  a real action through the poller, checks title naming and suffixing, and checks no repeat run.

## Static close-out

- `git diff --check` and `bash -n dev/scripts/smoke/3-flows.sh` passed. No compile, lint,
  test execution, app launch, browser or codex process was run.
- No new engine public API was needed. Existing engine `actions::run_on` now propagates failed
  edits/scripts; all new model APIs are in app `actions`, `watch_folder`, `library`, and core
  `portfolio::create`. New Slint globals are exported from main.slint and wired in main.rs.
- References inspected read-only: Acrobat `aapp/Actions_R_RHP.aapp`, JavaScript/action strings;
  Stirling `AutomationImportModal.tsx`, `FileGrid.tsx`, `PortfolioService.java`; existing
  PdfCraft `actions.rs`, `js.rs`, `doc_attachments.rs`, the create/JS APIs and installed Slint
  TextEdit widget declarations. No competitor code was copied verbatim.
- Merge seams: small blocks in app `main.rs`, `lib.rs`, `ui/main.slint`, `reader_doc/mod.rs`,
  `reader_ui/mod.rs`, `reader.slint`, `reader-toolbar.slint`, core `lib.rs`,
  `tests/engine_patches.rs`, the tail of smoke `3-flows.sh`, workspace/app Cargo manifests.
- All nine components received implementation and tests. They are **implemented, awaiting
  compilation and UI verification**, not jointly graded parity. Remaining scope is listed per
  component above; no component was left completely untouched.

## Pass 2 · active

- C-batch-002: Tools and reader ▸ More tools now reach Guided actions. Duplicate step keeps
  configuration, instructions and prompts beside the original; the editor enforces 100 steps
  before adding, and freezes all configuration fields during execution. Acrobat reference:
  `aapp/Actions_R_RHP.aapp` CreateWorkflowRdr / ManageWorkflowsRdr. Test
  `duplicate_step_keeps_configuration_and_enforces_the_step_limit` (written, not run).
  not automatable: open Guided actions from Tools and the reader; duplicate, reorder and edit a
  prompted step — brief rule 1 forbids this worker from launching the app.
- Reference lookup correction: archive paths start at `aapp/` and `frontend/`, without the app-name
  prefix given in the board. Listing those archives established the paths before reading them.

- C-batch-003: prompted text/JavaScript can be changed for the current file in the pause panel;
  passwords can be entered in a masked field before Run and are cleared from the UI, never
  serialized. Each completed output has Open and Show in folder. The runner now keeps one
  engine session for the entire sequence instead of writing/reopening once per step, preserving
  password authentication between steps. Private batch undo revisions are discarded after each
  step to avoid retaining up to 100 full-document histories. New public engine API `actions::run_guided`;
  app `actions::run_file_configured`. Existing `run_on` / `run_file` remain compatible wrappers.
  Tests `prompted_settings_affect_this_run_without_changing_the_saved_action`,
  `a_guided_action_opens_encrypted_input_and_keeps_its_protection`; cancellation regression now
  uses a valid canonical engine-created PDF. Tests written, not run. No dependency.
  Left: different passwords for individual inputs require separate runs; cancellation remains
  between steps. not automatable: enter a password, edit the prompted watermark text, open each
  completed output — app launch prohibited by brief rule 1.

- C-batch-006: the reader toolbar and Tools page now open a custom command palette, showing every
  saved custom set's commands with Run on PDFs and Manage custom tools. The picker expands from
  12 to 20 commands: clockwise/counterclockwise/half-turn rotation, remove images/links, and
  Author/Subject/Keywords join Title. Test `custom_commands_round_trip_and_apply_rotation_and_metadata`
  in core engine_patches exercises all command IDs plus an executed multi-command set (not run).
  Left: commands act on chosen files and publish copies; applying a set to the current unsaved
  reader state and arbitrary toolbox tools are still absent. not automatable: create two custom
  sets and launch one command from the reader's palette — app execution prohibited by rule 1.

- C-batch-004: Actions accepts dropped JSON files as well as choose/paste; native Stirling
  Automate and Folder Scanning imports translate equivalent rotation and form flattening,
  with a Stirling export button for the same subset. Unknown tools/settings and
  rasterization are refused by step number before saving, never silently discarded. Source:
  Stirling `utils/automationConverter.ts`, `useRotateOperation.ts`, `useFlattenOperation.ts`,
  `useChangeMetadataOperation.ts`. Tests `stirling_export_import_preserves_equivalent_settings_and_order`,
  `stirling_import_refuses_unknown_tools_settings_and_rasterization` (written, not run).
  Left: unsupported Stirling tools/settings, Acrobat sequ files. No dependency.
  not automatable: drop the exported JSON onto Actions and verify the independent named copy —
  actual drop delivery requires running the app (brief rule 1).

- C-batch-008: a watch now persists its action, folders, naming options and handled size/time
  revisions in `folder-watch.json`. Resume saved watch and Forget watch live in Actions; recursive
  watching is optional, never follows symlinks, and is bounded to 10,000 folders / 100,000 PDFs.
  Completed polls checkpoint; a checkpoint failure stops visibly instead of repeatedly processing.
  Test `a_saved_recursive_watch_resumes_without_repeating_handled_revisions` covers default exclusion,
  recursion, save/load, no repeat, changed-file retry and Forget (written, not run).
  Left: one watch at a time, explicit Resume after reopening, no background daemon. A crash after
  publishing but before checkpoint can produce another uniquely named output on resume.
  not automatable: configure, Stop, restart, Resume, drop nested PDFs, Forget — app launch forbidden.

- C-files-008: Library is reachable from Tools and reader ▸ More tools; filters by name/archive
  folder, preserves ZIP folders in entry metadata, refreshes versions after reader Save and on
  opening the page. Working copies from interruptions are listed with Open working copy; normal
  tab closure removes only its own checkout. Test `checkout_recovery_and_release_keep_versions_and_other_checkouts`
  covers recovery discovery, saved versions and cleanup isolation; ZIP regression checks folder
  retention. Tests written, not run. Older entry.json files load with an empty folder value.
  Left: no nested folder tree UI; ZIP folders appear in file details and search. Working copies
  from Save As or whole-process exit remain recoverable, without automatic age-based deletion.
  not automatable: save a library PDF, return to Library, observe the new version and working copy;
  close the tab and check its working folder is gone — app launch prohibited by rule 1.

- C-organize-024: Create portfolio now opens a Slint composition panel with a cover title, Add,
  Remove and Up/Down before saving. Tools and reader ▸ More tools reach it directly. A second
  reader entry opens an embedded-file browser with Open/Save copy. Collection items carry their
  chosen order independently of the PDF name tree's required lexical sort. Test
  `a_portfolio_keeps_the_chosen_order_separate_from_the_sorted_name_tree`; smoke 253 updated for
  composition (written, not run). New modules portfolio_ui, reader_ui/portfolio and portfolio.slint.
  Left: embedded previews, folders and portfolio layout themes. The browser reuses the attachment
  safeguards for non-PDF launch. not automatable: compose mixed files, reorder, name the cover,
  create, browse and open/save the originals — app execution forbidden by rule 1.

- C-platform-005: console `this.pageNum` now starts at the visible reader page; local page/focus,
  Print and Save As requests reach the reader (dialogs open for user choices; no silent print).
  Console ▸ Document scripts lists/adds/edits/deletes named scripts through undoable engine edits.
  Network URL/Submit requests remain listed without execution per brief rule 3. Beep is a toast.
  Tests `javascript_console_returns_local_viewer_requests_and_named_document_scripts`,
  `console_page_context_matches_the_reader_and_clamps_out_of_range_pages` (written, not run).
  New public engine `Session::run_javascript_at`; old method remains a compatible wrapper.
  Left: debugging/breakpoints, a sound for Beep, simultaneous dialog requests (first one opens),
  field focus on checkbox/radio (text and choice fields supported).
  not automatable: execute page navigation/focus, Print, Save As; manage a document script and Undo
  — UI launch forbidden by brief rule 1.

- C-batch-005: Actions now separates Suggested, Saved actions and Custom tools through a list
  filter, retaining stable source selection through filtering. Suggestions explicitly explain
  Save a copy. Unicode names at the 256-byte limit can be copied without invalidating the saved
  action. Test `copied_names_remain_valid_at_the_unicode_name_limit` (written, not run). The five
  first-pass offline templates remain immutable; online discovery stays excluded by brief rule 3.
  not automatable: filter to Suggested, view a preset, save a renamed copy, find it under Saved
  and confirm the original preset remains — app execution prohibited by rule 1.

- C-batch-004 source correction: backend `MetadataController.java:174–184` replaces **all**
  standard metadata even when fields are omitted. The initial frontend-only mapping would have
  preserved omitted fields, which is not equivalent. Metadata import/export is now explicitly
  refused; its rejection joins the regression test. Rotation and form flattening remain exact
  supported translations. No unsupported operation is silently converted or dropped.

- C-batch-003 final boundary: cancellation is checked again after the last engine step, before
  publishing the unique output. Test `stopping_during_the_last_step_does_not_publish_its_output`
  (written, not run). The configured runner takes an explicit `publish` predicate; the GUI's
  Stop token supplies it, while the compatible unattended wrapper always permits publication.

## Pass 2 static hand-off

- Fixed Stop remains visible outside the editor's scroll; step controls split into two rows at
  the minimum width. Library has a visible search prompt and separates its long Delete action.
  Portfolio composition says Save portfolio to distinguish its button from the panel heading.
  Smoke 251–253 selectors were adjusted to the new header/panel arrangement, not executed.
- New dependencies this pass: none. First-pass app `zip 4.6.1` (`deflate`, no default features)
  still needs inclusion in the app's Cargo.lock dependency list when the orchestrator builds.
- New public engine APIs: `actions::run_guided`, `Session::run_javascript_at`. Existing
  `run_on` / `run_javascript` signatures remain compatible. New Step variants are handled by
  IDs/labels/arguments/apply in engine actions. App's configured runner additionally takes a
  final publication predicate. `Op::RunJavaScript` now carries a page; JavaScript replies carry
  requests. All local callers/tests were updated. No CLI-only commands were added.
- New files: `src/action_import.rs`, `src/portfolio_ui.rs`, `src/reader_ui/portfolio.rs`,
  `ui/screens/portfolio.slint` (all under sheaf-app). ARCHITECTURE.md maps them.
- Merge seams: the small exported-struct/global blocks in main.slint; install/module blocks in
  main.rs; reader_doc Op/Reply/match cases; reader_ui reply cases and one public portfolio module;
  reader toolbar entries at the end of More tools; Tools' new shortcut row; flow's JSON drop and
  checkout-close hooks; TextField's optional enabled property; engine actions/js wrappers;
  tails of engine_patches/portfolio/form tests and smoke 251–253. Preserve other batches' edits.
- Remaining scope: engine command subset and draft recovery after quitting (002); passwords
  per input, progress inside engine steps (003); most Stirling tools and Acrobat sequ (004);
  online suggestion discovery excluded by mandate (005); current unsaved-document custom runs
  and arbitrary toolbox commands (006); daemon/multiple watches/automatic resume and encrypted
  inbox passwords (008); folder-tree view and whole-process checkout cleanup (files-008);
  portfolio embedded previews/folders/themes (organize-024); debugging, audible Beep and non-text
  field focus (platform-005). No component was impossible or left untouched in this pass.
- `git diff --check` and `bash -n dev/scripts/smoke/3-flows.sh` pass; no cargo/rustc/clippy/test,
  app, browser, Xvfb or codex invocation occurred. Functional tests are written proofs awaiting
  execution, aesthetic scores are provisional code review, and performance is unmeasured.
- Problem-solving close-out: original mandate addressed on all nine components; evidence is in
  pushed diffs and named tests; pending validation is genuinely external because brief rule 1
  forbids it here. Remaining implementation is listed separately above and is not described as
  a validation dependency.

- C-batch-002 draft guard: switching/new/import (including a dropped file) detects unsaved edits
  or unsaved copies and offers Keep editing / Discard changes. Invalid imports leave the draft
  alone. Saved suggested templates remain independent. Test
  `draft_guard_detects_edits_and_unsaved_copies_but_allows_unchanged_templates` (written, not run).
  Left: recovering an unsaved action draft after quitting the whole process.
  not automatable: edit, switch, Keep editing; import a file, Discard; reopen the saved template —
  app execution prohibited by brief rule 1.

## Provisional author grades · pass 2

Static assessment only: the named tests exist but have not run; all aesthetics need rendering,
all performance scores need measurement. No 9 is proposed; no joint closure is requested yet.

- C-batch-002: f8 a6 p7 — `duplicate_step_keeps_configuration_and_enforces_the_step_limit`,
  `crates/sheaf-app/src/actions.rs:242`; `draft_guard_detects_edits_and_unsaved_copies_but_allows_unchanged_templates`,
  `crates/sheaf-app/src/actions_ui.rs:471`: step bounds, independent copies, unsaved-draft detection.
- C-batch-003: f8 a6 p7 — `prompted_settings_affect_this_run_without_changing_the_saved_action`,
  `crates/sheaf-app/src/actions.rs:285`; `stopping_during_the_last_step_does_not_publish_its_output`: isolated prompt edits and final cancellation.
  `a_guided_action_opens_encrypted_input_and_keeps_its_protection`, `crates/sheaf-core/tests/engine_patches.rs:68`: password/protection regression.
- C-batch-004: f5 a6 p7 — `stirling_export_import_preserves_equivalent_settings_and_order`,
  `crates/sheaf-app/src/action_import.rs:84`; `stirling_import_refuses_unknown_tools_settings_and_rasterization`: exact supported subset, rejected incompatible semantics.
- C-batch-005: f8 a6 p7 — `copied_names_remain_valid_at_the_unicode_name_limit`,
  `crates/sheaf-app/src/actions.rs:230`; first-pass `suggested_actions_are_valid_offline_templates_and_survive_copy_edits`: valid independent offline copies.
- C-batch-006: f7 a6 p7 — `custom_commands_round_trip_and_apply_rotation_and_metadata`,
  `crates/sheaf-core/tests/engine_patches.rs:86`: all 20 command IDs round-trip, rotation/author apply; reader palette in actions.slint.
- C-batch-008: f7 a6 p7 — `a_saved_recursive_watch_resumes_without_repeating_handled_revisions`,
  `crates/sheaf-app/src/watch_folder.rs:221`: recursion, persisted handled revisions, change retry, Forget.
- C-files-008: f8 a6 p7 — `checkout_recovery_and_release_keep_versions_and_other_checkouts`,
  `crates/sheaf-app/src/library.rs:303`: working-copy recovery, immutable saved versions, isolated cleanup.
- C-organize-024: f8 a6 p6 — `a_portfolio_keeps_the_chosen_order_separate_from_the_sorted_name_tree`,
  `crates/sheaf-core/tests/portfolio.rs:50`: Collection sort/order; composition and embedded-file browser in portfolio.slint.
- C-platform-005: f8 a6 p7 — `javascript_console_returns_local_viewer_requests_and_named_document_scripts`,
  `crates/sheaf-app/src/reader_doc/tests/forms_and_files.rs:233`: page context, local requests, named scripts and Undo;
  `console_page_context_matches_the_reader_and_clamps_out_of_range_pages`, `crates/sheaf-core/tests/engine_patches.rs:103`: page clamp.

## Files touched in pass 2

- `ARCHITECTURE.md`
- `crates/sheaf-app/src/action_import.rs`
- `crates/sheaf-app/src/actions.rs`
- `crates/sheaf-app/src/actions_ui.rs`
- `crates/sheaf-app/src/flow.rs`
- `crates/sheaf-app/src/lib.rs`
- `crates/sheaf-app/src/library.rs`
- `crates/sheaf-app/src/library_ui.rs`
- `crates/sheaf-app/src/main.rs`
- `crates/sheaf-app/src/portfolio_ui.rs`
- `crates/sheaf-app/src/reader_doc/mod.rs`
- `crates/sheaf-app/src/reader_doc/tests/forms_and_files.rs`
- `crates/sheaf-app/src/reader_ui/javascript.rs`
- `crates/sheaf-app/src/reader_ui/mod.rs`
- `crates/sheaf-app/src/reader_ui/portfolio.rs`
- `crates/sheaf-app/src/watch_folder.rs`
- `crates/sheaf-app/ui/main.slint`
- `crates/sheaf-app/ui/screens/actions.slint`
- `crates/sheaf-app/ui/screens/javascript.slint`
- `crates/sheaf-app/ui/screens/library.slint`
- `crates/sheaf-app/ui/screens/portfolio.slint`
- `crates/sheaf-app/ui/screens/reader-toolbar.slint`
- `crates/sheaf-app/ui/screens/tools.slint`
- `crates/sheaf-app/ui/widgets/text-field.slint`
- `crates/sheaf-core/src/portfolio.rs`
- `crates/sheaf-core/tests/engine_patches.rs`
- `crates/sheaf-core/tests/portfolio.rs`
- `dev/par-notes-sol-batch.md`
- `dev/scripts/smoke/3-flows.sh`
- `third_party/pdfcraft-engine/src/actions.rs`
- `third_party/pdfcraft-engine/src/js.rs`

## Pass 3 · active

- C-batch-002: Acrobat Action Wizard now offers native Rust compression presets (low,
  recommended, extreme, lossless), configured page numbers (six positions, starting number,
  size and token format), and PDF/A-2b/3b conversion. PDF/A refuses publication when the native
  verifier still finds issues, with their first cause; it never claims Ghostscript-level repair.
  Dedicated controls replace the free-text setting for those tools. The list filter's string-to-int
  Slint error is fixed. Drafts checkpoint every two seconds and recover after restart, including
  incomplete new actions. Tests `wizard_native_options_validate_and_number_pages_before_compression`,
  `partial_action_drafts_recover_without_becoming_saved_actions` (written, not run).
  Left: Protect, Split, Word/image export and arbitrary toolbox settings; last two seconds of
  typing may be lost on immediate process death. Output controls follow in C-batch-003.
  not automatable: choose each native option, restart and recover draft; compile/render is
  forbidden by brief rule 1. No new dependency, network or external executable.

- C-batch-003: Acrobat output options now persist with an action: each input's folder or a
  chosen folder, prefix/suffix, unique copies by default, explicit atomic replacement when
  requested. Run on a folder adds all direct PDFs in sorted order without following symlinks.
  Optional per-input masked password prompts replace the single shared password; secrets are
  never serialized or checkpointed. Test
  `output_options_keep_originals_unless_explicitly_overwritten_and_accept_folder_inputs`
  (written, not run). No new dependency.
  Left: non-PDF output formats, recursive batch input and running a full sequence across all
  open unsaved documents. Password retry is a new run after the per-file failure.
  not automatable: same-folder run, overwrite toggle, two differently encrypted inputs and
  Cancel at password prompt — GUI execution forbidden by brief rule 1.

- C-batch-004: Stirling Automate/Folder Scanning import now also translates compression quality,
  header/footer page numbering and English skip-text OCR. Order, page position/start/size/text
  and unsupported-setting refusal have written coverage in
  `stirling_native_import_keeps_order_and_discloses_conversion_limits`. Import notes persist in
  the description and appear in status: native compression/OCR differ from Ghostscript/Tesseract,
  numbering uses Sheaf margins/Helvetica. Linearization, target size, image cleanup, middle-page
  numbering, padding, selected pages and unknown tokens are refused. Export refuses output settings
  it cannot preserve. References: Stirling useCompressOperation/useOCROperation/
  useAddPageNumbersParameters and CompressController. No external programs or network added.
  Left: Acrobat .sequ, split/convert/watermark and metadata replacement; these require parsers or
  tool settings beyond the currently supported native equivalents. Import is deliberately explicit
  about approximations rather than claiming lossless interoperability. Tests written, not run.

- C-batch-005: Acrobat's Archive Documents now has native, verification-gated PDF/A-2b and
  PDF/A-3b presets; Prepare a Form detects fields; Optimize for Sharing uses the native image
  compression preset. Nine offline suggestions now explain their concrete limits before copying.
  Test `suggested_archive_form_and_sharing_actions_are_native_editable_copies` and the existing
  suggestion independence test (written, not run). No dependency or external program.
  Left: Make Accessible needs real tag/reading-order remediation; Optimize for Web needs genuine
  linearization. Neither is advertised under a misleading name. Online discovery remains excluded.

- C-batch-006: custom commands now apply to the open unsaved PDF through the reader's existing
  undoable edit path (metadata, rotations, watermark/header/footer/numbering, flattening, removal
  and sanitization). The palette enables Apply to open PDF only for supported commands and a
  live reader tab; export/recognition/PDF-A and prompted jobs retain their batch flow, visibly
  explained in the panel. Commands can have custom labels and separators, saved/imported with
  their set. Naming is consistently Actions, including the palette help.
  Tests `custom_tool_commands_edit_the_current_document_and_undo` (engine via core tests),
  `suggested_copies_and_custom_tools_are_independent` (label/separator round-trip), written not run.
  Public engine helper `actions::command_edit`. New reader_ui/custom_tools.rs, no dependency.
  Left: arbitrary toolbox tools, icons and dedicated named panels on Tools; running multiple
  commands together on the unsaved document. Current-document OCR/compression need their existing
  progress jobs, so they are not applied as if they were ordinary reader edits.
  not automatable: edit a document, apply custom metadata/rotation and Undo without losing earlier
  edits — app launch forbidden by brief rule 1.

- C-batch-008: saved folder watches now resume on launch; up to two independent inboxes run
  concurrently without locking the Actions editor. Stop folder watches is separate from Stop
  batch. Each inbox has an independent checkpoint, naming pattern ({name}, {title}, {counter})
  and optional successful-input archiving under output/Processed inputs; failed inputs remain.
  Directory scans yield every 512 entries and poll completed inbox snapshots every three seconds,
  avoiding a 100000-file traversal every second. The watcher stages privately even if the saved
  action asks to overwrite or use the input folder. New test
  `watches_keep_independent_patterns_archive_inputs_and_scan_incrementally` covers naming,
  original-byte archiving, separate saved configs and bounded scan snapshots (written, not run).
  Left: watches still require the Sheaf process (no OS service/daemon); at most two simultaneous
  watches on this memory-constrained app; polling, not OS file notifications. A crash between
  output publication and checkpoint can duplicate an output; collisions remain unique copies.
  not automatable: restart into two configured watches, edit another action, stop watches,
  verify successful inputs moved and failed ones retained — app execution forbidden.
  No new dependency or network service.

- C-files-008: Library now displays a responsive thumbnail grid, with first-page native PDF
  previews queued on the existing bounded probe pool (256 cached revisions), folder navigation
  including intermediate ZIP folders, multi-selection, Export selected and confirmed Delete
  selected. Both batch operations report partial errors and preserve original files; export
  collisions keep separate copies. Typing/filtering and selecting use the in-memory catalogue;
  Refresh/reader Save/mutations reload metadata. Test
  `cached_library_filter_tree_and_batch_actions_keep_versions_and_collisions` covers filtering
  while metadata is offline, folder hierarchy, deduplicated selections and colliding exports
  (written, not run). No dependency, LibreOffice process or network.
  Left: thumbnails for non-PDF/encrypted files (file icon fallback), drag-to-folder organization,
  virtualized very-large grids and sorting controls. Folder paths show the full hierarchy instead
  of a collapsible tree. All render/performance grades remain provisional.
  not automatable: add mixed PDFs/ZIPs, browse folders, filter, multi-select across filters,
  export/delete and verify thumbnails at minimum size — app launch forbidden by brief rule 1.

- C-organize-024: a portfolio now opens directly in a full document browser rather than only
  its cover/attachment modal. Name, Description and Size are distinct columns sourced from
  embedded-file metadata; folders present in item names can be filtered. Preview renders the
  first PDF page, bounded images or the first 64 KiB of text entirely locally; unsupported Office
  and other formats explain Open/Save copy without spawning LibreOffice. View cover returns to
  the reader. Decoded previews cap files at 32 MB and image allocation at 64 MB.
  Test `portfolio_browser_uses_embedded_description_size_and_native_text_preview` proves metadata
  and original text preview (written, not run), alongside the existing core portfolio tests.
  Left: editing arbitrary collection-schema fields, real PDF Collection folder creation, sorting,
  multi-page previews and themes. Existing generated file order and original-byte preservation
  remain covered by core tests. No dependency, external conversion program or network.
  not automatable: open a portfolio, preview PDF/image/text/Office, check description/size columns,
  Save copy and View cover — app launch/render forbidden by brief rule 1.

- C-platform-005: console adds Run selection and Ctrl/Cmd+Enter (selection if present, whole
  script otherwise), with UTF-8 byte-boundary validation. Its script manager now lists/edits/
  deletes document scripts and custom field keystroke/format/validate/calculate/mouse-up scripts
  through existing undoable edits. New engine `js::ConsoleScript`, `Document::console_scripts`,
  document-thread EditableScripts request/reply. Network LaunchUrl/Submit are explicitly explained
  by the UI dispatcher, without execution; multiple dialog requests also explain the skipped one.
  Console output survives reopening on the same document.
  Tests `run_selection_keeps_unicode_boundaries_reverse_ranges_and_visible_network_refusals`,
  `console_manager_lists_and_edits_document_and_field_scripts_with_undo` (written, not run).
  Left: debugger/breakpoints (the engine exposes no debugger), page/link action editing and the
  complete Edit All JavaScripts inventory. The manager explicitly says these are absent instead
  of presenting the subset as Acrobat's complete tool. No new dependency; network stays excluded.
  not automatable: select multiline/Unicode code, Ctrl+Enter, save/delete a field script and Undo,
  issue LaunchUrl/Submit plus two dialog requests — app execution forbidden by brief rule 1.

## Pass 3 · shared visual correction

- All four batch surfaces now use Sheaf PrimaryButton/SecondaryButton and Check controls rather
  than std-widgets buttons/check boxes. Added the missing enabled state, keyboard activation and
  accessibility semantics to SecondaryButton and Check, keeping their incumbent tokens/geometry.
  Shorter control rows separate JS execution from output controls and custom-command labels from
  their two targets. Tools now says Actions, matching the sidebar. Native ComboBox/TextEdit remain
  for their existing text/menu behavior. No new visual system or assets.
- not automatable: keyboard traversal, disabled controls, minimum-size layouts and light/dark
  comparison against Tools/reader — this worker must not launch or render the app under brief
  rule 1. The shared SecondaryButton/Check changes need merge care and orchestrator UI checks.

- C-batch-004 static follow-up: malformed parameter objects/types now fail instead of becoming
  default page numbering. Stirling export also refuses command labels/separators it cannot keep.
  These cases extend `stirling_native_import_keeps_order_and_discloses_conversion_limits`.
  Backend PageNumbersController confirms {n}/{total}; native font/margin differences stay disclosed.

- C-batch-008 static follow-up: recheck revisions after a completed scan, before publication and
  after the archival copy. An input changed during processing remains in the inbox for its next
  stable revision; it is not deleted as if it were the processed revision. Test
  `a_watch_never_archives_or_publishes_an_input_changed_during_its_run` (written, not run).
  Revision identity remains size/mtime; same-size rewrites preserving timestamps are not detected.

- C-files-008 static follow-up: catalogue initialization and saves while Library is hidden no
  longer enqueue its thumbnails; first visible opening does. The existing bounded probe pool
  remains the only thumbnail worker path. not automatable: start on Home and open Library to
  verify deferred previews — app execution forbidden.

- C-organize-024 static follow-up: the full portfolio browser catches clicks on blank areas,
  keeping them from reaching the covered reader. not automatable: click browser margins while
  the reader has an active drawing tool — app execution forbidden.

- C-batch-002 proof refinement: the native-options regression also extracts the produced page's
  text and asserts Page 7 survives every compression preset, rather than checking only page count
  and configuration parsing. Test written, not run.

- C-files-008 lock correction: static inspection confirmed the older app zip dependency was
  still missing from Cargo.lock's sheaf-app package list. Added the existing resolved `zip 4.6.1`
  entry there manually; its version/checksum/transitive packages were already locked. No cargo,
  dependency resolution, install or test was run. This supersedes the prior lock-refresh warning.

- C-platform-005 static follow-up: the EditableScripts document-thread request replaces the old
  document-only request instead of leaving a production-unused variant. The existing
  `javascript_console_returns_local_viewer_requests_and_named_document_scripts` regression now
  exercises that request. Engine Document::document_scripts remains unchanged and compatible.

## Pass 3 · final hand-off

- Completed implementation work: all nine components. Parity status: all remain partial; the
  specific missing behavior and its reason are listed in their Pass 3 entries. No untouched or
  abandoned component. No network features or OS daemon. No Ghostscript/LibreOffice process.
- Verification executed: git diff --check across the pass; bash -n dev/scripts/smoke/3-flows.sh;
  Impeccable static detector returned []. This is not compilation, test execution, visual proof
  or performance measurement. Existing smoke 250–253 was syntax-checked, not run or expanded;
  new GUI flows have explicit not-automatable hand-off steps above.
- New engine public items: Step::Compress, Step::ConvertPdfA, Step::PageNumbers,
  actions::number_settings, actions::command_edit, js::ConsoleScript,
  Document::console_scripts. The existing engine methods remain compatible. Internal reader
  EditableScripts request/reply replaces its former document-only inventory request.
- App model additions: OutputOptions and draft persistence; cached filter/folders and BatchReport;
  per-inbox watch configs, pattern/archive options and incremental traversal; selection/request
  helpers. New modules javascript_console.rs and reader_ui/custom_tools.rs.
- Merge seams: small blocks in app lib.rs, reader_doc/mod.rs, reader_ui/mod.rs, ui/main.slint,
  shared SecondaryButton/Check, Tools naming, reader_doc/tests/forms_and_files.rs, core
  tests/engine_patches.rs and Cargo.lock's app package dependency list. No Cargo manifest changed.
- Remaining watcher boundary: identity is size/mtime, and a writer can still race the last
  revision check. Archiving does not provide a cross-process file lock; same-size timestamp-
  preserving rewrites cannot be detected by this poller. Native preview uses existing pdfium
  rendering for PDFs, Rust image decoding/text previews, and no Office subprocess.

Files touched in pass 3:

- `Cargo.lock`
- `crates/sheaf-app/src/action_import.rs`
- `crates/sheaf-app/src/actions.rs`
- `crates/sheaf-app/src/actions_ui.rs`
- `crates/sheaf-app/src/javascript_console.rs`
- `crates/sheaf-app/src/lib.rs`
- `crates/sheaf-app/src/library.rs`
- `crates/sheaf-app/src/library_ui.rs`
- `crates/sheaf-app/src/portfolio_ui.rs`
- `crates/sheaf-app/src/reader_doc/mod.rs`
- `crates/sheaf-app/src/reader_doc/tests/forms_and_files.rs`
- `crates/sheaf-app/src/reader_ui/custom_tools.rs`
- `crates/sheaf-app/src/reader_ui/javascript.rs`
- `crates/sheaf-app/src/reader_ui/mod.rs`
- `crates/sheaf-app/src/reader_ui/portfolio.rs`
- `crates/sheaf-app/src/watch_folder.rs`
- `crates/sheaf-app/ui/main.slint`
- `crates/sheaf-app/ui/screens/actions.slint`
- `crates/sheaf-app/ui/screens/dialogs.slint`
- `crates/sheaf-app/ui/screens/javascript.slint`
- `crates/sheaf-app/ui/screens/library.slint`
- `crates/sheaf-app/ui/screens/portfolio.slint`
- `crates/sheaf-app/ui/screens/tools.slint`
- `crates/sheaf-app/ui/widgets/check.slint`
- `crates/sheaf-core/tests/engine_patches.rs`
- `dev/par-notes-sol-batch.md`
- `third_party/pdfcraft-engine/src/actions.rs`
- `third_party/pdfcraft-engine/src/js.rs`

Grades below are author estimates from source, with unrun tests as specified by brief rule 1.
Aesthetics and performance require the orchestrator's actual render/measurement. No joint 9.

- C-batch-002: f7 a7 p8 — engine_patches.rs:113 `wizard_native_options_validate_and_number_pages_before_compression` verifies number settings/text with all native compression presets; actions.rs:415 `partial_action_drafts_recover_without_becoming_saved_actions` covers restart recovery. Protect/Split/export remain absent.
- C-batch-003: f8 a7 p8 — actions.rs:427 `output_options_keep_originals_unless_explicitly_overwritten_and_accept_folder_inputs` covers naming, same-folder, unique/replace paths and folder intake. Non-PDF output/all open unsaved documents remain absent.
- C-batch-004: f6 a7 p8 — action_import.rs:165 `stirling_native_import_keeps_order_and_discloses_conversion_limits` covers extra translations, visible limits and malformed-setting refusal. .sequ and many Stirling tools remain unsupported.
- C-batch-005: f8 a7 p8 — actions.rs:448 `suggested_archive_form_and_sharing_actions_are_native_editable_copies` covers the four added templates and copy independence. Full accessibility remediation/linearization remain absent.
- C-batch-006: f7 a7 p8 — engine_patches.rs:142 `custom_tool_commands_edit_the_current_document_and_undo` preserves earlier unsaved edits through command Undo; label/separator serialization extends the existing custom-copy regression. Arbitrary tools/icons/dedicated panels remain absent.
- C-batch-008: f8 a7 p7 — watch_folder.rs:341 `watches_keep_independent_patterns_archive_inputs_and_scan_incrementally` covers multiple configs, rename/archive and bounded snapshots; :376 `a_watch_never_archives_or_publishes_an_input_changed_during_its_run` covers detected revision changes. No OS daemon; two watches maximum.
- C-files-008: f8 a7 p7 — library.rs:363 `cached_library_filter_tree_and_batch_actions_keep_versions_and_collisions` covers offline metadata filtering, folders, multi-file export/delete and collisions. Non-PDF previews and grid virtualization remain absent.
- C-organize-024: f8 a7 p8 — reader_ui/portfolio.rs:87 `portfolio_browser_uses_embedded_description_size_and_native_text_preview` covers metadata and text preview; existing core portfolio tests cover originals/order. Real collection folders/arbitrary schema editing remain absent.
- C-platform-005: f8 a7 p8 — javascript_console.rs:24 `run_selection_keeps_unicode_boundaries_reverse_ranges_and_visible_network_refusals`; engine_patches.rs:157 `console_manager_lists_and_edits_document_and_field_scripts_with_undo`. No debugger or page/link script editor.

## Pass 4 · 27 October · author fixes after Opus review 2

### Tatouages
- Scope: sol-batch only, par/sol-batch. Brief rule 1 forbids compilation, tests, app and codex.
- No network features. Native Rust first; no Ghostscript or LibreOffice invocation.

### Polaroid
- Fourth-pass implementation complete; awaiting the orchestrator suite and Opus review. Each
  component is committed and pushed independently. Static review only;
  the orchestrator owns compilation, runtime, visual checks and the joint grade.

- C-batch-002: Action Wizard now offers Keep/Delete selected pages and configurable watermark /
  header-footer steps: font, size, page ranges, watermark opacity, angle and offsets, six header
  positions. Existing plain-text steps stay compatible. Test
  `wizard_page_ranges_and_mark_settings_edit_only_selected_pages` (written, not run).
  Left: encrypt, insert, split and export as intermediate tools; direct controls rather than the
  compact settings text are still needed. This closes the mark-settings engine gap, not all tools.
  not automatable: add these four steps, edit settings, save/reopen and run on mixed page counts —
  execution forbidden by the brief. No new dependency. Public: Step's four new variants,
  actions::page_selection. Native font dependency was already present.

- C-batch-003: Save to PDF, Word, PowerPoint, HTML, RTF, text, Markdown or an all-pages PNG/JPEG
  ZIP. Native existing writers/renderers, 150 dpi images, one atomic archive publication; Office
  layout remains approximate and this is stated in the editor. Folder runs optionally recurse,
  refuse excessive depth/entry counts and skip symlinks. Test
  `action_save_to_formats_use_native_writers_and_recursive_inputs_skip_symlinks` (written, not run).
  Left: run whole actions over all open unsaved reader documents; extracting a disk copy would
  lose those edits, so that route was not used. No new dependency (existing zip/native exporters).
  not automatable: choose each output format and recursive folder intake, cancel a run — brief
  forbids app execution. Public: OutputOptions::format, actions::folder_inputs_recursive.

- C-batch-004: Stirling Remove/Extract Pages, standard metadata replacement / delete-all, terminal
  PDF→DOCX/TXT/RTF conversions now translate to native steps and output settings. Omitted standard
  metadata is cleared, matching MetadataController rather than silently keeping old fields.
  Test `stirling_page_metadata_and_terminal_conversions_preserve_or_refuse_settings` (not run).
  Source: Stirling MetadataController, MetadataRequest, PdfToWordRequest/PdfToTextOrRTFRequest,
  removePages parameters in the reference archive. Added Step::SetMetadata / RemoveMetadata.
  Left: .sequ (no trustworthy schema/parser), split (multi-output runner needed), tiled watermark
  (Sheaf has a single mark; translating it would change appearance), other unimplemented tools,
  nonempty date/trapped values and custom metadata maps. These are refused, not guessed.
  No new dependency. not automatable: paste/import each new operation and inspect its draft —
  app execution forbidden. Export back to Stirling still refuses newly inexact conversions.

- C-batch-005: added Prepare Accessibility (text-only), a useful native subset of Acrobat's Make
  Accessible. Prompts for the real document language, tags simple text PDFs, sets display title
  and structure tab order in one atomic engine edit. Refuses existing tags, images and forms;
  description and prompt require heading/reading-order review and do not claim PDF/UA.
  Test `accessibility_suggestion_tags_text_and_preserves_existing_tags_by_refusal` (not run).
  Left: full accessibility remediation and Optimize for Web linearization. No linearized writer
  exists in this engine; setting a flag would not produce hint tables or byte ordering, so no
  misleading preset was added. No external process or new dependency. Public: Step::PrepareAccessibility.
  not automatable: copy the suggestion and verify language prompt and visible limitations —
  app execution forbidden.

- C-batch-006: custom sets can now include every existing toolbox tool (Redact, Compare, Organize,
  Edit, conversion…) with the tool's real Lucide icon, label and separator. A Custom tools button
  on Tools opens their panel. Interactive shortcuts open the real dedicated tool and are refused
  in unattended sequences; native edit commands still apply to unsaved reader documents.
  Test `custom_toolbox_shortcuts_validate_real_tools_and_never_run_as_batch_edits` (not run).
  Left: dedicated persistent custom panels / user-selected icons; toolbox shortcuts require their
  normal file selection rather than claiming to modify unsaved reader bytes. No new dependency.
  Public: Step::OpenTool; ActionCommandRow icon/interactive and ActionBridge toolbox model.
  not automatable: create a mixed custom set, open its tools from Tools and reader, verify icons
  and the unattended refusal — app execution forbidden.

- C-batch-008: removed the two-inbox cap. Concurrent inbox workers serialize native scan/process
  jobs so adding watches cannot multiply OCR/render memory. Unix revision identity includes
  nanosecond ctime, detecting same-size mtime-preserving rewrites without rereading entire PDFs;
  other platforms stream a content fingerprint through a 64 KB buffer. Old checkpoints migrate
  conservatively as new revisions. Interactive custom tools are refused for unattended watches.
  Test `watch_detects_same_size_rewrites_with_restored_modification_time` (written, not run).
  Left: OS notifications, out-of-process service and any-number scalability measurements. The
  poller still runs only while Sheaf is open; portable event APIs/service installers are outside
  this correction. Windows hashing adds disk reads each scan. A writer can still race publication;
  there is no cross-process input lock. No new dependency/public API.
  not automatable: launch three saved inboxes, restart, then stop during a queued scan — execution
  forbidden. Poll interval remains 3 s after a completed scan, 100 ms between scan slices.

- C-files-008: library sorting by name/date/size in either direction, native raster thumbnails,
  explicit movement of selected files into folders, and bounded 96-card pages. Filtering/sorting
  resets to page one; selection uses stable file IDs and batch selection spans pages. Moving
  catalogue membership leaves file bytes and version history untouched; errors report completed
  moves. Test `library_sort_move_and_raster_thumbnail_keep_file_versions` (written, not run).
  Left: pointer drag/drop between folders and content thumbnails for Office/text files. Pagination
  bounds rendered cards rather than implementing a virtualized Slint grid; catalogue sorting still
  uses the in-memory catalogue. No new dependency. Public: library::sort_entries,
  raster_thumbnail, Library::move_to_folder; LibraryBridge sort/page/total/move-folder fields.
  not automatable: >1000 files, sort both ways, next/previous, mark across pages and move into a
  folder, verify thumbnail fallback — app execution forbidden.

- C-organize-024: portfolio browser columns now sort both directions (size numerically), preserving
  original attachment IDs for Open/Save/Preview after sorting. Embedded PDFs have previous/next
  preview navigation with page count and stale-result guards, rather than first-page-only viewing.
  Test `portfolio_sort_keeps_attachment_identity_and_previews_every_pdf_page` (written, not run).
  Left: real /Collection folder creation/tree and arbitrary schema editing. Existing slash-name
  filtering is not presented as a /Folders tree; implementing that needs a schema-aware document
  mutation path, not an attachment-name rewrite. No new dependency. PortfolioRow::source and
  PortfolioBridge sort/descending/preview-page/preview-pages added.
  not automatable: sort each column, open the selected original after sorting, navigate every
  preview page and switch attachments while a preview loads — app execution forbidden.

- C-platform-005: Edit All JavaScripts now lists and edits existing page open/close and link
  activation/additional-event scripts as well as document/field scripts. String or stream sources,
  including Unicode, use bounded native PDF decoding. Edits preserve unrelated event actions and
  /Next chains; deleting a chained action refuses instead of destroying the rest of the chain.
  Every mutation uses Edit::SetActionScript with normal permissions/Undo. New resets the target
  before creating a document script. Test
  `console_edits_page_and_link_scripts_preserving_other_events_chains_and_undo` (not run).
  Left: debugger, breakpoints, stepping, adding new page/link targets via UI, editing scripts
  deeper in /Next chains. Engine lacks a suspend/resume debugger; an editor highlight would not
  provide execution debugging. Network requests remain visibly unexecuted. No new dependency.
  Public: js::ActionScriptTarget, ConsoleScript::target, Edit::SetActionScript. New private module
  action_scripts.rs; JavaScriptBridge/DocumentScriptRow carry original page/link targets.
  not automatable: edit/save/delete a listed page or link script, Undo, reopen; inspect refusal
  for deletion with /Next — app execution forbidden.

- C-batch-005/006 static correction: moved the new argument validation into from_id (the id
  mapper must stay a pure string match), and updated the existing preset inventory regression
  from nine to ten, asserting the accessibility prompt. Existing
  `custom_commands_round_trip_and_apply_rotation_and_metadata` covers all Step variants' IDs.

- C-batch-003 static follow-up: native image sets check cancellation before each page, and ZIP
  stores already-compressed page images without recompressing them. Test
  `native_action_exports_stop_before_rendering_cancelled_pages` (written, not run); the new
  recursive intake test also uses the suite's existing no-clone assertion style.

- C-batch-003 final follow-up closes the open-document input gap: Run on open PDFs snapshots each
  reader's actual working bytes/password (after completed edits), processes those bytes, and
  saves new output copies in the chosen folder. It does not reopen stale disk files or change
  open tabs. Busy readers refuse capture; immutable Arc snapshots survive subsequent tab changes.
  Test `actions_on_open_documents_use_unsaved_working_bytes_and_keep_disk_originals` (not run).
  This supersedes the earlier Pass 4 "all open unsaved documents" leftover. Public: ActionInput,
  actions::run_bytes_configured. not automatable: edit two open PDFs without saving, run an
  action, check outputs contain both edits and originals stay unchanged — execution forbidden.

- C-batch-002 final follow-up adds native AES-256 Protect as a final PDF step. A masked prompt
  requests each file's open password at runtime; saved actions must have an empty password and
  cannot disable that prompt. Draft flush clears its argument too. Native encryption uses the
  existing engine, without Ghostscript. Tests `password_actions_prompt_per_file_and_never_persist_passwords`
  and `wizard_protection_encrypts_the_final_output_with_only_the_prompted_password` (not run).
  This supersedes "encrypt absent" above. Left: permission-password settings, insert/split tools
  within a sequence, direct mark-setting controls. Public: Step::Protect. No new dependency.
  not automatable: use different passwords for two outputs, cancel the password prompt, inspect
  exported action/draft JSON and reopen outputs — app execution forbidden.

- Final static hardening: the draft serializer itself clears Protect arguments (not only the
  UI flush), extending `password_actions_prompt_per_file_and_never_persist_passwords` to draft
  recovery. Portfolio browse applies the displayed initial A–Z order before its first click.

- C-batch-008 static follow-up: recheck the input revision after native format conversion and
  immediately before publication. `watched_exports_recheck_inputs_after_native_conversion`
  covers a writer changing the input during that new export phase (written, not run).

## Pass 4 · final hand-off

- All nine components received implementation changes; none was left untouched. The changes are
  ready for orchestration, not certified as running. All nine remain partial against full main-
  competitor parity; the specific remaining gaps and reasons are recorded above. No joint 9.
- Rule 1 respected: no cargo/rustc/clippy/test suite/app/Chrome/Xvfb/codex execution. New tests
  are written only. GUI cases have not-automatable entries above, with the execution prohibition.
- Static verification: full-pass `git diff --check` clean; Impeccable's mechanical detector clean
  on the modified Slint surfaces. No visual, performance or end-to-end validation was possible
  within this worker's mandate; those grades below are estimates for the counterpart to assess.
- No network features, service/daemon, Ghostscript or LibreOffice process added. Native Rust
  export, compression, encryption, tagging, image decode and PDF rendering are used. Office
  exports disclose approximate layout; unsupported semantics refuse instead of silently changing.
- No dependency, manifest or Cargo.lock change in this pass. All used libraries were already
  present. No dependency fetch is needed for these changes.
- Public engine additions: Step::{KeepPages, DeletePages, WatermarkOptions, HeaderFooterOptions,
  SetMetadata, RemoveMetadata, PrepareAccessibility, OpenTool, Protect}; actions::page_selection;
  js::ActionScriptTarget; ConsoleScript::target; Edit::SetActionScript. New private engine module
  action_scripts.rs is registered in lib.rs and mapped in ARCHITECTURE.md.
- Public app additions: ActionInput, actions::run_bytes_configured, actions::folder_inputs_recursive,
  OutputOptions::format; library::{sort_entries,raster_thumbnail}, Library::move_to_folder.
  Slint models carry command icons/interactive kind, output format/toolbox choices, library sort/
  pages, portfolio source indices/sort/preview pages and JavaScript page/link event targets.
- Merge seams: small engine lib.rs enum/label/permission/dispatch blocks and expanded Step enum;
  reader_ui/JavaScriptBridge/PortfolioRow models and core engine_patches.rs integration tests.
  App snapshots use Snap::bytes (working file), not Snap::display (display-only appearances).

Files touched in this pass:

- `ARCHITECTURE.md`
- `crates/sheaf-app/src/action_import.rs`
- `crates/sheaf-app/src/actions.rs`
- `crates/sheaf-app/src/actions_ui.rs`
- `crates/sheaf-app/src/library.rs`
- `crates/sheaf-app/src/library_ui.rs`
- `crates/sheaf-app/src/portfolio_ui.rs`
- `crates/sheaf-app/src/reader_ui/custom_tools.rs`
- `crates/sheaf-app/src/reader_ui/javascript.rs`
- `crates/sheaf-app/src/reader_ui/portfolio.rs`
- `crates/sheaf-app/src/watch_folder.rs`
- `crates/sheaf-app/ui/screens/actions.slint`
- `crates/sheaf-app/ui/screens/javascript.slint`
- `crates/sheaf-app/ui/screens/library.slint`
- `crates/sheaf-app/ui/screens/portfolio.slint`
- `crates/sheaf-app/ui/screens/tools.slint`
- `crates/sheaf-core/tests/engine_patches.rs`
- `dev/par-notes-sol-batch.md`
- `third_party/pdfcraft-engine/src/action_scripts.rs`
- `third_party/pdfcraft-engine/src/actions.rs`
- `third_party/pdfcraft-engine/src/js.rs`
- `third_party/pdfcraft-engine/src/lib.rs`

Author grades (source estimates; all named tests are written, not run):

- C-batch-002: f8 a7 p8 — `crates/sheaf-core/tests/engine_patches.rs:536` `wizard_page_ranges_and_mark_settings_edit_only_selected_pages`: selected pages/mark settings; wizard_protection_encrypts_the_final_output_with_only_the_prompted_password covers native encryption.
- C-batch-003: f8 a7 p8 — `crates/sheaf-app/src/actions.rs:576` `actions_on_open_documents_use_unsaved_working_bytes_and_keep_disk_originals`: working bytes rather than disk; action_save_to_formats_use_native_writers_and_recursive_inputs_skip_symlinks covers formats/recursive inputs.
- C-batch-004: f7 a7 p8 — `crates/sheaf-app/src/action_import.rs:237` `stirling_page_metadata_and_terminal_conversions_preserve_or_refuse_settings`: page/metadata translation, terminal conversion and explicit refusals.
- C-batch-005: f8 a7 p8 — `crates/sheaf-core/tests/engine_patches.rs:575` `accessibility_suggestion_tags_text_and_preserves_existing_tags_by_refusal`: text-only tagging, real language, displayed title and existing-tag refusal.
- C-batch-006: f8 a7 p8 — `crates/sheaf-app/src/actions.rs:557` `custom_toolbox_shortcuts_validate_real_tools_and_never_run_as_batch_edits`: all current catalogue tools round-trip as interactive shortcuts.
- C-batch-008: f8 a7 p7 — `crates/sheaf-app/src/watch_folder.rs:442` `watch_detects_same_size_rewrites_with_restored_modification_time`: same-size/time rewrite detection and checkpoint persistence; watched_exports_recheck_inputs_after_native_conversion guards late changes.
- C-files-008: f8 a7 p8 — `crates/sheaf-app/src/library.rs:417` `library_sort_move_and_raster_thumbnail_keep_file_versions`: sort orders, native image thumbnail, folder move retains immutable versions.
- C-organize-024: f8 a7 p8 — `crates/sheaf-app/src/reader_ui/portfolio.rs:142` `portfolio_sort_keeps_attachment_identity_and_previews_every_pdf_page`: numeric sort and stable attachment IDs, second-page preview and bounds.
- C-platform-005: f8 a7 p8 — `crates/sheaf-core/tests/engine_patches.rs:590` `console_edits_page_and_link_scripts_preserving_other_events_chains_and_undo`: page/link source edits preserve other events, chains and Undo.
