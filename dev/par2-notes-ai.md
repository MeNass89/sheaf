# Round 30, batch ai: notes

AI is in scope (Nassim, 27/10/2026). Every request sends only the document's text, never the file,
to the provider chosen in Preferences ▸ AI: OpenAI, Anthropic (Claude), Google Gemini, Mistral, xAI,
any OpenAI-compatible server (base URL), or a local Ollama / llama.cpp server. Automatic (the
default) uses a local server when one answers. Keys are encrypted in `ai-keys.secure` with the
profile's key, as the saved signatures are. No test calls a real provider: core and CLI tests run a
mock `TcpListener` that speaks each provider's format, and the GUI steps point the compatible
provider's address at a mock Python server.

Code: `crates/sheaf-core/src/ai/` (`mod.rs` providers, requests, errors; `tasks.rs` chunking,
map-reduce, ask, summarize, translate, podcast, slides, classify; `deck.rs` slides to PDF/PPTX;
`spaces.rs`), `crates/sheaf-app/src/ai_store.rs` (prefs + encrypted keys), `src/ai_ui.rs`,
`src/reader_ui/ai_doc.rs`, `ui/ai.slint`, `ui/screens/ai-panel.slint`,
`ui/screens/preferences-ai.slint`; CLI `sheaf ask | summarize | translate`.

## Components

- C-ai-001: the AI panel beside a document (✦ in the toolbar), its header naming the provider and
  model, the first-use note ("only this document's text, never the file, to <provider>", OK / Turn off
  AI), suggestion chips (Summarize, Main points, What is this about?, Podcast script, Slides, and
  Explain the selection when text is selected), Preferences ▸ AI ▸ Open the AI panel with every
  document; PDF Expert's AI Copilot panel (tests: `ai_store` tests, smoke 360).
- C-ai-002: ask about the document, some pages or the selection, with follow-up questions keeping the
  conversation; long documents mapped then reduced; answers cite pages; Acrobat's AI Assistant Q&A
  (`a_long_document_is_mapped_then_reduced`, `a_selection_is_asked_alone_and_earlier_turns_are_kept`,
  CLI `ask_sends_the_chosen_pages_text_to_the_provider_named_by_flags`, smoke 360).
- C-ai-003: summarize the whole document, pages or the selection, as paragraphs or main points; Copy,
  Save… (.txt/.md/.pdf); PDF Expert's Summarize (`summaries_translations_podcasts_and_slides_are_asked_for`,
  CLI `summarize_and_translate_read_the_environment`, smoke 361).
- C-ai-004: translate the selection, pages or the whole PDF into 14 offered languages (any language in
  the CLI), every part translated in order, saved as text or PDF; PDF Expert's Translate (same tests,
  smoke 361).
- C-ai-005: podcast script (two hosts) and slide deck, saved as PowerPoint through Sheaf's PDF to
  PowerPoint export or as a 16:9 PDF opened in Sheaf; Acrobat's Generate podcast / presentation
  (`a_deck_is_written_as_pdf_and_powerpoint`, `an_outline_becomes_slides`, smoke 362).
- C-ai-006: AI spaces: New space (with the open document), Add PDFs…, remove, delete; questions and
  summaries read every file, sources labelled "file.pdf, p. 3"; kept in `ai-spaces.json`; Acrobat's
  AI spaces (`spaces_are_saved_and_named_once`, `a_space_reads_every_file_with_its_name`, smoke 363).
- C-ai-007: classify the space's PDFs (or PDFs picked) into the categories typed, "Other" when none
  fits, or into categories the model names; results grouped by category; Stirling's Classify
  (`documents_are_classified_into_the_categories_given`, `answers_are_matched_to_the_categories`, smoke 363).

## Round 31

- Answers stream into the panel as the provider writes them (OpenAI-style and xAI/Mistral/compatible
  server-sent events, Anthropic's content_block_delta events, Gemini's streamGenerateContent?alt=sse,
  Ollama's JSON lines); a server that ignores `stream` is read whole. Stop keeps what was written.
  Notes of a long document's parts are not streamed, only the final answer
  (`answers_are_streamed_in_each_providers_format`, smoke 360 checks `"stream": true`).
- Error and empty states: no provider → a "No AI provider yet" card with Set up AI…; key missing or
  refused, AI off → Open AI settings…; network, rate limit, provider error, Stop → Try again; an empty
  space says how to fill it (smoke 364).
- Spaces: a space's file opens in its own tab when clicked; steps 360 and 363 recalibrated (the
  question field and the narrower New space / Add PDFs… buttons).

## Round 32

- C-ai-006: spaces renamed (Rename puts the cursor in the name; unique names kept), and PDFs dropped on
  the document while the panel shows a space join it (the drop hint says so; other files refused);
  Acrobat's AI spaces (`drops_go_into_the_space_shown_on_that_tab`, smoke 365). Not automatable: an
  external file drop — Xvfb has no XDND drag source; the routing is the unit test above.
- C-ai-005: Save audio… on a podcast script records it with the computer's voices (eSpeak NG's male
  and female variants of the Read aloud voice on Linux, two installed voices with `say` on macOS and
  SAPI on Windows), one per host, 0.4 s between turns, into one WAV; Acrobat's Generate podcast
  (`a_podcast_script_is_spoken_by_two_voices_into_one_wav`, `espeak_speaks_a_podcast_when_installed`,
  smoke 366). macOS and Windows synthesis is written but untested here (needs those machines).
- C-ai-004: a translation into any script saved as PDF: each character set in the first face that
  draws it (bundled craft fonts, then the system's open-licensed fonts), embedded as CID fonts with
  ToUnicode (searchable, copyable); Arabic and Thai shaped with PdfCraft's shaper, Arabic and Hebrew
  lines right to left (`a_translation_in_any_script_is_saved_as_a_searchable_pdf`, smoke 367).

## Round 33 (Sol's review of round 31, /tmp/f5/v2/rev31ai.txt)

- C-ai-001: the panel has its own pane: the pages are laid out beside it, never under it (PDF
  Expert's Copilot); its tools fold once a message arrives (Tools unfolds them), so the
  conversation has the room (smoke 368: the page's last word shows left of the panel).
- C-ai-002: each page an answer cites is a button that shows it ("p. 3"; in a space,
  "report.pdf · p. 3" opens that file there), Acrobat's citations (`cited_pages_are_found_in_an_answer`,
  smoke 368).
- C-ai-003: Main points come with the sentence that states each one; Highlight in PDF puts a real
  highlight annotation on each sentence, the point as its note, one Undo for all; PDF Expert's
  highlight main points (`main_points_carry_their_page_and_sentence`, smoke 369).
- C-ai-004: a right-to-left line keeps numbers and Latin runs in reading order, the point of 12.50
  and the colon of 10:30 inside them (`numbers_in_a_right_to_left_line_keep_their_order`,
  `a_number_in_an_arabic_line_keeps_its_value`).
- C-ai-005: decks in any script: slide text Helvetica cannot draw is set in an embedded font, in the
  PDF and so in the PowerPoint made from it (`a_deck_in_any_script_keeps_its_text_in_pdf_and_powerpoint`).
- C-ai-006: Export space… writes a `.sheafspace` file with paths relative to it; Import space… reads it
  back wherever the folder went (`a_shared_space_moves_with_its_folder`, smoke 370). Acrobat's shared
  spaces with recipients and comments need an online service: out of scope for a local app.
- C-ai-007: Classify writes each PDF's category in the PDF itself (`/Category` and its Keywords, as an
  incremental update, so signatures stay valid); the space lists its files grouped by category, with
  a filter (`a_category_is_kept_in_the_pdf_itself`, smoke 363 reads them back with `pdfinfo -custom`).
- Smoke: step 336's restore line (convert batch) had landed after step 367 in a merge, breaking a run
  of the AI steps alone ("AF: unbound variable"); moved back to the end of 336.

## 28/10/2026 — polish to 9/9/9 (branch wip/ai-polish)

### C-ai-005 Turn a document into a podcast or a slide deck

- Gap (author a8, round 34): eSpeak voices sound robotic; non-Western decks had no bold titles.
- Change, voices: `ai::audio` lists the computer's voices and ranks them for the script's language
  (`guess_language`: by script, then by common short words for Latin text). macOS: `say -v ?`,
  Premium > Enhanced > standard > Eloquence voices, novelty voices (Bubbles, Zarvox…) never chosen.
  Windows: OneCore voices (Windows.Media.SpeechSynthesis, spoken through WinRT from PowerShell),
  Natural or Neural ones first, then the desktop SAPI voices. A voice in both lists is kept once.
  Linux: Piper when installed with a `.onnx` model and its `.json` (`$PIPER_VOICES`, then
  `~/.local/share/piper*` and `/usr/share/piper-voices`), one voice per speaker of a multi-speaker
  model, else eSpeak NG's male and female variants as before. The two hosts get the best voice and
  then the best other one, of the other sex when both are known. Voices that write different
  formats (Piper 16 kHz, eSpeak 22 kHz) are joined in the first turn's format: mixed down and
  resampled. The podcast message has two voice lists, First host and Second host. Each defaults to
  the best two, and the choice is kept in Preferences (`ai.podcast_voices`). Under gui-smoke
  (`SHEAF_TEST_SPEECH`) the list is eSpeak NG's only, so the smoke does not depend on the machine.
- Change, decks: slide titles in a non-Western deck are set in a bold face when one draws the
  character: BIZ UDPGothic Bold (bundled) for Japanese, the system's DejaVu/Liberation/Noto Sans
  Bold for Cyrillic and Greek. With no bold face (Thai, Arabic, Georgian) the regular face is
  filled and stroked: synthetic bold, embedded as `Face,Bold` with ForceBold and the same font
  file. PdfCraft's PowerPoint export reads bold from the font name, so the .pptx titles are bold
  too (`b="1"`).
- Tests: `macos_hosts_get_the_premium_and_enhanced_voices_of_two_sexes`,
  `the_second_host_is_of_the_other_sex_when_as_good`, `windows_hosts_get_onecore_and_natural_voices_first`,
  `linux_hosts_get_piper_voices_before_espeak`, `the_language_of_a_script_is_guessed`,
  `voices_of_two_formats_are_joined_in_the_first` (unit, sample listings);
  `japanese_and_cyrillic_decks_have_bold_titles` (title font `BIZUDPGothic-Bold…` / a Bold face,
  points regular, text read back, pptx title run bold), `a_title_without_a_bold_face_is_drawn_in_synthetic_bold`
  (Thai: `,Bold` font, `2 Tr`, text read back); `espeak_speaks_a_podcast_when_installed` now
  goes through the ranking. Smoke 366 is unchanged (Save audio… still records a WAV).
- Not proved here: `say` and PowerShell synthesis need a Mac and a Windows machine (the listings
  are parsed from real sample outputs; the commands themselves are not run). Piper is not installed
  on the VPS: its ranking is tested on a model folder made by the test, but Piper never spoke.
- Self-grade: F 9 · A 9 · P 9.

### C-ai-007 Let AI sort documents into categories

- Gap (author a8, round 34): the category filter was a plain drop-down list.
- Change: the space's categories are chips, as in Stirling: "All 4", then "Invoices 3"… with the
  number of files of each in a muted figure. A click shows that category only, a second click shows
  them all again. Tab reaches each chip and Space or Return selects it; screen readers get a checkable
  button named with its count. Chips wrap on as many lines as needed. This is the app's chip
  (`redact-panel.slint`'s `Chip`), given an optional `count` and keyboard focus. Focus comes by Tab
  only, so a click on Redact's or Options' chips still leaves the focus in the field being typed in.
- Tests: smoke 363 adds a check that the chips read "All 2" and "Animals 2"; the filter logic
  (`space-act filter`) is unchanged. `every_interface_text_has_its_french` covers the new texts.
- Self-grade: F 9 · A 9 · P 9.

## Left, and why

- Right-to-left layout is per line (no full Unicode bidi algorithm for mixed nested runs); Arabic
  needs the bundled Arabic craft font to be shaped.
- A character no installed or bundled face draws is still refused, naming it.
- Default model names (gpt-5-mini, claude-sonnet-4-5, gemini-2.5-flash, mistral-small-latest, grok-4,
  llama3.2) are starting points; any model can be typed in Preferences ▸ AI.

## New dependency

- `ureq` 3.5 (default features off, `rustls`): a small blocking HTTP client, TLS by rustls with the
  ring provider and webpki roots. Pure Rust apart from ring's assembly (ring was already in the lock).
