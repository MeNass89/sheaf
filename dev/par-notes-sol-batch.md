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
