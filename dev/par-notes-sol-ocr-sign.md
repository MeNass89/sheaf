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

## Polaroid

- Active: eight-component OCR/sign/security batch. Signature storage implementation written and inspected;
  no tests executed. Next: local OCR language backend, search catalog and security policies.
- Dependencies added: none.
