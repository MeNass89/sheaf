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

## C-platform-014 macOS menu conventions (28/10, Opus, MBP)

Seen through System Events on the MBP: Slint's app menu had About, Services, Hide, Quit; ours added Quit to File
and Preferences… to Edit, and Window had no Minimize. Now on a Mac (PlatformBridge.mac): Settings… (⌘,) in the
app menu after About (src/mac_menu.rs: Slint rebuilds the whole menu bar on every change, so the item is put
back on NSMenuDidAddItemNotification), no Quit in File nor Preferences in Edit, Minimize (⌘M) and Zoom in Window.
Checked on the MBP (System Events clicks; probes in an instrumented copy there; CGWindowList): the menus' items in
that order, Settings… still there after the menu bar is rebuilt; Settings… reaches the app (preferences-asked 1,
main.slint then shows Preferences); Zoom takes the window from 1280 × 832 to 1800 × 1130; Minimize takes the
1280 × 832 window off the screen and a 120 × 118 tile shows at the Dock's edge (AXMinimized reads another of the
app's windows, the menu bar's, hence false). Preferences itself is not seen (Slint's text is not in the AX tree and
screen capture is refused over ssh).

## C-preferences-003 six interface languages (28/10, Opus)

Gap (Sol rev36a): only English and French. Now English, French, German, Spanish, Italian, Portuguese
(Preferences ▸ Interface ▸ Language, each named in itself; "As the system's language" picks any of them from the
locale, else English). Catalogues translations/{de,es,it,pt}/LC_MESSAGES/sheaf-ui.po: 1,746 texts each, translated
from the English msgids by four parallel translators (Opus), assembled and checked by a script (same msgids and
order, none empty, every {} / %n kept; one quote style per language: „…“ de, «…» es/it, “…” pt; Portuguese of
Portugal, understood in Brazil); `msgfmt --check` clean. Four messages built plurals with an "s" ("page{}"): now a
singular and a plural text each (Seites otherwise). i18n.rs reads every catalogue (`CATALOGUES`); the test
`every_interface_text_has_its_translation_in_every_language` fails on any text missing in any language; search by
name folds accents of all five (ä ö ü ß ñ ã õ…). Not done: right-to-left languages beyond the existing RTL layout
switch; plural rules beyond singular/plural.
## C-platform-007 AppleScript and Shortcuts (28/10, Opus, on the MBP)

- Gap (Sol): only CLI and MCP; no Shortcuts action, no AppleScript interface.
- Change: a scripting dictionary `crates/sheaf-app/macos/Sheaf.sdef` (bundled by scripts/bundle-macos.sh;
  Info.plist NSAppleScriptEnabled + OSAScriptingDefinition), Preview's Shortcuts names: rotate pages, delete
  pages, insert pages (another PDF's or a blank page, after page N), get pages, combine, add bookmark, search,
  get text, page count, export pages (JPEG or PNG); a file changes in place unless `saving in`. Cocoa Scripting
  hands each command to `SheafScriptCommand` (NSScriptCommand subclass, src/scripting_mac.rs): parameters read
  from the Apple Event, the command suspended, `scripting::run` (src/scripting.rs, the sheaf_core functions the
  CLI calls) on a worker thread, resumed with NSURL/NSNumber/NSString/NSArray or an error (-10000 and the
  message). `open` is AppKit's (reader). Two unit tests in scripting.rs (insert/delete in place without
  leftovers, extract/search/text). First try with bare NSAppleEventManager handlers: commands returned nothing
  once NSAppleScriptEnabled was set (Cocoa Scripting took the events), hence the NSScriptCommand class.
- Mac evidence (MBP, minimal debug Sheaf.app in /tmp/sheafcap, `osascript`): `sdef` lists 10 commands;
  page count → 4; rotate pages … by 90 range "1-2" saving in rotated.pdf → rotations [90, 90, 0, 0]; delete pages
  range "4" → 3 pages; insert pages from one.pdf after page 1, then a blank after page 4 → 5; get pages "2-3" →
  two.pdf (2); combine {two, one} → 3 pages; add bookmark titled "Chapitre 2" at page 2 → shown in the reader's
  outline; export pages … with PNG → {two-1.png, two-2.png}; search invoice.pdf for "the" → {1}; get text range "1"
  → the invoice text; errors: `Sheaf got an error: …/none.pdf: No such file or directory (os error 2) (-10000)`,
  `invalid page range "40": the document has 5 pages (-10000)`; `open` → combined.pdf in the reader
  (/tmp/sheafcap/as-snap/opened.png). Script: dev/.tmp/as-test.sh (not tracked).
- Not checked: a shortcut built in the Shortcuts app (its Run AppleScript sends the same Apple Events; building
  one needs the GUI). No native App Intents action (needs a Swift extension).
- Self-grade F 9 · A 8 · P 9 (A: Preview ships Shortcuts actions; here Shortcuts goes through Run AppleScript).
## BAMA pass: theme and widgets (28/10/2026, Opus)

Toward Apple's macOS design rules. `Theme.mac` (set in src/platform_ui.rs from `cfg!(target_os = "macos")`; debug
builds can force it with `SHEAF_TEST_MAC_THEME=1` to check the Mac metrics in a snapshot elsewhere) and
`Theme.system-accent` (macOS only, `+[NSColor controlAccentColor]` in sRGB, read once at start; not compiled here).
Anything that moves or resizes controls is macOS-only: the GUI smoke clicks at fixed places.

All platforms (colours, states, bugs):
- Accent: `accent-pressed` is a shade of the accent (darker 20 % light, brighter 20 % dark) instead of black/white.
  On macOS with the default accent choice, the accent is the system accent with hover, pressed, menu and option
  selection derived from it, white text on it; elsewhere Sheaf red unchanged.
- Tab bar: Home tab and "+" take the full tab height (Home sat higher and clipped); a tab's name stops before the
  unsaved dot as before the ×.
- Check boxes, radio cards and the position grid take the accent (were green beside a red accent). Smoke 305
  (`tick_x`, was `green_x`) now looks for the accent red.
- Focus ring of check boxes and secondary buttons only for keyboard focus (Tab); a click still focuses them.
- Buttons: labels weight 500; a disabled primary is grey on grey (pill-bg, text-disabled), not white on grey.
- Sidebar menu: the selected row is the tint and the accent icon; plain weight and text colour; no 4 px bar
  (`Theme.menu-bar` removed with it).
- Tool card hover: soft border (option-border) instead of black/white.
- InfoBox `warning`: amber box (`warning-bg`, new) with the warning triangle in `Theme.warning`; the four warning
  notes of options.slint use it ("This PDF looks scanned", OCR unavailable ×2, LibreOffice missing).
- Status colour not alone: result notes title and the result warning get a warning triangle; actions' failure
  lines a red triangle.
- Option footer "Runs on this computer…" wraps (placed by hand as InfoBox's; a flexbox row cannot follow a
  wrapped height).
- Segments: unselected background is the field colour (stands out in dark mode); `option-selected-bg` is a tint of
  the chosen accent (was always red-tinted, even with a blue accent) and in dark mode with Sheaf red #4A2A27,
  further from the card colour. A segment is never narrower than its label + 8 px (+ 24 px on macOS).

macOS only:
- Metrics: fs-body/fs-button/fs-menu/fs-tab 13 px, fs-card 13 px, fs-section 15 px, fs-page-title 22 px;
  button, field, combo and segment heights 28 px; menu-item 30 px; action 36 px; grid-spacing 20 px;
  card-tile-top 12 px; menu-gap 12 px; preferences Section padding 20 px.
- Check box labels too long for the panel end in an ellipsis instead of widening the row past its edge.
  Wrapping (asked) is not possible as the widget stands: Check sits in FlexboxLayout rows (dictionaries list,
  reader sheets) and Slint 1.18 reports a binding loop for a height that follows the width there. Kept
  macOS-only because the smoke clicks option panels' boxes at fixed places (2-documents-and-tools.sh:88).
- Preferences ▸ Accent colour: the first swatch shows the system accent.
- Preferences segments sized to their labels (`Segment.fit-width`) instead of fixed 90–254 px widths: many smoke
  steps click preference rows by coordinates, not only through `pref_find`.
- Encrypt with certificates sheet as tall as its content (the smoke clicks its buttons at fixed places in the
  760 × 680 sheet, 4-flows.sh:320).

Skipped: Versions dialog emphasis (in reader files, not this pass's). Seen, not changed: in dark mode on macOS the
selected segment's blue label on its blue tint reads weakly; Linux option labels wider than the panel still clip.
## C-platform-001 Check for updates (28/10, Opus, vpsn)

**What they do.** iLovePDF Desktop: Help ▸ Check for Updates (`MainPanel.qml:1774–1791`), "Checking for
updates...", then "iLovePDF Desktop is up to date" / "Update Available — There are some updates available. Do you
want to update now?"; on Windows its update manager installs in place, elsewhere it opens ilovepdf.com/desktop; a
preference "Check for updates when starting the application" and Help ▸ Release notes. Acrobat: Help ▸ "Check for
Updates..." (`Primary.txt:785`), "Checking for updates..." (3148), an Updater preferences panel with a frequency
(17301, AvUpdaterPanelName).

**What Sheaf does.** Check for Updates… in Help (Windows, Linux) and in the app menu after About on a Mac
(`mac_menu.rs`, put back after every Slint menu rebuild, as Settings…). Core `sheaf_core::updates`: GET
`api.github.com/repos/MeNass89/sheaf/releases/latest` over the existing `ureq` (User-Agent `Sheaf/<version>`, GitHub
media type, 20 s timeout); drafts, prereleases and prerelease tags ignored; semver compare (`0.10.0 > 0.9.9`,
`1.0.0 > 1.0.0-rc.1`); this computer's download picked by extension and processor (`.dmg` on macOS; `.msi`, `.exe`,
then the portable `windows` zip; `.AppImage`, `.deb`, then the `linux` tarball; a file naming another processor is
never chosen, one naming ours beats a universal one). The dialog: checking → up to date ("Version 0.2.1 is the latest
version", or no release published yet) / Sheaf X is available, the notes as plain text (Markdown read by
pulldown-cmark), the file Download opens with its size, Skip This Version, Later, Download (opens the file, else
the release page, in the browser; nothing installed) / an error said plainly (offline, GitHub's rate limit with the
time it lifts, an HTTP error, an unreadable answer) with Try Again. Preferences ▸ Startup ▸ Updates: Only when I
ask (default, DECISIONS 28/10: C-platform-013's offline promise) / Once a day, plus Check Now; the same choice as a
tick box in the dialog. Daily check at launch, counted when it starts; a newer, not-skipped version shows a small
notice bottom right (Not Now / See What's New), never a dialog by itself. Debug builds read
`SHEAF_TEST_UPDATES_URL`. French for every new text.

**Tests.** `sheaf-core` unit (`updates::tests`, 6): tags parsed, compare, GitHub answers (404, draft, prerelease,
403 rate limit with reset, 429, 5xx, garbage), outcome, asset choice per OS/arch including today's package names,
notes to text. `sheaf-core/tests/updates.rs` (3): a mock GitHub on a local `TcpListener` thread — a newer release
found with this computer's download and the request's User-Agent and Accept checked; same version and 404 up to
date; 403 rate limit, 500 and a closed port (offline). `sheaf-app` unit (`updates::tests`, 4): the daily check off by
default then once a day (clock set back), skipped version quiet but a newer one told, sizes, old prefs load with
the check off; `tests/history.rs` round-trips the new prefs; `every_interface_text_has_its_french`.

**Not verified here.** `mac_menu.rs` is compiled only on macOS: not compiled on vpsn (the MBP is the place:
the item after About, still there after a menu rebuild, opening the dialog). No GUI smoke step (one Xvfb run by
hand, below). The real GitHub address is not called by any test (no release published yet: it would say "No
release has been published yet").

**Run by hand (Xvfb :157, debug build).** A mock GitHub (python http.server answering v9.1.0 with a
linux-x64 tarball) at `SHEAF_TEST_UPDATES_URL`, prefs with the daily check on: at launch one GET
`/repos/MeNass89/sheaf/releases/latest` with User-Agent `Sheaf/0.2.1`, `last_check` written; the notice "Sheaf 9.1.0
is available." bottom right with Not Now / See What's New; See What's New opens the dialog: "Sheaf 9.1.0 is
available", "You have version 0.2.1.", the notes as text (headings, bullets), "Download opens
Sheaf-9.1.0-linux-x64.tar.gz (58 MB) in your browser", the tick box on, Skip This Version / Later / Download. The
notice's text was elided at 440 px: widened to 540 px.

**Self-grade.** F 9 · A 8 · P 9. A 8: the dialog and notice read well, but the menu item on a Mac is unseen and
there is no GUI smoke step in the suite yet.

## BAMA on every platform (29/10/2026, Opus)

Nassim (29/10): the BAMA design pass applies on every platform, not only macOS. The `Theme.mac ? a : b` and
`PlatformBridge.mac ? a : b` conditions that only kept the Linux smoke's click coordinates are gone: the Mac value
is the value everywhere. `SHEAF_TEST_MAC_THEME` is removed (nothing left to show: the metrics are the same
everywhere, and the system accent is read only on macOS).

Now on every platform:
- Type scale: fs-body, fs-menu, fs-button, fs-tab, fs-card 13 px; fs-section 15 px; fs-page-title 22 px.
- Heights: buttons, fields, combos and segments 28 px; menu items 30 px; actions 36 px. Grid spacing 20 px,
  card-tile-top 12 px, menu-gap 12 px; preferences Section padding 20 px.
- Check box labels too long for the panel end in an ellipsis.
- Preferences segments sized to their labels (`fit-width`), including the two rows added after the pass
  (Startup ▸ Updates, Plugins On/Off).
- Encrypt with certificates sheet as tall as its content.
- Reader toolbar: folds the edit group below 1320 px and shows one page-theme button (at 1280 px Save was cut off
  on Linux too). SmallButton takes its label's own width.
- Grey canvas behind the pages (#E6E6EB light, #1C1C21 dark), page shadow 6 px #00000040.
- Text format bar: the hairlines between its groups; the font chip's own width.
- Print sheet: the button row spans the card under the preview; labels trailing-aligned at 74 px, 6 px apart.
- Redact panel: Mark whole page on its own row, the pages field under it.

Changed while checking (real bugs the smoke showed):
- Segment: never narrower than its label + 8 px, 12 px a side when the row has room (`preferred-width`). With a
  hard +24 px minimum, Watermark's Place Text | Place Image | Remove ran past the 248 px options panel ("Rem"
  clipped) and Layer's "Below the content" past its edge; on macOS too with a long translation.
- Reader toolbar `z: 1`: the grey canvas, drawn after the toolbar, hid the tips hanging below its buttons ("Use a
  tool on this PDF").
- Shapes options rows: 4 px between buttons (Fill, Loupe, Mask, Article and Polygon, Cloud, Polyline touched).
- Document Properties ▸ When opened: labels 56 px ("Window" was clipped at 48 px).

`mac` conditions kept, and why:
- main.slint menus: Quit, Preferences… and Check for Updates… in the app menu on a Mac (mac_menu.rs), in File, Edit
  and Help elsewhere; Minimize and Zoom in Window on a Mac only; Import from iPhone or iPad / Scanner in File (Mac
  features).
- reader.slint: the Done button's tip "⌘↩" / "Ctrl+Enter" (key symbols).
- Theme.mac: only `follows-system` (the system accent, macOS `controlAccentColor`; Sheaf red elsewhere) and the first
  accent swatch in Preferences showing the system accent.
- Continuity Camera: recents.slint (From iPhone or iPad), sign-panel.slint (Camera and iPhone row and its hint),
  reader-panel.slint (Insert from iPhone or iPad…).

Smoke recalibrated (positions measured on the new screenshots, else found by OCR):
- gui-smoke.sh: ZOOM_X 705 → 750, ZOOM_MENU_X 736 → 778 (one page-theme button); `at` and `span` also match a
  word OCR read with punctuation before it (" mirrored.pdf", "(Podcast", "[Remove]", "(off").
- gui-parallel.sh: picks the first free block of displays from 140 (another worktree's run on 140–143 would have
  had its Xvfb stopped by ours).
- 1-reader: 2c text format row (Mono-relative places for Serif, B, +, and the text box's I, align right, size);
  6g zoom read at ZOOM_X.
- 2-documents: tool search and Word files opened read the panel's title / compare without spaces (13 px bold tab
  titles OCR badly); watermark preview's text, size and Red found from the "Font" label and by word; Watermark ▸
  Remove by word; the Link card check reads $LE inside the eval (OCR's "$e" broke it under `set -u`); the edit
  group's Redact… through the folded edit button (413,61) and its menu.
- 3-flows: recent searches read the 88 px search field; When opened's Cancel read on a wider strip; the initials
  pad's Cancel read on the pad's lower part, whatever the styles typed push (label shows field and Use; a shot
  65b-initials-typed).
- 4-flows: dashed button from "10"/"100" (−48 px); U and • from Mono (+129, +183); split chip read on the chip
  alone (80 × 26) and N0/N1 inside the eval; Check Spelling's Change All from Ignore (+96); JavaScript console's
  Run from Scripts (−203); portfolio's Save from Add (+149); Actions page (sidebar crop, search field 330,322,
  filter 340,286, lists read from y 335, Save action strip); Encrypt with certificates (rows from the title, Save
  from Close read on a column strip); print defaults (Grey, On, Colour, Make default by word); Startup row from
  "log"; Report an Issue's Save read on the dialog's lower half; tick boxes' Include likewise; the card ID's "Use
  a digital ID" by "digital"; plugins' Off also as "off"; system display profile by "system's"; PDF/A tries the
  wrench at WRENCH_X first.

Screenshots looked at (1280 × 800, Xvfb): Home (Recents), Tools, Compress options, Preferences light and dark,
the reader with its toolbar (light and dark), the text format bar and the text box row, the edit group's menu,
Shapes options, Watermark options, Redact PDF, Print sheet, Encrypt with certificates, Document Properties,
Initials pad, Check spelling, JavaScript console, Create PDF Portfolio, Report an issue, AI panel, Actions page,
Startup and Plugins preferences. Seen, not changed: a right-edge tool tip runs past the window ("Use a tool on
this PDF" cut at x 1280); Check spelling's Stop sits on its own row; the AI panel's Import space… wraps under New
space.

Verified: `bash scripts/test.sh gui` "PASS gui" / "GREEN (layers: gui; not a full run) (972s)" (five shards, displays
150–154 picked beside another worktree's run); `cargo.sh clippy -p sheaf-app --all-targets -- -D warnings` and
`cargo.sh test -p sheaf-app` green. "a bookmark is renamed in place" failed once while clippy and the tests ran
beside the GUI run, passed alone and in the last run (load, not the change).

## C-platform-012 native builds on the three systems (29/10, Opus)

Notarisation out of scope (DECISIONS 28/10). New workflow `.github/workflows/build.yml` (push to build/*, or Run
workflow): run 40851924338 on build/r40e — macos-latest (Xcode 26.6) Sheaf-0.2.1-arm64.dmg 110 MB, windows-2022
zip 123 MB, ubuntu-24.04 tarball 120 MB, each smoke-tested (scripts/smoke.sh: info, render, merge, ocr) and kept as
artifacts; nothing published. Bugs the first runs found and fixed: fetch-fonts' sparse-checkout patterns rewritten
by Git Bash on Windows (no fonts fetched); onnxruntime's CoreML version checks needing clang's runtime when the
release targets macOS 11 (`___isPlatformVersionAtLeast`, pdfcraft-ocr build.rs); Xcode 15's actool unable to read
AppIcon.icon (newest Xcode on the runner; the icns alone otherwise); the home-path leak check counting
onnxruntime's own build paths. Also built on the MBP (Xcode 27.1): the DMG opens, the app launches from it with its
menus, Assets.car (dark-mode icon) present, CLI smoke ok; copied to iCloud temp as Sheaf-0.2.1-r40-arm64.dmg.

## Redesign W1: tokens, widgets, tooltip layer, side-panel width (29/10/2026, Opus)

DESIGN.md §4 and §6, DESIGN-PLAN W1. Branch wip/w1-tokens (from dev/phase-4-5 + wip/design-spec).

- `ui/theme.slint`: the token table (colour roles window / chrome / canvas / hairline / fill-1..3 / field /
  text..text-3 / status / tooltip / toast, accent-text, on-accent, focus-ring), type scale with leadings, radii
  r-xs..r-xl, shadows, metrics, side-column widths; globals `Space` and `Motion` (every duration 0 under Reduce
  Motion). The old names stay as aliases pointed at the new roles, so every screen already wears the new
  palette; W2–W5 migrate to the new names, the aliases go after them. Accent: Sheaf blue first (the system's
  on macOS), Sheaf red second; `Theme.accent-swatches` feeds Preferences' row.
- Tooltip layer `widgets/tooltip.slint`: `TooltipBridge` (hover / show-now / leave / cancel), `TooltipLayer`
  last child of `AppWindow` (and of the gallery), `TipArea` for new widgets. 600 ms rest, warm 300 ms, out
  60 ms, a press cancels; below the anchor, flipped above past H − 8, x clamped to [8, W − w − 8], rails to the
  right (flipped left), max 280 then wraps, shortcut in `text-2`. ToolButton, PanelTab, SmallButton,
  IconButton, the collapsed Home menu and the link destination all go through it: no control draws a tip.
- Widgets: `toolbar.slint` (ToolbarButton, ToolbarText, ModeSegment, RailButton, SidebarTabs),
  `button.slint` (Button with secondary / primary / destructive / large, keyboard-only focus ring),
  `controls.slint` (PopupButton, Checkbox, Radio, Switch, Chip, Swatch, InspectorSegment, FocusRing),
  `surfaces.slint` (Popover, MenuRow, ContextMenu, Sheet, SheetButtons, ToastView), `home-parts.slint`
  (ToolTile, DropArea), `resize-handle.slint`; SortCombo, MenuItem, DashedBox, tab strip restyled.
  `widgets/gallery.slint` + SHEAF_GALLERY=1 (debug): every widget over the 8 px grid; a snapshot name starting
  with "gallery" draws that window.
- Tab strip: 38, `chrome`, Home pinned first 120 wide, tabs 240 → 120 (then on to 72 rather than scroll, so
  twelve tabs stay in sight: a small departure from the spec's 120 floor), monochrome icon, × on hover and on the
  selected tab, the selected tab `window` with hairline sides, one hairline at every boundary.
- Side panel: `ResizeHandle` (8 wide, centred on the hairline, col-resize, accent while hovered or dragged,
  double click = default), 200–480 and at most 45 % of the window. Narrow (< 240) the page actions fold Extract
  and Number pages into ⋯ (Delete stays), the Notes "Hide all" becomes its eye icon, the two Notes popups share the
  row (88–112 each): nothing clipped at 200.
- Default width stays **248**, not the spec's 260: at 260 the handle (to x 264) took the smoke's drags that start
  at x 260 on the canvas and every canvas click moved 12 px; W3 re-lays the reader and recalibrates those
  clicks anyway, so it flips `Theme.sidebar-default` and `reader_ui::PANEL_WIDTH` together.
- Outside W1's files, the minimum: reader.slint (link tip through the layer; panel `z: 1` so its handle is over
  the canvas; `window-width`), reader-panel.slint (handle, folds above), preferences.slint (swatch list).
- `platform_ui.rs`: Reduce Motion (macOS NSWorkspace, GNOME enable-animations, SHEAF_REDUCE_MOTION), gallery.

Smoke recalibrated: 1-reader 6i (the link tip read by OCR "Page 2" under the pointer: the tip is light now, a
mean-brightness test no longer sees it); 2-documents `ntabs` (counts the strip's boundary hairlines at y 10, the
purple tab tiles are gone; used by 4-flows too); 4-flows `red_at` and `tick_x` (the accent is blue), the preview
banner OCR in one-line mode (`--psm 7`: tesseract dropped "Preview of no" where the banner straddles the page edge).

Screenshots /tmp/f5/w1/ (self-snapshots, Xvfb :128–:130): reader light/dark 1280 and 1024, gallery light/dark,
tips (trash above the bar, right edge clamped at 1280 and 1024, panel tab over its separator, Notes toolbar over
the popups, rail to the right), narrow panel 200 (Notes, Pages), wide 480, reset.

Second pass (29/10, to green): one UI fix, placeholders of `widgets/text-field.slint` in `text-3` (they used
`text-disabled`, which the token table made fainter: the recents search, Redact's page range and AI's categories no
longer read). The selected tab keeps 140 for its name among many. Smoke lookups recalibrated on the new chrome, no
assertion changed: tab × at 342 / 582 and the dot corner at 560 (240-wide tabs after the 120 Home tab; 54, 76),
the first document tab by place (34), the tab menu read on the menu alone (79), twelve tabs' active name alone
(12), Run again by "Run" and the tool tab's files read inside the dashed edge (10b), Library's Add files… through a
new `at_on_accent` helper in gui-smoke.sh (white on the accent), the plugin's Allow and run on the dialog's lower
part (307), the print sheet's title on its top row (300), Redact's mark row from its heading down (77).
GUI runs only under `flock /tmp/f5/gui.lock`. Default panel width stays 248 (W3 switches to 260).
## Redesign W2: Home / library (29/10/2026, Opus)

DESIGN.md §5.1, DESIGN-PLAN W2. Branch wip/w2-home (from wip/w1-tokens). Matched against
docs/design/mocks/home.png: same rows, same columns, a few px apart at most.

- `ui/screens/home-kit.slint` (new): `HomeMetrics` (source list 220 / 52, margins 32, top 28, tiles and cards
  152, gaps 16), `SourceRow`, `PageTitle` and `SectionHeader` (Flexbox rows: right to left they read from the
  right), `LinkButton` (View All), `EmptyState`, `LibraryTile` (W1's ToolTile + the star + "Soon"),
  `TileGrid` (as many columns as fit; `one-row` narrows the six of Tools for you down to 104 before wrapping),
  `HomeFileCard` (152×180, page 96×124, "7 pages · 2 h ago", hover Show in Folder / star / Remove, right-click
  menu), `FileGrid`, `OpenArea` (dashed accent box, Open Files capsule + New ⌄ menu: Blank Document, From
  Clipboard, From Screenshot, From Scanner…, From iPhone or iPad… on a Mac), `SearchField`, `SortMenu` (Newest,
  Name, Size, then Clear Recents).
- Source list (main.slint `Sidebar`): Recents, Favourites, Library, Tasks, Actions, All Tools; Help at the
  bottom (opens the guide). No Preferences row: ⌘, (and the menu bar) open it, still as a Home page until W5's
  window. Icons only under 1120 wide (so 1024 keeps five cards a row, the reader's sidebar docking width).
  `Page` enum: `tools` → `all-tools`, `favourites` added (Rust `Page::AllTools`; reader_ui/saving.rs's one
  reference renamed).
- Recents: title + Customize, Open area, Tools for you (six tiles, View All), Recent files (search, order),
  Favourites (starred files, then starred tools). Home's old "Recent tools" row is gone (history.rs
  `recent_tools` deleted with it). Favourites page: files then tools, empty state. Customize sheet: three
  section checkboxes and six popups; saved in prefs `home_tiles` / `home_hidden`; Restore Defaults.
  `HomeBridge` global carries tiles, choices, sections, the starred-file count. The Annotate tile is not a
  catalogue tool: it opens the Open dialog (the reader). Default six: Edit, Annotate, Organize Pages, Convert
  to PDF (JPG to PDF), Sign, Compress.
- All Tools: title, Custom Tools…, search 240; category chips; monochrome tiles by category, Favourites first;
  search results as tiles with the chosen one framed (↑ ↓ ↩ as before).
- Tasks (was Tasks History), Library, Actions: Home's margins and title row, W1 buttons; Tasks rows are a list
  with a monochrome glyph; Clear History… confirms in a W1 Sheet.
- Tool tab: the options panel is the inspector material (`window`, hairline), its title `fs-title-3`, the Run
  button the panel's one accent fill (32 tall, same place: the smoke's 1140,755 still lands on it).
- 32 strings in fr/de/es/it/pt (and "Organize Pages", "Compress").

Smoke recalibrated: gui-smoke.sh `RECENTS_ROW`, `ALL_TOOLS_ROW`, `TOOLS_SEARCH`, helpers `tile NAME` (an All
Tools tile by OCR) and `home_new WORD` (New ⌄ then its item by OCR); `jpg_to_pdf` through them. 2-documents:
9 (⌘, instead of the Preferences row), 9b/9c (All Tools row, the Compress star found from its name), 10 (the
Tools for you tile's star, the recent card's star 138 above its name, search fallback 140 left of Newest,
`at Tasks`), 11 (⌘,). 3-flows: 21c/21d/21e/22 (tile Merge/Split), 26/71 (search field), 28/36 (`prefs_open` =
⌘,), 45/45b/75 (`home_new`). 4-flows: 81/90/100/101/330 (`home_new`, tile Merge), 3xx wrench searches, 306
(French source list reads "outils" and "Favoris", English "Tools" and "Favourites": Preferences has no row),
300 (the RTL title row read in its own 44 px band: a whole-width strip OCRs as noise), 350–356 (Actions' rows 42 px
higher: New action, New custom tool and Run on PDFs… joined the title row; the crops under a scroll to the editor's end and
the run report at the foot keep their places; 353 reads the prompt's Continue, now its primary button, with
`at_on_accent`, and waits for it). `tile_star X Y`: a tile's star from its
name, by the 168 pitch (a two-word name's first word is off the tile's centre).

Screenshots /tmp/f5/W2/ (self-snapshots, Xvfb :131): home light/dark 1280 and 1024, All Tools, Customize,
Tasks dark, a tool tab.

## Redesign W5: the Settings window (29/10/2026, Opus)

DESIGN §5.8, DESIGN-PLAN W5, BAMA settings.md. Branch wip/w5-settings (from wip/w1-tokens).

- Settings is its own window (`ui/screens/settings-window.slint`, `src/settings_window.rs`): 520 wide, six icon
  tabs 72×44 (General, Appearance, Reading, Tools, AI, Shortcuts; the chosen one on `fill-2`), one form per tab on
  the `chrome` page, cards (`window`, `r-md`, hairline) with a small-capital title, rows 36 tall with the label as a
  statement and the controls on the trailing edge (`settings-form.slint`: SettingsCard, SettingsRow, SettingsBlock,
  SettingsSwitch, SettingsSwatch). Applies at once; as tall as its tab up to 640, then it scrolls; reopens on the
  last tab (for the session); nothing has a focus ring at open. Title: the tab on macOS, "Settings" elsewhere.
  ⌘W / Escape close it, ⌃Tab steps the tabs.
- Rows regrouped: General (startup and updates, language and direction, where results go, default PDF app),
  Appearance (System / Light / Dark radios, six accent swatches with the system's first on macOS, background behind
  pages, smoothing and boxes as switches, display profile sRGB / The system's / Other…, graphics card or processor),
  Reading (zoom, layout, reopen position, page numbers, units, guides, stamp dates, author name, spelling), Tools
  (Tools page, printing defaults as rows, security, digital signatures and time stamp servers, plugins), AI (the
  existing AI card), Shortcuts (the existing list). Choices are radios (2–3) or popups (4+), never segments.
- Slint gives each window its own globals: the Settings window mirrors the main window's (Prefs, SettingsBridge,
  PlatformBridge's settings members, AiBridge's pref members, PluginBridge, Picked.tsa-servers, UpdateBridge.auto,
  Theme) after each of its callbacks (each handed to the main window's) and every 250 ms while shown. A new setting
  in a tab must be added to `sync` and `forward!` there.
- Opens from ⌘, (app keys and the reader), Sheaf ▸ Settings… (macOS, `preferences-asked`), Edit ▸ Settings…
  (renamed from Preferences…), Home's Preferences row (W2 removes it), Help ▸ Customize Shortcuts… (Shortcuts tab),
  and the AI panel's Open AI settings… (AI tab; it set a Home page that no longer exists). The Preferences page and
  `Page.preferences` are gone; the legacy sections left (Security, Plugins, Keyboard, AI) wear the new card.
- Self-snapshot: a name starting with "settings" draws the Settings window (open it first with Ctrl+,).
- Not done in W5 (time): the Print sheet at 864×640 with its preview column, Export / Protect / Sign sheets on the
  `Sheet` widget, the 360 dialogs and the toast's new place; "Page theme when a document opens" (needs a pref
  setter in reader_ui); texts still saying "Preferences" in other streams' files ("API key in Preferences",
  "Add a time stamp server in Preferences ▸ Signatures.").

Smoke recalibrated: helpers in gui-smoke.sh (`settings N` opens the window on tab N and reads its geometry, `sfind`
scrolls its form to a word, `sswitch`, `skey`, `stype`, `sclose`); 2-documents 9 (Reading ▸ Author name), the ⌘,
check (the window and its tab bar), 11 (Appearance radios); 3-flows 28 (Reading radios), 36 and its reset
(swatches 26 apart from the trailing edge); 4-flows 99, 221, 223, 225, 229, 230, 301, 302, 303, 306 (Language
popup), 307, 308, 321, 341, 364c (Open AI settings… shows the AI tab), 371; 226 and 228 (Edit menu on a text field)
use Home ▸ Tools' search: the main window's menu bar acts on the main window, Settings has none on Linux.
Step 28's Fit page is clicked beside Actual size and now also asserts `default_zoom` 1 (its OCR miss had left
Actual size on, the under-120 % check still passed, and step 33's two pages sat off the points it reads); 33 waits
up to 5 s for the pages under load; 302 finds the caption "Trusted locations" (its notes also say "locations" and
"folder"). A real bug the smoke found: the time stamp server field had collapsed to nothing (a FlexboxLayout gives
horizontal-stretch room only with `alignment: stretch`). GUI layer GREEN on 215791c (325+ checks, four shards).

Screenshots /tmp/f5/W5/ (self-snapshots, Xvfb :151–:153): every tab light and dark, the screen with Settings open at
1280×800 and 1024×768.
## Redesign W6: the Tools popover, the sheets and dialogs, the leftovers, the audit (30/10/2026, Fable)

DESIGN.md §1 (command map), §3, §5.5, §5.9, §5.10; what W1–W5 named as not done. Branch wip/w6-finish (from int/redesign 09d7aff). Target dir
`~/.cache/cargo-target/sheaf-w6` (the finished W3 dir renamed: a fresh copy filled the disk; W4's dirs went with it).

- **Tools popover** (`ui/screens/reader-tools.slint`, `src/reader_ui/toolbox.rs`): 320×480 under the mode bar's wrench (⌘K, Tools ▸
  All Tools…), its search field with the keyboard as it opens, the rows from Rust for what is typed (`ReaderBridge.tools-query` →
  `tool-rows`): nothing typed, Favourites, then the catalogue's categories (only the tools that take a PDF: the converters to PDF
  stay on Home ▸ All Tools), Plugins, Custom tools (ActionBridge.commands that apply to the open document); typed, the
  catalogue's search order (`sheaf_core::tools::search`, so "cmprs" and "bates" still find their tool) then the plugins and custom
  tools holding the words. ↑ ↓ move (headers skipped, the list scrolled to the row), ↩ chooses, Escape closes; the current row is
  the accent fill, as a menu's.
  - A chosen tool goes through `toolbox::pick` → `to_tools` (the document saved first, as the Tools button did) → `show`: a plugin
    runs on the file (`plugins_ui::open_with`), a custom tool through `custom-current`, a tool that needs the page (Organize, Edit,
    Sign, Redact) or more files (Merge, Compare) opens its tool tab with the document in it, every other catalogue tool shows its
    options in the **inspector**: the reader's own `TabState` carries the tool (`tool`, `files`, `options`, `phase`), so the panel
    is `OptionsPanel` (options.slint) itself, keyed on `reader.tool-id` (the row's `tool` string stays "read"), with one pinned
    **Run** (`ReaderBridge.run-tool` → `run::run_tool`, the same job path) and a progress bar + Cancel while it runs. The inspector
    opens by itself at 292 wide (the tool tab's panel width, so every option sits where the smoke measured it; DESIGN §5.5 allows
    220–340), its × or Escape closes the tool (`toolbox::close`). Done (`run::on_done` → `toolbox::finished`): the result opens in
    its own tab (a PDF) or is shown in its folder, a toast says "Compressed to 480 KB (64% smaller!)" or the tool's result title,
    the reader's tool state is cleared. **Deviation from §3**: the result does not replace the document in its tab with an
    "Undo Compress" (that is a document-level undo of a whole rewrite; the output file opens beside the original instead, as the
    tool tab's result page did).
  - The menu bar reaches every popover tool by name: File ▸ Export To ▸ (Word, Excel, PowerPoint, Images, Text or HTML, PDF/A),
    Edit PDF ▸ (Page numbers…, Watermark…, Adjust colours…, Crop…), Pages ▸ (Extract…, Remove…, Split…, Rotate all…), Protect ▸
    (Set / Remove password…), Tools ▸ (Compress…, OCR…, Repair…, Compare…), Forms ▸ Sign with Certificate…. The batch tool tab
    stays behind Home ▸ All Tools (decision taken).
- **Menu bar** (main.slint, DESIGN §1 and BAMA menus.md): File, Edit, View, Go, Annotate, Edit PDF, Forms, Pages, Protect, Tools,
  Window, Help. View: Show/Hide Sidebar and Show/Hide Inspector (labels follow the state; the reader reports `inspector-open` to
  Rust), Sidebar ▸ Thumbnails…Layers (⌥⌘1…6), Page Display ▸, Page Theme ▸ Day / Sepia / Night (⌥⌘D ⌥⌘S ⌥⌘N). Go ▸ Add
  Bookmark ⌘D. `platform.rs` COMMANDS 45–55 added (inspector, six sidebar tabs, add-bookmark, three themes); Document Properties
  moved from ⌘D to **⌘I**; Next Page Layout from ⌥⌘2 (now Outline) to **⌥⌘L**. The reader's key scope answers the same keys when
  the menu bar is hidden (the smoke), with the Mac's ⌥-symbols (¡ ™ £ ¢ ∞ § ∂ ß ˜ ¬). ⌃⌘I (Ctrl+Alt+I) now works while the search
  field has the keyboard (its key handler toggles the inspector).
- **Mode bar: icons only, decided.** Both PDF Expert captures (05-annotate-toolbar.jpg, 07-edit-mode.jpg) show a mode bar of
  icons without words; the spec (§2, §5.2) sizes it 7×48; BAMA toolbars.md says a symbol first, text only when no symbol fits. Seven
  labelled segments (≈ 90 px each) would not fit between the page field and the "Aa" button at 1280 with the sidebar open (488 px),
  and a label on the selected segment alone would shift the row on every mode change. Where am I is answered by the `fill-3` pill,
  the rail, and the inspector's header naming the mode. Every segment has its tooltip and accessible label with the shortcut.
- **Annotate inspector** (reader-inspector.slint): one section per property, the control alone under its title (the "Colour /
  Colour" doubling is gone): Colour as W1 swatches, Stroke as four widths, Opacity as a slider with its value. Read and Organize
  sections carry no title (the header already says Document / Pages). Organize: "3 of 7 selected" and a **Thumbnail size** slider
  (61–163 % = 120–320 px of the grid's 196-wide page; `ViewerBridge.zoom-to`).
- **Print sheet** (reader-sheets.slint, DESIGN §5.9, 12c-print-dialog.png): 864×640, a 320-wide preview column on `canvas` with the
  page and a "Page 1 of 7" capsule (‹ › and ← → step the sheet: `print::preview` takes the sheet now, `images::page_png_bytes_at`),
  the form on the right as rows (Printer, Copies, Pages, Paper size, Orientation, then a "Sheaf" group: Scale, Layout, Two-sided,
  Colour, Comments) with W1 popups, radios and checkboxes; Make Default left, Cancel · Print right; Return prints, Escape cancels.
- **Dialogs** (dialogs.slint `Modal`): 360 wide, `window`, radius 12, the `sheet` shadow, rising 8 px; title 13/600, message 13
  `text-2`, buttons 8 apart; `SecondaryButton` on the token table (every dialog built on Modal follows).
- **Toast** (main.slint, surfaces.slint `ToastView`): bottom centre 20 above the edge, 36 tall, an **Undo** word
  (`flow::toast_undo`, the active document's undo); "3 pages deleted · Undo" on a page delete (pages.rs).
- **Export / Protect / Sign sheets: not built as sheets.** Their options are the tools' panels, which now open in the inspector from
  the popover and the menus (one surface per tool; a 480 sheet would be a second copy of options.slint with its own Run). Noted as a
  deviation from §5.9.
- **Settings ▸ Appearance ▸ Page theme when a document opens** (Day / Sepia / Night popup, `Prefs.page-theme`, `home::set_page_theme`;
  the reader's own theme change pushes the pref too). Every "Preferences" text now says Settings (plugins, security, time stamp,
  the AI errors in sheaf-core); the guide's Markdown and PDF regenerated (`make-guide.sh`).
- Smoke recalibrated: `key ctrl+d` → `ctrl+i` (properties, 5 places), `ctrl+alt+2` → `ctrl+alt+l` (page layout, 5 places), 4-flows 340
  and 344 no longer click the old page's search field (the popover's has the keyboard). Every wrench flow (`click $WRENCH_X 61;
  type; Return`) now lands in the popover: the same first result, then the options at x 1004–1264 and Run at (1140, 755) as before.
- 77 new texts in fr/de/es/it/pt (`missing.py` reports 0).
- The GUI layer caught three real bugs on the way to green: a document tab's row never ran `fill_result`, so the
  result toast had no words; a wrapping `Text`'s preferred width is its longest word's and an invisible action
  still took the toast row's spacing, so short toasts broke their file name ("moved- away.pdf"); a non-PDF output
  opened a file manager over the window (now the toast names the file). Smoke recalibrated besides: 4-flows 220
  and 304 read the longer menu bar (1000 px), 250 finds Run under Clear output, 300 and 301 the new print sheet
  (title band, radio dot 30 px left of Grey, the two-sided box 95 px right of its label, Make Default read on its
  own row), 320 the 360 dialog's error line, 364 "API key in Settings"; 3-flows 39 opens Rotate's tool tab from Home
  (the inspector has no card to measure), 66 reads the result toast inverted; 2-documents 11 the toast 20 px above
  the edge.
- Screenshots `/tmp/f5/W6/` (self-snapshots, Xvfb :170–:172): `*-1280x800-light`, `*-1280x800-dark`, `*-1024x768-light`
  of alltools, settings-general, settings-appearance, reader-read(-inspector), reader-annotate-inspector,
  reader-annotate-highlight, reader-organize-inspector, tools-popover (screen), tool-watermark-inspector,
  tool-compress-inspector, print-sheet, reader-search, toast-deleted, dialog-unsaved, home. GUI layer: PASS gui on
  7ef6f83 (fourth run; 1354 s).
