
### Round 25 (27/10, gaps left after round 24)

- C-optimize-005: Document properties ▸ Space used (Acrobat's Audit Space Usage): images, fonts, page
  content, attached files, metadata and the document's structure, sizes and shares (test
  `the_space_a_file_uses_is_told_by_what_takes_it`); Compress ▸ Advanced ▸ Unembed standard fonts
  (Acrobat's Unembed fonts: whole simple standard faces' programs out, subsets kept; test
  `standard_fonts_are_unembedded_and_subsets_kept`).
- C-output-008: the print sheet shows its first sheet as it will print (pages per sheet, booklet,
  grey, without comments), drawn again as they change (test `the_preview_shows_the_sheet_as_it_will_print`).
- C-reader-055: on Windows, Pause reading pauses the voice and resumes it where it was (the voice
  speaks asynchronously and follows a control file). not automatable: Windows's voice, no Windows
  session in CI.
- C-organize-005: Turn pages upright also turns scans with no text: each is read by OCR turned each
  way, and turned the way that reads clearly more real words (`enhance::scan_upright_turns`; test
  `a_scan_upside_down_is_found_by_reading_it`).
- C-files-017: the system's context menu on a PDF offers Sheaf's tools (Compress, Merge, Convert to
  Word, Protect, Sign, Make searchable): `sheaf-app --tool=<tool> files` opens that tool with them;
  Linux's desktop entry (the package's and the one Sheaf writes) lists them as actions, Windows's
  registration adds them as the PDF's verbs (test `a_tool_is_asked_for_on_the_command_line`).
  macOS's Services menu is left (an app extension, built on the Mac).
- C-annotate-027: a picture stamp whose page turn is only in its appearance's matrix keeps it when
  turned again (its turn read from the matrix).
- C-preferences-005: Preferences ▸ Rendering ▸ Use the graphics card or Draw with the processor
  (Acrobat's Use 2D GPU acceleration), from the next start (Slint's OpenGL or software renderer).
  Colour management (ICC display profiles) is left.
- C-preferences-006: Preferences ▸ Show art, trim and bleed boxes (outlined green, blue, red when they
  differ from the crop box; Acrobat's) and Page numbers as printed or by their place (Preview's Use
  logical page numbers).
- C-platform-002: Help ▸ Keyboard shortcuts (? or ⌘/, More tools ▸ Keyboard shortcuts): the reader's
  keys by what they do, in three columns (Acrobat's and PDF Expert's list).
- C-reader-064 (part): a PDF Portfolio (`/Collection`) opens on its Files panel, said; each file opens
  (a PDF in a tab, others in their app). Previewing non-PDF files in the reader is left (they go to
  a tool that makes a PDF of them, as iLovePDF does).
- C-output-009: More tools ▸ Copy the file (the document as it is now put on the clipboard as a file,
  to paste in a folder, a chat or a mail: PDF Expert's Share ▸ Copy) and Share with AirDrop (macOS's
  sheet). Smoke 110. not automatable: AirDrop (macOS only).
- C-preferences-010: a tool opens with the settings it last ran with (iLovePDF's remembered
  compression level, Acrobat's settings), what belongs to the files it ran on left at its defaults.
- C-sign-013 / C-preferences-014: Preferences ▸ Signatures (Acrobat's Identities & Trusted
  Certificates): the digital IDs signed with (kept when one is picked, added from a file, removed)
  and the certificates trusted (added from a .cer, .der or .pem file, removed) (test
  `a_certificate_file_is_read_in_der_or_pem`).
- C-convert-010: a heading over one row of words in two columns wide apart exports as a two-column
  table (test `a_heading_over_one_row_of_words_in_two_columns_is_a_table`); two short columns of
  prose close together stay text.
- C-edit-010: paragraphs copied with the Edit tool are pasted as text again, editable, in their font,
  size and colour; drawings and pictures copied with them as one item; one undo step (test
  `objects_copied_are_pasted_as_objects_without_what_lay_between_them`).
