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

## Left, and why

- Right-to-left layout is per line (no full Unicode bidi algorithm for mixed nested runs); Arabic
  needs the bundled Arabic craft font to be shaped.
- A character no installed or bundled face draws is still refused, naming it.
- Default model names (gpt-5-mini, claude-sonnet-4-5, gemini-2.5-flash, mistral-small-latest, grok-4,
  llama3.2) are starting points; any model can be typed in Preferences ▸ AI.

## New dependency

- `ureq` 3.5 (default features off, `rustls`): a small blocking HTTP client, TLS by rustls with the
  ring provider and webpki roots. Pure Rust apart from ring's assembly (ring was already in the lock).
