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

**331 components, 110 at ≥ 9.** Per domain (at 9 / total): ai 0/7, annotate 22/38, batch 1/9, compare 2/7, convert 2/20, edit 4/29, files 8/19, forms 4/17, optimize 1/15, organize 8/27, output 3/15, platform 1/14, preferences 2/14, reader 40/64, security 6/18, sign 6/18.
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

Done 21/10: engine speed (`third_party/`), stale grades audited, shapes/eraser/stamps, notes panel (author, date,
filter, sort, replies, status, hide, summary, remove all), two pages, properties, zoom field, page labels, bookmarks,
attachments, toolbox Compress flow audited against iLovePDF (`dev/re/evidence/F-001/sheaf-compress-*`).
Next (audits of 22/10 by the interim reviewer; joint 9/10 rule, see DECISIONS):
- C-annotate-007 text box: inline typing, font family and bold (PdfCraft's FreeText appearance is Helvetica only:
  needs `pdfcraft-annot` patched in third_party). C-edit-016 page numbers: font family, bold/italic/underline.
- C-organize-001 a full-window page grid in the reader with PDF Expert's ops strip.
- C-output-007 print preview, paper size and orientation, custom scale, selected pages.
- C-sign-003 several script fonts and a live preview of the typed name.
- C-annotate-013 drag a line's ends, Shift 0/45/90°, arrowheads; C-annotate-023 opacity slider, own fill colour, dashes
  (dashes need the engine's Style).
- Toolbox: what each tool lacks is in `dev/board/audit-toolbox-2026-10-22.md` (audit against iLovePDF's QML).
  `dev/.tmp/capture-tools.sh` (not kept: it is a dev aid; rewrite from the HANDOFF note if needed).
- Copy/paste of annotations; menu bar.

## How components reach 9 now
Author proposes, a second agent reviews (Sol when Codex is under 90 %, else an independent Opus agent, logged in
DECISIONS); both agree, then `board.py grade`. Reviews read diffs in /tmp/rv and gui-smoke screenshots
(`SMOKE_KEEP=1 bash scripts/test.sh gui`).

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
- Before a full suite (~13 min): `build-safe.sh clippy --workspace --all-targets -- -D warnings` (seconds when
  built) catches the test code a signature change broke; twice on 22/10 a full run died on it.
- One test file: `build-safe.sh test --workspace --test <name>` (seconds). Never `-p <crate>` alone: features
  are then not unified with the workspace and every dependency rebuilds (> 10 min, 22/10).
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
