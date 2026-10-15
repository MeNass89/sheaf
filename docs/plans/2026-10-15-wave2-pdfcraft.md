# Sheaf Wave 2 — Ship every "Soon" tool on the PdfCraft engine

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** All 12 tools currently marked "Soon" in Sheaf work in the GUI and the CLI: OCR, PDF→Word, PDF→PowerPoint, PDF→Excel, PDF→PDF/A, Edit (add text and images), Page numbers, Watermark, Crop, Sign, Redact, Compare. "Open With" from Finder works too.

**Architecture:** We add one adapter module per tool in `sheaf-core`, behind a thin façade over [storytold/pdfcraft](https://github.com/storytold/pdfcraft)'s engine. PdfCraft is Apache-2.0 (compatible with our GPL-3.0 distribution) and pinned by git rev. The flow is: `pdfcraft_engine::Session::open(bytes, password)`, then `Session::apply(id, Edit::…)` or one of its tool methods, then `save_full_bytes`, then our own `paths::atomic_write`. Our existing pdfium/lopdf tools stay as they are; we only add.

**Tech:** pdfcraft-engine @ `d3a91c5c5f7b33dd39bc2ed8b5d3916960736d24` (with its `[patch.crates-io]` for hayro*, lopdf). OCR models from `ocrs` (`text-detection.rten` 2.5 MB, `text-recognition.rten` 9.7 MB) are shipped in `Sheaf.app/Contents/Resources/models/`.

**Verified on 15/10/2026 (probe /tmp/pc-probe):**
- `pdfcraft-engine` builds as a git dependency only with PdfCraft's patches (`[patch.crates-io] hayro, hayro-interpret, hayro-syntax, hayro-jbig2, lopdf` → the same git rev). Without them: 10 compile errors (missing `ColorReplacement`, `ocg_overrides`…).
- `Session::new()` → `open("g.pdf", None, Arc<Vec<u8>>, None)` → `save_full_bytes(id)` round-trips WelcomeGuide.
- `pdfcraft_engine::ocr::available()` is false until the models are present (search path: `$PDFCRAFT_MODELS`, `<exe>/../Resources/models`).
- Cost: about +15 MB stripped binary. The bundle goes from 29 MB to about 56 MB with the models. That is still about 6× smaller than the original.

## Global constraints
- Wave-1 constraints still hold: no overwrite, `.part` then claim, pdfium lock for pdfium use, clean room, GPL-3.0 distribution.
- **Our lopdf becomes PdfCraft's patched lopdf** (the `[patch.crates-io]` applies to the whole workspace). Its patches are safety fixes on top of 0.45.0 (overflow, png predictor bounds). Every wave-1 test must still pass unchanged.
- The PdfCraft engine is not thread-safe per `Session`. Each job creates its own `Session` on its worker thread, and nothing is shared. We do not know whether PdfCraft's renderer (hayro) is thread-safe across sessions. Task 1 tests it. If it is not, we add a second global lock `pdfcraft_lock()`.
- Each new tool exists in three places: an `sheaf-core` fn with the signature `fn x(input: &Input, opts, out: &Path) -> Result<Outcome>`, a CLI subcommand, and a GUI panel. Each has at least one integration test on a real PDF, and a `qpdf --check` on its output where the output is a PDF.
- Tool panels copy the original's options (iLovePDF's UI, observed) wherever PdfCraft supports them.
- Licensing: PdfCraft's crates and their dependencies are added to the cargo-about notices automatically. We add a "Built on PdfCraft (Apache-2.0)" line in THIRD_PARTY.md and the README.

## Tasks
1. **Engine bridge:**
   - add the dependency and the patches;
   - write `sheaf-core/src/pdfcraft.rs` (open with password, which maps PdfCraft errors to `SheafError`, then save through `atomic_write`);
   - run the whole wave-1 suite under the patched lopdf;
   - stress-test 8 threads with 8 Sessions at once (open + render + save) to settle the hayro thread-safety question.
2. **Page numbers + Watermark + Crop.** These map to `Edit::NumberPages`, `AddHeaderFooter` and `AddWatermark` (text or image, opacity, rotation, position), and `SetPageBox` (crop by margins, all pages or the current page). Each gets its options panel.
3. **PDF→PDF/A.** Uses `Edit::ConvertPdfA` (A-2b, A-3b), then reports verification issues with `pdfa_verify`. The test validates with veraPDF if it is installed; otherwise it checks that preflight `verify` returns no issues.
4. **OCR.** Uses `ocr::recognize_file` (dpi, language, skip pages that already have text) with a progress bar. A `fetch-models.sh` script downloads the models, and the bundle ships them. The test: render a page of text to an image, turn the image into a PDF (no text layer), run OCR, then search for the words.
5. **PDF→Word/PowerPoint/Excel.** Uses `export_office(OfficeFormat)`. The test opens the .docx, .pptx and .xlsx as zip files and checks that the expected text is in them. The UI says plainly "layout is approximate", as PdfCraft documents.
6. **Compare.** Uses `compare` (word-level) plus `compare_visual`. It writes a report PDF, and the result page shows the counts (added, removed, changed). The UI is two file slots, Old and New, matching the original's Compare tool.
7. **Redact.** Search for text (with PdfCraft's patterns: e-mails, phones, cards…) or mark pages, then `ApplyRedactions`. After that, `audit_redactions` must report no faux redaction, and that is asserted in the test. This is a minimal page viewer with the matches highlighted and checkboxes.
8. **Sign.** Two parts:
   - (a) a visual signature: drawn, typed with `typed_signature_graphic`, or an image, placed by clicking on a page preview;
   - (b) an optional digital certificate (a .p12 file + password, using `DigitalId`).
   The test signs with a test .p12 generated by `openssl`, then checks that `is_signed` is true and `pdfsig`/`qpdf` see the signature.
9. **Edit (add text and images).** Uses `Edit::AddText` and `Edit::AddImage` on a page viewer: click to place, drag to move, set font size and colour.
10. **Page viewer widget.** Tasks 7, 8 and 9 all need it. It renders pages at screen scale on the probe pool, scrolls, maps a click to page coordinates, and draws overlay rectangles. It gets built first, as Task 6b, before tasks 7–9.
11. **"Open With" from Finder.** A winit/AppKit `application:openURLs:` hook (objc2), and the PDF document types go back into Info.plist.
12. **Bundle + bench + release v0.2.0.** Rebuild the DMG, publish the GitHub release, update the README's tool table.

## Review focus
1. Password-protected input on every new tool (PdfCraft's own password path) leads to our password dialog.
2. Redact never leaves the text extractable under the black box: the audit test runs on every redaction fixture.
3. OCR on a page that already has text does not duplicate the text (`skip_text_pages`).
4. Watermark, page numbers and crop on rotated pages land in the visual position the user expects.
5. Sign: the output stays valid after our `atomic_write` copy (the bytes are written as they are, with no re-save that would break the signature).
