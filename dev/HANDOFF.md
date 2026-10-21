# Sheaf — handoff (Linux VPS → MacBook Pro, 2026-10-21)

Building and testing now go back to the **MacBook Pro**; the Linux VPS only wrote code (2 cores, 7 GB RAM: a full
build takes ~50 min and twice ran out of memory). Method: the **`dev` skill** (brain: `claude-code/skills/dev/`).
Read its SKILL.md, then this file, then `dev/DECISIONS.md`.

## Where things are

| What | Where |
|---|---|
| Brief, decisions | `dev/BRIEF.md`, `dev/DECISIONS.md` (append every decision) |
| The board | `dev/board/components.json` (331 components), rendered `dev/BOARD.md`; decisions per domain in `dev/board/decisions/<domain>.py` → `python3 dev/scripts/merge_prep.py decide` regenerates the table in DECISIONS.md |
| Raw features | `dev/board/raw/<app>.json` (1,254 gestures) merged in `dev/board/features.json`; instructions `EXTRACT.md`, `MERGE.md` |
| Inspiration code | `dev/re/archive/*.tar.zst` (git-ignored, on the VPS and on the Mac where they were made) → `python3 dev/scripts/archive.py extract <app>` |
| UI references | `dev/re/evidence/{pdfexpert,preview,ilovepdf}/` + README |
| Features in progress | `dev/features/F-001-save-dialog-click.md`, `F-002-reader.md` |

## Board status

`board.py status`: **331 components, 1 at ≥ 9** (offline, no account). Grades are still Sheaf 0.2.1's: nothing written
since has been built, tested and graded. Strong components: 68; normal 137; light 126. Order of work (Nassim):
(a) Save button after Compress, (b) reader/annotator at PDF Expert's level, (c) editing, (d) polish; then `board.py next`.

## Branch `dev/phase-4-5` (pushed), last commit = WIP

**The last commit does not compile yet.** The last successful build was before `src/reader_doc.rs`; expect a short
round of compile errors (types moved from `reader_ui.rs` to `reader_doc.rs`, `Provisional`, `ReaderMark`).

- **F-001 Save dialog** (`src/dialog.rs`): every `rfd` call is now an `AsyncFileDialog` with the window as parent
  (a sheet on macOS), polled by `slint::spawn_local`. Cause found: rfd's blocking `run_modal` nested in winit's mouse
  dispatch. Watermark image and certificate pickers go through the `Picked` global. **To prove on the Mac:** Compress →
  Save sheet → click Save writes the file; every other dialog (Open, folder, image, .p12) answers clicks.
- **F-002 Reader** (`src/reader.rs` pure logic + tests, `src/reader_ui.rs`, `src/reader_doc.rs`, `ui/screens/reader.slint`):
  PDFs opened from Home, drop, Recents or Open With get a document tab: toolbar (panel, select/highlight/underline/
  strike/note/pen/text box/edit text/add image, 5 colours, page field, zoom 25–400 %, fit width/page, Day/Sepia/Night,
  search, undo/redo, tools, save), side panel (Pages, Outline, Notes), continuous scroll, floating bar on a selection,
  links and ⌘[ ⌘], ⌘S incremental save, Save As, unsaved prompt on tab and window close. The PdfCraft document lives on
  its own thread (an annotation on a 70 MB book took 1.5 s on the UI thread); highlights and pen strokes are drawn
  provisionally until the page re-renders. **Seen working on Linux before the thread move:** opening a 1,982-page
  book with its outline, outline jump, streamed search ("1/188+"), selection + floating bar, a highlight rendered.
- **Edit text in place** (tool 7): click a paragraph → its text in an editor → `Edit::EditTextBlock`; empty space →
  `Edit::AddText`; tool 8 places an image. Not yet run.

## Next, in order

1. Build (`cargo build`), fix the compile errors, `cargo test --workspace`, `cargo clippy -- -D warnings`.
2. Drive F-001 on macOS; capture; tick its criteria.
3. Drive F-002 through every criterion of its feature file (open, scroll keys, zoom, outline, search, select/copy,
   each annotation tool, delete, undo/redo, ⌘S then reopen, Save As, unsaved prompt, themes, links); capture each state
   next to PDF Expert's evidence; measure open time, annotation latency, RSS on the 1,982-page book. Then
   `board.py grade` every component F-002 lists.
4. Then edit (c), polish (d), then `board.py next`.

## Linux notes (for later Linux work)

- The app needs `libxkbcommon-x11-0` at run time (missing on a bare Ubuntu: winit panics).
- Without a window manager (Xvfb), keyboard input needs `xdotool windowfocus --sync` first.
- Build only through `dev/scripts/cargo.sh` (light debug info, optimised dependencies, two jobs): full debug info made
  the target dir 16 GB; unoptimised PdfCraft took 30 s to open the big book (7 s optimised, debug build).
- Never run the app and a build at the same time on a 7 GB machine.

## Rules carried over

- Commits: `git -c user.name="Sheaf contributors" -c user.email="267055942+MeNass89@users.noreply.github.com" commit`,
  message ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No personal names, emails, home
  paths or machine details in tracked files. Merge PRs locally. Push freely; tags and releases: ask Nassim.
- Third-party code stays out of git (archives are ignored); the board stores paths and line numbers only.
- Talk to Nassim in French, vouvoiement, Alfred; 🦇 only emoji; short.
