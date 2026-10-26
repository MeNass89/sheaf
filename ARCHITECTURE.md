# Architecture

The map of the code: read this first, then the `//!` header of the file you open. Change it in the same
commit as any file added, moved, renamed or deleted.

## Layout

| Folder | Responsibility |
|---|---|
| `crates/sheaf-core` | The engine, no UI: one module per tool (`compress.rs`, `redact/`, `sign.rs`, `watermark.rs`…), plus `pdfcraft.rs` (bridge to the PdfCraft engine), `engine.rs` (pdfium), `paths.rs` (safe output paths), `script.rs` (typed-signature handwriting fonts, bundled in `assets/fonts` with their licences), `facts.rs` (what Document Properties reads: restrictions, PDF/A, dates, page boxes), `form_scan.rs` (Detect fields on a scanned form: its lines and boxes found in the pixels), `info.rs` (the Author written on the PDFs Sheaf makes). Every write goes through a temp file. |
| `crates/sheaf-cli` | The `sheaf` command (`main.rs` dispatch, `args.rs` the clap arguments), its `--json` output (`agent.rs`) and the MCP server (`mcp.rs`). |
| `crates/sheaf-app` | The Slint GUI: Rust in `src/`, the interface in `ui/` (`main.slint` root, `screens/`, `widgets/`, `icons/` Lucide, `theme.slint`). |
| `scripts/` | `test.sh` (the one test suite), packaging (`bundle-macos.sh`, `package-linux.sh`, `package-windows.sh`, `make-dmg.sh`), downloads (`fetch-pdfium.sh`, `fetch-models.sh`), `smoke.sh` (a packaged CLI, release only). |
| `vendor/` | pdfium and the OCR models, fetched by the scripts above (git-ignored). |
| `third_party/` | Six PdfCraft crates patched (annot: text box fonts; edit: page number and watermark fonts, text read for export; cos and engine: speed on large files, text box edits; export: PDF to Word tables and paragraphs; ocr: accented Latin with PaddleOCR's model), used through `[patch]` (its README says what and how to update). |
| `dev/` | How the app is built: brief, decisions, parity board, features, handoff, dev scripts (`dev/HANDOFF.md` first). |
| `docs/` | Benchmarks and plans. |

Password fields can opt into the reveal control in `ui/widgets/text-field.slint` (the authored
`icons/eye.svg` and `eye-slash.svg`); Protect uses the strength rule in core `security.rs`.
Recents exclusions are saved separately from task history by `history.rs` (`hidden-recents.json`).
The completed toolbox mission is recorded in `dev/SOL_TOOLBOX.md`.

## Entry points

- GUI: `crates/sheaf-app/src/main.rs` builds the window from `ui/main.slint` and wires every callback.
- CLI: `crates/sheaf-cli/src/main.rs` (dispatch); its subcommands and arguments in `args.rs`.
- Tool catalogue (what the Tools grid shows): `crates/sheaf-core/src/tools.rs`.

## Build, test, run

```sh
bash scripts/test.sh            # every test layer: lint (clippy), unit (cargo test), gui (Xvfb); one verdict line
bash scripts/test.sh gui        # one layer while iterating; a unit of work closes on a full run
cargo run -p sheaf-app          # the GUI (sh dev/scripts/build-safe.sh build -p sheaf-app on a small machine)
```

## Redaction in `crates/sheaf-core/src/redact/`

| File | What it holds |
|---|---|
| `mod.rs` | The public API: `Query`, `TextMatch`, `Needle`, `Mark`, `RedactReport`, `find`, `mark_*`, `apply`. |
| `matching.rs` | How a searched text is found in a string (case, spacing, Unicode form) and removed from it. |
| `clean.rs` | The clean-up outside the page content after PdfCraft (`post_pass`): metadata, bookmarks, comments, fields, attachments, scripts, hidden forms, unused fonts. |
| `audit.rs` | The deny-by-default audit of every object of the final bytes (`check_scrubbed`) and its exemptions. |
| `verify.rs` | The check of the written pages (`verify`): PdfCraft's redaction audit, then pdfium's text under the marks. |
| `reach.rs` | What the pages paint and draw with (`Reach`), exempt from the audit. |
| `content.rs` | Content streams by byte range: comments, marked content, inline images, shown text. |
| `pdf.rs`, `names.rs` | PDF strings, streams and XMP decoded; the names and keys PDF defines (structure, not text). |

## The GUI in `crates/sheaf-app/src`

| File | What it holds |
|---|---|
| `state.rs` | Tabs (tool tabs and document tabs) and how they reach the Slint model. |
| `windows.rs` | Sheaf's windows (one process each): Merge All Windows asks the others, over a local port, for their documents. |
| `flow.rs`, `run.rs`, `jobs.rs`, `dest.rs`, `files.rs`, `pool.rs` | A tool tab: adding files, options → job, running off the UI thread, where outputs go. |
| `home.rs`, `history.rs`, `saved_options.rs`, `prefs.rs`, `store_file.rs` | Recents, task history (with each task's options, for Run again), preferences, their JSON files. |
| `reader_ui/` | The reader (a PDF open in its own tab): `mod.rs` state and opening; `pointer`, `edits`, `forms`, `sign`, `pages`, `search`, `nav`, `tools`, `saving`, `print_sheet`, `properties`, `prepare` (Prepare form: the Form tool, fields' card, Detect fields); `model.rs` pushes the state to Slint. |
| `reader_doc/` | The reader's document on its own thread: edits (`Op`) in, snapshots (`Reply`) out, saving (`mod.rs`); its tests in `tests/` (`clipboard.rs`: copy and paste; `forms_and_files.rs`: forms, signatures, attachments). |
| `reader.rs` | Reader logic without UI types: themes, text selection, outline. |
| `print.rs` | Printing through CUPS `lp`: printers, page ranges, `lp` options. |
| `viewer/` (`mod.rs` layout, zoom, view ↔ PDF mapping; `renders.rs` caches, render queue, drawing a page), `viewer_ui.rs` | Page layout, zoom and rendering shared by the reader and the Redact/Sign/Edit panels. |
| `edit_ui.rs`, `sign_ui.rs`, `sign_preview.rs` | The Edit and Sign tool panels. |
| `wm_preview.rs` | Add watermark's live preview: page 1 marked with the panel's options, redrawn as they change. |
| `dialog.rs`, `password.rs`, `open_with.rs`, `icons.rs` | File dialogs, password prompt, files opened from outside, icons. |

## Where to change what

| Change | Where |
|---|---|
| A new tool | engine in `sheaf-core/src/<tool>.rs` + `tools.rs`; options in `sheaf-app/ui/screens/options.slint` (its controls in `options-parts.slint`) and `run.rs`; a CLI subcommand in `sheaf-cli/src/args.rs` (arguments) and `main.rs` (dispatch) |
| A reader tool, shortcut or panel | `ui/screens/reader.slint` (composition, pages, keys), `reader-toolbar.slint` (top bar: tools, page field, zoom, search, More menu), `reader-options.slint` (the tool's options bar over the pages), `reader-sheets.slint` (signature pad, print, properties), `reader-form.slint` (Prepare form's field boxes and card), `reader-panel.slint` (side panel), `reader-parts.slint` (small controls) + `src/reader_ui/<part>.rs` (behaviour); state fields in `ui/flow-types.slint` (`ReaderState`, `ReaderBridge`) |
| An edit to the open document | `Op` in `reader_doc/mod.rs`, sent from `reader_ui/edits.rs` |
| Colours, sizes, fonts | `ui/theme.slint` |
| A preference | `prefs.rs` + `ui/screens/preferences.slint` |
| A test | a layer already exists for it in `scripts/test.sh`: unit tests next to the code or in `crates/*/tests/`, GUI checks in `dev/scripts/smoke/*.sh` (the steps, in order; `dev/scripts/gui-smoke.sh` holds the setup and helpers and runs them) |
| PDF to Word quality | `dev/scripts/word-corpus.py`: Word originals → PDF → Sheaf's export, measured against the original (words in order, tables, no text boxes, pages); not in the suite (needs LibreOffice and python-docx) |

## Vocabulary

- **Tool tab**: a tab running one tool (Compress, Merge…) on files. **Document tab** / **reader**: a PDF open to read and annotate.
- **Annotation** (comment): highlight, note, ink, text box, shape (rectangle, oval, line, arrow); stored in the PDF. **Signature**: ink annotation flagged `fill_sign`.
- **Snapshot** (`Snap`): what the document thread reports after each change; the UI draws from it.
- **Provisional**: drawn at once by the UI before the engine confirms an edit.
- **Job**: one tool run off the UI thread.
