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

## Polaroid

- Active: implement the remaining independent local conversion and print-production work.
- Verification: source inspection only; build and execution belong to the orchestrator.

- C-optimize-014 (partial): More ▸ More tools ▸ Fix page hairlines (0.25 pt) raises strokes in the
  selected pages' own content, or the current page (Acrobat's Fix Hairlines). It measures the
  width at painting time through the current transform, honours saved graphics state, ExtGState
  widths and UserUnit, and restores the width after painting. Add black crop marks (Acrobat's Add
  Printer Marks) enlarges the sheet, preserves effective trim/bleed/art boxes, clips the original
  content to its previous crop, and adds eight black crop marks outside the bleed. Repeated clicks
  add no duplicate marks. Each command is undoable. Tests
  `page_hairlines_are_measured_after_transforms_and_undone_in_one_step`,
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
  the default-layout preview cache. Markdown/mail/PostScript retain their existing converters.
  Tests `html_print_settings_keep_resources_and_override_the_page_rule`,
  `html_print_settings_reject_invalid_margins_and_existing_bases`,
  `html_layout_reaches_the_job_without_reusing_the_default_conversion`,
  `local_html_prints_on_the_chosen_paper_with_its_relative_picture` (requires LibreOffice).
  Left: URL capture and screen mode; URL capture is a network feature forbidden by the batch brief.
  Files with an explicit `<base>` require Original layout; non-UTF-8 HTML requires Original layout.
  Inspected iLovePDF `HtmlToPdfTool.qml` and the existing Office pipeline. No dependency added.
  not automatable: visually check the HTML controls in both themes — app execution forbidden in this worker.
