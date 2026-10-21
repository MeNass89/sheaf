# Architecture

The map of the code: read this first, then the `//!` header of the file you open. Change it in the same
commit as any file added, moved, renamed or deleted.

## Layout

| Folder | Responsibility |
|---|---|
| `crates/sheaf-core` | The engine, no UI: one module per tool (`compress.rs`, `redact.rs`, `sign.rs`, `watermark.rs`…), plus `pdfcraft.rs` (bridge to the PdfCraft engine), `engine.rs` (pdfium), `paths.rs` (safe output paths). Every write goes through a temp file. |
| `crates/sheaf-cli` | The `sheaf` command (`main.rs`), its `--json` output (`agent.rs`) and the MCP server (`mcp.rs`). |
| `crates/sheaf-app` | The Slint GUI: Rust in `src/`, the interface in `ui/` (`main.slint` root, `screens/`, `widgets/`, `icons/` Lucide, `theme.slint`). |
| `scripts/` | `test.sh` (the one test suite), packaging (`bundle-macos.sh`, `package-linux.sh`, `package-windows.sh`, `make-dmg.sh`), downloads (`fetch-pdfium.sh`, `fetch-models.sh`), `smoke.sh` (a packaged CLI, release only). |
| `vendor/` | pdfium and the OCR models, fetched by the scripts above (git-ignored). |
| `third_party/` | Two PdfCraft crates patched for speed on large files, used through `[patch]` (its README says what and how to update). |
| `dev/` | How the app is built: brief, decisions, parity board, features, handoff, dev scripts (`dev/HANDOFF.md` first). |
| `docs/` | Benchmarks and plans. |

## Entry points

- GUI: `crates/sheaf-app/src/main.rs` builds the window from `ui/main.slint` and wires every callback.
- CLI: `crates/sheaf-cli/src/main.rs`.
- Tool catalogue (what the Tools grid shows): `crates/sheaf-core/src/tools.rs`.

## Build, test, run

```sh
bash scripts/test.sh            # every test layer: lint (clippy), unit (cargo test), gui (Xvfb); one verdict line
bash scripts/test.sh gui        # one layer while iterating; a unit of work closes on a full run
cargo run -p sheaf-app          # the GUI (sh dev/scripts/build-safe.sh build -p sheaf-app on a small machine)
```

## The GUI in `crates/sheaf-app/src`

| File | What it holds |
|---|---|
| `state.rs` | Tabs (tool tabs and document tabs) and how they reach the Slint model. |
| `flow.rs`, `run.rs`, `jobs.rs`, `dest.rs`, `files.rs`, `pool.rs` | A tool tab: adding files, options → job, running off the UI thread, where outputs go. |
| `home.rs`, `history.rs`, `prefs.rs`, `store_file.rs` | Recents, task history, preferences, their JSON files. |
| `reader_ui/` | The reader (a PDF open in its own tab): `mod.rs` state and opening; `pointer`, `edits`, `forms`, `sign`, `pages`, `search`, `nav`, `tools`, `saving`, `print_sheet`; `model.rs` pushes the state to Slint. |
| `reader_doc.rs` | The reader's document on its own thread: edits (`Op`) in, snapshots (`Reply`) out, saving. |
| `reader.rs` | Reader logic without UI types: themes, text selection, outline. |
| `print.rs` | Printing through CUPS `lp`: printers, page ranges, `lp` options. |
| `viewer.rs`, `viewer_ui.rs` | Page layout, zoom and rendering shared by the reader and the Redact/Sign/Edit panels. |
| `edit_ui.rs`, `sign_ui.rs`, `sign_preview.rs` | The Edit and Sign tool panels. |
| `dialog.rs`, `password.rs`, `open_with.rs`, `icons.rs` | File dialogs, password prompt, files opened from outside, icons. |

## Where to change what

| Change | Where |
|---|---|
| A new tool | engine in `sheaf-core/src/<tool>.rs` + `tools.rs`; options in `sheaf-app/ui/screens/options.slint` and `run.rs`; a CLI subcommand in `sheaf-cli/src/main.rs` |
| A reader tool, shortcut or panel | `ui/screens/reader.slint` (toolbar, pages, sheets, keys), `reader-panel.slint` (side panel), `reader-parts.slint` (small controls) + `src/reader_ui/<part>.rs` (behaviour); state fields in `ui/flow-types.slint` (`ReaderState`, `ReaderBridge`) |
| An edit to the open document | `Op` in `reader_doc.rs`, sent from `reader_ui/edits.rs` |
| Colours, sizes, fonts | `ui/theme.slint` |
| A preference | `prefs.rs` + `ui/screens/preferences.slint` |
| A test | a layer already exists for it in `scripts/test.sh`: unit tests next to the code or in `crates/*/tests/`, GUI checks in `dev/scripts/gui-smoke.sh` |

## Vocabulary

- **Tool tab**: a tab running one tool (Compress, Merge…) on files. **Document tab** / **reader**: a PDF open to read and annotate.
- **Annotation** (comment): highlight, note, ink, text box, shape (rectangle, oval, line, arrow); stored in the PDF. **Signature**: ink annotation flagged `fill_sign`.
- **Snapshot** (`Snap`): what the document thread reports after each change; the UI draws from it.
- **Provisional**: drawn at once by the UI before the engine confirms an edit.
- **Job**: one tool run off the UI thread.
