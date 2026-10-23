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
- 24/10 05:00, speed and word places. Page 1 of the MICI scan (1654 × 2339): detection 2.6 s, reading 67 lines
  41.6 s (onnxruntime would take ~2.5× less on the same lines). This VM has one physical core: rten sizes its pool
  from physical cores, and forcing 2 threads gives 170 % CPU but no gain. Lines are now read on every core at once
  (`read_latin`), for Macs' performance cores; not measurable here. Word places: words take ocrs' boxes in order
  when the counts match; when ocrs found two words as one box ("cause inconnue"), each word keeps where the model
  read it, as tall as the box: the line reads whole again ("de cause inconnue pouvant"). Test
  `words_read_take_the_boxes_found_in_order`.
  Measured (debug build, deps optimised, 1 physical core): 43 s for a dense French page, 237 MB.
