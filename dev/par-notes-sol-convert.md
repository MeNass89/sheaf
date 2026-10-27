# sol-convert — 27 October 2026

## Mandate (Tatouages)

- Work only in `par/sol-convert`; six components from `/tmp/f5/par/sol-convert.json`.
- The common brief forbids builds, compiler, tests, app, browser and Codex execution. Tests below
  are written for the orchestrator, never reported as passing.
- No network features. Do not choose LibreOffice bundling on Nassim's behalf (HANDOFF).

## Components — pass 1

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

## Integration — pass 1

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

## Polaroid — pass 1 handoff (superseded below)

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

## Pass 2 — C-convert-002

- C-convert-002: Tools ▸ Excel to PDF now offers saved print layout or one whole sheet per PDF page
  (iLovePDF's spreadsheet conversion layout). Saved layout keeps the original cached conversion;
  whole sheets use Calc's real `SinglePageSheets` export and bypass that cache. The app explicitly
  says hidden sheets are included and saved paper/print areas are ignored. Batch semantics,
  isolated temporary Office profile, timeout and atomic free output name are preserved.
- Primary evidence: installed LibreOffice `share/registry/main.xcd`'s `SinglePageSheets`, and
  [LibreOffice PDF CLI parameters](https://help.libreoffice.org/latest/en-US/text/shared/guide/pdf_params.html).
  Test `whole_sheet_export_keeps_outside_print_area_and_hidden_sheet_content` builds a real XLSX
  with a print area and hidden sheet, checks both layouts' text/pages, source preservation and
  qpdf cleanliness. `whole_sheet_export_does_not_apply_calc_settings_to_a_word_file` rejects a
  wrong source type. `whole_sheet_layout_is_only_used_for_spreadsheets_and_bypasses_html` checks
  the app job route. Written, not run; integration requires LibreOffice and PDFium.
- Left: sheet-by-name selection and editing cell print ranges inside Sheaf; a real Word/PPT fidelity
  corpus and font substitutions. The previous pass's packaged-runtime discovery is present, but
  bundling/distributing LibreOffice remains Nassim's reserved decision (HANDOFF), not implemented.
- Integration: `Job::Office.whole_sheets: bool` added; constructors updated. New public core
  `office::spreadsheet_to_pdf`. No dependency or preference field added. Whole-sheet layout may
  require a second conversion after the ordinary card preview; the default still uses its cache.
- not automatable: Excel tool's layout choice in both themes and compare wide/hidden sheets on
  output — app execution forbidden by rule 1.
- LIGNES : max 3 (whole-sheet layout radio card and its information) · cases > 6 : aucune.
- Provisional author grade: f8 a8 p8; unexecuted source and tests, no parity claim.

## Pass 2 — C-convert-005

- C-convert-005: the existing HTML-to-PDF app controls now also work on legacy encodings and
  ordinary explicit base addresses (iLovePDF's local HTML page settings). UTF BOMs take priority;
  declared HTML encodings are decoded without replacement, with Windows-1252 as the legacy
  fallback. The temporary copy declares UTF-8, including when no original declaration existed.
  Relative bases resolve against the source file; their target and escaped queries are kept.
  Original layout still uses the original converter/cache. The app explains local-only support.
- Source scanner ignores comments and script/style content; the attribute parser handles quoted
  and unquoted values. Numeric HTML entities in a base address still ask for Original layout;
  they are not silently resolved to a different resource. Malformed/unsupported encodings also
  give an actionable Original layout error.
- Tests `html_encodings_keep_accents_and_normalize_the_declaration`,
  `html_base_uses_the_original_file_and_keeps_target_and_escaped_queries`, and real-LO
  `legacy_html_with_its_own_relative_base_keeps_its_text_and_local_picture`; existing paper tests
  remain, and `html_print_settings_reject_invalid_margins_and_existing_bases` was renamed to
  `html_print_settings_reject_invalid_margins` because ordinary bases now work. All unexecuted.
- New direct dependency: `encoding_rs = "0.8"` in workspace and core manifests. Pure Rust, already
  present transitively in Cargo.lock and the worker's registry. The direct sheaf-core lock edge
  was added by hand to that existing locked package; no version resolution, cargo or lock
  generator was executed.
- Left: browser viewport/screen media, JavaScript rendering and one-long-page mode. Network URL
  capture remains deliberately out of scope. Inspected iLovePDF's actual archive member
  `code/rcc/iLovePDF/tools/HtmlToPdfTool.qml` (screen size and One long page controls) and the
  installed encoding_rs API. LibreOffice is not a browser rendering engine.
- not automatable: inspect local-only copy and custom paper output with accented text, a relative
  base and pictures in both themes — app/test execution forbidden by rule 1.
- LIGNES : max 3 (local-files information) · cases > 6 : aucune.
- Provisional author grade: f7 a8 p8; source-only, no full browser parity claim.

## Pass 2 — C-convert-018

- C-convert-018 (partial): PDF to PDF/A now refuses to publish a copy with unresolved built-in
  checks by default. The Slint options expose Keep a copy even if checks fail; opt in to keep the
  previous salvage behaviour and its clause/message report. The choice survives Run again.
  A failed strict conversion deletes its temporary file and preserves existing output files.
  New `pdfa::to_pdfa_with_policy`; existing `to_pdfa` keeps its public/CLI compatibility behaviour.
- Tests `strict_archival_output_keeps_existing_files_and_publishes_no_failed_copy`, expanded
  `pdfa_job`, and `options_come_back_as_they_were_without_their_passwords`. Written, not run.
  App job fixtures explicitly select their old permissive policy; normal app jobs default strict.
- Inspected again: pinned preflight `Level`, `verify`, `convert`; core `pdfa.rs` and its tests;
  iLovePDF's `PdfToPdfATool.qml` (Allow Downgrade); local Ghostscript's `PDFA_def.ps` prefix.
  The new choice controls unresolved issues, **not** iLovePDF's valid lower-conformance downgrade.
  Levels remain 2b/3b. PDF/A-1b and a/u conversion/validation are still **not implemented**.
  Ghostscript could provide a separate 1b conversion route, but needs profile/runtime discovery,
  execution safeguards and an independent standards validator; none was added or claimed run.
- Integration: `Job::PdfA.allow_incomplete` and `ToolOptions.pdfa_allow_incomplete` added, with job
  constructors, Slint restore/export and task-history serialization updated. No new preference
  or dependency for this component. The app explicitly calls its check structural, not full ISO.
- not automatable: strict failure, permissive partial result, batch mixed failures, Run again's
  option restored and light/dark options — runtime forbidden by rule 1.
- LIGNES : max 3 (archival-check information) · cases > 6 : aucune.
- Provisional author grade: f8 a8 p8; standard-level gap remains, source-only, no parity claim.

## Pass 2 — C-convert-019

- C-convert-019 (partial, replaces pass 1's no implementation): Print production now offers
  Save as PDF/X-3:2002, an offline Ghostscript conversion of the whole current working document.
  The operator chooses a CMYK/Gray **output** ICC profile and names the press condition. Profile
  header/class/channels/size are checked. The source document and its encryption/undo state are
  untouched; a temporary copy is converted, with bounded log tails and a 120-second child timeout.
  An existing destination is kept through atomic_write. Output is explicitly unprotected.
- The actual written file must have PDF 1.3, PDF/X-3:2002 metadata, GTS_PDFX output intent with
  the right channel count, and no unembedded fonts in the engine check before publication.
  These are limited structural checks, **not full ISO conformance validation**; the Slint panel
  and export completion say so. PDF/E, PDF/X-1 and PDF/X-4 are **not implemented**.
- Primary evidence: Acrobat `aapp/Standards_R_RHP.aapp`, engine catalog/commands, installed
  Ghostscript `PDFX_def.ps`, ICC header of the installed FOGRA39L profile, and
  [Ghostscript's documented PDF/X route](https://ghostscript.readthedocs.io/en/gs10.05.1/VectorDevices.html#creating-a-pdf-x-document).
  A full PDF/X validator was not found (`command -v verapdf` returned no path; veraPDF alone
  would not supply general PDF/X/PDF/E verification anyway). No Ghostscript executable was run.
- Tests `press_profiles_reject_rgb_and_postscript_strings_cannot_escape`,
  `pdfx3_uses_the_chosen_press_profile_and_preserves_the_working_document`,
  `a_converter_that_claims_success_without_pdfx_and_a_timeout_publish_nothing`. All written,
  not run. The real export test requires Ghostscript and a real CMYK profile (environment
  `SHEAF_TEST_PRESS_PROFILE`, else the known Linux FOGRA39L profile), plus PDFium/poppler/qpdf.
- Public core module `press`, APIs `Options`, `validate`, `find_ghostscript`, `to_pdfx3`,
  `to_pdfx3_with`. Reader `Op::ExportPdfX` and `Reply::PdfXExported` handle the document thread.
  No new menu index, no crate dependency. External runtime: Ghostscript from PATH or
  SHEAF_GHOSTSCRIPT; macOS Homebrew's normal paths also detected. Ghostscript is not bundled.
- not automatable: choose ICC/condition, export unsaved changes, wrong/RGB ICC, missing runtime,
  existing destination, password/permission cases and both themes — rule 1 forbids execution.
- LIGNES : max 4 (PDF/X section, title + three lines) · cases > 6 : aucune.
- Provisional author grade: f4 a7 p6; genuine conversion code, but unexecuted and no standards
  validator, PDF/E, or full Acrobat parity. Counterpart must review after the green merged suite.

## Pass 2 — final static integration audit

- ReaderUi::send already increments in_flight. Removed the extra manual increments from the
  new preflight/PDF-X paths; their replies now balance exactly one send count. Do not copy the
  older accessibility caller's extra increment into these paths.
- Preflight page links also recognize structural-check `- page N:` and bleed `- [warn] Page N`
  diagnostics. Expanded the row test for those forms and for a document title containing "page".
- Added only the direct encoding_rs edge to sheaf-core in Cargo.lock, using the already locked
  package/version/checksum. No cargo invocation, dependency resolver or compiler was run.
- not automatable: preflight and PDF/X on a previously clean document must leave it clean after
  completion, with Undo/close state restored — runtime execution forbidden by rule 1.

## Polaroid — pass 2 handoff, current

- Status: source implementation delivered for all six components; all six remain **partial**.
  Six component commits were pushed individually, then the request-count/dependency correction.
  No component was closed or board-graded. No counterpart review has happened; it belongs after
  the orchestrator merges, builds and runs the full suite green.
- Done: Slint preflight with custom profiles and page links; print-production sheet with stroke,
  scope and page-box controls; whole-sheet Excel option; HTML encoding/base preservation;
  strict/permissive archival publication choice; ICC-guided offline PDF/X-3 export.
- Not done: LibreOffice distribution and fidelity corpus; browser/URL/long-page HTML; extra PDF/A
  levels and full ISO validation; PDF/E and other PDF/X versions; separations, output simulation,
  full fixup/audit profiles, nested artwork hairlines, trapping and flattening preview.
- Runtime dependencies: existing LibreOffice; optional Ghostscript for PDF/X-3. Core adds the small
  pure-Rust encoding_rs direct dependency already present in the lock. No network service added.
- Verification actually performed: live source/API inspection, all relevant Office/PDF-A job
  constructors searched, static send/reply count audit, git diff --check, final Impeccable static
  detector `[]`, local and remote branch heads compared. No cargo, rustc, clippy, tests, app,
  Chrome, Xvfb or codex executed. No render was visually judged.
- New tests and amended tests are named under each component above. The orchestrator must execute
  them; LibreOffice, Ghostscript, the real press ICC and the existing PDFium/poppler/qpdf setup
  are needed for their corresponding integration checks. Optional-runtime skips are not passes
  proving conversion parity.
- First action: integrate this branch, reconcile reader menu indices 20–22 with other workers,
  compile Slint/Rust and run the real suite. Then inspect both sheets and options in both themes,
  source cleanliness after inspections, unsaved-document export, permissions and cancellation.
  New callbacks are wired in main.rs; no extra menu index beyond pass 1 was allocated.
- Integration fields: Job::Office.whole_sheets; Job::PdfA.allow_incomplete;
  ToolOptions.pdfa_allow_incomplete (including restore and saved history); ReaderState's
  production/preflight/press fields. No new Prefs field. New engine API printcheck::Profile and
  Document::print_readiness_with. Core press exports and the renamed PrintPreflight operation
  are described above.
- Control: mandate **partial**, with independent source work delivered on all six; evidence **yes**
  for code/static checks, **not yet** for runtime; dependencies **yes**, named above, while the
  omitted standards/advanced engines are unfinished work rather than a claimed external block;
  independent work **yes within this pass**, with all remaining feature gaps retained explicitly.

## Pass 2 changed files (complete)

- Cargo.toml, Cargo.lock; crates/sheaf-core/Cargo.toml.
- crates/sheaf-app/src/jobs.rs, main.rs, run.rs, saved_options.rs, reader_doc/mod.rs,
  reader_ui/mod.rs, reader_ui/model.rs, reader_ui/prepress.rs.
- crates/sheaf-app/tests/jobs.rs.
- crates/sheaf-app/ui/flow-types.slint, screens/options.slint, screens/reader-toolbar.slint,
  screens/reader.slint, screens/reader-prepress.slint, screens/reader-production.slint.
- crates/sheaf-core/src/lib.rs, html.rs, html_source.rs, office.rs, pdfa.rs, preflight.rs, press.rs.
- crates/sheaf-core/tests/engine_patches.rs, office.rs, pdfa.rs, preflight.rs, press.rs.
- third_party/pdfcraft-engine/src/printcheck.rs; dev/par-notes-sol-convert.md.

## Author grades — provisional, source-only

C-convert-002: f8 a8 p8 — whole_sheet_export_keeps_outside_print_area_and_hidden_sheet_content,
crates/sheaf-core/tests/office.rs:276; saved print areas vs whole/hidden sheets, source preserved;
UI crates/sheaf-app/ui/screens/options.slint:641. Bundling/fidelity/sheet selection still missing.

C-convert-005: f7 a8 p8 — legacy_html_with_its_own_relative_base_keeps_its_text_and_local_picture,
crates/sheaf-core/tests/office.rs:312; legacy text and a real relative local picture under custom
paper settings. Screen media/URL/long page remain absent.

C-convert-018: f8 a8 p8 — strict_archival_output_keeps_existing_files_and_publishes_no_failed_copy,
crates/sheaf-core/tests/pdfa.rs:99; failed structural checks prevent publication unless opted in;
UI crates/sheaf-app/ui/screens/options.slint:1099. Extra levels and full validation still missing.

C-convert-019: f4 a7 p6 — pdfx3_uses_the_chosen_press_profile_and_preserves_the_working_document,
crates/sheaf-core/tests/press.rs:8; ICC-guided PDF/X-3 conversion, marker/font checks and source kept;
UI crates/sheaf-app/ui/screens/reader-production.slint:75. PDF/E and full validation not delivered.

C-optimize-012: f6 a7 p7 — print_preflight_reports_low_resolution_and_keeps_the_document_unchanged,
crates/sheaf-core/tests/preflight.rs:5; changing real profile thresholds changes findings without
editing; UI crates/sheaf-app/ui/screens/reader-prepress.slint:8. No separations or automatic fixes.

C-optimize-014: f5 a7 p7 — print_box_margins_use_each_media_origin_and_failed_changes_leave_it_intact,
crates/sheaf-core/tests/engine_patches.rs:117; media-relative boxes, failure atomicity and undo;
UI crates/sheaf-app/ui/screens/reader-production.slint:8. Advanced print-production gaps above.

All cited tests are WRITTEN, NOT RUN. These numbers are not joint grades or closure requests.
LIGNES : max 5 (preflight finding: title + four lines) · cases > 6 : aucune.

## Pass 3 — C-convert-018

- C-convert-018: restored the previous copy-plus-issues default, including old task history
  without the new policy field (iLovePDF's permissive conversion choice). Explicit strict mode
  still prevents publication and survives Run again. UI now starts checked; `pdfa_job` checks
  permissive then strict, and `archival_history_keeps_the_old_copy_and_issues_default_but_honours_strict`
  covers migration and strict round-trip. Tests written, not run (brief rule 1).
- Left: PDF/A-1b and a/u conversion, valid level downgrade, full ISO validator. The pinned Rust
  preflight supports 2b/3b only; no relabelled output or unsupported certification added.
- No dependency. not automatable: fresh PDF/A screen has Keep a copy checked; Run again restores
  an explicitly unchecked strict choice — worker cannot launch the application.
- Grade: f7 a8 p8, partial; fixes the regression, does not close standard-level parity.

## Pass 3 — C-convert-002

- C-convert-002: Excel to PDF has an explicit Built-in data report, entirely Rust (no runtime
  discovery, child process or network). XLS/XLSX/XLSB/ODS data pages keep cell addresses, stored
  values and accents; long values wrap and paginate. Visible sheets by default, numbered sheet
  ranges select hidden sheets deliberately and preserve order. Existing outputs and sources
  stay intact. The app permits this mode without LibreOffice and bypasses the preview cache.
- Tests `native_data_report_selects_sheets_preserves_cells_and_needs_no_office_runtime`,
  `native_data_report_wraps_long_values_and_refuses_missing_glyphs_atomically`,
  `native_spreadsheet_choice_keeps_sheet_selection_and_bypasses_the_office_cache`. None skips
  for missing LibreOffice; written, not run. Uses lopdf assertions without external PDF tools.
- Dependency: existing pure-Rust `calamine` moved from dev to regular core dependencies; already
  workspace-declared and locked, no new package/version or lock change. New public module
  `sheaf_core::spreadsheet`; `Job::Office.data_sheets: Option<String>` added and constructors
  updated. No preference field.
- Left: faithful Office import still requires LibreOffice (unbundled); Word/PowerPoint options,
  sheet selection in the faithful converter, charts/styles/formula recalculation in the native
  report. Native mode reports raw dates and supports Western European text only; other glyphs
  fail clearly with Saved print layout guidance, without publishing a damaged PDF. Not parity.
- not automatable: without LibreOffice select data report, choose sheets, Run, inspect both themes
  and confirm Run again restores layout/ranges — app execution forbidden.
- Grade: f8 a8 p8 (partial). LIGNES : max 5 (data-report information) · cases > 6 : aucune.
