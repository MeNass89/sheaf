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

## Polaroid

- Active: eight-component OCR/sign/security batch. Signature storage implementation written and inspected;
  no tests executed. Next: local OCR language backend, search catalog and security policies.
- Dependencies added: none.
