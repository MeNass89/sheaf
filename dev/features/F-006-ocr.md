# F-006 · OCR a scanned PDF (C-optimize-006)

Main app: iLovePDF (OCR to a searchable PDF, language choice). Also Acrobat (Recognize Text, deskew), PDF Expert
(Recognize Text, language), Stirling (OCRmyPDF: languages, skip-text/force).

## Closing criteria
- **Function.** A scanned page in any Latin-script language becomes searchable and selectable with its words right,
  accents included; pages with text are skipped (or read with only new words added); protected scans; cancel; a
  batch. Proof: `crates/sheaf-core/tests/ocr.rs`, the MICI fiche scanned at 200 dpi (French) read back.
- **Aesthetics.** The page looks the same (invisible text layer over the untouched scan); the selection lands on
  the words; the options show the language list; progress per page.
- **Performance.** Seconds per page and peak memory on a 2-CPU machine, release build.

## Journal
- 24/10 02:30, baseline: PdfCraft reads with ocrs, whose recognition model knows unaccented Latin only (its
  `LANGUAGES` was English alone): the French MICI scan came out "Pr?valence", "d?but ? tout ?ge" (PdfCraft issue
  #524). Debug build: 174 s for 4 pages, 682 MB. Tesseract `fra` reads it right but is C++ to package on three OSes.
- 24/10 03:30, accents: PaddleOCR's PP-OCRv5 Latin recognition model (Apache-2.0, 7.5 MB ONNX, 32 languages) reads
  the lines ocrs finds (`third_party/pdfcraft-ocr/src/latin.rs`: crop with a quarter line of room, 48 px high,
  greedy CTC, each character's time step placing its word; words keep the boxes ocrs found under them). rten loads
  the ONNX file as published. `scripts/fetch-models.sh` fetches it (pinned revision, SHA-256); the three packages
  ship it with its notice. MICI scan: "Révision 25/10/2023", "Prévalence = 1/1000, début à tout âge avec un pic de
  fréquence à 20-30 ans"; debug 135 s for 4 pages, 254 MB. 29 languages offered.
  Test: `accented_letters_are_read` (red without the model: "Pr?valence?lev?e??ge").
  Left: "→" read as "?" (the text layer's font has no arrow), a line's words in a few places out of order, speed in a
  release build, deskew (Acrobat).
