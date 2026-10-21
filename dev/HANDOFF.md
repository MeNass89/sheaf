# Sheaf — handoff (VPS, 2026-10-21 ~05:30)

Sheaf is built and tested on the **Linux VPS** (the MacBook Pro is for another project). Method: the **`dev`
skill** (brain: `claude-code/skills/dev/`). Read its SKILL.md, then this file, then `dev/DECISIONS.md`.

## Where things are

| What | Where |
|---|---|
| Brief, decisions | `dev/BRIEF.md`, `dev/DECISIONS.md` (append every decision) |
| The board | `dev/board/components.json` (331 components), rendered `dev/BOARD.md`; per-domain decisions in `dev/board/decisions/<domain>.py` (`python3 dev/scripts/merge_prep.py decide` regenerates the table in DECISIONS.md — it resets grades, use only for new components) |
| Raw features | `dev/board/raw/<app>.json` → `dev/board/features.json`; instructions `EXTRACT.md`, `MERGE.md` |
| Inspiration code | `dev/re/archive/*.tar.zst` (git-ignored) → `python3 dev/scripts/archive.py extract <app>` |
| UI references | `dev/re/evidence/{pdfexpert,preview,ilovepdf}/`; Sheaf proofs in `dev/re/evidence/F-00N/` |
| Features | `dev/features/F-001…F-004` |

## Board status

**331 components, 69 at ≥ 9.** Per domain (at 9 / total): ai 0/7, annotate 14/38, batch 0/9, compare 0/7, convert 0/20, edit 4/29, files 3/19, forms 4/17, optimize 0/15, organize 6/27, output 0/15, platform 1/14, preferences 1/14, reader 26/64, security 2/18, sign 4/18.
Grades below 9 say what is missing in `next`. `python3 dev/scripts/board.py next` gives the next component.

## What exists now (branch `dev/phase-4-5`, pushed)

- **F-001** every file dialog async and attached to the window (`src/dialog.rs`); the Mac click is still to prove.
- **F-002 reader** (`src/reader_ui/`, `src/reader_doc.rs`, `src/reader.rs`, `ui/screens/reader*.slint`): document
  tabs; outline, thumbnails, notes (author, date; hide all, summary, remove all), search (parallel, texts kept);
  highlight/underline/strike, note, pen, eraser, text box (size), shapes (rectangle, oval, line, arrow; fill,
  opacity), stamps (dated too), signature (place, resize), edit text and images, forms; undo/redo; ⌘S and Save As
  (incremental); print (CUPS); Document properties (⌘D); Day/Sepia/Night; two pages / cover; reopen as left;
  links and history; PDF Expert's shortcuts.
- **F-003 pages panel**, **F-004 forms** as before.
- PdfCraft patched for large files in `third_party/` (a 1,982-page book: annotation 0.19 s, save 1.1 s).

## Tests: one suite

- `bash scripts/test.sh` runs every layer and ends on one verdict line: `lint` (clippy `-D warnings`), `unit`
  (`cargo test --workspace`), `gui` (`dev/scripts/gui-smoke.sh`: the real app on Xvfb :98 driven with
  xdotool, checked on the files it writes with qpdf/pdftotext; needs a CUPS printer named PDF from
  `printer-driver-cups-pdf` for the print check). On this VM Cargo goes through `build-safe.sh` (memory cap).
  `bash scripts/test.sh gui` runs one layer while iterating; a unit closes on a full run. CI runs the same script.
- Outputs go to `dev/.tmp/` and are deleted on green (`SMOKE_KEEP=1` keeps the smoke screenshots to file a proof).
- After a green suite: `python3 dev/scripts/prune-target.py --fix` deletes the Cargo builds the suite no longer
  uses (stale profiles filled 7 GB on 21/10), then `python3 dev/scripts/hygiene.py --fix`.
- Every new component, bug or manual check adds its test to the existing layer (dev skill, house rule 1).
- Code review by Sol (Codex) before merging broad changes: write the diff to a file and run `codex exec … -s
  danger-full-access` with a "do not modify any file" prompt (Codex's bwrap sandbox cannot start on this VM).
  Check the Codex weekly quota first (newest `~/.codex/sessions` rollout with `rate_limits`); stop at 90 %.

## Levers (dev skill step 5.0): shared blockers first, then `board.py next`

Done 21/10: engine speed (`third_party/`, +8 components at 9), stale grades audited (+11 regraded).
Next, by components lifted:
1. Done: tests for C-edit-026 (now 9), C-reader-063 and C-files-010 (8: lock badge, separate windows). Was: (password
   open), C-files-010 (tab switch).
2. Done: author and date in the Notes panel, author in Preferences (C-annotate-030, C-reader-028, C-preferences-008 at 9).
3. Done: Shapes tool (R) with fill and opacity (C-annotate-012 at 9; 013, 023 at 8; 009 at 7). Left: drag a line's
   ends, a free colour picker and fill colour, text box font family (needs PdfCraft), style remembered per tool (024).
4. View modes: done two pages and cover alone (C-reader-015, 016 at 9; `Viewer::spread`). Left: single page (014),
   horizontal scroll (003), rotate the view (017).
5. **Menu bar** with every reader command: C-platform-014, and the menu entries C-reader-039 / 040 lack.
6. **Copy / paste of annotations and objects** (C-edit-010, C-annotate-026): PdfCraft has no duplicate edit and the
   snapshot lacks strokes, fill and line ends; needs reading the annotation's dictionary (or XFDF export/import).
7. Single page mode (C-reader-014), horizontal scroll (003), rotate view (017: the text layer is in page view space,
   so a view rotation must go through PDF space everywhere).

## Known limits worth the next session

- PdfCraft is patched in `third_party/` (README there): on a PdfCraft rev bump, re-apply the two changes. Asked
  Nassim whether to send them upstream.
- Edit mode (C-edit-001…014): move/resize existing objects, font choice, the glyph problem above.
- Print: preview, paper size, Windows (C-output-007 at 8); toolbox tools' remaining gaps (convert, optimize).

## Building on the VPS (7.9 GB RAM, no swap, 2 vCPU) — read before any build

- `dev/scripts/cargo.sh` waits while class-recorder records a lecture (`join_teams.py` running): never build over
  it. A suite started then simply waits.

- **Only** `sh dev/scripts/build-safe.sh build -p sheaf-app` (or `test --workspace --no-fail-fast`): it stops
  the test app and the ChatGPT desktop app (Nassim: it stays closed), waits for ≥ 3 GB free, caps the build at
  free − 700 MB and gives it OOMScoreAdjust=1000 so a shortage kills the build, not the session. rustc on
  sheaf-app peaks at ~2.4 GB (Slint's generated module). Twice on 21/10 the global OOM killer took the session.
- `dev/scripts/cargo.sh`: one target dir (`~/.cache/cargo-target/sheaf`), opt-level 2 dependencies, light debug info, no incremental.
- GUI tests: `systemd-run --user --unit=sheaf-gui -p MemoryMax=900M -p OOMScoreAdjust=1000 … bash
  dev/.tmp/run-app.sh <pdf>` on Xvfb :99, driven with xdotool (`windowfocus --sync` first; hold clicks ~0.1 s);
  dialogs need the XFCE desktop `:1` (GTK portal; `systemctl --user set-environment DISPLAY=:1` was needed).
- The app needs `libxkbcommon-x11-0` at run time on Linux.

## Rules carried over

- Commits: `git -c user.name="Sheaf contributors" -c user.email="267055942+MeNass89@users.noreply.github.com" commit`,
  message ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No personal names, emails, home
  paths or machine details in tracked files. Merge PRs locally. Push freely; tags and releases: ask Nassim.
- Talk to Nassim in French, vouvoiement, Alfred; 🦇 only emoji; short.
