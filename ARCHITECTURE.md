# Architecture

The map of the code: read this first, then the `//!` header of the file you open. Change it in the same
commit as any file added, moved, renamed or deleted.

## Layout

| Folder | Responsibility |
|---|---|
| `crates/sheaf-core` | The engine, no UI: one module per tool (`compress.rs`, `redact/`, `sign.rs`, `watermark.rs`…), plus `pdfcraft.rs` (bridge to the PdfCraft engine), `engine.rs` (pdfium), `paths.rs` (safe output paths), `script.rs` (typed-signature handwriting fonts, bundled in `assets/fonts` with their licences), `facts.rs` (what Document Properties reads: restrictions, PDF/A, dates, page boxes), `form_scan.rs` (Detect fields on a scanned form: its lines and boxes found in the pixels), `info.rs` (the Author written on the PDFs Sheaf makes), `layers.rs` (a copy with layers hidden, for the screen), `grayscale.rs` (Compress ▸ Convert to grayscale), `textdocs.rs` (Markdown, e-mail and PostScript to PDF), `adjust.rs` (Adjust colours), `system_fonts.rs` (the computer's open-licensed fonts a paragraph edited with Edit can be set in), `spell.rs` (check spelling: the system's Hunspell dictionaries read by `spellbook`, else a word list), `bookmarks.rs` (`sheaf bookmark`). Every write goes through a temp file. |
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

## Guided actions

- `crates/sheaf-app/src/action_import.rs`: lossless supported Stirling Automate/Folder Scanning JSON translation.
- `crates/sheaf-app/src/actions.rs`: portable action JSON, ordered tool configurations,
  suggested actions, persistence and sequential execution without replacing originals.
- `crates/sheaf-app/src/actions_ui.rs` and `ui/screens/actions.slint`: Home's Actions page,
  step editor, filtered suggested/saved/custom lists, reader custom palette, import/export,
  worker-thread progress and editable pauses. Guided actions is reachable from Tools and the reader.

- `crates/sheaf-app/src/watch_folder.rs`: stable-file polling, duplicate suppression and
  optional title-based output names, recursive scanning and persisted resume checkpoints.

- `crates/sheaf-core/src/portfolio.rs`: PDF Collection catalog, readable cover, original
  embedded files, chosen order and duplicate-name handling; `src/portfolio_ui.rs` and
  `ui/screens/portfolio.slint` compose files from Tools/Actions/reader, and
  `src/reader_ui/portfolio.rs` browses embedded originals with Open and Save copy.

- `crates/sheaf-app/src/library.rs`, `src/library_ui.rs` and `ui/screens/library.slint`:
  private local files, immutable versions, reader working copies, export, restore and bounded
  ZIP imports with retained folder metadata, file filters and recoverable working copies.
  The reader Save hook checkpoints files and refreshes the list; tab closure releases its checkout.

- `crates/sheaf-app/src/reader_ui/javascript.rs` and `ui/screens/javascript.slint`: active
  document JavaScript console and named document scripts; `reader_doc` executes at the visible
  page and returns output, form changes and local viewer requests for navigation/focus/print/Save As.

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
| `reader_ui/` | The reader (a PDF open in its own tab): `mod.rs` state and opening; `pointer`, `edits`, `forms`, `sign`, `pages`, `search`, `nav`, `tools`, `saving`, `print_sheet`, `properties`, `prepare` (Prepare form: the Form tool, fields' card, Detect fields), `speech`, `loupe`, `folder_search`, `layers` (the Layers panel), `look_up` (a selection in the dictionary), `accessibility` (the check, its fixes, its report); `model.rs` pushes the state to Slint. |
| `reader_doc/` | The reader's document on its own thread: edits (`Op`) in, snapshots (`Reply`) out, saving (`mod.rs`); its tests in `tests/` (`clipboard.rs`: copy and paste; `forms_and_files.rs`: forms, signatures, attachments). |
| `reader.rs` | Reader logic without UI types: themes, text selection, outline. |
| `print.rs` | Printing through CUPS `lp`: printers, page ranges, `lp` options. |
| `viewer/` (`mod.rs` layout, zoom, view ↔ PDF mapping; `renders.rs` caches, render queue, drawing a page), `viewer_ui.rs` | Page layout, zoom and rendering shared by the reader and the Redact/Sign/Edit panels. |
| `edit_ui.rs`, `sign_ui.rs`, `sign_preview.rs` | The Edit and Sign tool panels. |
| `wm_preview.rs` | Add watermark's live preview: page 1 marked with the panel's options, redrawn as they change. |
| `dialog.rs`, `password.rs`, `open_with.rs`, `icons.rs` | File dialogs, password prompt, files opened from outside, icons. |
| `platform.rs`, `platform_ui.rs` | The menu bar's commands and their shortcuts (the user's, Preferences ▸ Keyboard shortcuts), check spelling while typing (the card under an editor), tools hidden when they cannot run here, the right-to-left interface; the `PlatformBridge` global (`ui/platform.slint`). |

## Where to change what

| Change | Where |
|---|---|
| A new tool | engine in `sheaf-core/src/<tool>.rs` + `tools.rs`; options in `sheaf-app/ui/screens/options.slint` (its controls in `options-parts.slint`) and `run.rs`; a CLI subcommand in `sheaf-cli/src/args.rs` (arguments) and `main.rs` (dispatch) |
| A reader tool, shortcut or panel | `ui/screens/reader.slint` (composition, pages, keys), `reader-toolbar.slint` (top bar: tools, page field, zoom, search, More menu), `reader-options.slint` (the tool's options bar over the pages), `reader-sheets.slint` (signature pad, print, properties), `reader-form.slint` (Prepare form's field boxes and card), `reader-panel.slint` (side panel), `reader-parts.slint` (small controls) + `src/reader_ui/<part>.rs` (behaviour); state fields in `ui/flow-types.slint` (`ReaderState`, `ReaderBridge`) |
| An edit to the open document | `Op` in `reader_doc/mod.rs`, sent from `reader_ui/edits.rs` |
| Colours, sizes, fonts | `ui/theme.slint` |
| A preference | `prefs.rs` + `ui/screens/preferences.slint` (spelling, keyboard, interface: `prefs.rs` `platform` + `preferences-platform.slint`) |
| A menu bar command | `src/platform.rs` `COMMANDS` (at the end) + its `MenuItem` in `ui/main.slint`'s `MenuBar`, reading `PlatformBridge.keys[<its index>]` |
| A reader tool's letter | `src/platform.rs` `TOOLS` (by tool number; reader.slint asks `PlatformBridge.tool-for-key`) |
| Right to left for a row | a `FlexboxLayout` with `flex-direction: PlatformBridge.rtl ? row-reverse : row` (Slint's HorizontalLayout has no direction) |
| A test | a layer already exists for it in `scripts/test.sh`: unit tests next to the code or in `crates/*/tests/`, GUI checks in `dev/scripts/smoke/*.sh` (the steps, in order; `dev/scripts/gui-smoke.sh` holds the setup and helpers and runs them; `dev/scripts/gui-parallel.sh` runs them in three shards on their own displays on a machine with 4 cores or more, `dev/scripts/smoke-helpers.py` gathering the helpers each part defines) |
| PDF to Word quality | `dev/scripts/word-corpus.py`: Word originals → PDF → Sheaf's export, measured against the original (words in order, tables, no text boxes, pages); not in the suite (needs LibreOffice and python-docx) |

## Vocabulary

- **Tool tab**: a tab running one tool (Compress, Merge…) on files. **Document tab** / **reader**: a PDF open to read and annotate.
- **Annotation** (comment): highlight, note, ink, text box, shape (rectangle, oval, line, arrow); stored in the PDF. **Signature**: ink annotation flagged `fill_sign`.
- **Snapshot** (`Snap`): what the document thread reports after each change; the UI draws from it.
- **Provisional**: drawn at once by the UI before the engine confirms an edit.
- **Job**: one tool run off the UI thread.

`signature_store.rs` keeps signature and initials payloads in an authenticated encrypted local file with a separate per-profile key (owner-only on Unix). Legacy Secret Service / Keychain / Windows DPAPI profiles and plaintext preferences migrate on successful read; new saves need no external program.

Core `ocr_local.rs` supplies installed offline Tesseract script packs to the existing OCR render/apply pipeline.

Core `search_index.rs` builds/searches versioned catalogs (`sheaf index` / `search-index`); folder search consumes a `.sheaf-index.json` catalog when present.

Core `security_policy.rs` imports/exports reusable AES-256 settings, applied by `security::protect_with_settings` (`save-security-policy` / `apply-security-policy` in the CLI).

Core `ocr_review.rs` lists hidden runs and corrects/removes their searchable text without repainting the scan (`review-ocr` / `correct-ocr`).

The reader's More → More tools menu also opens `reader_ui/ocr_review.rs`, `catalog.rs`,
`security_policy.rs` and `signature_manager.rs`; their Slint surfaces are in `ui/screens/`
(`ocr-review.slint`, `catalog.slint`, `security-policy.slint`, `signature-manager.slint`).
Callbacks and state travel through `ReaderBridge`/`ReaderState` in `ui/flow-types.slint`.
OCR review uses document-thread operations, so corrections participate in Undo/Redo/Save.
Catalog parsing/search and policy application run off the UI thread. `security_policies.rs`
keeps password-free named policy files under the application data folder's `security-policies/`.
OCR options retain every selected language; `OcrOptions::use_local_engine` selects local
Tesseract explicitly, with the same option in the OCR panel and CLI `--local-engine`.
