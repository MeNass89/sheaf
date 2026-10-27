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
