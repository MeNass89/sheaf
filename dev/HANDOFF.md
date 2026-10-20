# Sheaf — handoff (MacBook Pro → VPS vpsn, 2026-10-21 00:30)

From now on Sheaf lives on **vpsn** (`/home/nassim/Projects/sheaf`). The MacBook Pro belongs to the game-porting
project; do not plan work that needs it. Method: the **`dev` skill** (brain: `claude-code/skills/dev/`). Read its
SKILL.md first, then this file, then `dev/DECISIONS.md`.

## Where things are

| What | Where |
|---|---|
| Brief, decisions | `dev/BRIEF.md`, `dev/DECISIONS.md` (append every decision you make) |
| Inspiration apps | `dev/board/apps.json`: Acrobat, PDF Expert, iLovePDF Desktop, Stirling-PDF, macOS Preview |
| Their code (archives) | `dev/re/archive/*.tar.zst` (git-ignored, 14 MB, copied to vpsn, sha256 in `dev/re/inventory.json`). `python3 dev/scripts/archive.py extract <app>` → `dev/re/sources/pretty/<app>/` |
| UI reference screenshots | `dev/re/evidence/{pdfexpert,preview,ilovepdf}/` + `README.md` (states captured, walls hit) |
| Sheaf's own feature inventory | `dev/board/ours.json` (272 gestures, `file:line`, honest limits) |
| RE helper tools | `dev/scripts/re-tools/` (zbin decoder for Acrobat, NIBArchive decoder, Qt rcc reader, Swift type extractor) |
| Board tooling | `dev/scripts/board.py`, `locate.py`, `archive.py`, `hygiene.py` |

What each archive holds (richest files first):
- **acrobat** (Reader 26.001): `strings/Primary.txt` (19,013 UI strings), plugin tables (PPKLite security, AcroForm forms, Comments, DigSig, Checkers accessibility), `aapp/` (50 tool definitions, XML), `resources/eve|exv` (dialog layouts), `Reader.sdef`.
- **pdfexpert** (3.13.4): `strings/…/Localizable.strings.txt`, `MainMenu.strings.txt`, `nibs/…/MainMenu.nib.txt` (menus + shortcuts), `symbols/*.types.txt` (class names: HighlightTool, AnnotationToolbarPanelViewController…), App Intents actions.
- **preview** (11.0): `Localizable.loctable.en_GB`, decoded nibs (menus with shortcuts, annotation inspector, preferences), symbol types.
- **ilovepdf** (2.3.0): `code/rcc/iLovePDF/**.qml` (448 real QML files: tools, parameters, reader/PdfViewer, signature, edit), `strings/iLovePdfDesktop.en.po.txt` (1,933 entries), C++ method names, 49 changelogs.
- **stirling** (2fe512a, after v3.2.0): full repo minus binaries. **MIT except `app/proprietary/`, `app/saas/`, `engine/`** — translate only from MIT parts.

## State of the dev phases

1. Brief — done (`BRIEF.md`).
2. Inspirations — done (Nassim approved the four, added Preview).
3. Acquire + RE + archive + delete — **done on the Mac**; installers deleted after archiving.
4. **Build the board — START HERE.** `features.json` and `components.json` are empty.
   - Extract raw features per app with cheap agents (Haiku/Luna), one per app area, from the archives
     (schema: `references/board-schema.md`). Include `ours.json` as our side.
   - Merge into components per domain; run `locate.py`; then **you** decide main app, `main_why`, `take`,
     weight, and grade Sheaf honestly on function / aesthetics / performance.
   - Domains to expect: reader (scroll, zoom, outline, thumbnails, search, night/sepia themes, tabs),
     annotate (highlight, underline, strike, notes, ink, shapes, stamps), edit (text in place, images, links,
     whiteout), organise pages, forms, sign, security/redact, convert, compress/OCR, compare, batch/pipelines,
     share/print/save, preferences, CLI/MCP.
5. Loop — after `board.py check` passes.

## Seeds from Nassim (put them on the board as strong components)

- **Save dialog after Compress: clicking "Save" does nothing; only Enter works.** Function bug, fix first.
- **Edit PDF has too few functions** → parity with PDF Expert's edit and annotate modes.
- **Sheaf must become a reader**: open any PDF to read and study (outline, thumbnails, continuous scroll,
  highlights, notes, night/sepia), not only a toolbox. PDF Expert is the main app for reading/annotating;
  Preview is the performance floor and a reading-UI reference.
- **General polish and responsiveness**: drive every interaction (mouse, keyboard, Enter/Escape, drag) and fix
  every rough edge. A button that needs Enter instead of a click is a function bug.

## Building and testing on vpsn (Linux x86_64, 2 vCPU, 7 GB RAM)

- Installed: Rust 1.99.1 (minimal), qpdf, poppler-utils, ripgrep, zstd, Xvfb, libfontconfig1-dev,
  libxkbcommon-dev, libudev-dev. `sh scripts/fetch-pdfium.sh && sh scripts/fetch-models.sh` before the first build.
- RAM is tight: build with `cargo build -j 2`; set one `CARGO_TARGET_DIR=$HOME/.cache/cargo-target/sheaf`
  for every worktree; keep ≤ 2 worktrees; `cargo clean -p sheaf-app` after release builds.
- GUI on Linux: `xvfb-run -s "-screen 0 1920x1080x24" target/debug/sheaf-app` and capture with
  `import -window root` (ImageMagick) or `xwd`; compare with `dev/re/evidence/` screenshots.
- macOS-only checks (Finder Open With, the DMG, Gatekeeper) cannot be done here; note them as pending for a
  Mac session rather than blocking on them.
- Disk: 18 GB free after cleanup. Run `python3 dev/scripts/hygiene.py --fix` at the end of every session.

## Rules carried over

- Commits: `git -c user.name="Sheaf contributors" -c user.email="267055942+MeNass89@users.noreply.github.com" commit`,
  message ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No personal names, emails,
  home paths or machine details in tracked files. Merge PRs locally (GitHub's merge button stamps his
  personal email). Push freely; releases/tags are outward-facing: ask Nassim.
- Third-party code stays out of git (archives are ignored); the board stores paths and line numbers only.
- Talk to Nassim in French, vouvoiement, Alfred; 🦇 only emoji; short.
