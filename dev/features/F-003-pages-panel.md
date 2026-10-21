# F-003 — Organise pages from the reader

Status: closed (organize-001 and -002 at 8, see board)
Components: C-organize-001, 002, 003, 007, 009, 013, 014
Why: PDF Expert organises pages from its thumbnails panel; Sheaf only had separate tools (Organize, Rotate, Remove).

## Done
The Pages panel selects pages (click, ⇧ range, ⌘ toggle), drags them to a new place (drop line), and its strip
rotates, inserts a blank page, duplicates, extracts (Save dialog, the new PDF opens) and deletes. Every operation
is a PdfCraft edit on the document thread: undoable, saved with ⌘S.

## Proofs
dev/re/evidence/F-003/; page counts and order checked in saved files with qpdf and pdftotext (21/10/2026).

## Left
A full-window page grid (PDF Expert's organiser view), a drag ghost image.
