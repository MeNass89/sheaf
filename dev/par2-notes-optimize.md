# Round 30 · optimize

## Tatouages

- Mandate 28/10/2026: `/tmp/f5/v2/brief.md` and the round-31 request; branch `par2/optimize`, merged baseline `d5c3a54` (dev/round30). Only this worktree; one build/test at a time through wbuild (10 GB/four shared slots); own display `:114`, never `:99`. Preserve checks, run all 340–344 together, commit/push each component. Completion requires green app/lint/unit/GUI and verified remote source commits.
- Grades below are provisional author estimates, not independent agreement. No component is certified at 9.

## Polaroid

Closed — round 31, 2026-10-28. The dev/round30 baseline is merged; all assigned failed flows are fixed, the whole GUI range 340–344 passes together, and required lint/unit are green. Source commits d53919a (monitor ICC) and fac1808 (calibrated optimize flows) are pushed to par2/optimize. No production source edits after the successful app/GUI runs.

Proof: app build EXIT 0 and cmp-equal own target/copy; GUI /tmp/f5/v2/optimize-r31-gui-2.txt (ALL PASS); lint run-u6371 (PASS lint/GREEN/EXIT 0); unit run-u6413 (PASS unit/GREEN/EXIT 0). Both suites use JOBS=1 to keep generated Slint analyses inside the 10 GB cap. Notes below archive this mission's constraints, evidence and remaining parity gaps together. No active work or dependency remains for the round-31 request. Component grades stay provisional and below 9.

Recovery: pre-merge source remains in immutable stash 07b524b406a4e5e8324465be2b963f48cf40bf77. Stash references are shared across worktrees; restore by immutable object hash. Display :114 only; :99 and the recorder were untouched.

## Components

- C-optimize-005: Acrobat's **PDF Optimizer / Fast Web View**. Users can request a real local Ghostscript PDF 1.3 transparency pass and qpdf linearization, in that order, before atomic publication. Page count, lack of transparency and linearization are checked. Cancellation kills/waits for the owned process; forms, layers, signatures, tags and attachments are refused rather than silently lost. Existing explicit tag removal can prepare an untagged copy. Metadata removal is reapplied after Ghostscript. Target-size reporting uses the final bytes and progress stays monotonic.
  Tests passed: `optimizer_linearizes_and_flattens_before_publication`, `optimizer_cancellation_and_interactive_content_never_publish`, `advanced_optimizer_keeps_progress_monotonic_and_reports_postprocessing_size`; all 22 compression integration tests. An unchanged CMYK grayscale test exposed an existing unsupported YCCK→CMYK decoder request; the compressor now reads raw JPEG channels and converts their actual color transform, and the unchanged test passes.
  Left: Acrobat's selective flattening and raster/vector balance. GUI step 340 now passes on the built app, checking real PDF 1.3, qpdf linearization, four pages and extracted text. Estimate f8 a7 p8.

- C-optimize-006: iLovePDF's **Searchable OCR**. Complete installed Tesseract packs are now preferred by default, avoiding RTEN for those languages; missing Latin packs retain the bundled offline fallback. The explicit checkbox requires local packs instead of silently falling back.
  Tests passed: `local_ocr_prefers_installed_language_packs_and_falls_back_to_bundled_latin`, `automatic_installed_ocr_matches_explicit_local_text_and_preserves_scan`; all 14 active OCR integration tests, including accents, rotated labels, skewed scans, encryption, cancellation and no duplicate text. On this shared host the short-page comparison was automatic 11.939 s / explicit 12.573 s; this is **not** the MICI performance benchmark.
  Left: the recorded 15.5 versus 11.75 s/page gap is not certified closed. The original four-page MICI scan was not found in the main checkout/work area; `dev/features/F-006-ocr.md:74` and the existing ignored release benchmark were inspected. Bundled RTEN performance on installations without Tesseract is unchanged. GUI step 344 now passes: automatic OCR writes a qpdf-clean searchable PDF containing “Sheaf OCR test 2026”. Estimate f8 a7 p8.

- C-optimize-007: iLovePDF's **OCR Languages Folder / language list**. Local Chinese, Japanese and Korean packs are discovered beside the executable, in macOS Resources, Linux share/sheaf/tessdata, or SHEAF_TESSDATA. Debug builds also discover ignored vendor/tessdata. Portable packs do not need a copied configs/tsv file: TSV is requested directly, fixing the actual plain-text result exposed by the recognition tests.
  Tests passed: `portable_ocr_language_folders_accept_real_packs_and_reject_empty_folders`, `portable_cjk_packs_are_found_in_windows_macos_and_linux_layouts`, `installed_cjk_packs_recognize_and_save_searchable_unicode_without_repainting`. Actual recognitions: Japanese `日本語の文草です`, Chinese `中文识别测试`, Korean `한국어문서입니다`. Each saved PDF extracts the expected script, stays qpdf-clean and renders exactly like its source scan. Three committed scan fixtures total 14,587 bytes.
  Left: release installers still need to supply Tesseract and traineddata; Linux-only work cannot certify Windows/macOS packaging. No product download/network feature was added. Estimate f8 a7 p8.

- C-optimize-012: Acrobat's **Output Preview**. The actual current working page renders into selectable Cyan/Magenta/Yellow/Black and painted spot masks. Work runs off-thread on captured bytes; closing/replacing the preview cancels its owned renderer and stale/closed-tab results are discarded. Size, plate count, page range and timeout are bounded. Existing preflight target profiles/reports remain available.
  Test passed: `output_preview_reads_painted_process_and_spot_plates_without_editing`, including actual Cyan and BrandOrange spot paint, an empty Black plate, cancellation, invalid page and unchanged source bytes. GUI step 342 now passes: all four actual plate buttons are located and Black→Yellow→Black changes the image-only ink density, with source SHA-256 unchanged.
  Left: combined plate toggles, calibrated press ICC soft proof, total-ink inspection, editable check profiles and complete PDF/X certification. `reader_ui/prepress.rs`, `reader-prepress.slint`, core preflight/press and actual Ghostscript tiffsep output were inspected. Estimate f7 a7 p8.

- C-optimize-014: Acrobat's **Flattener Preview**. Print Production can compare the selected working page with its real PDF 1.3 conversion, without changing the document or undo history. Only that page is converted. Existing undoable hairline, printer-mark and page-box tools remain available.
  Test passed: `flattener_preview_renders_the_actual_conversion_and_keeps_source_bytes`, including transparent artwork, the actual second page, cancellation and unchanged source bytes. GUI step 343 now passes: both Original and Flattened PDF 1.3 render “Page 1”, and the source SHA-256 is unchanged.
  Left: trapping, highlighting individual affected objects and raster/vector balance. Trapping was not implemented; `reader-production.slint`, `reader_ui/prepress.rs`, `press.rs` and the existing engine print-production paths were inspected. Estimate f7 a7 p8.

- C-preferences-005: Acrobat's **Display colour management**. Choose an RGB monitor ICC, cache the real sRGB→monitor transform, apply it after page tinting to pages/thumbnails and invalidate both split panes immediately. The preference persists; reset restores sRGB. Invalid/printer/device-link/oversized profiles refuse before replacing the active profile; a missing saved profile visibly falls back to sRGB. Alpha is preserved.
  Tests passed: `display_icc_changes_monitor_values_and_preserves_alpha`, `display_icc_refuses_printer_profiles_and_bad_files`, running the actual desktop module through the small core integration harness. `monitor_icc_is_applied_to_real_rendered_pages_without_changing_pdf_bytes` and `prefs_round_trip_and_corrupt_means_default` pass in the final full unit suite. GUI step 341 passes: a98.icc is persisted and reset to sRGB.
  Left: physical monitor/OS compositor confirmation; the real desktop profile selection/persistence smoke passes. No claim of calibrated press proof. Estimate f8 a7 p8.

## Dependencies and integration

- Direct `moxcms 0.8.1` in the app and test-only core harness: pure Rust, already resolved through image, no additional external package in Cargo.lock. The core harness includes the exact desktop source file, not a reimplementation.
- Optional installed Ghostscript and qpdf for requested optimizer/preview operations; Ghostscript was already used by PDF/X. CI tool installation now includes Ghostscript so the real new process tests run.
- Optional local Tesseract/traineddata. Test-only ignored vendor/tessdata has official tessdata_fast Chinese 2,469,156 bytes, Japanese 2,471,260, Korean 1,677,415, plus installed English/French links. These packs are not committed or silently downloaded by the app. Ignored models/craft-fonts link to the main checkout's existing resources.
- Shared seams are small blocks: Advanced/ToolOptions add two booleans; Prefs adds an optional monitor path and the history literal is updated. Output preview has its own global/struct instead of extending ReaderState. Module/export/callback additions are next to their peers. Smoke helpers use optimize_ names.
- Reference inspected: Acrobat `aapp/OptimizePDF_R_RHP.aapp` (Fast Web View / image-font settings), iLovePDF `strings/iLovePdfDesktop.en.po.txt` (OCR Languages Folder / Document languages / OCR PDF).
- A tool-free unblock consultation used claude-review, session 2290629e-f9d5-4b72-8644-81bd7f4c00e7; briefing and JSON under ignored dev/.tmp/build-consult-*. This was not a final review or grade agreement.

## Historical round-30 verification (superseded by round 31 below)

- Core lib, own service run-u4801: `test result: ok. 113 passed; 0 failed; 0 ignored`.
- Compression integration, run-u4812: `test result: ok. 22 passed; 0 failed; 0 ignored`.
- Actual ICC module, run-u4819: `test result: ok. 2 passed; 0 failed; 0 ignored`.
- OCR, run-u4819: `test result: ok. 14 passed; 0 failed; 1 ignored` (the pre-existing release-only MICI benchmark). PyMuPDF is unavailable, so its optional MuPDF-encryption subcase is skipped; the qpdf foreign-encryption cases pass.
- CJK, run-u4819: `test result: ok. 1 passed; 0 failed; 0 ignored`, with all three installed scripts actually run, as quoted above.
- The generic wbuild cargo pipe masks ordinary Cargo failures with its final EXIT 0; actual test-result lines above were checked from the own unit journals. Suite/app commands propagate their real result.
- App build: repeated EXIT 1 / cgroup oom-kill at the unchanged 6 GB limit. Own examples: run-u4445, run-u4448, run-u4476, run-u4738, run-u4758, run-u4771, run-u4841. Metadata-only single-job app check passed before the final small ICC error-message correction; this is not an executable proof.
- Final suite lint: RED (session11008, own run-u4830), three chunks_exact_to_as_chunks errors. Fixed mechanically with the same complete RGBA chunks, no checks disabled; rerun required.
- Final suite unit: pending.
- Smoke340–344: **not run, no PASS lines**. No app binary was produced. Display:114 remains unused; :99 and the recorder were untouched.
- not automatable here: original MICI timing on its one-core target — source input absent and this is a shared build host; physical monitor calibration / Windows-macOS install behavior — hardware/platform unavailable.

## Handoff

Ready to merge par2/optimize for round 31: app, required lint/unit, and the full GUI range 340–344 all pass. Source commits d53919a and fac1808 are pushed. Use JOBS=1 for the suite layers under the 10 GB cap. No assertion was weakened; ink density now measures the page image alone, and the flattener check reads both rendered pages and verifies unchanged source bytes. Component grades remain provisional and below 9, with parity gaps retained above. The round-30 no-build/no-smoke warnings are historical and superseded.

## Round-31 execution archive

Tatouages (2026-10-28): merge dev/round30; build only with wbuild optimize (10 GB/four shared slots), one build/test at a time; GUI display 114 only; run 340–344 together until all pass; preserve checks; commit and push par2/optimize. User explicitly authorizes the push.

Polaroid: baseline fast-forwarded to d5c3a54 (dev/round30). Existing uncommitted display ICC work preserved in immutable stash 07b524b406a4e5e8324465be2b963f48cf40bf77 and restored, with additive conflicts resolved. Smoke 344 moved after 343, before 350. First app build pending, exec session 24226. Historical round-30 screenshots show 340/344 still on Tools with an empty search field; preview screenshots show plate buttons at x198–368 whereas old OCR crops cut them at x250/300 and find the selected-name header instead. Next: reproduce 340–344 on the new build, then calibrate only the proven faulty interaction/crops and inspect actual outputs.

Round-31 calibration: exact historical screenshots reproduce the plate-label failure with the old crop; a 180×180 crop at (195,175) returns complete unselected button labels. 342 now waits for and clicks the actual Black button, then reads Cyan after it becomes unselected; this excludes the "Black shows ink" help text and retains both image-density comparisons. 343 chooses the actual Flattened button, then reads the now-unselected Original button. 340/344 explicitly click the visible Tools search field before typing. No existing assertion removed or relaxed. Pending: execution on the freshly built local app.

Build acquired its slot at 2026-10-28 00:14 UTC. Disk shared by all workers dropped to 536 MB free. Deleted only six obsolete own-cache products (four old test executables and two generated sheaf_app .rmeta files, 505 MiB) from ~/.cache/cargo-target/w2-optimize/debug/deps; no source or other worker's cache touched. Current app build remains active, session 24226.

First round-31 app attempt ended `EXIT 101`, decisive error `No space left on device` (pdfcraft-sign). Removed older duplicate own-cache variants (1,639 MiB, 953 products; Cargo regenerates required variants), then relaunched the same authorized build, exec session 19110. Free disk rose to 4.0 GB during retry. Current app build in progress. Impeccable scoped context loaded for inherited ICC preferences; no new surface/redesign; existing Segment/Theme/RTL patterns retained.

343 additionally checks actual page text in both original and converted image regions and verifies the source SHA-256 remains unchanged; the old button-presence assertions remain. This closes a false-pass risk in the previous UI-only assertion while preserving its intent.

App build succeeded at 2026-10-28 01:00 UTC: `Finished dev profile ... in 42m 49s`, `EXIT 0`; own target binary equals `dev/.tmp/sheaf-app-under-test` by cmp (235 MB). The running wrapper was edited externally during its execution (its new disk-space guard is present now), producing a trailing shell EOF after EXIT 0; current `bash -n /tmp/f5/v2/wbuild.sh` passes. App bytes and successful Cargo build are independently verified. GUI first full range 340–344 active, session 85487, results /tmp/f5/v2/optimize-r31-gui-1.txt, display 114.

First full GUI 340–344: all five steps and the real-profile isolation check passed. All five screenshots visually inspected. Review caught an additional false-positive risk: 342's density crop overlapped the selected plate button and started below the page's top text. Moved the same 500×300 image measurement to (450,180), entirely inside the page image, retaining the exact 0.0003 density threshold and both Black→Yellow→Black comparisons. Full range rerun required after this strengthening.

Second full GUI range, after the image-only 342 measurement fix: every step 340,341,342,343,344 passed together; real user profile unchanged; ALL PASS. Proof: /tmp/f5/v2/optimize-r31-gui-2.txt and dev/.tmp/steps-114/*.png. Required lint now running; unit next. No GUI assertion was weakened; 343 and the image measurement were strengthened.

First required lint run ended EXIT 1 before the suite could report a code diagnostic. The user journal confirms own run-u6339.service `Failed with result 'oom-kill'`: Cargo's two concurrent generated sheaf-ui Clippy invocations exceeded the shared 10 GB cgroup budget. Retrying the exact suite via wbuild with JOBS=1 (session 10992), preserving -D warnings/all targets. Unit will also use JOBS=1. No source workaround or reduced test coverage.

Required lint final: own run-u6371.service, JOBS=1 — `PASS lint`, `GREEN (layers: lint; not a full run) (406s)`, wrapper `EXIT 0` (/tmp/f5/v2/optimize-r31-lint-2.txt). Own journal PID 1559696 verifies these verdicts. Full unit suite active, session 54639, own main shell PID 1585486; output wrapper /tmp/f5/v2/optimize-r31-unit.txt. No production source edits after the successful app/GUI runs.


## Final round-31 results

- Build: `Finished dev profile ... in 42m 49s`; wrapper `EXIT 0`; copied executable cmp-equal to own target. The initial failure was shared disk exhaustion; the retry built successfully. The trailing shell EOF came from an externally edited live wrapper; its current syntax is valid and both later suite invocations succeeded.
- Lint, own run-u6371: `PASS lint`; `GREEN (layers: lint; not a full run) (406s)`; wrapper `EXIT 0`. First JOBS=2 attempt hit the 10 GB cgroup OOM cap; JOBS=1 is the verified command budget, with identical coverage and -D warnings.
- Unit, own run-u6413: `PASS unit`; `GREEN (layers: unit; not a full run) (274s)`; wrapper `EXIT 0`. The required complete workspace and pdfcraft-edit/export/ocr layers ran. Before the suite removed its successful log, /tmp/f5/v2/optimize-r31-unit-evidence.txt captured passing real ICC render/profile/persistence, optimizer, flattener and ink-plate tests. CJK integration is included in the workspace suite; all three traineddata packs were present locally.
- GUI, final whole-range run: all five steps 340–344 and profile isolation passed together, `ALL PASS`, /tmp/f5/v2/optimize-r31-gui-2.txt. Every result screenshot was inspected on the first identical source build; the final image-density crop measures only actual page content (Black capture density 0.00496667). Final screenshots remain under dev/.tmp/steps-114/.
- Static: git diff --check and bash -n pass; Impeccable detector returned [] on both ICC Slint targets. No new round-31 dependency; the inherited moxcms direct/test dependency is pure Rust and already transitive through image.
- Source publications: d53919a and fac1808, each committed as Sheaf contributors <sheaf@users.noreply.github.com>, each pushed separately. Remote fac1808 verified before this archival note commit.
- Completion audit: mandate yes; evidence yes; no external dependency blocks this round; all independently executable authorized work completed. Full competitor parity is not claimed: selective flattening/raster-vector balance, original MICI performance benchmark, release CJK packaging, calibrated press proof/combined plates, trapping and physical monitor/compositor confirmation remain as listed per component.

Final GUI proof:

```text
PASS 340 optimizer saves flattened Fast Web View (1059 380; 1050 608; 1041 543)
PASS 341 display ICC persists and resets to sRGB (348 610; /usr/share/color/icc/ghostscript/a98.icc; None)
PASS 342 output preview lists real CMYK plates (964 689; 349 737; 223 193; 238 229; 228 263; 226 299)
PASS 343 flattener preview compares original and converted page (947 715; 382 581; 234 192; 241 227)
PASS 344 automatic OCR writes searchable text (1053 579; Sheaf OCR test 2026  )
PASS the real user profile was not touched
ALL PASS
```

## C-optimize-014 Trap Presets (28/10, Opus)

Gap (Sol rev35B): no trap setting or calculation; the sheet said "Trapping is unavailable".
Acrobat's Trap Presets only store settings; the traps come from an Adobe In-RIP Trapping device at print time.
Sheaf draws them itself on the separated plates (`crates/sheaf-core/src/trap.rs`): where two abutting colours
each have an ink the other lacks by more than the step limit, the lighter colour (neutral density, Adobe's
process values) spreads under the darker by the trap width; the black width into black or rich black; close
densities straddle the edge (sliding trap); colour reduction lightens what spreads. Ghostscript's own
`-dTrapX` was tried first and rejected: on 10.02 it corrupts tiffsep and psdcmyk plates (values like 21 15 76
on flat colour).
- Function: presets with Acrobat's settings and defaults (0.25 pt, black 0.5 pt, step 10 %, black limit 100 %,
  black density 1.6, sliding 70 %, reduction 100 %), kept and deleted in the preferences; Preview traps (Output
  Preview of the trapped plates, at true width); Save trapped plates… (one 8-bit TIFF per page and ink, 72–2400
  dpi, current page or all). Tests: `trap::tests::*` (6, one on a real Ghostscript page), history.rs round trip,
  smoke 346 (yellow 1 px under the cyan edge at 300 dpi, the preview titled with the preset).
- Not done: image trap placement options (images trap like flat colour), trap join and end styles (a raster
  trap has none), assignment of presets to page ranges stored in the PDF (the sheet's current/all pages scope
  is used instead).
- Performance (release, 8 cores): a Letter page at 300 dpi with an edge every 60 px traps in 0.78 s at Default,
  2.4 s at 1 pt / 2 pt; only pixels near an edge are scanned (chamfer distance), rows in parallel.
- Aesthetics: the Trapping section matches the sheet's other sections (two- and three-up fields, preset buttons
  as the press presets); screenshots dev/.tmp only (smoke 346-trap-sheet, 346-traps).

## C-optimize-006 OCR speed re-measured (28/10, Opus)

The MICI scan rebuilt (the 4-page fiche at 200 dpi, grey, 1654 × 2339: /tmp/f5/mici/mici-scan.pdf from
pdftoppm -r 200 -gray). This VM now: 8 vCPU = 4 physical cores (EPYC 7B12). Tesseract 5 `fra`, one page after
another: 9.2 s for 4 pages (2.3 s/page; 1.95 s/page at OMP_THREAD_LIMIT=1); four at once: 29.9 s. Sheaf release
benchmark (`release_ocr_mici_benchmark…`, prepacked): warm median 3.33 s/page, 1789 words; CPU 317 s user in 99 s
(3.4 cores busy). Reading pages two to four at once (run_with on worker threads) gave 3.17 s/page: within noise,
reverted. The gap is per-core model speed (Conv 72–75 % of model time, RTEN_TIMING), as on 24/10: 1.45× Tesseract.
The lever is still a faster inference engine for the recognition model (onnxruntime, DECISIONS 24/10).

## C-optimize-006 OCR recognition on onnxruntime (28/10, Opus)

Decision of 28/10 (DECISIONS: onnxruntime for the recognition model). PaddleOCR's Latin model now runs on
onnxruntime 1.31 through the `ort` crate (2.0.0-rc.15, pinned); detection stays on rten (0.8 s of a page's
time). `third_party/pdfcraft-ocr/src/onnx.rs`: one environment a process (onnxruntime refuses a second), a
pool of sessions with one thread each (lines are already spread over every core by `read_latin`), the
convolution weights packed once and shared (`PrepackedWeights`), no memory pattern (each line has its own
width). rten stays the fallback: when onnxruntime is not built for the target, when it cannot load the
model (stderr says why), or with `SHEAF_OCR_ENGINE=rten`. The benchmark line now names the engine and can
write the searchable PDF (`SHEAF_OCR_BENCH_OUT`).

Same release binary, same VM (8 vCPU = 4 cores), back to back, other builds running (load 4–9):

| | warm median s/page (prepacked) | words | peak RSS (/usr/bin/time %M) | CPU user, 2 × 4 runs |
|---|---|---|---|---|
| rten (`SHEAF_OCR_ENGINE=rten`) | 4.00 (3.33 measured quiet earlier today) | 1789 | 450 MB | 376 s |
| onnxruntime | 1.41 (1.55 with shared weights under more load) | 1789 | 841 MB (639 MB with MALLOC_ARENA_MAX=2, as cargo.sh's 1) | 226 s |
| Tesseract 5 fra | 2.3 | | 8 MB | |

The words and their places are identical: the benchmark's equality check passed across a run whose first
engine was onnxruntime and second rten, and `pdftotext -raw` of the two searchable PDFs is byte for byte the
same. Words of the source found (`pdftotext -raw` of "ITEM-R2C 282 - MICI-V2.pdf", per page, each source
word counted once): p1 96.1 %, p2 97.7 %, p3 95.9 %, p4 88.9 %, all 95.7 % (1680/1755), accented 368/384:
unchanged. Tried and dropped: no CPU arena (no memory saved, slower). Memory is the cost: one session per
line read at once, each with its own buffers.

Platforms (pyke's prebuilt static libraries, `build.rs` sets `cfg(onnxruntime)`, the `ort` dependency is
target-specific to match): Linux x64 and arm64, macOS arm64, Windows x64 MSVC. Elsewhere (macOS x64, …)
rten reads the lines as before. Packaging:
- Linux: statically linked, +24 MB binary (sheaf-core's ocr test binary 44.5 → 68.6 MB, gzip +8.9 MB).
  pyke's library needs glibc 2.38 and GCC 13's libstdc++ to link: release.yml's Linux job moves from
  ubuntu-22.04 to 24.04, so the package needs glibc 2.39 (README updated). To keep 22.04-era distros, the
  other way is Microsoft's own `libonnxruntime.so` (manylinux) loaded at run time and shipped in `lib/`.
- macOS arm64: statically linked (the archive is libonnxruntime.a only; CoreML and Foundation frameworks are
  system ones). Size about the same as Linux; not built here.
- Windows x64: statically linked, but pyke's build imports `DirectML.dll` (18.5 MB, in the same archive):
  `copy-dylibs` puts it in target/release and package-windows.sh ships it beside Sheaf.exe. Not built here.
- Licences: `licenses/onnxruntime-LICENSE.txt` (MIT) in every package, a THIRD_PARTY.md row.
