# Rounds 30–31 — convert

## Tatouages

- 2026-10-27: `/tmp/f5/v2/brief.md` and `convert.json` define this batch. Work only in `/tmp/f5/w2-convert`, branch `par2/convert`. Display :113 only; no network features; one build command at a time. One commit and push per component. Mac-only verification must be disclosed.
- The task-specific brief supersedes the general dev startup full GUI suite and shared-file splitting: use the prescribed worker build and scoped smoke steps, keep shared-file changes small.

## Polaroid

Round 31 worker handoff, 2026-10-28: corrections and verification complete for `/home/nassim/f5/w2-convert` (`/tmp/f5/w2-convert`), branch `par2/convert`. Round30 merged through b3135d9. No child agents; only display :113. This closes this worker assignment, not all Sheaf parity work.

Own app: required wrapper EXIT 0, 6m33s (run-u6083), 10 GB cap. Copied binary and own target binary have identical SHA-256 `eba4877a213970e66d85902e7ec7d7f9b06ef0b378320de857de63f74433e56e`; both changed Slint files predate it. Required lint: PASS lint / GREEN (472s) / EXIT 0 (run-u6207, peak 6.2G). Required unit: PASS unit / GREEN (255s) / EXIT 0 (run-u6345, peak 6.1G). Final whole GUI range 330–336: all four steps pass together, real profile untouched, ALL PASS. Final strict-result screenshot was opened and visually checked: the refusal text fits inside its dark background. Assertions were not weakened.

Evidence: `/tmp/f5/v2/convert-r31-gui-final.log`, `dev/.tmp/steps-113/{330-scanner-driver,331-scanner-insert,335-pdfa-unicode-result,336-pdfa-strict-result}.png`, systemd journal units above. Build profile and scanner-focus commits were individually pushed; PDF/A fix and these handoff notes belong to the final component commit. Next owner: orchestrator, merge `par2/convert` and run the full GUI suite. No build/test process remains owned by this worker.

## Components

- C-convert-010: iLovePDF-style two-row text tables use alignment and wide gutters, without a capital-letter requirement. Lowercase and uncased scripts are covered by `two_row_text_tables_accept_lowercase_and_uncased_cells`; the generated XLSX is reopened by `two_lowercase_text_rows_are_exported_as_four_separate_cells` in both sheet modes. Verified: Excel 10/10 and core library 113/113 (`run-u4441`), with a failing pre-fix control (`run-u4430`).
- C-convert-011: iLovePDF-style editable slide tables retain the source height even below the former half-scale floor. Padding is tightened before reducing the font; there is no 50% floor (`pptx_dense_small_type_table_has_no_half_scale_floor_or_fixed_margins`). Verified: pdfcraft-export 45/45 (`run-u4437`), including OOXML package checks; pre-fix control fails at 560 pt (`run-u4432`). Very small rows below DrawingML's 1 pt text limit remain a practical limitation; no claim of full arbitrary-density parity.
- C-convert-007: Stirling-style CommonMark image syntax, reference pictures, nested link/image syntax, escaped alt text and original source-base resolution. `markdown_images_are_images_with_escaped_alt_and_nested_destinations`, `markdown_conversion_keeps_source_base_and_refuses_remote_pictures`, `markdown_relative_and_reference_pictures_survive_original_and_paper_conversion`. Verified: Office/Markdown 16/16 (`run-u4441`), including painted red image pixels in original and paper layouts; pre-fix image control fails (`run-u4431`). Remote images are refused under the brief's offline-only requirement. ZIP Markdown bundles/ebook input remain absent: searched `epub|mobi|ebook|azw` in app/core; `export.rs` only provides EPUB output. This closes the named image gap, not every Stirling other-format input.
- C-convert-006: Acrobat's Scan to PDF now has a SANE path on macOS instead of unconditional refusal; Finder can find Homebrew/MacPorts installations. WIA and SANE share decoded-image validation, remove stale/partial output and preserve driver error text. `sane_scans_at_300_dpi_to_a_path_with_spaces_and_rejects_driver_failures`, `success_without_a_page_or_with_invalid_pixels_is_refused`, `finder_without_a_shell_path_finds_an_installed_scanner`, smoke 330. All three adapter unit tests passed in core 116/116 (`run-u4605`); final whole-range smoke 330 passes with the own app built under 10 GB. not automatable: native macOS Image Capture/scanner hardware — this Linux worker cannot build/run Apple's framework or attach the device; SANE fallback is implemented, native Image Capture integration is not. No claim of native Preview/Acrobat parity.
- C-organize-011: Preview's Insert from Scanner uses that same adapter. Smoke 331 covers real process invocation (fake driver, no image-copy seam), saved inserted pixels, original page order and Undo/Redo. The shared adapter passes its three process/pixel/error tests (C-convert-006, cbcfd3d); round31 restores reader focus after page-menu operations; final whole-range smoke 331 passes with saved page counts 5/4/5, matching green pixels and original page order. Native Image Capture integration remains absent; clipboard path unchanged.
- C-convert-018: iLovePDF's eight conformance levels are implemented in core and registered in app/CLI; core 116/116 and PDF/A 11/11 pass (`run-u4605`). The profiles preserve tags, check all used text codes including Form/annotation appearances, validate missing tags/language/alt text, keep requested metadata and gate publication on the strict policy. Supplementary Unicode and ligatures are covered; unsupported composite encodings/inherited CMaps and excessive complexity must report an issue rather than claim conformance. Source: [PDF Association, Appendix A](https://pdfa.org/wp-content/uploads/2024/06/Understanding-private-data-in-PDFA.pdf). CLI integration 24/24 and PDF/A core 11/11 pass; global lint is green. Round31 fixes wrapped refusal notices and calibrates toolbar navigation against the real tooltip, including the custom-tool layout. Own app build, lint/unit and final whole-range GUI 335/336 pass.
- C-convert-019: not completed against full Acrobat parity. Inspected `press.rs`, `reader_ui/prepress.rs`, `preflight_profiles.rs`, press tests and Acrobat Standards plugin inventory. Existing local PDF/X-1a, X-3, X-4 exports, ICC selection and structural validation were found; PDF/E conversion is absent. A new engineering-standard converter and validator are beyond these targeted fixes; PDF/X markers are not a substitute. Both existing press integration tests pass in the final required unit suite. Handoff inspection: `crates/sheaf-core/src/press.rs:11` enumerates only X1a/X3/X4; `crates/sheaf-app/src/reader_ui/prepress.rs` routes this print export; `crates/sheaf-core/src/preflight_profiles.rs` explicitly provides readiness checks rather than full certification. No PDF/E implementation is delivered for C-convert-019.
- C-convert-020: deliberately excluded by the task brief, hard rule 4 (no online services). Inspected `options.slint`'s local-processing/LibreOffice labels and job routing. Online/local switching would contradict the mandate. Existing offline behaviour is retained; no full iLovePDF choice parity claimed. C-convert-020 therefore has no server implementation or new UI test: adding one would violate the explicit task scope, rather than close a local defect.

## References inspected

- iLovePDF `tools/PdfToExcelTool.qml`, `tools/PdfToPowerPointTool.qml`, `tools/PdfToPdfATool.qml`, `models/ConformanceModel.qml`; Stirling `ConvertMarkdownToPdf.java` (CommonMark and image resources); Preview scanner resource inventory; Acrobat Standards/Scan plugin inventory. Archives read in place, none extracted wholesale.

## Dependencies and hygiene

- Quota checked from the live worker session: 20% of the weekly window consumed (the local `oai-usage.py` probe was unavailable; session rate-limit event used).

- New direct dependency: `pulldown-cmark` 0.13.4 (already transitive), HTML rendering feature; new transitive `pulldown-cmark-escape` 0.11.0. Pure Rust, local parsing, no runtime service or download. The former ad-hoc Markdown parser is deleted.
- Startup hygiene: 58.4 GB free; three findings: orchestrator's 18 worktrees, active test temp files, pre-existing oversized `run.rs`. Scanner logic is moved to its own module and `run.rs` becomes smaller. Other workers' trees/caches and recorder untouched.
- Wrapper observation: `/tmp/f5/v2/wbuild.sh` streams child output to the systemd journal and its targeted-test pipeline can report EXIT 0 even after cargo termination. Targeted runs will be checked against the actual cargo test verdict, not just the wrapper exit. Suite commands preserve their own verdicts.

## Historical verification observations (before this resume)

- Red controls observed on pre-fix code: Excel lowercase table reports 0 tables instead of 1 (`run-u4430`); Markdown image test renders `!<a>` (`run-u4431`); PPTX 100-row table reports 560 pt instead of 500 pt (`run-u4432`). Production fixes restored after each control.
- First core library run: 112 passed / 1 pre-existing CMYK grayscale failure. Root cause is `compress::cmyk_jpeg_rgb` refusing YCCK output in its identity-Decode branch; the existing test generates YCCK via ImageMagick. Added native YCCK passthrough plus YCbCr-to-CMYK interpretation without changing the test. This prerequisite fix needs a separate commit; orchestrator should reconcile it with the optimize worker's CMYK work.
- Excel's legacy letter fixture placed four short words in the exact wide aligned geometry of a 2×2 table, and depended on the removed case rule to reject it. That same x=220 fixture is retained and now asserts one table and the exact four cell values under the requested geometric rule; a second x=110 prose-sized-gap fixture still asserts zero tables. No fixture was dropped and no salutation dictionary or case heuristic was added. This geometry cannot distinguish a deliberately spaced salutation from a 2×2 table without semantic source tags.

- Targeted green after the prerequisite fix and the documented Excel fixture correction: core library 113/113; Excel 10/10; Office/Markdown 16/16 (including painted red pixels); existing PDF/A 8/8 (`run-u4441`); pdfcraft-export 45/45, including dense PPTX and package checks (`run-u4437`). The later a/u tests were not part of this run.

- Impress is not installed (`libsdlo.so` absent): the pre-existing `pptx_opens_in_libreoffice` check skips at runtime. PPTX geometry and OOXML package tests ran; no PowerPoint/Impress visual opening is claimed. not automatable: open the dense exported deck in PowerPoint/Impress — that application is unavailable on this worker.
- PDF/A reporting will say “Checks passed” / “did not pass checks”: unsupported validation is not proof of an ISO violation, and passing built-in structural checks is not full ISO certification. The same producer is used by CLI and app; existing exact-wording tests are updated with the new result copy.

- PDF/A integration exposed two pre-existing writer/preflight bugs: PDF/A-1 retained an oversized trailer Size after dropping object streams (now reset before writing; every level is qpdf-clean), and preflight read font binary bytes as CMYK operators. A 92 KB local patch of the existing pure-Rust `pdfcraft-preflight` crate now uses existing `pdfcraft-content` to parse actual painting streams. The positive tagged-text archive test failed before the fix and passes all three a levels after it. No external validator or full ISO certification is claimed.

- Build prerequisite: two app builds exceeded the prescribed 6 GB cap, including a serial attempt (`run-u4663`, EXIT 1, OOM). The 47 MB generated Slint module is now isolated in `crates/sheaf-ui`, with its public types re-exported from sheaf-app. Canonical UI sources stay in place, existing tests are unchanged; the generated-only crate has no redundant standalone test harness. Rebuilding under the same cap.

- Memory investigation: serial compilation rules out concurrent Cargo jobs as the sole cause; isolating the UI reduces loaded PDF metadata but its generated crate alone still reached the cgroup limit (`run-u4756`, kernel CONSTRAINT_MEMCG, PID 1126788). Next controlled change: optimize only generated sheaf-ui at level 1, retaining debug assertions/test seams in the application. All existing tests remain enabled. Lint is run first to catch API or lint errors independently of code generation.

- Further memory hypothesis: this rustc driver imports glibc malloc/free and does not expose a jemalloc configuration string (`nm -D` / binary search). Next build/test processes set MALLOC_ARENA_MAX=1 in the existing Cargo wrapper, preserving explicit caller settings and the 6 GB cap. This bounds allocator arenas rather than changing app functionality. Primary reference: [GNU libc allocation parameters](https://www.gnu.org/software/libc/manual/pdf/libc.pdf), M_ARENA_MAX. The Slint maintainer also documents [large-UI compile memory](https://github.com/slint-ui/slint/discussions/12646); our dev build already has LTO off and many codegen units, so that release-profile advice does not establish a fix here.

- Global lint reached the application after generated UI type-checking. One inherited Rust 1.99 Clippy error remains in `viewer/renders.rs::over_grid`: constant-size `chunks_exact_mut(4)`. Replaced with the equivalent `as_chunks_mut::<4>().0.iter_mut()`; the existing `the_transparency_grid_shows_through_the_empty_paper` test covers the operation. Orchestrator: reconcile this one-line iterator change with reader work; it does not change pixels or remainder handling.

- Lint then exposed an inherited binary compile error: `home::open_guide` calls `crate::store_file`, but main imported only jobs from the library. Added `store_file` beside the existing jobs import (one shared-file edit), so the guide resolves the canonical library module without duplicating it. The full binary compile/check validates this namespace repair.

- Next global check found the existing `Prefs` literal in `tests/history.rs` missing `transparency_grid`. Added it as true, so `prefs_round_trip_and_corrupt_means_default` also verifies that non-default value survives storage; no assertion was removed.

- Final lint proof (run-u4837): `PASS lint`; `GREEN (layers: lint; not a full run) (345s)`; wrapper `EXIT 0`. Real child output on this VM is queried with `journalctl --user _SYSTEMD_USER_UNIT=run-u4837.service -o cat`; querying `-u` alone showed only manager records. Unit suite is running next (session 42723), under the original 6 GB cap. No 8 GB exception has been used.

- Unit run-u4843 still exceeded 6 GB while generating machine code, despite isolated UI, opt-level=1 and one malloc arena. Lint remains green; no GUI binary exists. A final in-cap allocator trial pins glibc's mmap/trim thresholds to 128 KiB (the documented initial defaults), disabling their adaptive growth so freed large compiler buffers can be returned promptly. Existing jemalloc shared libraries were not found; no native dependency was installed. Core/CLI tests run independently before retrying the required unit layer.

- Latest core run-u4945: 116/116 library tests and 11/11 PDF/A integration tests passed, including the new CMap expansion/overflow guards. First CLI run: 23/24 passed; the added 1b-positive case reused the existing transparent PNG and correctly failed the PDF/A-1 transparency rule. The original transparent 2b/3b cases are retained, u cases extend them, and a separate opaque source tests 1b success. A new explicit assertion also preserves the transparent-1b rejection and incomplete-copy report.

- Corrected CLI run-u5002: `pdfa_subcommand_reports_conformance ... ok`, `pdfa_exits_1_when_rules_are_still_broken_but_writes_the_file ... ok`, 24/24 integration tests passed. Required unit layer retried at 6 GB with fixed allocator thresholds (session 94820); no higher-cap authorization received or used.

- Unit run-u5009 also reached the 6 GB cap with fixed allocator thresholds. Returning generated UI to the normal unoptimized dev profile (removing the speculative opt-level=1 override), while retaining fixed allocation thresholds, is the final stable-profile in-cap trial. No experimental Slint module feature, feature removal, native allocator install or borrowed binary is used. The eight component commits are pushed through b3537e5; this does not mean the branch is runnable yet.

## Resume verification (2026-10-27)

- Initial `git status --short --branch`: `par2/convert...origin/par2/convert`, modified `Cargo.toml` and this file only. Initial `git log dev/round29..HEAD`: ten existing commits, from e6db6d8 through b3537e5.
- Merge: f2fffcb, pushed to origin/par2/convert. The merged round29 snapshot is c81df70 (the shared branch may keep moving while other workers finish). CMYK resolution was corrected in 91d73a9 after a diff inspection caught duplicated context; the resulting compress.rs exactly matches c81df70. Both commits are pushed; the required suite will check this code.
- Goal / checks: app built from this branch, lint and unit EXIT 0, scanner and PDF/A smoke PASS, final notes and branch pushed. No network conversion or native Image Capture implementation is added.

### Provisional author grades and remaining scope

These are conservative author estimates, not joint parity closure. Aesthetics/performance stay below 9 because this resume has no fresh competitor comparison or throughput benchmark. Final execution evidence follows below once obtained.

- C-convert-006: f7 a8 p8 — `scanner.rs:94` validates the actual process arguments/pixels/errors; smoke 330 checks the app path. SANE fallback needs local scanner software; native macOS Image Capture/hardware remains unverified.
- C-convert-007: f8 a8 p8 — `textdocs.rs:385` and `tests/office.rs:336` cover CommonMark pictures and painted pixels in both layouts. Ebook input and ZIP Markdown bundles remain absent.
- C-convert-010: f8 a8 p8 — `excel.rs:649` and `tests/excel.rs:255` cover lowercase/uncased aligned tables and reopened XLSX cells. The named case-rule gap closes; no new broad layout fidelity audit.
- C-convert-011: f8 a8 p8 — `third_party/pdfcraft-export/src/pptx.rs:793` checks all 100 editable rows within 500 pt. Fonts below DrawingML's 1 pt minimum and visual PowerPoint/Impress comparison remain unverified.
- C-convert-018: f8 a8 p8 — `tests/pdfa.rs:165`, `:188`, `:216`, Unicode unit tests, CLI integration and smoke 335/336 cover all identifiers, preserved tags, missing mappings and strict refusal. Built-in checks are not a complete ISO validator.
- C-convert-019: f5 a7 p8 — existing `tests/press.rs:8` and `:39` check PDF/X-3 output and refusal/timeout paths. No PDF/E converter exists; full Acrobat standards parity is not delivered. X-1a/X-4 depend on Ghostscript version.
- C-convert-020: f6 a8 p8 — `ui/screens/options.slint:1382` states local processing. Online switching is intentionally excluded by brief hard rule 4; not closed against iLovePDF's full feature.
- C-organize-011: f7 a8 p8 — shared scanner adapter unit tests plus smoke 331 check the real driver process, pixel placement, page order and Undo/Redo. Native macOS acquisition remains unverified.

- Resume app attempt 1: `bash /tmp/f5/v2/wbuild.sh convert app` returned `EXIT 1`, systemd run-u5076, 21:46:28 UTC. Kernel evidence names rustc PID 1264145 and `constraint=CONSTRAINT_NONE ... global_oom`, anon RSS 4,802,904 KiB. This is shared-host exhaustion, not the worker's 10G cgroup ceiling; the host had ~3.2 GiB available immediately afterward. No code diagnostic or runnable binary was produced. Next: required lint, then serial codegen retry after memory becomes available.

## Round 31 investigation

- Required range is 330–336 (330, 331, 335, 336 exist). Evidence from round30's real `smoke-2/331-scanner-insert.png`: five-page unsaved document after the 4/4/4 disk check; the matching app log ends after InsertPagesFrom without save/undo. `reader-panel.slint`'s menu callback omitted the focus return used by the toolbar menus. Restored `root.focus-pages()` after page operations; the unchanged 331 check will verify saved pixels/order and all three history states.
- PDF/A failures: archived screenshots show AI Assistant; preceding suite steps left a custom toolbox command in actions.json, adding a 32 px button to the reader bar. This shifts the tool button from x1236 to x1268. PDF/A assertions stay unchanged; navigation must find the tooltip instead of trusting one coordinate.
- Round31 smoke navigation now checks the rendered “Use a tool” tooltip at the two toolbar positions before opening PDF/A. Step336 seeds one valid saved custom toolbox, then restores actions.json after quitting; this reproduces the full-suite state even when only the worker range runs. Shell syntax and generated helper syntax pass; Impeccable detector reports `[]` for the one changed Slint line.
- First round31 build acquired its slot after ~9 minutes, then failed `EXIT 101`: `No space left on device (os error 28)` while compiling rustls. Disk was 99% full. Removed only this worker's old test executables, obsolete sheaf metadata/libraries and abandoned rmeta directories (38 artifacts, 1.40 GiB). Disk then had 4.1 GiB available. Serial app retry started, tool session 42465. No other worker artifacts touched.
- App retry succeeded: `EXIT 0`, `Finished dev profile [unoptimized + debuginfo] in 7m 49s`. Own binary at dev/.tmp/sheaf-app-under-test. Full worker range launched with required runsteps2 command on :113, session 41327; required lint launched serially after app build, session 95748. Disk available 2.0 GiB.
- First whole-range run: PASS 330; PASS 331 (5/4/5, srgb(42,156,75)); PASS 335 (U/2); FAIL 336. Strict-options screenshot proves PDF/A-2a, Keep unchecked, no-output text; strict-result screenshot is identical options, no output. Source: flow.rs toast defaults to 3 seconds; step waited 3 seconds before OCR, missing the expected refusal. Changed only that delay to 0.5 seconds, preserving every assertion; whole range rerun started as session 78441.
- Lint client session 95748 ended with status 143 without verdict, but actual service run-u5988.service remains active with scripts/test.sh lint and clippy-driver in this target. Do not start another build/test until its real state is known; capture its result or rerun wrapper after completion.
- Second whole-range run again passed 330/331/335 and failed 336. New decisive evidence: 336-pdfa-strict-result.png shows the actual strict refusal, but its multiline text overflows Toast's fixed 36 px background; “did not pass” is white on the light options page and unreadable. No output file exists and the input remains one page. This is an app defect, not a reason to change the assertion. Toast now wraps and sizes its height to the text, retaining 36 px minimum and normal single-line appearance.
- Stopped only our outdated lint service run-u5988.service before rebuilding this Slint correction; its client was already detached and no lint verdict had been produced. New required app build session recorded in the next receipt; then rerun whole range and both required suites. Disk capacity is now 29 GiB following external cleanup; our cleanup stayed limited to our cache.
- Toast rebuild succeeded via the required wrapper: `EXIT 0`, finished in 6m33s. Own binary copied at 00:43 UTC; whole 330–336 range now running on :113, tool session 98429.
- Whole-range run after Toast rebuild: PASS 330 (srgb(42,156,75)); PASS 331 (5/4/5 and matching pixels/order); PASS 335 U/2; PASS 336 strict refusal/no output; real profile untouched; ALL PASS. Required lint now running with JOBS=1, session 46204, to keep generated UI lib/test compilations serial within 10 GB. Moved the strict-result screenshot immediately before OCR so the saved evidence also captures the three-second notice; final whole-range run will be repeated after suites to verify this last test-only ordering change.

- Final required lint: `PASS lint`, `GREEN (layers: lint; not a full run) (472s)`, wrapper `EXIT 0`; service run-u6207, 6.2G memory peak. Unit launched serially with JOBS=1.

- Final required unit: `PASS unit`, `GREEN (layers: unit; not a full run) (255s)`, wrapper `EXIT 0`; service run-u6345, 6.1G memory peak. Includes workspace tests and pdfcraft-edit/export/ocr libraries. Workspace evidence read live: core 132/132, CLI 24/24, Office 16/16, PDF/A 11/11, press 2/2, scanner process test green.

## Final round 31 receipt

- Build: `Finished dev profile [unoptimized + debuginfo] target(s) in 6m 33s`, wrapper `EXIT 0`.
- Lint: `PASS lint`; `GREEN (layers: lint; not a full run) (472s)`; wrapper `EXIT 0`.
- Unit: `PASS unit`; `GREEN (layers: unit; not a full run) (255s)`; wrapper `EXIT 0`.
- Final GUI command: `bash /tmp/f5/v2/runsteps2.sh /tmp/f5/w2-convert 113 330 336`.
- Final saved refusal capture: `dev/.tmp/steps-113/336-pdfa-strict-result.png`, 2026-10-28 01:09:56 UTC; inspected visually. UI detector: `[]`. `git diff --check`: clean.
- No new round31 dependencies. Native macOS scanner hardware/Image Capture, PDF/E and server conversion retain the limitations stated above; no parity grade is raised from these regression fixes alone.

```text
PASS 330 From scanner invokes SANE at 300 dpi and opens the scanned pixels (srgb(42,156,75))
PASS 331 Insert from scanner preserves pixels, page order and Undo/Redo (5/4/5; srgb(42,156,75))
PASS 335 PDF/A-2u selection writes the actual U/2 identifier
PASS 336 strict PDF/A-2a refuses untagged content without saving a false archive
PASS the real user profile was not touched
ALL PASS
```

## C-convert-019 PDF/E keeps 3D (28/10, Opus)

Gap (Sol rev36a): the PDF/A-2b step under PDF/E removed 3D annotations, which PDF/E-1 exists to keep. `pdfe.rs`
now gives 3D annotations a held subtype before that step and their own back after it; PDF/E's check no longer
counts PDF/A's ban on 3D. Test `pdfe_keeps_a_3d_annotation_and_its_model` (tests/pdfa.rs): a page's 3D
annotation and its U3D stream come through, the file checks clean, no held subtype is left.

## Round 36 (28/10, Opus, on the MBP): macOS scanner, LibreOffice 26 pictures, fonts in every build

- Scanner on macOS (C-convert-006, C-files-018, C-organize-011): Preview's panel, ImageKit's IKDeviceBrowserView
  and IKScannerDeviceView, in Sheaf started again as `--scan-to` (src/scan_mac.rs; AppKit needs a main thread).
  USB and network scanners, Show Details; with none, a message says what to do. Checked on the MBP (no scanner
  there): panel drawn by SHEAF_TEST_SCAN_SHOT, DEVICES 0 / SHARED 0 and the message centred.
- LibreOffice 26.8 (the MBP's) no longer loads linked local pictures in HTML (24.2 did): local `<img>` go in as
  data URIs (html_source::inline_pictures), HTML and Markdown, both layouts.
- Craft fonts (Arabic, Japanese, Thai, Georgian) were embedded only by builds through dev/scripts/cargo.sh: the
  .dmg and CI built without them. `.cargo/config.toml` sets CRAFT_FONTS_DIR; release scripts require it.
- macOS unit layer green on the MBP (cargo test --workspace + pdfcraft libs). Test-only fixes: canonical paths
  (/private/var), a fixture page without /Resources (qpdf 12 warns), lopdf's text of LibreOffice 26 PDFs.
