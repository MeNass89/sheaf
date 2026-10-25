# F-011 · Merge: pages chosen per file (C-organize-021)

Main app: iLovePDF (Merge: drop files, reorder cards, merge). Acrobat's Combine adds a page choice per file.

## Closing criteria
- **Function.** Merge takes two or more PDFs (and open documents, C-files-015), in the card order; each PDF's
  card caption is its page choice ("All 4 pages"), clicked to type ranges in the order wanted ("4, 1-2", "3-"; Enter
  or leaving the field keeps it, Escape drops it, "all" or empty = every page). A choice outside the file says so
  on the card ("Pages 9: only 1–3") and holds the run back. Proof: `pages::merge_pages` (sheaf-core tests
  `merge_takes_the_pages_chosen_for_each_file`), `files::tests::a_merge_page_choice_reads_as_its_count_or_says_what_is_wrong`,
  gui-smoke 21d (output pages yellow red green light grey, the refused choice).
- **Aesthetics.** The caption turns to the accent colour when a choice is set, warning colour when invalid; the field
  sits in the caption's place with a hint ("e.g. 1-3, 5").
- **Performance.** One pdfium pass: pages copied by range (`copy_pages_from_document`), whole files appended.

## Journal
- 25/10: built. First try: the caption could not be clicked (the card's TouchArea lay over it); the field now lives
  inside it, as the hover buttons. Manual run: a.pdf "4,1-2", b.pdf "9" (refused, Merge disabled) then "3":
  a-merged.pdf has 4 pages, yellow red green light grey. Became gui-smoke 21d.
