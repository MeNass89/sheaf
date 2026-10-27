# Parallel batch sol-ocr-sign — 27/10/2026

## Tatouages

- Work only in `par/sol-ocr-sign`. No compiler, build, lint, test, app, browser or codex runs (parallel brief rule 1). The orchestrator owns compilation and runtime verification.
- No network features or new native dependencies. References are inspected from archives without extraction.

## Component notes

- C-sign-009: kept signatures and initials leave `prefs.json` for the system credential store
  (Preview's Keychain; Linux Secret Service via `secret-tool`, macOS `security`, Windows DPAPI via
  PowerShell). Old profiles migrate after the store accepts them; a locked store never falls back
  to plaintext or overwrites a saved secret, and new signatures remain available for the session.
  A failed save is shown in the reader. Tests `signatures_and_initials_leave_preferences_and_round_trip_as_a_secret`,
  `an_unreadable_store_is_not_erased_when_preferences_are_saved`.
  Existing preferences round-trip fixture now tests nonsensitive settings; the new unit test owns
  signature serialization. `not automatable: save, restart, reuse and delete a mark with each OS
  credential store, including locked-store behaviour — requires logged-in desktop/keyring access`.
  Left: platform integration must be run; no runtime parity claim from this unbuilt worker.

- C-optimize-006 (partial): the Latin recognizer now prepacks its matrix weights once when loaded,
  reusing them for every line instead of packing on each inference. RTen's installed source
  (`ModelOptions::prepack_weights`) documents this inference optimization. Test
  `prepacked_recognition_keeps_the_words_and_their_placement` compares packed and unpacked results.
  New public item: `Ocr::load_with_prepacked_weights` for accuracy/performance comparisons.
  Left: release timing on the MICI scan against the recorded 15.5 s/page and Tesseract's
  11.75 s/page; no claim that this closes the performance grade without the orchestrator's run.

- C-optimize-007 (partial): the existing OCR language selector offers installed Chinese,
  Japanese, Korean (horizontal and vertical), Arabic, Hebrew, Cyrillic, Greek, Hindi, Thai and
  Vietnamese packs through offline Tesseract, as iLovePDF's `LanguageOcrModel.qml` does.
  The same engine job preserves skip-text, duplicate filtering, page geometry and cancellation.
  Tests `local_ocr_keeps_cjk_words_lines_and_vertical_geometry`,
  `malformed_local_ocr_fails_instead_of_losing_a_page_silently`; locale regression updated to
  account for an installed Japanese pack. New engine public items `Line`, `Word`, `Turn`
  reexports and `OcrJob::run_with`.
  Left: Tesseract executable and language packs are optional external runtime requirements,
  not bundled; scripts absent locally are not offered. This does not reach iLovePDF's roughly
  100-language bundled/downloadable coverage. No downloads or network code added.
  `not automatable: select an installed CJK language in the OCR panel and inspect selection
  against a real scan — needs the orchestrator's app and matching local language pack`.

- C-optimize-015: a set of PDFs gets a reusable, versioned full-text catalog (Acrobat's
  Catalog / Add search index), built by `sheaf index a.pdf b.pdf -o catalog.json`, searched by
  `sheaf search-index catalog.json "phrase"`. Rebuild by rerunning Index to a fresh output;
  delete or exchange the JSON as an ordinary file. Unicode phrases cross line ends; changed,
  replaced or removed sources invalidate searches. Folder Search uses a `.sheaf-index.json`
  beside the chosen folder when available and falls back to parsing changed/new PDFs.
  Protected files are refused to avoid leaking their text. Test
  `a_saved_catalog_finds_unicode_phrases_and_refuses_changed_sources`.
  Existing workspace dependencies serde and serde_json now also linked by sheaf-core (no new
  version/package). Left: a dedicated GUI catalog manager; no Acrobat `.pdx` interchange.

- C-security-008: reusable named security policies are saved/exported as versioned JSON,
  imported with strict validation, and applied to PDFs with passwords supplied separately
  (Acrobat's Security Policies). CLI `save-security-policy NAME --no-copying … -o policy.json`
  and `apply-security-policy FILE --policy policy.json --user-password … -o protected.pdf`.
  Permissions and the encrypt-metadata choice survive exchange; unknown security fields and
  versions are refused. No passwords saved in policies. Tests
  `policies_exchange_permissions_and_metadata_but_never_credentials`,
  `a_policy_with_unknown_security_fields_or_versions_is_refused`,
  `an_imported_policy_applies_restrictions_and_leaves_metadata_clear`.
  New public core module `security_policy` and `security::protect_with_settings`.
  Left: dedicated GUI policy manager and Acrobat policy-file compatibility.

- C-sign-012 (partial): Sheaf's visible signing and invisible certification APIs now accept
  an explicit offline `TimestampAuthority` and invoke the engine's `sign_with_timestamp`
  (Acrobat's signature timestamp / PAdES B-T). Invalid/mismatched responses fail before writing;
  no silent fallback to an untimestamped signature. Tests
  `a_local_authority_timestamps_visible_and_invisible_certificate_signatures`,
  `an_invalid_timestamp_response_never_writes_an_untimestamped_signature` use a local authority.
  New public core items `sign::sign_with_timestamp`, `sign::certify_with_timestamp`.
  Left: TSA server selection and transport remain absent: brief rule 3 explicitly prohibits
  network features. A local adapter does not confer trust on a self-signed TSA. No GUI claim.

- C-optimize-009 (partial): `sheaf review-ocr FILE` lists hidden OCR runs with their page
  boxes and possible decoding artifacts; `correct-ocr FILE --page N --line N --expected OLD
  --text NEW -o fixed.pdf` corrects the hidden text, or `--not-text` removes it (Acrobat's
  Correct Recognized Text / This is not text). Visible text and stale review rows are refused.
  Tests `corrected_and_rejected_ocr_runs_change_search_text_but_never_the_scan`,
  `ocr_correction_refuses_visible_text_and_stale_review_rows` check scan pixels and searchable text.
  New public items core `ocr_review::{ReviewLine, review, correct}`, `TextLine::invisible`.
  Left: GUI suspect highlighting, per-suspect Accept/Next and retained model confidence are
  absent. This is an accessible CLI correction workflow, not Acrobat GUI parity.

- C-security-007 (not implemented): inspected Acrobat's `PPKLite.Primary.txt` entries
  Encrypt with Certificate / Encrypt all contents / Except metadata / Only attachments,
  core `security.rs`, PdfCraft COS `security::encrypt_dict`, the crypt crate's
  `SecurityHandler::open`, lopdf's `encryption.rs` and the signing crate's `cms.rs`.
  The reader's crypt handler explicitly rejects every `/Filter` except `/Standard`; its
  EncryptDict has no recipient envelopes, and the current CMS code supports signatures,
  not EnvelopedData. A writer alone would make documents Sheaf cannot reopen. Recipient CMS
  envelopes, `/Adobe.PubSec` file-key derivation, private-key unlocking and the recipient UI
  remain to implement together; no misleading certificate-encryption switch or output added.
  The independent except-metadata setting was implemented for password policies, not claimed
  as certificate encryption. No test added for an implementation that does not exist.

## Integration checks written (not run)

- Secure-storage smoke helpers now read Secret Service rather than plaintext preferences;
  smoke 240 asserts marks are absent from JSON and present in the system store. Linux GUI
  smoke now requires `secret-tool` and an unlocked Secret Service session. No fallback.
- `an_offline_ocr_adapter_keeps_skips_cancellation_and_unicode_placement` tests the new engine
  adapter from sheaf-core, without any model or Tesseract binary.
- `a_catalog_build_reads_real_pdfs_and_reuses_only_unchanged_pages` and
  `protected_sources_and_cancelled_catalog_builds_are_refused` exercise real PDF extraction,
  deduplication, persistence, cancellation and the protected-text guard.
- All new CLI arguments have help text, as required by `every_option_and_value_has_help`.
- Signature-store cache avoids OS queries on unrelated saves, and refuses writes when marks
  could not be loaded at startup (including a keyring unlocked later in the same session).

## Worker handoff / Polaroid — 27/10/2026

- Delivered: implementation for C-sign-009, C-optimize-015 and C-security-008, with GUI/runtime
  limits above; partial C-optimize-006, C-optimize-007, C-optimize-009 and C-sign-012;
  C-security-007 explicitly not implemented. No board grades changed or 9/10 closure claimed.
- Code proof: commits `de004b3` through `3177f2c` pushed to `origin/par/sol-ocr-sign`.
  The final documentation commit follows these. Branch started from `d501e13`.
- Checks actually performed: source/archive inspection, `git diff --check`, and
  `bash -n dev/scripts/smoke/1-reader.sh dev/scripts/smoke/3-flows.sh` succeeded.
  No cargo, rustc, clippy, test suite, app, Chrome, Xvfb or codex invocation.
- Compile handoff: no new Rust package/version. sheaf-core now directly depends on the
  workspace's existing serde/serde_json; the orchestrator's first Cargo invocation will update
  those direct dependency edges in Cargo.lock (not regenerated by this worker).
- Runtime handoff: secure signatures need Linux `secret-tool` + an unlocked Secret Service;
  macOS uses built-in `security`; Windows uses built-in PowerShell/DPAPI. Extra OCR scripts
  require optional local Tesseract + the matching traineddata packs. No downloads implemented.
- Required next action: orchestrator merges, compiles/lints, runs unit/integration and GUI smoke,
  measures release OCR time against the MICI baseline, and reports failures back. macOS and
  Windows credential adapters still need live platform verification. No macOS-only build work.
- Additional storage tests: `an_unloaded_profile_cannot_clear_a_store_that_later_becomes_available`,
  `failed_migration_preserves_an_existing_legacy_signature_without_writing_new_plaintext`.
  Credential writes are read back before plaintext legacy data is removed. A failed migration
  leaves the original JSON unchanged and never writes the newly selected mark in plaintext.
- Additional locale/CLI tests: `chinese_locales_choose_the_installed_script`,
  `ocr_correction_requires_one_action_and_positive_review_indexes`,
  `catalog_and_security_policy_commands_are_accessible_from_the_cli`.
- Control: mandate covered for every assigned component (including explicit nonimplementation);
  evidence linked to source/tests/commits; remaining execution belongs to the orchestrator under
  rule 1; independent permitted changes completed. Runtime success remains unverified.

## Touched files

- `ARCHITECTURE.md`
- `crates/sheaf-app/src/home.rs`
- `crates/sheaf-app/src/lib.rs`
- `crates/sheaf-app/src/prefs.rs`
- `crates/sheaf-app/src/reader_ui/folder_search.rs`
- `crates/sheaf-app/src/signature_store.rs`
- `crates/sheaf-app/tests/history.rs`
- `crates/sheaf-cli/src/args.rs`
- `crates/sheaf-cli/src/main.rs`
- `crates/sheaf-core/Cargo.toml`
- `crates/sheaf-core/src/lib.rs`
- `crates/sheaf-core/src/ocr.rs`
- `crates/sheaf-core/src/ocr_local.rs`
- `crates/sheaf-core/src/ocr_review.rs`
- `crates/sheaf-core/src/search_index.rs`
- `crates/sheaf-core/src/security.rs`
- `crates/sheaf-core/src/security_policy.rs`
- `crates/sheaf-core/src/sign.rs`
- `crates/sheaf-core/tests/engine_patches.rs`
- `crates/sheaf-core/tests/ocr.rs`
- `crates/sheaf-core/tests/ocr_review.rs`
- `crates/sheaf-core/tests/search_index.rs`
- `crates/sheaf-core/tests/security.rs`
- `crates/sheaf-core/tests/sign.rs`
- `dev/par-notes-sol-ocr-sign.md`
- `dev/scripts/smoke/1-reader.sh`
- `dev/scripts/smoke/3-flows.sh`
- `third_party/pdfcraft-edit/src/text.rs`
- `third_party/pdfcraft-engine/src/ocr.rs`
- `third_party/pdfcraft-ocr/src/latin.rs`
- `third_party/pdfcraft-ocr/src/lib.rs`

## Second pass — C-optimize-009

- C-optimize-009: More → More tools → Correct recognized text opens a reader panel beside
  the scan (Acrobat's Correct Recognized Text). Previous, Accept and next, Correct and next,
  and This is not text operate on the live working document, with a highlighted run and page
  navigation. Corrections use the reader's undo/redo/save pipeline; stale text and visible runs
  are checked on the document thread. Other document changes refresh the review.
  Tests written: `live_ocr_review_sees_unsaved_corrections_and_undo_and_refuses_missing_pages`,
  `review_completion_and_empty_layer_are_distinct`; existing pixel-preservation tests still apply.
  New public core helpers: `ocr_review::review_document`, `ocr_review::correction_edit`.
  No new dependency. Not run (brief rule 1).
- `not automatable: More tools → Correct recognized text on an OCR scan; check the highlighted
  run, Accept/Previous, correct, reject, undo/redo, save/reopen and empty/finished states at
  small and large window sizes — app execution forbidden to this worker; orchestrator must run`.
- Left: recognition confidence and suspect-only filtering are absent; decoding artifacts are
  labelled as such. Panel rendering, keyboard traversal and large-document timing unverified.
- Proposed grade: C-optimize-009: f8 a7 p7 — live correction/undo test above, panel in
  `ui/screens/ocr-review.slint`, thread validation in `reader_doc/mod.rs`. No joint 9 claim.

## Second pass — C-optimize-015

- C-optimize-015: More → More tools → Search catalog opens a dedicated manager (Acrobat's
  Catalog). Add/remove saved PDFs, build/save a JSON catalog, open/import an existing catalog,
  search phrases, and open a result at its page, including an already-open PDF. Source paths
  distinguish equal file names. Rebuild saves to a new file; source changes invalidate old hits.
  Build progress, cancellation, missing/changed sources, errors and no results have visible states.
  Work runs off the UI thread; closing drops late replies. Folder Search still reuses a catalog
  saved as `.sheaf-index.json` in its folder.
- Tests written: `catalog_manager_can_reopen_sources_and_rebuild_after_a_removal`,
  `closing_a_catalog_cancels_work_and_reopening_cannot_accept_its_reply`,
  `catalog_selection_deduplicates_sources_in_order`. New public helper `Catalog::sources`.
  No new dependency; tests not run under rule 1.
- `not automatable: More tools → Search catalog; add two PDFs, remove/readd one, build, search,
  open a result, reopen the catalog, cancel a build, then change a source and search again —
  app execution forbidden to this worker; orchestrator must check small/large windows too`.
- Left: no Acrobat PDX interchange, folder picker/recursive catalog creation, or incremental
  updates; rebuild rereads sources. Catalog search cancellation discards the reply but does not
  interrupt an in-progress file fingerprint. Large catalogs and visual fit remain unmeasured.
- Proposed grade: C-optimize-015: f8 a7 p7 — manager/rebuild tests above,
  `ui/screens/catalog.slint`, `reader_ui/catalog.rs`. No joint 9 claim.

- Catalog output follows the actual path returned by the atomic writer: an existing name is
  preserved and the new catalog receives a numbered name. The rebuild test checks both files.

## Second pass — C-security-008

- C-security-008: More → More tools → Security policies opens a dedicated manager (Acrobat's
  Security Policies): create, name, edit, save and delete reusable policies; import JSON for
  review, export it, then apply the selected saved settings to a protected copy of the open PDF.
  Permission and metadata checkboxes use the existing house controls. Applying asks for confirmed
  open/owner passwords; restrictions require a distinct owner password. Unsaved reader changes
  and read-only documents are refused. Encryption runs off the UI thread; status shows the
  actual output path or error. Success and switching policies clear the password controls.
- A local `security-policies/` library stores only typed policy settings, no credentials.
  Unknown/corrupt entries are reported rather than silently overwritten. Same-name Save updates
  that policy; a changed name saves another policy, leaving the original available for deletion.
- Tests written: `a_policy_library_survives_edits_import_export_and_deletion_without_passwords`,
  `a_corrupt_policy_is_reported_without_overwriting_the_library`,
  `applying_a_policy_requires_confirmed_passwords_and_a_distinct_owner_for_restrictions`.
  Existing `an_imported_policy_applies_restrictions_and_leaves_metadata_clear` covers encrypted
  output. New public app module `security_policies`; no new dependency. Nothing executed.
- `not automatable: Security policies → create/save/edit, import/export, delete; apply with bad
  confirmations and a distinct owner password, reopen output and inspect permissions/metadata;
  verify fields clear after success and policy switch — rule 1 forbids this worker's app runs`.
- Left: Acrobat's certified/encrypted .acrobatsecuritysettings exchange, certificate policies,
  trust/signature preferences exchange and live visual/platform verification. This is password
  policy management; certificate encryption is not claimed.
- Proposed grade: C-security-008: f7 a7 p8 — library and password-validation tests above,
  `ui/screens/security-policy.slint`, `reader_ui/security_policy.rs`. No joint 9 claim.

## Second pass — C-optimize-007

- C-optimize-007: all installed Tesseract traineddata packs are now offered, including packs
  outside the initial script list. Known languages retain readable names; unlisted/custom packs
  show their exact installed pack name. Orientation/equation data and unsafe pack names are
  excluded. Nothing downloaded or bundled. The OCR panel's language list scrolls separately.
- Every checked language now reaches recognition (previously only the first was used).
  Built-in Latin languages share the multilingual Latin model; mixed scripts use a single
  Tesseract call with all selected installed packs (e.g. `eng+jpn`). Missing Latin packs in a
  mixed selection are reported before OCR starts. Horizontal/vertical packs must be run in
  separate jobs; they cannot silently assign one orientation to the other's words.
  CLI accepts `--language en+ja`. Local Tesseract can also run when bundled models are missing.
- Tests written/updated: `every_installed_language_pack_is_offered_but_orientation_data_is_not`,
  `mixed_scripts_use_all_selected_packs_and_refuse_missing_or_conflicting_packs`,
  `ocr_job_uses_every_selected_language`. No new dependency or public item.
- `not automatable: OCR panel → select English + installed Japanese, run a mixed scan; verify
  searchable text in both scripts, missing-pack feedback, vertical-only geometry and scrolling
  through 100 installed languages — app/OCR execution forbidden by rule 1`.
- Left: no pack download/bundling; names of unknown packs are their identifiers. Available
  packs are discovered once at process startup, so local installation requires an app restart.
- Proposed grade: C-optimize-007: f8 a8 p8 — selection/pack tests above, `ocr_local.rs`,
  `run.rs::ocr_options`, `ui/screens/options.slint`. Live multilingual accuracy unverified.

## Second pass — C-optimize-006

- C-optimize-006: OCR options now offer Use installed Tesseract, mirrored by CLI
  `ocr --local-engine`. Latin scans can use the already-supported local engine without loading
  the bundled recognizer; page skipping, duplicate filtering, progress and output geometry use
  the same engine job. It is an explicit alternative for big scans, not an unmeasured speed claim.
  The selected engine survives Run again/history; absent local packs fail with a clear message.
- Inspected the profile in F-006 and DECISIONS 24/10: 89% of inference time was Conv, detection
  0.8s vs recognition 18.3s; native ONNX Runtime was deliberately deferred. Adding more unrelated
  detection tweaks cannot demonstrate closing this gap. No new native library or int8 model added.
- Tests written/updated: `local_ocr_is_explicit_for_latin_and_automatic_for_extra_scripts`,
  `local_ocr_engine_and_mixed_languages_are_explicit_cli_options`,
  `ocr_job_uses_every_selected_language`,
  `options_come_back_as_they_were_without_their_passwords`.
  Public `OcrOptions` gains `use_local_engine: bool` (default false); no dependency added.
- `not automatable: time the same MICI scan in release with bundled recognition and with Use
  installed Tesseract/fra, compare text accuracy and word placement, measure cold and warm RSS —
  no builds, OCR or app execution allowed by rule 1; orchestrator owns this benchmark`.
- Left: native bundled recognition's 15.5s/page vs 11.75s/page baseline remains unmeasured after
  prepacking. Optional Tesseract is a user path, not bundled-engine performance parity.
- Proposed grade: C-optimize-006: f9 a8 p8 — prior OCR integration/placement tests plus engine
  selection tests above; UI in `options.slint`. No performance 9 without the release benchmark.

## Second pass — C-sign-009

- C-sign-009: More → More tools → Manage signatures opens a full manager (PDF Expert's
  Edit Signature). Preview saved marks, choose the active one, reorder Up/Down, delete,
  edit a typed name/font/ink/width or a saved drawing, and create a new saved mark (four max,
  matching the existing library). Saved drawing points are converted back into pad coordinates.
  Reordering selects the first mark, consistently with the existing reader tray.
- Updates persist through the same system credential store. A failed manager save/delete restores
  the previous visible list and reports the error. Unsaved editor changes stay local to the sheet;
  the existing one-time-use checkbox remains in Fill & Sign. Initials keep their existing editor.
- Tests written: `signature_edits_preserve_position_and_reject_a_stale_selection`,
  `saved_drawing_can_be_edited_without_changing_its_normalized_points`.
  Existing credential migration, failure and plaintext-exclusion tests still apply. No new deps.
- `not automatable: create/edit/reorder/select/delete saved marks, close/restart and reuse;
  repeat with a locked/missing keyring and ensure failed deletion remains visible — requires
  desktop credential stores and app execution prohibited to this worker by rule 1`.
- Left: platform credential adapters and manager rendering are still unverified on real desktops.
  The existing four-mark limit remains. No cloud/shared signatures (outside this batch).
- Proposed grade: C-sign-009: f8 a8 p8 — mutation/drawing tests above, `signature_manager.rs`,
  `ui/screens/signature-manager.slint`; secure-store tests from pass 1. No joint 9 before runtime.

- More tools now scrolls within the reader's available height, so these added entries remain
  reachable at the app's 600px minimum height. Runtime menu scrolling is part of GUI handoff.

## Second pass — C-optimize-009 follow-up

- The correction panel now reserves space in the reader rather than covering the scan; it
  temporarily closes the left sidebar and restores it on closing. Failed corrections keep
  the current run instead of advancing past an unresolved error.
- Added Possible decoding errors only, retaining each run's original page/line target. This
  supersedes the earlier second-pass note that suspect filtering was absent. It is explicitly
  a decoding-artifact filter, not recognition confidence. Test written:
  `suspect_filter_preserves_the_original_page_and_line_targets`.
- GUI handoff additionally checks sidebar restoration, scan fit beside the panel, filtering,
  failed-correction position and Correct/Next within the filtered set. No execution here.

- OCR review hides its old highlight while refreshing and bounds-checks page navigation, so
  deleting/reordering pages cannot render an obsolete review target. GUI handoff includes
  removing the reviewed page while the panel is open and confirming the refresh is safe.

- Signature preview slots now preserve the saved list's cardinality even for an unreadable
  legacy mark; Edit/Delete cannot shift onto the following entry because its thumbnail failed.
  Test written: `unrenderable_saved_signatures_keep_their_library_positions`.

## Second pass — C-sign-012 (no further implementation)

- Re-inspected core `sign.rs::sign_inner`/`certify_inner`, engine
  `Session::sign_with_timestamp`/`timestamp_document`, the sign crate's
  `TimestampAuthority::timestamp(request)`, and both local-authority integration tests.
  Visible and invisible signing already route the supplied authority through the engine, and
  an invalid response prevents writing. Reason, location and visible certificate appearance
  keep their existing GUI. No new CLI-only signature capability was added in pass 1 or pass 2.
- The outstanding reviewer gap is a timestamp server. It remains excluded by the user's
  no-network instruction. A preselected response file is not a substitute: RFC 3161 responses
  must match the newly signed bytes' request. A self-signed local clock is not a trusted server.
  No server field, fabricated certified time, or misleading file-picker switch was added.
- No code change or new test for this component in pass 2. Existing written tests:
  `a_local_authority_timestamps_visible_and_invisible_certificate_signatures`,
  `an_invalid_timestamp_response_never_writes_an_untimestamped_signature` (not executed here).
- Proposed grade: C-sign-012: f8 a8 p8 — core `sign.rs` and tests above; the real server gap is
  still open. No joint 9 or runtime validation claimed.

## Second pass — C-security-007 (not implemented)

- Re-inspected core `security.rs::protect_with_settings`, COS `security::encrypt_dict`,
  the locked PdfCraft crypt source `crates/crypt/src/lib.rs::{EncryptDict,SecurityHandler::open}`,
  and sign source `crates/sign/src/cms.rs` at the Cargo.lock revision d3a91c5. Acrobat's
  `aapp/Protect_R_RHP.aapp` and `strings/PPKLite.Primary.txt` were read directly from its archive.
- Verified: the crypt handler still rejects every non-Standard /Filter; the parsed encryption
  dictionary has no recipient envelopes. The CMS implementation is SignedData for signatures,
  not recipient EnvelopedData. The app exposes digital IDs for signing, with no private-key
  recipient-unlock path. Password policy encryption cannot stand in for these contracts.
- No certificate-encryption implementation was added. A workable next unit must add CMS
  recipient envelopes plus Adobe.PubSec key derivation and private-key opening, then integrate
  writer, reader and recipient controls together. An offline OpenSSL envelope writer alone
  would not make the reader open the resulting PDF. No output or nonfunctional GUI switch added.
  This is unfinished implementation, not a claim that offline certificate encryption is impossible.
- Tests: none added for an absent feature. Next verification: recipient A opens, unrelated B
  fails, multiple recipients open, metadata/attachment encryption modes agree in an independent
  reader, and Sheaf reopens its own output. The orchestrator must run any eventual tests.
- Proposed grade: C-security-007: f0 a0 p0 — absence confirmed at crypt
  `SecurityHandler::open` and COS `encrypt_dict`; no supported certificate-encryption path.

- The OCR review controls scroll at short window heights; Close and correction actions cannot
  be stranded below the 600px app minimum. This remains a written GUI smoke check, not a render.

## Second-pass handoff / Polaroid — 27/10/2026

- Project: Sheaf, batch sol-ocr-sign, second pass. Branch `par/sol-ocr-sign`; base `10bfcf6`.
  Status: implementation handoff active; compilation/runtime and counterpart review pending.
- Six components advanced: C-optimize-006/007/009/015, C-sign-009, C-security-008. Four dedicated
  GUI surfaces now expose the previously CLI-only workflows plus signature management. They are
  reached from the reader's More → More tools. Every component's remaining gap is stated above.
- C-sign-012: inspected again, no second-pass implementation; network TSA is explicitly excluded.
  C-security-007: inspected again, no certificate-encryption implementation. Its missing reader,
  envelope and private-key contracts remain work, not a claimed external blocker or impossibility.
- Checks performed: repeated `git diff --check` on each component and the complete second-pass
  diff; static source/API inspection. Impeccable's static detector returned `[]` for the Slint
  paths; that does not establish rendered layout or accessibility. No tests or runtime executed.
  No cargo, rustc, clippy, app, Chrome, Xvfb or codex invocation in this pass.
- Every component commit was pushed immediately. Final documentation follows `9c7ec96`.
  No new dependency in this pass. Pass 1's existing serde/serde_json direct edges in sheaf-core
  still require the orchestrator's Cargo.lock integration if not already done elsewhere.
- Public API changes in this pass: core `ocr_review::{review_document,correction_edit}`,
  `Catalog::sources`, `OcrOptions::use_local_engine`; app `security_policies` module.
  No Prefs field added, so no additional history Prefs literal update is needed.
- Runtime requirements remain optional local Tesseract + selected packs, Linux Secret Service
  and secret-tool for saved signatures, macOS security/Keychain, Windows PowerShell/DPAPI.
- Next action: merge, build/lint, run unit/integration + the GUI checks specified per component;
  benchmark OCR in release on the MICI baseline; send failures back. Only after the full suite is
  green does counterpart opus-edit review read-only for the joint grades. No board grade changed.
- Mandate control: six independent implementable components advanced, every CLI addition has an
  app control, no forbidden execution/network feature added; proofs name source and written tests;
  unimplemented/untested work stays explicit. The two remaining gaps are retained, not relabelled
  as finished. This pass is ready for integration, not a claim of end-to-end parity.

## Second-pass proposed grades (all tests are written, not run)

- C-optimize-006: f9 a8 p8 — `crates/sheaf-core/src/ocr.rs:278`,
  `local_ocr_is_explicit_for_latin_and_automatic_for_extra_scripts`: deliberate engine selection;
  release speed still needs the recorded benchmark.
- C-optimize-007: f8 a8 p8 — `crates/sheaf-core/src/ocr_local.rs:131`,
  `every_installed_language_pack_is_offered_but_orientation_data_is_not` and
  `mixed_scripts_use_all_selected_packs_and_refuse_missing_or_conflicting_packs`: pack coverage
  and all selected scripts; installation/accuracy remain unverified.
- C-optimize-009: f8 a7 p7 — `crates/sheaf-core/tests/ocr_review.rs:40`,
  `live_ocr_review_sees_unsaved_corrections_and_undo_and_refuses_missing_pages`: working-document
  edits and undo; reader panel/filter exist, rendering and large-document speed unverified.
- C-optimize-015: f8 a7 p7 — `crates/sheaf-core/tests/search_index.rs:29`,
  `catalog_manager_can_reopen_sources_and_rebuild_after_a_removal`: source management and
  nondestructive rebuild; GUI and large-catalog timings still need execution.
- C-sign-009: f8 a8 p8 — `crates/sheaf-app/src/reader_ui/signature_manager.rs:98`,
  `signature_edits_preserve_position_and_reject_a_stale_selection`: edits/order and stale guards;
  credential adapters and manager rendering still need live platform checks.
- C-sign-012: f8 a8 p8 — `crates/sheaf-core/tests/sign.rs:473`,
  `a_local_authority_timestamps_visible_and_invisible_certificate_signatures`: local adapter;
  server remains absent by instruction, so no closure.
- C-security-007: f0 a0 p0 — locked PdfCraft `crates/crypt/src/lib.rs:148` at d3a91c5 and
  `third_party/pdfcraft-cos/src/security.rs:50`: Standard-only reader, no recipient envelopes;
  no implementation or test claimed.
- C-security-008: f7 a7 p8 — `crates/sheaf-app/src/security_policies.rs:52`,
  `a_policy_library_survives_edits_import_export_and_deletion_without_passwords`: reusable
  settings and exchange; Acrobat security-settings/certificate-policy parity still absent.

## Second-pass touched files

- `ARCHITECTURE.md`
- `crates/sheaf-app/src/home.rs`
- `crates/sheaf-app/src/lib.rs`
- `crates/sheaf-app/src/main.rs`
- `crates/sheaf-app/src/reader_doc/mod.rs`
- `crates/sheaf-app/src/reader_ui/catalog.rs`
- `crates/sheaf-app/src/reader_ui/mod.rs`
- `crates/sheaf-app/src/reader_ui/model.rs`
- `crates/sheaf-app/src/reader_ui/ocr_review.rs`
- `crates/sheaf-app/src/reader_ui/security_policy.rs`
- `crates/sheaf-app/src/reader_ui/sign.rs`
- `crates/sheaf-app/src/reader_ui/signature_manager.rs`
- `crates/sheaf-app/src/run.rs`
- `crates/sheaf-app/src/saved_options.rs`
- `crates/sheaf-app/src/security_policies.rs`
- `crates/sheaf-app/ui/flow-types.slint`
- `crates/sheaf-app/ui/screens/catalog.slint`
- `crates/sheaf-app/ui/screens/ocr-review.slint`
- `crates/sheaf-app/ui/screens/options.slint`
- `crates/sheaf-app/ui/screens/reader-toolbar.slint`
- `crates/sheaf-app/ui/screens/reader.slint`
- `crates/sheaf-app/ui/screens/security-policy.slint`
- `crates/sheaf-app/ui/screens/signature-manager.slint`
- `crates/sheaf-app/ui/widgets/panel-button.slint`
- `crates/sheaf-cli/src/args.rs`
- `crates/sheaf-cli/src/main.rs`
- `crates/sheaf-core/src/ocr.rs`
- `crates/sheaf-core/src/ocr_local.rs`
- `crates/sheaf-core/src/ocr_review.rs`
- `crates/sheaf-core/src/search_index.rs`
- `crates/sheaf-core/tests/ocr_review.rs`
- `crates/sheaf-core/tests/search_index.rs`
- `dev/par-notes-sol-ocr-sign.md`

## Third pass — C-optimize-015

- C-optimize-015: phrase search uses an in-memory trigram index and cached normalized pages,
  including CJK substrings and punctuation; no source-byte hashing or whole-vocabulary scan
  per query. Source freshness uses size + modification time. Changed/missing PDFs are excluded
  individually and the manager reports their count; `validate` remains an explicit byte audit.
- Build accepts recursive local folders (symlink directories excluded). `index --update OLD`
  and rebuilding in the manager reuse unchanged pages, add/replace changed sources and purge
  omitted sources. Failures/cancellation keep the old catalog. No new dependencies.
- Tests written (not run): `indexed_substrings_and_incremental_reuse_do_not_require_a_readable_pdf_body`,
  `incremental_folder_catalog_reuses_unchanged_sources_and_purges_removed_ones`, updated
  `a_saved_catalog_finds_unicode_phrases_and_excludes_changed_sources` and real PDF integration.
- Left: no PDX interchange; GUI recursive folder picker absent (CLI accepts directories).
  Metadata-preserving edits require explicit `validate`; no release benchmark under rule 1.
  `not automatable: reopen/build a catalog, change/delete one source, search the others, then
  rebuild and inspect stale-source feedback — app execution forbidden to this worker`.
- Proposed grade: C-optimize-015: f8 a7 p8 — tests above; actual large-catalog timing unverified.

## Third pass — C-sign-009

- C-sign-009: new saves use Rust ChaCha20-Poly1305 authenticated local storage, without
  secret-tool/security/PowerShell writes or read-back subprocesses on the UI thread. Each
  save has a fresh nonce; the separate random per-profile key and ciphertext are owner-only
  on Unix, atomically replaced. No 64K-character Windows payload limit. Legacy system-vault
  profiles migrate after successful reads; unreadable legacy data is never overwritten.
- This supersedes the earlier mandatory-keyring design. The key is local and protects against
  other local accounts (Windows inherits profile ACLs); it does not claim Keychain-equivalent
  isolation from the signed-in account. Legacy migration may still invoke its old vault once
  at startup. Future saves and normal local loads need no outside program or unlock prompt.
- New dependency: workspace + sheaf-app `chacha20poly1305 = 0.10`, alloc only (pure Rust,
  orchestrator must fetch/update lockfile); existing workspace getrandom linked by sheaf-app.
- Tests written, not run: `local_signatures_survive_restart_without_external_tools_or_size_caps`
  (>64K drawn data, restart, ciphertext and Unix permissions),
  `damaged_or_missing_signature_keys_never_clear_a_saved_library`; previous migration guards.
  Smoke helpers now decrypt the local file using Python cryptography (smoke-only requirement).
- `not automatable: create/reorder/edit/delete/once-only marks, restart with no secret-tool,
  reopen and reuse on Linux/Windows/macOS — app/platform execution forbidden by rule 1`.
- Proposed grade: C-sign-009: f8 a8 p8 — persistence tests above; no live platform verification.

## Third pass — C-optimize-009

- C-optimize-009: recognition confidence now survives the built-in Latin and Tesseract paths,
  placement, PDF save/reopen and reader review. Each new OCR word has its own marked-content
  scope, preventing same-baseline words from merging into one correction target. Suspects now
  include words below 85% confidence, not just decoding artifacts. Existing external/older
  layers explicitly show unknown confidence. Corrections set the reviewed word's score to 1;
  undo restores it. The scan and neighboring words remain untouched.
- The reader's document thread caches review pages: targeted corrections only reread their
  page; arbitrary edits and undo/redo invalidate the snapshot. Accept/Previous/filter do not
  extract PDF text again. UI labels now say OCR suspects; the position shows confidence.
- Tests written, not run: `word_confidence_survives_save_and_corrections_leave_neighboring_words_and_scan_intact`,
  `local_ocr_preserves_low_confidence_and_refuses_invalid_scores`, updated CJK TSV test and
  reader review-state tests. No dependency. Public fields added: OCR `Word::confidence`,
  `PlacedWord::confidence`, `TextLine::ocr_confidence`, `ReviewLine::confidence`; new core
  `ocr_review::ReviewCache`. Callers constructing PlacedWord literals must set confidence.
- Left: old/external OCR layers may still group words in line runs and lack confidence; Sheaf
  does not invent it. New word grouping may affect OCR text-export line boundaries and must
  be checked by orchestrator integration tests. Large-file/GUI timing remains unmeasured.
  `not automatable: OCR a scan, filter suspects, correct one word, accept/next, undo, save/reopen;
  check confidence labels and highlight — running the app is forbidden by rule 1`.
- Proposed grade: C-optimize-009: f8 a8 p8 — confidence/neighbor/pixel/undo test above; no joint 9.

## Third pass — C-security-008

- C-security-008: policies now retain high/low print resolution, changes allowed (none,
  page assembly, forms, comments+forms, all checked actions), and AES-128/AES-256 compatibility.
  These choices are editable in the existing GUI and CLI (`--print-quality`, `--changes`,
  `--algorithm`), survive the local library/import/export, and reach actual PDF permission
  bits and crypt filters. Existing booleans can further restrict, never broaden, the choices.
  Restricted output gets a distinct generated owner password if none is supplied by CLI;
  the GUI requires a separate confirmed owner password. PDF headers meet the AES minimum.
- Old v1 policies default to their original settings; unknown algorithms/options are refused.
  No new dependency. New core public types `ProtectionOptions`, `PrintQuality`, `ChangeLevel`,
  `EncryptionAlgorithm`; function `security::protect_with_options`; public policy `validate`.
  Slint policy-save callback has three appended choice indexes; model gets policy-choices.
- Tests written (not run): `policy_print_quality_change_levels_and_aes_compatibility_reach_the_encrypted_pdf`,
  `every_changes_allowed_level_only_removes_the_requested_pdf_permissions`,
  `old_policies_keep_defaults_and_unknown_compatibility_is_refused`,
  `policy_choice_indexes_map_to_exact_print_change_and_algorithm_settings`,
  `policy_compatibility_choices_and_incremental_folder_index_are_accessible`, plus expanded
  policy-library round-trip test (ensures saving an imported advanced policy cannot reset it).
- Left: no certificate policies, stored-password option, Acrobat proprietary settings exchange
  or trust-settings exchange. Passwords are still asked per apply; AES only, no legacy RC4.
  `not automatable: create/import advanced policy, save/edit/export/reopen, inspect print quality,
  changes and compatibility on protected output — app and tests forbidden by rule 1`.
- Proposed grade: C-security-008: f8 a7 p8 — PDF-output/round-trip tests above, unrun.

## Third pass — C-optimize-006

- C-optimize-006: fixed the review's inference risk by annotating the progress closure's
  `usize` arguments. Added ignored release benchmark
  `release_ocr_mici_benchmark_reports_cold_load_and_warm_page_time`: the same supplied MICI
  scan, bundled engine only, prepacked/unpacked variants loaded sequentially, cold model load
  recorded, one warm-up and three timed runs each, warm median per page, word/placement
  equivalence, explicit failure above the recorded 11.75 s/page target (overridable for host).
- Orchestrator must set `SHEAF_OCR_BENCH_PDF` to the original MICI scan and explicitly run the
  ignored test in release mode on an idle benchmark host. This worker did NOT execute it.
  No compiler/test/app invoked and no performance result fabricated. No new dependency.
- Left: the actual 15.5 vs 11.75 s/page gap is still UNMEASURED after prepacking; a test harness
  is not a measured optimization or parity proof. Further inference work depends on that run.
- Proposed grade unchanged from counterpart: C-optimize-006: f9 a8 p7 — existing accuracy
  coverage and new benchmark protocol; the speed gap remains open.

## Third pass — C-sign-012

- C-sign-012 remains partial: no app/CLI TSA transport. The caller-supplied offline API alone
  is not user-facing certified timestamping, as the counterpart correctly points out. A
  network TSA client/server selection is explicitly excluded by this batch's rule 3; none added.
- Fixed the timestamp fixture: separate TSA identity/key, critical timeStamping-only EKU and
  digitalSignature key usage, with assertions that it is not the document signer's certificate.
  Test `a_local_authority_timestamps_visible_and_invisible_certificate_signatures` is still
  not run. This proves only the intended offline adapter contract once executed; no trust
  or certified-clock claim. Existing invalid-response/no-output test remains.
- Proposed grade: C-sign-012: f7 a8 p8 — offline API and corrected fixture; user-facing
  certified timestamping remains open pending authorized TSA transport. No dependencies.

## Third pass — C-optimize-007 (gap not closed)

- Rechecked `scripts/fetch-models.sh` and `third_party/pdfcraft-ocr/src/lib.rs`: shipped asset
  declarations are text detection, unaccented recognition and the accented Latin model/dictionary.
  There is no Japanese/Chinese/Korean model manifest or asset in this worktree (`vendor/models`
  is absent here). The current non-Latin route still requires installed Tesseract + traineddata.
- No extra CJK label, unverifiable model adapter or download was added to pretend that a stock
  user can recognize Japanese. A real native route needs a pinned licensed compatible model,
  dictionary/token decoding, packaging and real multilingual accuracy/geometry fixtures.
  Those assets and runtime validation are not available in this worker's permitted environment.
  Network features stay out; no automatic language downloads introduced.
- Existing selection/TSV/vertical tests remain; this pass additionally retained TSV confidence
  under C-optimize-009. No separate implementation or new dependency for stock language coverage.
- Proposed grade unchanged: C-optimize-007: f8 a8 p8 —
  `mixed_scripts_use_all_selected_packs_and_refuse_missing_or_conflicting_packs`,
  `local_ocr_keeps_cjk_words_lines_and_vertical_geometry`; these are parsing/selection proof,
  not proof of bundled Japanese recognition. The counterpart's stock-install gap stays open.

## Third pass — C-security-007 (not implemented)

- Rechecked the locked crypt reader: `crates/crypt/src/lib.rs:147` rejects non-Standard
  filters; the local COS EncryptDict route still has no recipient-envelope read/write contract.
  Existing signing CMS primitives do not provide PDF public-key encryption by themselves.
- Certificate encryption remains absent. Closing it requires CMS EnvelopedData recipient
  envelopes, Adobe.PubSec key derivation/crypt filters, recipient private-key unlocking and
  reader support together. An output-only path would create a PDF Sheaf cannot reopen; none
  added. This is a substantial unimplemented native Rust feature, not a network dependency.
  No new API, dependency, test or certificate-encryption UI claimed.
- Proposed grade: C-security-007: f0 a0 p0 — no implementation. The original gap stays open.

- C-sign-009 follow-up: independent mark storage is also loaded when prefs.json is absent or
  corrupt; resetting unrelated preferences cannot hide saved signatures. Added unrun tests
  `kept_marks_load_even_when_unrelated_preferences_are_missing` and
  `changing_ciphertext_is_detected_before_replacing_any_kept_marks` (AEAD tamper refusal,
  no empty-library overwrite). Grade remains f8 a8 p8 until platform/runtime checks.

- C-optimize-015 follow-up: CLI catalog searches also report changed/missing exclusions;
  stale sources no longer masquerade as a complete no-match response. Existing stale-source
  search tests cover exclusion; this only wires the already-tested count into CLI notes.

## Third-pass handoff / Polaroid — 27/10/2026

- Project: Sheaf, sol-ocr-sign, branch `par/sol-ocr-sign`, third pass. Status: handed off for
  orchestrator compilation/runtime review; no component declared jointly closed.
- Implemented this pass: catalog search/index updates, reliable encrypted local marks,
  retained OCR word confidence + correction-page cache, detailed password-policy choices.
  Narrow fixes: OCR closure typing + ignored release benchmark, distinct TSA test identity.
- Unclosed: bundled OCR speed (not measured), stock non-Latin models (not shipped), older OCR
  line granularity/unknown confidence, GUI/platform execution, certified timestamp transport
  (network excluded), certificate encryption (unimplemented), certificate/proprietary policies.
- Code/evidence commits `018b9b5` through `ac49aaa` pushed after each component/follow-up.
  Documentation-only receipts for C-optimize-007 and C-security-007 explicitly record no closure.
- Actually checked: aggregate `git diff 5353386..HEAD --check`, shell syntax with `bash -n` for
  both touched smoke scripts, source/API inspection at locked revisions, Slint static detector
  for the two edited sheets (both returned `[]`). No build, lint, tests, app, browser or codex run.
- Compilation handoff: fetch new pure-Rust workspace/sheaf-app dependency chacha20poly1305 0.10
  (alloc only) and update Cargo.lock; link the existing getrandom workspace entry to sheaf-app.
  `PlacedWord` literals now require `confidence`; all local occurrences were updated. Also
  new `Word::confidence`, `TextLine::ocr_confidence`, `ReviewLine::confidence`, `ReviewCache`,
  Catalog `update`/`stale_sources`/`collect_inputs`, advanced protection enums/options and
  `protect_with_options`. Policy-save Slint callback appends three integer choice indexes.
- Runtime handoff: new marks need no outside save program; retain both signatures.secure and
  signatures.key on backup/move. Legacy vault migration can still need the old platform tool.
  The smoke environment needs Python cryptography for ciphertext inspection, not the app.
  Tesseract + local traineddata remain optional for extra scripts. No network features added.
- Next: orchestrator merges, fetches dependency, compiles/lints and runs the written tests and
  GUI flows; run the ignored release OCR benchmark on the original MICI scan, then counterpart
  reviews. No timing, rendering, cross-platform or test success is asserted by this worker.
- Delivery control: all eight component gaps explicitly dispositioned; source/test evidence
  linked below/above. Remaining feature work is distinguished from forbidden runtime checks.
  No external authorization requested or inferred. This is source delivery, not proven parity.

### Third-pass changed files (complete)

- `ARCHITECTURE.md`
- `Cargo.toml`
- `crates/sheaf-app/Cargo.toml`
- `crates/sheaf-app/src/main.rs`
- `crates/sheaf-app/src/prefs.rs`
- `crates/sheaf-app/src/reader_doc/mod.rs`
- `crates/sheaf-app/src/reader_ui/catalog.rs`
- `crates/sheaf-app/src/reader_ui/model.rs`
- `crates/sheaf-app/src/reader_ui/ocr_review.rs`
- `crates/sheaf-app/src/reader_ui/security_policy.rs`
- `crates/sheaf-app/src/security_policies.rs`
- `crates/sheaf-app/src/signature_store.rs`
- `crates/sheaf-app/ui/flow-types.slint`
- `crates/sheaf-app/ui/screens/ocr-review.slint`
- `crates/sheaf-app/ui/screens/security-policy.slint`
- `crates/sheaf-cli/src/args.rs`
- `crates/sheaf-cli/src/main.rs`
- `crates/sheaf-core/src/ocr.rs`
- `crates/sheaf-core/src/ocr_local.rs`
- `crates/sheaf-core/src/ocr_review.rs`
- `crates/sheaf-core/src/search_index.rs`
- `crates/sheaf-core/src/security.rs`
- `crates/sheaf-core/src/security_policy.rs`
- `crates/sheaf-core/tests/ocr.rs`
- `crates/sheaf-core/tests/ocr_review.rs`
- `crates/sheaf-core/tests/search_index.rs`
- `crates/sheaf-core/tests/security.rs`
- `crates/sheaf-core/tests/sign.rs`
- `dev/par-notes-sol-ocr-sign.md`
- `dev/scripts/smoke/1-reader.sh`
- `dev/scripts/smoke/3-flows.sh`
- `third_party/pdfcraft-edit/src/text.rs`
- `third_party/pdfcraft-ocr/src/lib.rs`

### Third-pass proposed grades (tests written, not executed)

C-optimize-006: f9 a8 p7 — `tests/ocr.rs:260` release benchmark protocol; speed gap unmeasured.
C-optimize-007: f8 a8 p8 — `ocr_local.rs` mixed-script/TSV tests; non-Latin packs still external.
C-optimize-009: f8 a8 p8 — `tests/ocr_review.rs:60` confidence/save/neighbor/pixels/undo contract.
C-optimize-015: f8 a7 p8 — `search_index.rs:244`, `tests/search_index.rs:48` memory search/reuse/purge.
C-sign-009: f8 a8 p8 — `signature_store.rs:247` restart/>64K/no-tool contract; platform checks pending.
C-sign-012: f7 a8 p8 — `tests/sign.rs:476` distinct TSA fixture; no user-facing TSA transport.
C-security-007: f0 a0 p0 — no implementation; Standard-only reader still blocks PubSec reopening.
C-security-008: f8 a7 p8 — `tests/security.rs:222` exact PDF permissions/AES compatibility contract.

## Fourth pass — C-optimize-015

- C-optimize-015: Acrobat's Advanced Search options now reach catalog searches in the reader
  and CLI: whole words, match case, all/any words, bounded Boolean AND/OR/NOT with grouping and
  quoted phrases, unordered proximity (10 intervening words in the UI, configurable in CLI),
  and explicitly English Snowball stemming. Inverted word/stem/trigram postings prune page
  candidates; Boolean negation never complements an approximate posting set. The manager's
  Add folder includes subfolders through the existing recursive incremental builder.
- Tests written: `catalog_advanced_queries_cover_case_words_boolean_stems_and_proximity`,
  `catalog_refuses_invalid_or_excessive_boolean_queries_and_conflicting_options`,
  `catalog_search_controls_keep_mode_and_independent_options`.
- New small pure-Rust offline dependency: `rust-stemmers = 1.2` in workspace and sheaf-core.
  Orchestrator must fetch/update Cargo.lock. New public items `SearchMode`, `SearchOptions`,
  `Catalog::search_with_options`. Old catalog files and `Catalog::search` retain phrase defaults.
- `not automatable: Catalog → Add folder, build, run each search mode and checkbox, malformed
  Boolean query, open hit and cancel; check narrow/wide layouts — rule 1 forbids app execution`.
- Left: PDX interchange and non-English stemming; render, memory/time and compilation unverified.
  No network code. Proposed grade f9 a8 p8 (static evidence only; no joint closure claim).
