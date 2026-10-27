# sol-batch · 2026-10-27

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
