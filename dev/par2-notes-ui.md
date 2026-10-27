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
- C-platform-005 (JavaScript), C-platform-006 (plugins), C-platform-012 (native builds; macOS
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
