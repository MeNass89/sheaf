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
