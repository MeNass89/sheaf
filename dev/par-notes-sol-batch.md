# sol-batch · 2026-10-27

## Tatouages

- Worktree `/tmp/f5/w-sol-batch`, branch `par/sol-batch`; only this batch's files.
- Brief rule 1: no cargo, rustc, clippy, tests, app, browser or codex execution. The orchestrator
  owns compilation, the suite and visual verification. No score of 9 is claimed here.
- Inputs have no DISAGREE entry: all nine are phase-4 zero baselines. Their definitions are the targets.

## Polaroid

Active: action storage/editor written; compilation and execution intentionally unverified.
Next: finish each batch component, write its proof names and remaining parity gaps here, push.

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
