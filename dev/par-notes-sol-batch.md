# sol-batch · 2026-10-27

## Tatouages

- Worktree `/tmp/f5/w-sol-batch`, branch `par/sol-batch`; only this batch's files.
- Brief rule 1: no cargo, rustc, clippy, tests, app, browser or codex execution. The orchestrator
  owns compilation, the suite and visual verification. No score of 9 is claimed here.
- Inputs have no DISAGREE entry: all nine are phase-4 zero baselines. Their definitions are the targets.

## Polaroid

Hand-off: all nine components have implementation, unit/integration tests and GUI smoke steps
where the debug dialog hook permits them. Static whitespace/shell-syntax checks passed; builds,
test runs and UI inspection are intentionally unverified under brief rule 1.
Target: `par/sol-batch`, with one pushed commit per component and a final static close-out.
Next authorized action belongs to the orchestrator: merge the branch, refresh Cargo.lock for
app's `zip 4.6.1` dependency, compile, run the suite and inspect the new screens. Report compile
errors back to this worker; do not grade parity from this implementation report alone.

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
