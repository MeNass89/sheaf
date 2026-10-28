# Sheaf — handoff (VPS, 2026-10-25)

Sheaf is built and tested on the **Linux VPS** (the MacBook Pro is for another project). Method: the **`dev`
skill** (brain: `claude-code/skills/dev/`). Read its SKILL.md, then this file, then `dev/DECISIONS.md`.

## Instruction de Nassim (24/10/2026 11:03 UTC, relayée par Alfred) — appliquée 25/10 (DECISIONS)

Codex est à **13 % de quota** (rollouts 07:31 UTC, reset le 30/10), pas 89 %. La décision du 21/10 dans
`dev/DECISIONS.md` (Opus en 2e voix jusqu'au 27/10) est **périmée** : Sol redevient la 2e voix des 9/10 conjoints
dès maintenant. À faire au prochain tour : (1) appendre dans DECISIONS.md la fin de la dérogation du 21/10 ;
(2) faire relire par Sol les composants validés avec Opus depuis le 21/10 ; (3) continuer le board. Rien d'autre
ne change. Nassim : « Limits are at 16% consumed bro just use codex !! »

## Two sessions on one VM (25/10)

Two Claude sessions worked Sheaf at once on 25/10. Their suites collided on Xvfb :98. The rule they agreed:
**one heavy job at a time** (build, suite, Sol); before one, check
`pgrep -f "[s]cripts/test.sh|[b]uild-safe.sh|[g]ui-smoke.sh|[c]odex exec"` and wait while anything matches;
each works in its own git worktree under `/tmp/f5/` and rebases on what the other pushed, never force-pushing.
The second session stopped mid-way; its branch `dev/annot-gaps` (C-annotate-001/007/009/019/022, C-reader-026,
attachments, craft-fonts) was rebased onto round 7 and is being finished here (its suite was red on in-place
typing and Edit text).

## Last landing (28/10/2026, round 36)

Verdict: ALL GREEN (bash scripts/test.sh, full run, round 36, 1375 s). macOS scanner (ImageKit panel, scan_mac.rs),
local pictures through LibreOffice 26, craft fonts in every build (.cargo/config.toml); macOS unit layer green on
the MBP (ssh nassimlecornet@mbpn, ~/sheaf-ci; tight disk: build with CARGO_PROFILE_DEV_DEBUG=0, delete test
binaries after). Pinch checked on the MBP with synthetic magnify events (CGEvent type 29, round 37 notes).
Next: Sol reviews the scanner trio and C-reader-010; BAMA pass.
Open: the `pages` test binary dies by SIGSEGV about one full run in five, never alone (100 runs under 4×16
threads, 15 under 120–400 MB caps: none). dmesg: four crashes, threads a_page_changed_in_size…,
pages_change_size…, a_flipped_page…: all through pages::resize/flip (FPDFPage_Flatten then transform_with_clip,
under pdfium_lock); one ip inside libpdfium.so +0x39859d, three ip = fault address (error 15: a jump through a
freed pointer). Suspect pdfium-render's reload after Flatten (PdfPageIndexCache keyed by page handle). Next step:
cores (apport drops them; set core_pattern for the suite), or reload the page by index after flatten().

## Where things are

| What | Where |
|---|---|
| Brief, decisions | `dev/BRIEF.md`, `dev/DECISIONS.md` (append every decision) |
| The board | `dev/board/components.json` (331 components), rendered `dev/BOARD.md`; per-domain decisions in `dev/board/decisions/<domain>.py` (`python3 dev/scripts/merge_prep.py decide` regenerates the table in DECISIONS.md — it resets grades, use only for new components) |
| Raw features | `dev/board/raw/<app>.json` → `dev/board/features.json`; instructions `EXTRACT.md`, `MERGE.md` |
| Inspiration code | `dev/re/archive/*.tar.zst` (git-ignored) → `python3 dev/scripts/archive.py extract <app>` |
| UI references | `dev/re/evidence/{pdfexpert,preview,ilovepdf}/`; Sheaf proofs in `dev/re/evidence/F-00N/` |
| Features | `dev/features/F-001…F-013` |

## Board status

**331 components, 249 at ≥ 9** jointly (27/10, after landing rounds 25–27 e83625e and the four joint 9s of the parallel batches (journals F-013 Round 25, F-015
parallel batches). Joint reviews of their components are running (Sol batches b30a–c on round 25,
Sol ↔ Opus pair reviews on the batches: /tmp/f5/par/review-*.txt on vpsn). The inspiration archives
are in the private repo MeNass-Co/sheaf-re. The GUI smoke runs in three shards at once on a machine
with 4 cores or more (dev/scripts/gui-parallel.sh, displays 140–142; never :99, the class recorder's).

## What exists now (branch `dev/phase-4-5`, pushed)

- **F-001** every file dialog async and attached to the window (`src/dialog.rs`); the Mac click is still to prove.
- **F-002 reader** (`src/reader_ui/`, `src/reader_doc/`, `src/reader.rs`, `ui/screens/reader*.slint`): document
  tabs; outline, thumbnails, notes (author, date; hide all, summary, remove all), search (parallel, texts kept);
  highlight/underline/strike, note, pen, eraser, text box (size), shapes (rectangle, oval, line, arrow; fill,
  opacity), stamps (dated too), signature (place, resize), edit text and images, forms; undo/redo; ⌘S and Save As
  (incremental); print (CUPS); Document properties (⌘D); Day/Sepia/Night; two pages / cover; reopen as left;
  links and history; PDF Expert's shortcuts.
- **F-003 pages panel**, **F-004 forms** as before.
- **F-005 PDF to Word** (closed 9/9/9): own docx writer in `third_party/pdfcraft-export` (ruled tables with merged
  cells, shading, turned labels, runs with fonts, superscripts, header/footer parts, symbol fonts); corpus
  `dev/scripts/word-corpus.py` (25 checks).
- **F-006 OCR** (9/9/8): PaddleOCR Latin model on rten (`third_party/pdfcraft-ocr`, 29 languages, accents), turned
  labels, deskew, glyphless Unicode text layer; 95.7 % of words against Tesseract's 96.4 % on the MICI scan.
- **F-007 redact by selection** (closed 9/9/9): drag over text (live, caret ends), double/triple click, result counts.
- **F-008 link tool** (C-edit-001, C-annotate-021, C-edit-021 closed 9/9/9): Link tool (L), ⌘K on a selection, Link web
  addresses, borders (PdfCraft patched to write the link's /AP); Redact… in the reader's edit group.
  Nits left: move/resize a link's box, underline style and click highlight menus, width tooltips.
- **F-009 page objects** (C-edit-007 closed 9/9/9): ⇧-click/marquee/press pick, drag together, arrow nudge, paragraph
  resize; drawings are objects (PdfCraft patched: pdfcraft-edit/src/paths.rs). Nits: drawings have no resize
  handles; multi-selection resize not offered.
- PdfCraft patched for large files in `third_party/` (a 1,982-page book: annotation 0.19 s, save 1.1 s).

## Tests: one suite

- `bash scripts/test.sh` runs every layer and ends on one verdict line: `lint` (clippy `-D warnings`), `unit`
  (`cargo test --workspace`), `gui` (`dev/scripts/gui-smoke.sh` sets up and holds the helpers; the steps are
  `dev/scripts/smoke/1-reader.sh`, `2-documents-and-tools.sh`, `3-flows.sh`, run in that order in its shell: the
  real app on Xvfb :98 driven with xdotool, checked on the files it writes with qpdf/pdftotext; needs a CUPS printer named PDF from
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
- Done 22/10: text box fonts (pdfcraft-annot patched: 12 standard fonts, size, alignment, grow to fit) → C-annotate-007
  and 009 at 9. Left: in-place typing in the box (the Comment pop-up covers it); a typed decimal size is rounded
  (`text_size_typed` sends whole points); callouts don't grow; C-edit-004 needs text outside WinAnsi (embedded font)
  and more families. C-edit-016 page numbers: font family, bold/italic/underline (the same font table could serve).
- C-organize-001 a full-window page grid in the reader with PDF Expert's ops strip.
- C-output-007 print preview, paper size and orientation, custom scale, selected pages.
- C-sign-003 several script fonts and a live preview of the typed name.
- C-annotate-013 drag a line's ends, Shift 0/45/90°, arrowheads; C-annotate-023 opacity slider, own fill colour, dashes
  (dashes need the engine's Style).
- Toolbox: what each tool lacks is in `dev/board/audit-toolbox-2026-10-22.md` (audit against iLovePDF's QML).
  `dev/.tmp/capture-tools.sh` (not kept: it is a dev aid; rewrite from the HANDOFF note if needed).
- Copy/paste of annotations; menu bar.

## Waiting on Nassim

- **LibreOffice for Office → PDF (C-convert-002 at 7, reviewer 8/9/7).** Conversion needs LibreOffice
  installed; iLovePDF needs nothing. Bundle a headless LibreOffice with Sheaf (size, licence MPL), or
  accept it as a requirement (DECISIONS entry)? Fidelity on real Word files also needs Carlito/Caladea
  (Calibri/Cambria stand-ins), absent on vpsn.
- C-reader-010 (pinch / trackpad zoom) is at 8 until a real macOS trackpad session: pinch on page 30+ of a long
  file (the point under the fingers stays put), slow and fast ⌘ + trackpad scroll, Magic Mouse. Not doable on the
  Linux VM; the MacBook Pro is reserved for another project.

## How components reach 9 now
Author proposes, a second agent reviews (Sol when Codex is under 90 %, else an independent Opus agent, logged in
DECISIONS); both agree, then `board.py grade`. Reviews read diffs in /tmp/rv and gui-smoke screenshots
(`SMOKE_KEEP=1 bash scripts/test.sh gui`).

## Known limits worth the next session

- PDF to Word leftovers (non-blocking): "Révision" right tab in the header, footer logo text, "± Budésonide"
  alignment (F-005 journal).
- OCR speed: onnxruntime for the recognition model is the lever to performance 9 (DECISIONS 24/10).

- PdfCraft is patched in `third_party/` (README there): on a PdfCraft rev bump, re-apply the changes (search
  "Sheaf:"). Asked Nassim whether to send them upstream.
- Edit mode (C-edit-001…014): move/resize existing objects, font choice. C-edit-002 at 9 (on-page preview, rewrap, notices). Its nits: ⌘S skips a paragraph emptied in the editor; undo/redo with the editor open keeps the old paragraph index (close the draft on undo)
- Print: preview, paper size, Windows (C-output-007 at 8); toolbox tools' remaining gaps (convert, optimize).
- Tabs (C-files-010 at 9), reviewer's small follow-ups: the bar's `active-at` goes stale when an inactive tab is
  dragged across the active one (Rust could send the active place with `tab-order`); unit tests for
  `state::remove_tab_row` / `move_in_order`; no auto-scroll while dragging a tab in an overflowing bar.

## Building on the VPS (7.9 GB RAM, no swap, 2 vCPU) — read before any build

- `dev/scripts/cargo.sh` waits while class-recorder records a lecture (`join_teams.py` running): never build over
  it. A suite started then simply waits.

- **Only** `sh dev/scripts/build-safe.sh build -p sheaf-app` (or `test --workspace --no-fail-fast`): it stops
  the test app and the ChatGPT desktop app (Nassim: it stays closed), waits for ≥ 3 GB free, caps the build at
  free − 700 MB and gives it OOMScoreAdjust=1000 so a shortage kills the build, not the session. rustc on
  sheaf-app peaks at ~2.4 GB (Slint's generated module). Twice on 21/10 the global OOM killer took the session.
- Never edit code while a suite runs: each layer compiles at its start, so the verdict would cover a mix
  (twice on 22/10). Docs only, or wait.
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

- Commits: `git -c user.name="Sheaf contributors" -c user.email="sheaf@users.noreply.github.com" commit`,
  message ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No personal names, emails, home
  paths or machine details in tracked files. Merge PRs locally. Push freely; tags and releases: ask Nassim.
- Talk to Nassim in French, vouvoiement, Alfred; 🦇 only emoji; short.
