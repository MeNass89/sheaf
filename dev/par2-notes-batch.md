# Round 30 — batch

## Tatouages
- Mandate: `/tmp/f5/v2/brief.md` and `batch.json`; only `/tmp/f5/w2-batch`, branch `par2/batch`.
- Serial builds/tests via wbuild, 6 GB cap; GUI only :115. Never :99 or class-recorder.
- Commit/push each component. No network features. Four-hour code stop about 20:55 UTC, 27 October 2026.
- All supplied `your_last_gap` fields refer to an empty phase-4 baseline, not a precise defect. The inspected last pass in F-015 had grades below 9; do not claim parity from the proposed grades.

## Components
- C-batch-002: Acrobat Action Wizard / Specify Settings. Watermark and header/footer use labelled text, font, size, page range, opacity, rotation and offset fields instead of pipe syntax. Existing portable arguments, text containing pipes/newlines, command order and per-step instructions are preserved. Test `labelled_mark_settings_round_trip_and_drive_the_selected_pages_and_appearance` (`src/action_settings.rs:60`); smoke 353 written. Commit 135e756. Left: insert/split action steps, permissions encryption and runtime/layout proof. Not closed at 9.
- C-batch-003: Acrobat run saved actions. Run on PDFs is in the fixed header. A bounded result summary accompanies a scrollable list of every file failure. Folder intake runs off the UI thread and is cancellable before processing, without a partial batch. Invalid prompted settings name the tool rather than saying cancelled. Tests `cancelled_folder_intake_returns_no_partial_batch_and_can_be_started_again` and `invalid_prompted_settings_report_the_tool_without_claiming_cancellation_or_publishing` (`src/actions.rs:721`, `:736`); smoke 354/356 written. Commit a0af0bc. Left: GUI mixed-input run, cancellation responsiveness and layout proof. Not closed at 9.
- C-batch-004: Stirling import/export pipelines JSON. Portable output prefix/suffix and `{pipelineName}` survive translation. `{outputFolder}` means the destination chosen for this run, as in Stirling's default fixture. Unsupported date/time, nested-folder naming and absolute paths fail explicitly. Pretty/escaped exports cannot exceed their own import limit. Tests `stirling_naming_round_trips_without_silently_dropping_pipeline_settings` (`src/action_import.rs:275`) and `every_exported_action_fits_the_import_limit_including_json_escaping` (`src/actions.rs:663`); smoke 350 written. Commit 3d27e15 plus the portable destination follow-up. Left: unsupported Stirling tools/settings, `.sequ`, GUI round-trip proof. Not closed at 9.
- C-batch-005: Acrobat suggested actions. Search matches all words across names and full descriptions; filtering keeps stable source indices. Multiline descriptions expose refusal/certification limits. Test `suggested_action_search_matches_all_words_in_names_and_descriptions` (`src/actions.rs:685`); smoke 351 written. Commit b04d5f4. Left: GUI copy/search/restart and readability proof. Not closed at 9.
- C-batch-006: Acrobat custom tools. Page commands are enabled without pretending the document has one page, and validate against its actual page count in the reader. The palette stores the reader's stable tab ID rather than its list index. Command labels use the real catalogue name or user label, including invalid drafts. Tests `custom_page_commands_are_available_then_validate_against_the_real_document` and `custom_toolbox_shortcuts_validate_real_tools_and_never_run_as_batch_edits` (`src/actions.rs:698`, `:615`); smoke 352 written. Commit 30039c4. Left: persistent custom panels/icons, GUI apply/Undo proof. Not closed at 9.
- C-batch-008: Stirling Folder Scanning. Uploads and queued nested directories can disappear without stopping other files or subsequent scans. A missing root inbox still produces an error. Tests `a_disappearing_upload_does_not_stop_other_files_or_the_next_scan` and `a_removed_nested_folder_is_skipped_between_scan_slices` (`src/watch_folder.rs:510`, `:530`); smoke 355 written. Commit 3ed33e4. Left: OS service/background processing after app exit, notifications, scale and GUI restart proof. Not closed at 9.

Paths above are relative to `crates/sheaf-app/`. All six component commits are pushed.

## Sources inspected
- Sheaf: actions.rs, action_import.rs, actions_ui.rs, actions.slint, watch_folder.rs, reader command dispatch, PdfCraft action engine, F-015 last sol-batch pass.
- Acrobat archive: `aapp/Actions_R_RHP.aapp` (CreateWorkflowRdr, ManageWorkflowsRdr, FindMoreWorkflowsRdr, CreateNewCustomToolRdr, ManageCustomToolsRdr).
- Stirling archive: `app/core/src/main/resources/static/pipeline/defaultWebUIConfigs/split-rotate-auto-rename.json` and `app/core/src/main/java/stirling/software/SPDF/controller/api/pipeline/PipelineDirectoryProcessor.java`.
- These sources establish feature contracts; they do not establish visual parity.

## Dependencies
No new runtime or registry package. The production-source integration harness adds dev-only `dirs` and `zip` to sheaf-core; both already exist in the workspace/app dependency graph. It includes the exact action/import/settings/watch sources without Slint, not copies or mocks. Utility commit f38a103.

## Verification
- Native harness, latest completed run: `test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.55s`. Proof `dev/.tmp/actions-backend-latest.log`. Includes the `{outputFolder}` assertion.
- Mechanical Slint detector: `[]`; shell syntax and `git diff --check` pass. No rendered QA yet.
- Lint: repeat required after fixing two baseline home.rs references to sheaf_app::store_file; first run failed on pre-existing Rust 1.99 `chunks_exact_to_as_chunks` in viewer/renders.rs:345. Same pixel iteration changed to `as_chunks_mut::<4>().0.iter_mut()`; existing transparency-grid regression covers rendering, not executed yet.
- Unit suite: pending.
- GUI smoke 350–356: written, not executed. No own app binary exists, so there are no PASS lines and no GUI grades can be certified.

## Build blockage and investigation
The worker’s app builds failed to compile within the wrapper's 6 GB cap. Own builds hit OOM at 6.0G, including JOBS=1, app-only codegen-units=256, opt-level=1, jemalloc arena/decay tuning, splitting the two large Action settings blocks into private nonrecursive Slint components, and app-only codegen-units=1 (run-u4855). The unsuccessful profile/allocator settings were removed. Generated Slint Rust remains about 48 MB. No test was disabled and no memory cap was raised.

The persistent blockage was discussed using `claude-review` (tools disabled), session 5796400c-6a70-4151-974e-7054bf53a4ff; opinion suggested frontend/MIR pressure and smaller constructors. That is a hypothesis, not measured phase attribution. Extraction was checked for static labels/callbacks/two-way binding equivalence, and the subsequent app build still OOMed. No senior review of the feature result occurred.

The suite now respects an explicit JOBS limit instead of recomputing up to 8 jobs from host RAM inside the 6 GB unit. All checks remain enabled.

## Polaroid
Active, 2026-10-27. Six component commits pushed; C004 portable output-folder follow-up prepared. Native repeat passed; next run required lint and unit suites, attempt scoped smoke only against this worktree's successful build. One-build 8 GB exception requested via async question; no approval received. Do not execute `dev/.tmp/wbuild-batch-8g.sh` without explicit approval. Finish with actual verdicts, honest sub-9 grades, and remaining work. Branch is not yet eligible for merge.
