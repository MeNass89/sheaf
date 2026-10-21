# F-004 — Fill forms in the reader

Status: closed
Components: C-forms-001, 002, 005, 017
Why: PDF Expert fills AcroForm fields in its reading view; Sheaf had no form support.

## Done
Fillable fields are tinted; a click on a text field opens an editor on it (↩ fills, Tab / ⇧Tab move between text
fields), check boxes and radio buttons change on click, combo and list boxes show their options. Clear form and
Flatten in the toolbar when the document has a form. Every change is a PdfCraft edit (`SetFieldValue`,
`ResetForm`, `Flatten`): undoable, saved incrementally.

## Proofs
dev/re/evidence/F-004/; saved values read back with `qpdf --json=2 --json-key=acroform`; flattening checked with
pdftotext (21/10/2026). Test file built by hand (text, check box, combo, text).
