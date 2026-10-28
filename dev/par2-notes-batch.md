# Round 31 — batch

## Tatouages
- Mandate: `/tmp/f5/v2/brief.md` and `batch.json`; only `/tmp/f5/w2-batch`, branch `par2/batch`.
- Serial builds/tests via the supplied wbuild wrapper (currently MemoryMax=10G); GUI only :115. Never :99 or class-recorder.
- Commit/push each component. No network features. Round 31 follows the 28 October request and supplied brief; the 27 October timed resume below is historical.
- All supplied `your_last_gap` fields refer to an empty phase-4 baseline, not a precise defect. The inspected last pass in F-015 had grades below 9; do not claim parity from the proposed grades.

## Components
- C-batch-002: Acrobat Action Wizard / Specify Settings. Watermark and header/footer use labelled text, font, size, page range, opacity, rotation and offset fields instead of pipe syntax. Existing portable arguments, text containing pipes/newlines, command order and per-step instructions are preserved. Test `labelled_mark_settings_round_trip_and_drive_the_selected_pages_and_appearance` (`src/action_settings.rs:60`); smoke 353 passes in the final round-31 run. Commit 135e756. Left: insert/split action steps, permissions encryption and independent competitor parity review. Not closed at 9.
- C-batch-003: Acrobat run saved actions. Run on PDFs is in the fixed header. A bounded result summary accompanies a scrollable list of every file failure. Folder intake runs off the UI thread and is cancellable before processing, without a partial batch. Invalid prompted settings name the tool rather than saying cancelled. Tests `cancelled_folder_intake_returns_no_partial_batch_and_can_be_started_again` and `invalid_prompted_settings_report_the_tool_without_claiming_cancellation_or_publishing` (`src/actions.rs:721`, `:736`); smoke 354/356 passes in the final round-31 run. Commit a0af0bc. Left: cancellation responsiveness and independent competitor parity review. Not closed at 9.
- C-batch-004: Stirling import/export pipelines JSON. Portable output prefix/suffix and `{pipelineName}` survive translation. `{outputFolder}` means the destination chosen for this run, as in Stirling's default fixture. Unsupported date/time, nested-folder naming and absolute paths fail explicitly. Pretty/escaped exports cannot exceed their own import limit. Tests `stirling_naming_round_trips_without_silently_dropping_pipeline_settings` (`src/action_import.rs:275`) and `every_exported_action_fits_the_import_limit_including_json_escaping` (`src/actions.rs:663`); smoke 350 passes in the final round-31 run. Commit 3d27e15 plus the portable destination follow-up. Left: unsupported Stirling tools/settings, `.sequ`, independent competitor parity review. Not closed at 9.
- C-batch-005: Acrobat suggested actions. Search matches all words across names and full descriptions; filtering keeps stable source indices. Multiline descriptions expose refusal/certification limits. Test `suggested_action_search_matches_all_words_in_names_and_descriptions` (`src/actions.rs:685`); smoke 351 passes in the final round-31 run. Commit b04d5f4. Left: independent competitor parity review. Not closed at 9.
- C-batch-006: Acrobat custom tools. Page commands are enabled without pretending the document has one page, and validate against its actual page count in the reader. The palette stores the reader's stable tab ID rather than its list index. Command labels use the real catalogue name or user label, including invalid drafts. Tests `custom_page_commands_are_available_then_validate_against_the_real_document` and `custom_toolbox_shortcuts_validate_real_tools_and_never_run_as_batch_edits` (`src/actions.rs:698`, `:615`); smoke 352 passes in the final round-31 run. Commit 30039c4. Left: persistent custom panels/icons and independent competitor parity review. Not closed at 9.
- C-batch-008: Stirling Folder Scanning. Uploads and queued nested directories can disappear without stopping other files or subsequent scans. A missing root inbox still produces an error. Tests `a_disappearing_upload_does_not_stop_other_files_or_the_next_scan` and `a_removed_nested_folder_is_skipped_between_scan_slices` (`src/watch_folder.rs:510`, `:530`); smoke 355 passes in the final round-31 run. Commit 3ed33e4. Left: OS service/background processing after app exit, notifications, scale and independent competitor parity review. Not closed at 9.

Paths above are relative to `crates/sheaf-app/`. All six component commits are pushed.

## Sources inspected
- Sheaf: actions.rs, action_import.rs, actions_ui.rs, actions.slint, watch_folder.rs, reader command dispatch, PdfCraft action engine, F-015 last sol-batch pass.
- Acrobat archive: `aapp/Actions_R_RHP.aapp` (CreateWorkflowRdr, ManageWorkflowsRdr, FindMoreWorkflowsRdr, CreateNewCustomToolRdr, ManageCustomToolsRdr).
- Stirling archive: `app/core/src/main/resources/static/pipeline/defaultWebUIConfigs/split-rotate-auto-rename.json` and `app/core/src/main/java/stirling/software/SPDF/controller/api/pipeline/PipelineDirectoryProcessor.java`.
- These sources establish feature contracts; they do not establish visual parity.

## Dependencies
No new runtime or registry package. The production-source integration harness adds dev-only `dirs` and `zip` to sheaf-core; both already exist in the workspace/app dependency graph. It includes the exact action/import/settings/watch sources without Slint, not copies or mocks. Utility commit f38a103.

## Prior verification — round 30
- Native harness, latest completed run: `test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.55s`. Proof `dev/.tmp/actions-backend-latest.log`. Includes the `{outputFolder}` assertion.
- Mechanical Slint detector: `[]`; shell syntax and `git diff --check` pass. Round-31 rendered QA and GUI proofs supersede this prior status.
- Lint: repeat required after fixing two baseline home.rs references to sheaf_app::store_file; first run failed on pre-existing Rust 1.99 `chunks_exact_to_as_chunks` in viewer/renders.rs:345. Same pixel iteration changed to `as_chunks_mut::<4>().0.iter_mut()`; existing transparency-grid regression covers rendering, not executed yet.
- Unit suite: pending.
- GUI smoke 350–356: written, not executed. No own app binary exists, so there are no PASS lines and no GUI grades can be certified.

## Prior build blockage and investigation — round 30
The worker’s app builds failed to compile within the wrapper's 6 GB cap. Own builds hit OOM at 6.0G, including JOBS=1, app-only codegen-units=256, opt-level=1, jemalloc arena/decay tuning, splitting the two large Action settings blocks into private nonrecursive Slint components, and app-only codegen-units=1 (run-u4855). The unsuccessful profile/allocator settings were removed. Generated Slint Rust remains about 48 MB. No test was disabled and no memory cap was raised.

The persistent blockage was discussed using `claude-review` (tools disabled), session 5796400c-6a70-4151-974e-7054bf53a4ff; opinion suggested frontend/MIR pressure and smaller constructors. That is a hypothesis, not measured phase attribution. Extraction was checked for static labels/callbacks/two-way binding equivalence, and the subsequent app build still OOMed. No senior review of the feature result occurred.

The suite now respects an explicit JOBS limit instead of recomputing up to 8 jobs from host RAM inside the 6 GB unit. All checks remain enabled.

## Polaroid — archived 28 October 2026
Closed for the round-31 regression repair in `/tmp/f5/w2-batch`, branch `par2/batch`: own app build EXIT 0; complete GUI 350–356 ALL PASS; lint and unit both EXIT 0 with JOBS=1. Five per-component fixes are pushed through 67ea7fb. This file archives the project's constraints, failures, verified fixes and final evidence. No source work remains for this request; the closure commit is followed by a direct remote HEAD comparison. Full competitor parity remains outside this regression verdict.

## Resume — 27 October 2026, 21:38 UTC
- Initial status: three uncommitted check/build fixes; eight commits after dev/round29.
- Merged dev/round29 as c30701d. Its home/store_file and transparency-grid fixes supersede the uncommitted duplicates; resolved home.rs to the upstream version.
- Retained scripts/test.sh JOBS override so suite runs respect the worker resource limit. Shell syntax and git diff --check passed.
- Required app build launched through the supplied wrapper. Remaining checks and verdicts will be recorded below.

## Round 31 — active
- Mandate: merge `dev/round30`, build serially with the supplied wrapper, run all smoke 350–356 together on :115, fix causes without weakening assertions, pass lint/unit, commit and push `par2/batch`.
- Merged `dev/round30` (d5c3a54). Preserved existing notes; upstream already carries the JOBS override, so kept its script without a duplicate change.
- Round-30 screenshots prove several calibration defects: Export Stirling is at y256; Search starts x230 at y345 (old crop clipped its first letters); filter y305; editor labels start x460; Apply prose at y326 precedes the actual button at y403. Correcting these targets retains every output assertion.
- Build attempt 1: wrapper session 21891 returned EXIT 101 after waiting for a slot, with an empty build.log. Live `df` proved the shared 106G disk full (0 available). Before cleanup, free space returned to 4.1G; no other worker files were removed. Retried the identical authorized wrapper only after verifying this changed external state: session 24322. Next run whole 350–356 on :115, inspect fresh screenshots and correct remaining causes.
- Static calibration check: actual round-30 captures, cropped with the new bounds and OCR at 200%, recognize Export, Search, Apply and Include. A populated search no longer has its placeholder, so the second query reuses the first observed field position. Field lookup stops above the viewport bottom and refuses to type if no label is found. All twelve feature assertions remain unchanged.

- App build succeeded (19m52s), wrapper reported EXIT 0. SHA-256 of own test binary equals own Cargo output: c15def5d0ea3b69e2cb3150f2a6203cba57caee6b9503886dcc9e2679711abd0. Wrapper emitted a trailing parse error after success while its live contents changed; current `bash -n` is green and the copied binary is verified.
- GUI attempt 1 terminated by SIGTERM before a final verdict; no PASS claimed. Captures prove keep-pages applies, search and limitations render, but Import crop cuts the button, built-in Save is 25px lower, and `xdotool type` needs `--` before negative settings. All corrected in the step. Attempt 2 session 71780 runs whole 350–356 on :115.

- GUI attempt 2 completed: 8 of 12 feature assertions passed; failures 350, 351 search/description, 352 Undo. 353 settings/output, 354 mixed inputs, 355 watch/restart, 356 recursive intake passed. Evidence `dev/.tmp/round31-smoke-2.log`.
- Remaining step causes: first-draw navigation can lose its click; wait for Actions header through an idempotent navigation helper. Pale Search placeholder is not a reliable OCR anchor; target its verified field rectangle after the header is ready.
- App root cause for 352: closing CustomToolsPanel removes the focused button without returning focus to the reader. File Save still dispatches globally, but Undo synthesizes keys to the lost focus; log has Apply + Save and no Undo. Added the same root take-keys restoration already used after the password modal closes. Existing GUI 352 remains the regression. Impeccable scoped hardening loaded; detector [] and diff/shell checks green. New build session 41690 through the supplied wrapper.
- Verified all 12 original feature `check` statements remain byte-for-byte unchanged from dev/round30.

- Final app build: EXIT 0 in 11m53s; test binary and own Cargo output both SHA-256 0a072c7d2f08247b4a235a1914293368b040ec97bc53b89406342de290e04fed.
- Whole-range GUI attempt 3: ALL PASS (12 feature assertions + real-profile isolation), 350–356 together on :115. Evidence `dev/.tmp/round31-smoke-3.log`, images and real output PDFs in `dev/.tmp/steps-115/`. 352 specifically proves 2 then 4 pages after Save/Undo/Save, with no extra click to restore focus.
- Visual inspection of fresh 350, 351 description, 352 undone, 353 settings and 356 results captures confirms the expected export, readable complete limitation text, restored four-page reader, -30 settings and two recursive outputs. Lint wrapper session 87960 is active; unit suite and commits/push remain.





- Lint attempt 1: EXIT 1 with compiler output ending during sheaf-ui checks and no compiler diagnostic. systemd run-u6246.service at 01:02:43 UTC confirms OOM kill, 10.0G peak. Retry uses JOBS=1 through the same 10G wrapper, without disabling any check; first compiler log saved as `dev/.tmp/round31-lint-first-cargo.log`, unit journal saved separately.

- Lint retry JOBS=1: EXIT 0, `dev/.tmp/round31-lint-final.log`; the one-suite script removed its per-layer logs on success as designed. Next run unit with JOBS=1 under the unchanged 10G wrapper.

## Round 31 — final verification
- C-batch-002: f8 a7 p8 — labelled text/range/appearance, literal pipe, negative rotation, prompt, order and real page-limited PDF result all pass smoke 353. Commit 02b0eb9.
- C-batch-003: f8 a7 p8 — mixed input reports and recursive folder copies, preserving both originals, pass smoke 354/356. Commit 67ea7fb.
- C-batch-004: f7 a7 p8 — saved action and exported Stirling output naming pass smoke 350; first-draw readiness and correct Export Stirling target calibrated. Commit 1c96579.
- C-batch-005: f8 a7 p8 — combined name/description search, independent copy, full limitations and saved filter after restart pass smoke 351. Commit 05ecc69.
- C-batch-006: f8 a7 p8 — custom command Save/Undo/Save produces 2 then 4 pages without an extra click; palette closure returns keyboard focus in `ui/main.slint:253`. Smoke 352, commit d778ec7.
- C-batch-008: f8 a7 p7 — unchanged saved recursive watch processes an incoming file, skips its handled revision after restart and continues the counter. Smoke 355 passes.
- Grades remain below 9: this repairs the requested regressions and establishes runtime proof, not a new independent full-parity review. Component limitations above remain. No dependency added.
- Own app build: EXIT 0; 11m53s. Test executable and own Cargo output SHA-256: 0a072c7d2f08247b4a235a1914293368b040ec97bc53b89406342de290e04fed.
- GUI: all 350–356 together on display :115, final run `dev/.tmp/round31-smoke-3.log`: 12 feature PASS lines, real-profile isolation PASS, ALL PASS. The twelve original feature assertions are byte-for-byte unchanged from dev/round30. Actual screenshots and output PDFs inspected in `dev/.tmp/steps-115/`.
- Lint: JOBS=1 supplied wrapper, EXIT 0; PASS lint; GREEN (layers: lint; not a full run) (398s). Proof `dev/.tmp/round31-lint-final.log` and `round31-lint-verdict.log`.
- Unit: JOBS=1 supplied wrapper, EXIT 0; PASS unit; GREEN (layers: unit; not a full run) (263s). Proof `dev/.tmp/round31-unit-final.log` and `round31-unit-verdict.log` (run-u6410.service).
- Shell syntax, git diff --check and Impeccable detector [] pass. No display other than :115 was started, killed or captured. No full GUI suite was run.
- Use JOBS=1 for these suite layers under the supplied 10G cap: the first lint attempt at two jobs reached 10.0G and was killed by the cgroup. The serial retry passed without changing source or disabling a check.
- Completion audit: mandate yes; observable proofs yes; external dependencies none; independent required work complete. Final action: archive this note in git, push, and compare remote HEAD with local HEAD.

Closure push checklist:
- Permissions: full standing authorization and explicit request to commit and push origin par2/batch; includes the notes archive and direct remote branch query.
- Check-in: read completion; expected 30s, bound 60s; inspect the real command status if exceeded.
- Proof: successful push receipt, remote refs/heads/par2/batch equal local HEAD, clean working tree.
