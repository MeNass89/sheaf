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
- 24/10 08:00, reviewer 8/8/6 on 35b1abd (words found 73.6 → 94.8 %, accented 0 → 95 %, placement median 0.5 pt).
  Blockers taken:
  - Characters WinAnsi cannot carry: the model reads α and → but the layer wrote them "?". Words that need it are
    now in `/PCUni`, a glyphless Unicode font (Type0, Identity-H, ToUnicode; Tesseract's 572-byte `pdf.ttf`,
    Apache-2.0, as OCRmyPDF). "anti-TNFα", "→ Cholangite" read back by pdftotext and pdfium; the page renders
    byte for byte the same. Test `words_beyond_winansi_are_kept_in_the_text_layer` (red: "anti-TNF? ?").
  - Turned text: ocrs finds a turned label as a tall box, part of one, or (large print) a stack of letters. Stacks
    in one column are joined, each tall box grown along its ink (against its own ground: white on a green cell
    counts), cut out, turned both ways and read; the surer reading replaces the upright one when it reads more.
    Words carry their `Turn` and are placed reading up or down the page. MICI: Clinique, Bio, Diagnostic,
    Endoscopie, Evolution, Prise en charge, Paraclinique, Coloscopie + iléoscopie, Médical, Chirurgical, FDR,
    Lésion macroscopique, Histo, DD all read. Tests `a_label_turned_on_its_side_is_read` (red: "C…_+SOE90■"),
    `a_region_turned_and_back_is_where_it_was`, `a_turned_label_grows_along_its_ink_on_a_dark_ground`,
    `a_turned_word_is_placed_reading_up_the_page`.
  Next: deskew, Tesseract benchmark on the same scan, release-build timing.
- 24/10 10:00, deskew: the MICI page turned 2.5° read 8.5 % of its words (lines run into each other across a dense
  page). The slant is now measured by projection profile (the angle at which the page's dark pixels fall into the
  sharpest rows; quarter degrees then fortieths, page reduced to 800 px; -2.52° found for -2.5°), the image
  straightened, read, and each word turned back by the same angle about the page's centre (`Word::tilt`, `pivot`),
  so it sits where the scan draws it. Each word is now followed by a space in the layer, as Tesseract writes it
  (pdftotext ran tilted words together). 2.5° page: 93.3 % of its words (`pdftotext -raw`), the straight page 93.4 %.
  Tests `a_scan_fed_in_askew_is_read` (red without: 4 of 102 words), `a_page_slant_is_measured_and_straightened`.
- 24/10 12:00, benchmark against Tesseract 5.3.4 (`fra`) on the same 4-page scan (words of the source found in the
  text: Sheaf via `pdftotext -raw`, Tesseract's own text output). Words that touch on a line (two read in one ocrs
  box) are now pulled apart so they extract as two (`apart`, test `words_that_touch_are_pulled_apart`): 94.6 →
  95.7 %. Splitting words at ocrs' gaps where the model read no space was tried and dropped (95.4 %).

  | | p1 | p2 | p3 | p4 | all | accented | time (4 pages) | memory |
  |---|---|---|---|---|---|---|---|---|
  | Sheaf, release | 96.1 % | 97.1 % | 93.6 % | 95.5 % | 95.7 % | 370/384 | 62 s | 234 MB |
  | Tesseract fra | 98.4 % | 96.4 % | 94.3 % | 95.0 % | 96.4 % | 369/384 | 47 s | 8 MB |

  Release build 2.5× faster than the debug one (62 s against 155 s); this VM has one physical core.
