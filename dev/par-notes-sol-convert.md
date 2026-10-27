# sol-convert — 27 October 2026

## Mandate (Tatouages)

- Work only in `par/sol-convert`; six components from `/tmp/f5/par/sol-convert.json`.
- The common brief forbids builds, compiler, tests, app, browser and Codex execution. Tests below
  are written for the orchestrator, never reported as passing.
- No network features. Do not choose LibreOffice bundling on Nassim's behalf (HANDOFF).

## Components

- C-convert-002 (partial): Office conversion can discover a packaged LibreOffice runtime beside
  Sheaf, ahead of a system installation (iLovePDF's offline Office conversion; test
  `a_packaged_office_runtime_is_found_without_a_system_installation`). `$SHEAF_SOFFICE` still wins.
  The runtime is **not bundled by this change**: distributing it, licence files, platform package
  integration and real Office corpus fidelity remain. Inspected `office.rs`, Office cache in
  `files.rs`, `tests/office.rs`, `dev/HANDOFF.md` and iLovePDF's `WordToPdfTool.qml`. The handoff
  explicitly reserves the distribution decision to Nassim. No dependency added.

- C-optimize-014 (partial): More ▸ More tools ▸ Fix page hairlines (0.25 pt) raises strokes in the
  selected pages' own content, or the current page (Acrobat's Fix Hairlines). It measures the
  width at painting time through the current transform, honours saved graphics state, ExtGState
  widths and UserUnit, and restores the width after painting. Add black crop marks (Acrobat's Add
  Printer Marks) enlarges the sheet, preserves effective trim/bleed/art boxes, clips the original
  content to its previous crop, and adds eight black crop marks outside the bleed. Repeated clicks
  add no duplicate marks. Each command is undoable. Tests
  `page_hairlines_are_measured_after_transforms_and_undone_in_one_step`,
  `page_hairline_repairs_honour_user_units_and_nonuniform_scaling`,
  `crop_marks_keep_inherited_page_boxes_and_do_not_invent_bleed` in `tests/engine_patches.rs`.
  Left: hairlines inside grouped Form artwork, patterns, text strokes and annotation appearances;
  configurable widths, general page-box editor, registration marks/colour bars, trapping and
  flattening preview. Inspected Acrobat `PrintProduction_R_RHP.aapp`, PdfCraft organize boxes,
  content parser, page-stream splice helpers, and engine edit/permission/undo handling.
  New public engine edits `FixPageHairlines`, `AddCropMarks`; public modules
  `pdfcraft_edit::prepress`, `pdfcraft_engine::prepress`. No dependency added.
  not automatable: verify both menu commands, page selection and undo in both themes — app
  execution forbidden in this worker.

- C-optimize-012 (partial): More ▸ More tools ▸ Print preflight report… saves Acrobat's Preflight
  style findings from the document **as edited**, on the document thread: effective image ppi,
  media/crop/trim/bleed boxes, bleed margins, output intents, unembedded fonts and structural
  PDF/A-2b issues. The report records source, check time, profile and its thresholds, and lists
  checks not performed. An existing report is kept. Test
  `print_preflight_reports_low_resolution_and_keeps_the_document_unchanged` exercises the engine
  check through sheaf-core (engine crate tests are excluded). Left: ink separations/output preview,
  editable profiles, automatic fixups and a full audit trail. Inspected PdfCraft `printcheck.rs`
  and Acrobat `PrintProduction_R_RHP.aapp`. No dependency added.
  not automatable: More tools ▸ Print preflight report… opens Save, writes and announces the report
  from an unsaved document — GUI execution forbidden in this worker.

- C-convert-005 (partial): HTML to PDF offers Original layout, A4 and Letter, portrait/landscape,
  and none/small/big margins (iLovePDF's HTML page settings). Explicit layouts insert print CSS
  after existing styles and preserve relative local resources with a file base. The output bypasses
  the default-layout preview cache. Markdown and mail use the same layout through their existing
  HTML converter (mail attachments kept); PostScript explicitly asks for Original layout.
  Tests `html_print_settings_keep_resources_and_override_the_page_rule`,
  `html_print_settings_reject_invalid_margins_and_existing_bases`,
  `html_layout_finds_tags_instead_of_comments_scripts_or_headers`,
  `custom_html_layout_explains_unsupported_encodings_and_postscript`,
  `html_layout_reaches_the_job_without_reusing_the_default_conversion`,
  `local_html_prints_on_the_chosen_paper_with_its_relative_picture`,
  `markdown_and_mail_use_the_chosen_html_paper_and_keep_attachments` (both require LibreOffice).
  Left: URL capture and screen mode; URL capture is a network feature forbidden by the batch brief.
  Files with an explicit `<base>` require Original layout; non-UTF-8 HTML requires Original layout.
  Inspected iLovePDF `HtmlToPdfTool.qml` and the existing Office pipeline. No dependency added.
  not automatable: visually check the HTML controls in both themes — app execution forbidden in this worker.

- C-convert-018 (**not implemented**): PDF/A remains 2b/3b. Inspected the core `pdfa.rs`, its
  integration tests, the app's `PdfToPdfA` job and level cards, the actual pinned
  `crates/preflight/src/lib.rs` (Level, verify and convert) and `xmp.rs`, plus iLovePDF's
  `PdfToPdfATool.qml` and Acrobat's `Standards_R_RHP.aapp`. The pinned engine implements only
  A2b/A3b. PDF/A-1b needs PDF 1.4 restrictions and transparency handling; a/u levels require real
  tagging/Unicode checks and repairs. A new label or XMP part number would not implement those
  standards. No converter or test was added for these absent levels. Ghostscript is installed on
  this worker but veraPDF is not; neither was executed. No claim of full standards validation.

- C-convert-019 (**not implemented**): no Save as PDF/X or PDF/E was added. Inspected Acrobat's
  `Standards_R_RHP.aapp` and `PrintProduction_R_RHP.aapp`, engine `catalog.rs`/`commands.rs`, and the
  pinned preflight crate. PDF/X is only a planned catalogue item in PdfCraft; there is no converter
  to expose, and the existing print-readiness report explicitly excludes PDF/X, overprint, spot
  colours and flattening. Real PDF/X needs a chosen press profile, output intent and version-specific
  colour/transparency/font rules; PDF/E needs a separate standards implementation. This batch does
  not supply them. No test added because no PDF/X or PDF/E conversion was implemented.

## Integration

- No new dependency, no Cargo manifest or lockfile change.
- `Job::Office` now has `html: Option<sheaf_core::html::Options>`; its constructor in
  `crates/sheaf-app/tests/jobs.rs` was updated. Other workers adding Office jobs must supply it.
- Public core modules added: `office_runtime`, `html`, `preflight`. Engine edits added:
  `FixPageHairlines`, `AddCropMarks`; new public `prepress` modules in edit and engine.
- Reader More tools appends indices 20–22. Reconcile indices with any other worker appending to
  that array. Slint callbacks and main.rs wiring are in small adjacent blocks.
- New engine/edit behaviour is exercised from `crates/sheaf-core/tests/engine_patches.rs`, not
  from the excluded vendored crate tests.
- All named tests are **written, not run**. The HTML integration test requires LibreOffice and
  skips when absent; integration tests using rendering need the existing PDFium setup. The
  orchestrator must compile and run the suite and perform the GUI checks above. In particular,
  confirm LibreOffice honours HTML print CSS on the supported platforms.
- Static checks performed: `git diff --check`; read API signatures from the actual pinned source;
  searched all Office job constructors; inspected undo/permissions/page-box refresh paths. The
  Impeccable static detector returned `[]` for the changed Slint targets; no rendered UI was judged.
- No board grades changed. None of the six components is claimed at parity or 9/10.

## Polaroid — handoff

- Status: worker implementation finished; ready for the orchestrator's compilation and review.
- Delivered: four partial implementations and their tests; two standards components inspected
  without an implementation. Constraints and all remaining gaps are recorded above.
- Verification limit: source inspection only. No cargo, rustc, clippy, tests, app, Chrome, Xvfb or
  codex executed. The remaining runtime checks belong to the orchestrator under the common brief.
- Next action: merge this branch with the other batches, reconcile shared Slint menu indices and
  callback blocks, then compile and run the actual tests and inspect the GUI.
- Problem-solving control: mandate **partial** (all six examined; four changed); evidence **yes**
  (code, tests, commit history); external dependency **yes** for Office packaging/network capture,
  **no** for absent standards implementations (not delivered, not disguised as an external block);
  independent work **partial** (printing/conversion subsets delivered; advanced gaps listed).

## Changed files

- App Rust: `crates/sheaf-app/src/jobs.rs`, `main.rs`, `run.rs`, `reader_doc/mod.rs`,
  `reader_ui/mod.rs`, `reader_ui/prepress.rs`; app integration test `crates/sheaf-app/tests/jobs.rs`.
- Slint: `crates/sheaf-app/ui/flow-types.slint`, `screens/options.slint`,
  `screens/reader-toolbar.slint`.
- Core: `crates/sheaf-core/src/lib.rs`, `office.rs`, `office_runtime.rs`, `html.rs`, `preflight.rs`;
  tests `crates/sheaf-core/tests/office.rs`, `preflight.rs`, `engine_patches.rs`.
- Vendored code: `third_party/pdfcraft-edit/src/lib.rs`, `prepress.rs`;
  `third_party/pdfcraft-engine/src/lib.rs`, `prepress.rs`.
- Notes: `dev/par-notes-sol-convert.md`.

## Pass 2 — C-optimize-012

- C-optimize-012: More ▸ More tools ▸ Print preflight… now opens a Slint sheet in the reader's
  house style (Acrobat's Preflight). Close-up / large-format presets and custom image/bleed
  thresholds change the actual checks, not just a report label. Results include source, time,
  the measured image/box/font/intent findings, and checks not performed. Page findings jump to
  their page. Save report keeps exactly the inspected snapshot, using a free name atomically.
  Test `print_preflight_reports_low_resolution_and_keeps_the_document_unchanged` now contrasts
  real findings under two profiles; `preflight_profiles_reject_thresholds_that_would_hide_failures`
  rejects invalid comparisons; `preflight_rows_keep_every_condition_and_split_long_findings`
  preserves all diagnostic words across rows of at most four lines plus a title.
- Left: ink separations, output simulation, editable fixup profiles, automatic corrections and a
  durable history of successive runs. Existing unembedded fonts cannot be repaired by this check.
- New engine API: `printcheck::Profile`, `Document::print_readiness_with`. `Op::PrintPreflight`
  now checks and replies with a report instead of writing a file; export writes the saved snapshot.
- No dependency. Tests written, not executed. Static inspection corrected calls to the engine's
  private inspector in its existing tests too. Reference inspected: Acrobat's real archive path
  `aapp/PrintProduction_R_RHP.aapp` (the input's prefixed path is not an archive member).
- not automatable: inspect light/dark sheet, presets, invalid values, page navigation, report Save,
  and an unsaved edit before rerun — worker app/build/test execution forbidden by rule 1.
- LIGNES : max 5 (preflight finding, one title + four lines) · cases > 6 : aucune.
- Provisional author grade: f6 a7 p7; source-only evidence, no parity claim. Independent counterpart
  review belongs after orchestrator merge/build/full green suite.

## Pass 2 — C-optimize-014

- C-optimize-014: More tools ▸ Fix hairlines / page boxes… and Add printer marks… open a proper
  Print production sheet (Acrobat's Print Production). The user chooses selected/current or all
  pages, types the minimum stroke (0.01–10 pt), or sets/resets Crop/Trim/Bleed/Art margins relative
  to each MediaBox in unrotated PDF units. Artwork is not moved. Crop marks remain black, outside
  existing bleed, with the sheet enlarged. Every action uses the permission/undo document path.
  The sheet explicitly says grouped art, text and comments are not hairline-repaired, and boxes
  should be set before adding marks. No new hidden CLI-only operation.
- Tests `print_boxes_preserve_margin_order_and_reject_bad_numbers` and
  `print_box_margins_use_each_media_origin_and_failed_changes_leave_it_intact`; existing stroke,
  UserUnit and crop-mark tests remain. Written only. No dependency or new engine edit.
- Left: Form/pattern/text/appearance hairlines, registration marks, colour bars, configurable mark
  dimensions, trapping and transparency flattening preview. Existing marks must be undone before
  changing trim/bleed then re-added; this pass does not reposition previously added marks.
- Preflight rows are now prepared once when its report arrives and shared with Slint (rather than
  rewrapping a large report on every reader model update).
- not automatable: light/dark production sheet, invalid stroke/margins, selection/all scope,
  changed boxes and one-step undo — runtime forbidden by rule 1.
- LIGNES : max 4 (Set page boxes / Crop marks, title + three lines) · cases > 6 : aucune.
- Provisional author grade: f5 a7 p7; source-only, not rendered or executed, no parity claim.
