# Batch opus-platform — notes (27/10/2026, not built: the orchestrator compiles and tests)

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

- C-edit-013: Acrobat's Check Spelling *dialog* (one pass over every comment and form field of the
  document, issues listed in a panel) is not done; only while typing. The underline is in the card under
  the editor, not inside the field: Slint's TextInput/TextEdit cannot style part of their own text.
- C-reader-060: grammar (Preview's Spelling and Grammar) is not checked: no offline grammar checker that is
  pure Rust and small.
- C-platform-014: the reader's one-letter tool keys are not shown as menu shortcuts (Tools ▸ Annotate lists the
  tools without them): a menu bar shortcut is taken before any text field, so a bare letter would stop
  typing; Preferences lists them instead. The menu bar is not mirrored right to left (Slint draws it left
  to right). Cut, Copy, Paste and Select All keep ⌘X ⌘C ⌘V ⌘A whatever keys the user adds: Slint's text
  fields answer those keys themselves.
- C-preferences-013: "API output handling" (Stirling) is a server setting with no meaning offline. The
  reader's other plain keys (arrows, Space, Delete, ⇧T, ?) are not customizable.
- C-platform-004, what Slint 1.18 blocks or what is left: (1) Slint has no window-wide layout direction, so
  each row is mirrored by hand (done: Home sidebar, tab bar, reader toolbar, reader side panel; left: the
  tool options bar over the pages, the side panel's own rows, the sheets and dialogs, the tool-flow pages).
  (2) Text alignment: `Text`'s default `horizontal-alignment` is `start`, which follows each string's own
  script (an Arabic string sits right, an English one left); Sheaf has no translations, so its English labels
  stay left-aligned except where set to `right` here (sidebar, tabs). (3) The `MenuBar` element has no
  direction: its menus stay File … Help from the left. (4) Icons are not flipped (the Side panel icon still
  points left): Slint has no mirrored image drawing short of a second icon file.
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
- New modules: `sheaf_core::spell`, `sheaf_core::bookmarks`; `sheaf_app::platform` (lib); `platform_ui` (bin,
  `mod platform_ui;` in main.rs, `platform_ui::install(&ui)` after `wire_reader`); `home::update_platform`.
- New Prefs field `platform: sheaf_app::platform::PlatformPrefs` (added to `tests/history.rs`'s literal, `tool_keys` included).
- New Slint files: `ui/platform.slint` (`PlatformBridge`, `SpellIssue`, `ShortcutRow`, exported from
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
