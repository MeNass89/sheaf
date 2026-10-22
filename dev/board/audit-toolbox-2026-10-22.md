# Toolbox audit, 22/10/2026 (interim second reviewer, against iLovePDF's QML and Stirling)

What each toolbox component below 9 still lacks. Delete a line when its component reaches 9.

- C-organize-001 Organize: several files; per-page rotate/duplicate and drag in the grid; blank page at a chosen spot.
- C-organize-004 Rotate: live-turning thumbnails, Left/Right buttons, Reset all; portrait/landscape and even/odd filters.
- C-organize-015 Split: "Merge all ranges into one PDF"; range rows with from/to thumbnails; count of files created.
- C-organize-016 Split: by size (MB/KB, allow compression) and by bookmarks; typable stepper; "N PDF will be created".
- C-organize-018 Crop: drag-a-box crop on the page; remove white margins; default margins 0 (10 mm today).
- C-organize-021 Merge: images and Office files (paper size, orientation, margin); per-file page ranges and rotation.
- C-organize-022 Merge: drag-and-drop file order; sort by name; expand a file into its pages.
- C-optimize-006 OCR: deskew, auto-rotate; real multi-language recognition.
- C-optimize-007 OCR: more than English; use every ticked language (run.rs keeps only the first).
- C-optimize-011 Repair: offer repair when the reader meets a damaged file; tests/repair.rs; wrong-extension detection.
- C-convert-001 one Convert entry routing by file type.
- C-convert-002 Word to PDF: .dot/.dotx/.dotm/.docm; Excel sheet/print-area options.
- C-convert-003 JPG to PDF: US Letter, per-image orientation, "Merge all images into one PDF".
- C-convert-005 HTML to PDF: URL field, page size / one long page, orientation, margin.
- C-convert-009/010/011 PDF to Office: detect scanned pages and suggest OCR; layout fidelity; table detection.
- C-convert-017 export options: retain layout vs flowing text.
- C-convert-018 PDF/A: 1b/1a/2u/2a/3u/3a and Allow Downgrade (the PdfCraft preflight has only 2b and 3b).
- C-edit-001/002/004/005/006/007/010 Edit tool: inside the reader; edit text in place; family/bold/align; colour
  picker; resize handles; move existing objects; copy/paste/Delete.
- C-edit-027 / C-security-001 Protect: restrictions with only a permissions password; hidden-info/sanitize entry.
- C-security-003 a Change password action.
- C-security-009..015 Redact (012 at 9): mode bar and fill colour; mark by text selection; page range/odd/even dialog; regex;
  undo for marks; preview of the redacted output.
- C-sign-003 several script fonts, colour; C-sign-004 background removal; C-sign-012 timestamp server, visible
  appearance, reveal on the certificate password; C-sign-017 signature details panel.
- C-compare-001 pick a file open in a tab; C-compare-003/004/006 in-app change list, overlay, summary.
- C-output-004 progress within a file and mid-job cancel for single-input jobs; C-output-012 rerun with the same options.
