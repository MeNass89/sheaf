# Round 30, worker ui (branch par2/ui)

## Done

- C-platform-004: right to left now also mirrors the reader's sheets (signature, print, document
  properties, background, versions, folder search, accessibility check, voices, help), Home's
  Recents, Tools and History pages, the result page's two columns and HTML to PDF's layout row
  (iLovePDF's right-to-left interface). Round 29's ab8bf70 was redone step by step, building after
  each: Home pages built; the reader sheets overflowed the build script's stack; bisected to the
  signature sheet, then to its typed-name font tiles. The construct: a FlexboxLayout row holding an
  item whose height follows its width (an Image sized `parent.width - 14px`, a word-wrapped Text).
  In a plain row Slint 1.18 reports a binding loop; inside a repeated row holding a repeater
  (`for row in [[0, 1], [2, 3]]: FlexboxLayout { for f in row: … }`) the compiler recurses until the
  stack overflows. Those rows (font tiles, Document properties' About lines), the print sheet's
  options and preview, and the result page's columns are placed by hand with `x` mirrored. Tests:
  `platform::tests::right_to_left_screens_have_no_row_that_cannot_reverse` (kept; it now also
  covers result and options, which had a plain row), smoke 300, and 223, 224, 230 still pass.
- C-preferences-010: Preferences ▸ Printing (iLovePDF's Printer preferences: paper size,
  grayscale; Acrobat's print defaults): paper, one or two sides, colour or grey, fit or actual size,
  comments, orientation, pages per sheet. The print sheet opens with them; its Make default keeps
  the sheet's choices (Preview's Save Current Settings as Preset). Tests:
  `settings::tests::print_defaults_start_as_the_sheet_did_and_bad_numbers_fall_back`,
  `tests/history.rs` (prefs round trip), smoke 301.
- C-preferences-014: Preferences ▸ Security (Acrobat's Enhanced Security, Trust Manager's Internet
  Access from PDF files and attachments in other apps, Privileged Locations): enhanced security on
  or off; a document's web link asks "Open a page of example.org?" (Allow, Block, Remember for this
  site) or every site is allowed or blocked; linked and attached non-PDF files may be refused
  outright; folders and files added as trusted locations skip the question; remembered sites listed
  and forgettable. Tests:
  `settings::tests::enhanced_security_asks_for_untrusted_documents_and_trusted_locations_skip_it`,
  `settings::tests::sites_are_hosts_without_port_user_or_case_and_mail_domains`, smoke 302.
- C-preferences-012: Preferences ▸ Startup (iLovePDF's "Open iLovePDF automatically after you log
  into the computer" and Processing warning): open Sheaf at login (XDG autostart entry on Linux,
  LaunchAgent on macOS, the user's Run key through `reg` on Windows) and asking before quitting
  while a task runs (new: quitting with a running task used to quit at once). Tests:
  `settings::tests::login_items_open_sheaf_from_the_systems_own_place`, smoke 303.
- C-platform-003: Help ▸ Report an Issue… (iLovePDF's Contact us): Report a problem or Suggest a
  feature, a message (required), up to five files (25 MB each), the computer's details shown and
  optional; Save report… writes one zip (report.txt and the files), Copy, or Open issue page fills
  a new GitHub issue in the browser. Sheaf itself sends nothing. Tests:
  `feedback::tests::a_report_needs_a_message_and_at_most_five_readable_files`, smoke 304.

## Round 31

- C-platform-004 (Sol's r30 gap): tick boxes (widgets/check.slint), radio cards and information
  boxes (widgets/radio-card.slint) mirror right to left: the box or circle on the label's right.
  The test now covers both widgets; smoke 305 checks the tick box's side in both directions.
- Steps 300–304 failed in round 30's suite only because step 311 (In a folder…) never quit its app
  nor cleared its dialog answer: its window kept the keyboard focus for the next steps (320–363
  too). 311 now ends with `answer ""; quit`.
- Report an Issue is 580 px wide so its four buttons fit on one row.

## Round 31, interface language

- C-preferences-003: Preferences ▸ Interface ▸ Language (iLovePDF's Language, Acrobat's "Same as the
  operating system"): As the system's language, English or Français, applied at once. Every text of
  ui/ (1,422 messages: Home, Tools, Preferences, reader toolbar, menus, sheets, panels, tool
  options, dialogs) is wrapped in `@tr` and its French bundled at build time from
  `crates/sheaf-app/translations/fr/LC_MESSAGES/sheaf-ui.po` (slint-build's bundled translations, no
  runtime dependency, nothing read from disk). The Tools page's names and sections, made in Rust, go
  through `sheaf_app::i18n::tr` (the same catalogue, compiled in); Tools search also matches the
  French names. `dev/scripts/po-update.py crates/sheaf-app` keeps the catalogue in step. Tests:
  `i18n::tests::every_interface_text_has_its_french` (every @tr, every `i18n::tr("…")`, every tool
  name and section; placeholders kept), `i18n::tests::languages_follow_the_choice_or_the_system_and_texts_fall_back_to_english`,
  smoke 306.
- Left: texts made in Rust other than the Tools page (toasts, status lines, reader messages, tool
  option summaries) stay English; only French besides English. The French was drafted by five
  agents from a shared glossary and spot-checked, not read line by line.
- Smoke helpers recalibrated for the taller Preferences (Language row): `shortcut_search` finds the
  field from the section's title and cancels a first recording click that scrolls the page; 301
  finds A4 from Letter (A4 alone does not OCR).

## Round 34 (base 50a04a8)

- C-preferences-003: messages built in Rust are French too: every toast literal and `format!` toast,
  file dialog titles, the search results' headings, folder search and catalog status lines go
  through `i18n::tr` / `i18n::trf` (a `{}` template filled in order); toasts also translate a known
  English message held in a variable. New texts of later screens wrapped in `@tr`. 1,698 messages,
  all with French (`i18n::tests::every_interface_text_has_its_french` now reads `i18n::trf("…")` too;
  the language is per thread in tests so a French test leaves the others English). Left: error texts
  from sheaf-core (the `{}` of "Can't preview {}: {}") stay English.
- C-platform-006: plugins (Acrobat's plug-ins, Stirling's custom scripts). A plugin is a folder with
  `plugin.json` (id, name, version, author, description, tools: id, name, `run` with `{input}` and
  `{output}`), in Sheaf's data folder `plugins/`. Preferences ▸ Plugins lists them (version, author,
  tools, whether run yet), turns them on and off, removes them (second click confirms), adds one from
  a folder (manifest checked, copied in), opens the folder; broken manifests are listed with why.
  Their tools join the Tools page (a Plugins section) and the action editor (a plugin step runs
  between the engine's steps). No plugin code is loaded: each run is a separate process on a copy of
  the PDF in a private temporary folder, stdin closed, stopped after 5 minutes or on cancel, its last
  stderr line reported, its output checked to be a readable PDF. Before a plugin version first runs,
  a warning names it, its author and the exact command (Acrobat's untrusted plug-in warning); adding
  its tool to an action asks too; an unallowed plugin in an action is refused. Sample:
  `dev/plugins/reviewed-stamp` (Python, sets the title to Reviewed by incremental update). Tests:
  `plugins::tests::*` (manifest checks, install/list/off/allow/remove, real process runs, errors,
  cancel, the sample), `actions::tests::a_plugin_step_runs_its_tool_between_the_engines_steps`,
  `plugins_ui::tests::outputs_go_beside_their_file_numbered_when_taken`, smoke 307.
- C-preferences-005: Preferences ▸ Rendering ▸ Use the system's profile (Acrobat's Use display
  profile): the display's ICC read from the X root window's `_ICC_PROFILE` (colord, GNOME, KDE,
  xcalib; XWayland included) at each start and when chosen, applied after rendering by the existing
  display transform; sRGB with a note when the system sets none. macOS (ColorSync) and Windows
  (`GetICMProfile`) not read yet: a chosen ICC works there. Tests:
  `display_color::tests::the_systems_display_profile_is_read_from_xprops_answer`, smoke 308 (a Display
  P3 profile set on the root window turns the page's red 255 0 0 into 234 51 35, white unchanged;
  helper `dev/scripts/smoke/set-icc.py`, fixture `display-p3.icc` written by moxcms).
- Keyboard shortcuts (found while calibrating 221/229): the recorder was one FocusScope at the
  section's end, so taking the keyboard scrolled Preferences 69 px under the pointer and the next
  click could miss. Each row now records its own keys; once done the window takes the keyboard back.

## Not automatable

- Quit warning: quitting with a task running shows "A task is still running" and Keep stops the
  quit — needs a job that stays running while the window is closed; no smoke tool makes one slow
  enough reliably.
- Login item on macOS and Windows: written by the code under test only on those systems (Linux
  path tested for real; the LaunchAgent text and `reg` arguments are unit-tested).

## Left, and why

- C-platform-001 (Check for updates), C-platform-008 (Sign in), the update check and proxy of
  C-preferences-012: network features, excluded by Nassim's decision. Startup says Sheaf works
  offline.
- C-preferences-003 (interface language): translating the whole interface (Slint `@tr` and
  bundled catalogues for thousands of strings) does not fit a round; not started.
- C-platform-005 (JavaScript), C-platform-012 (native builds; macOS
  notarisation needs Nassim's Apple account): not reached in the time.

## Notes for the orchestrator

- Builds: rustc on sheaf-app peaked above wbuild.sh's 6 GB cap (three oom-kills at 6.0G at
  18:06–18:10); this worker built with the same script at a 10 GB cap
  (`/tmp/f5/w2ui-scratch/wb.sh`). wbuild.sh's cargo output also never reaches the terminal
  (systemd-run without `--pipe`); the copy writes `dev/.tmp/run.log` and prints its tail.
- Open issue page opens github.com in the user's browser with the report in the address (as Look
  Up opens Wiktionary); Sheaf makes no request itself. Drop the button if that counts as a network
  feature.
- No new dependencies.
