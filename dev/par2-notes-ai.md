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

## Left, and why

- Audio for the podcast: Acrobat speaks it; Sheaf writes the script (a text-to-speech voice per host
  would need a TTS engine or a provider's audio API).
- PDF output of translations uses Helvetica (Western languages); other scripts are refused with a
  word to save as .txt, which keeps every language.
- Default model names (gpt-5-mini, claude-sonnet-4-5, gemini-2.5-flash, mistral-small-latest, grok-4,
  llama3.2) are starting points; any model can be typed in Preferences ▸ AI.
- Answers are not streamed: each request returns whole (the status line shows the part being read,
  and Stop cancels between requests).

## New dependency

- `ureq` 3.5 (default features off, `rustls`): a small blocking HTTP client, TLS by rustls with the
  ring provider and webpki roots. Pure Rust apart from ring's assembly (ring was already in the lock).
