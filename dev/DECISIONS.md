# Decisions — Sheaf (dev skill)

Each entry: date · decision · alternatives · reason · what would change it. Nassim reviews and overrides here.

## 2026-10-20 · Brief
**Decision.** Sheaf becomes the end-all PDF app: the one-off toolbox it already is (compress, OCR, merge…)
**and** the daily hub for reading, annotating and editing PDFs, at PDF Expert's level of polish.
**Alternatives.** Stay a toolbox only (Smallpdf/iLovePDF model). **Reason.** Nassim's direction (20/10): "the central
hub for editing, annotating, reading PDFs", e.g. a student studying. **Would change it.** Nassim.

## 2026-10-20 · Inspirations (phase 2)
**Decision.** Acrobat (breadth), PDF Expert (aesthetics, performance, reading/annotating/editing), iLovePDF Desktop
(toolbox model), Stirling-PDF (open-source code to translate), macOS Preview (performance floor, reading UI).
**Alternatives considered.** Foxit, PDFelement, Nitro (redundant with Acrobat), UPDF/PDFgear (closed, unverifiable
stack), Smallpdf (web, overlaps iLovePDF), Sejda (Electron, easy to read — kept as a fallback for in-place text
editing), PDF Arranger (GPL), pdf.js (viewer only, a later reference for text layers).
**Reason.** One app per north star plus one readable codebase; Nassim approved the four and added Preview (20/10).
**Would change it.** A component where none of the five has a good version → add a specialist app for it.

## 2026-10-20 · Known polish bugs reported by Nassim (seed the board)
- Save dialog after Compress: clicking "Save" does nothing; only Enter works. Function bug, P1.
- Edit PDF has too few functions. P1, to be measured against PDF Expert.
- No reading mode for long study sessions (outline, thumbnails, continuous scroll, highlights, notes). P1.

## 2026-10-20 · Work split across machines
**Decision.** Acquisition and reverse engineering on the MacBook Pro (the apps are macOS); building, testing and
screenshots on the VPS from tomorrow, using the Linux build and the archives. **Reason.** Nassim's plan.
