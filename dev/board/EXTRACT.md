# Raw feature extraction — instructions for one agent

You extract the user-facing features of ONE inspiration app (or one part of it) from its reverse-engineered
sources, one row per gesture. Sources are under `dev/re/sources/pretty/<app>/` (read-only; never copy code).
Use `rg`/`grep`/`sed -n` to read regions; never read a file over 2,000 lines in full.

## Output

Write a JSON array to the file path you were given. Each row:

```json
{ "app": "pdfexpert", "domain": "annotate", "area": "Highlight", "feature": "Highlight selected text",
  "behavior": "Select text, pick Highlight in the context menu or toolbar; five preset colours.",
  "evidence": ["Highlight", "HighlightTool"], "source": "code" }
```

- `app`: exactly the id you were given (acrobat | pdfexpert | preview | ilovepdf | stirling).
- `domain`: exactly one of
  `reader` (open, scroll, zoom, page modes, outline, thumbnails, bookmarks, search, themes, tabs, full screen),
  `annotate` (highlight, underline, strike, notes, ink, shapes, stamps, text box, comments list),
  `edit` (edit text/images/links in place, add text/image, whiteout, header/footer, watermark, background, page numbers),
  `organize` (insert, delete, rotate, reorder, extract, split, merge, crop, resize pages),
  `forms` (fill, create fields, flatten, data import/export),
  `sign` (signatures, initials, digital certificates, fill & sign),
  `security` (password, permissions, redact, sanitize, metadata removal),
  `convert` (PDF to/from Word, Excel, PPT, images, HTML, text, PDF/A),
  `optimize` (compress, OCR, repair, linearize, grayscale),
  `compare` (compare two PDFs),
  `batch` (batch processing, pipelines, actions, automation),
  `output` (save, save as, export, print, share, results screen, history),
  `files` (home screen, recents, open, drag & drop, tools grid, file browser, cloud),
  `preferences` (settings),
  `ai` (AI assistant, summarise, translate, chat),
  `platform` (CLI, API, scripting, accessibility checker, updates, licensing).
- `area`: short sub-area name (e.g. "Highlight", "Outline", "Compress").
- `feature`: the single gesture, ≤ 10 words, starting with a verb.
- `behavior`: one or two sentences: what the user does and what happens, options, shortcuts if known.
- `evidence`: 1–4 EXACT strings copied from the sources (menu label, command id, class name, i18n key/value).
  They are searched literally later, so copy them character for character, short (≤ 60 chars).
- `source`: `code` when the evidence comes from these files.

## Granularity and scope

- One row per distinct gesture (e.g. "Highlight text", "Change highlight colour", "Delete annotation" are three rows).
  Options of one tool (e.g. compression levels) go in `behavior`, not separate rows, unless they are separate commands.
- Aim for completeness over the PDF-relevant surface: typically 80–250 rows per app. Skip pure plumbing
  (licensing pop-ups, crash reporters, telemetry, Sparkle updates, sign-in walls) unless it is a user feature.
- Do not guess. If a feature is only hinted at, either skip it or write behavior "unknown: …".
- Valid JSON only (check with `python3 -m json.tool <file> > /dev/null`). No trailing commas.

When done, reply with: the output path, the row count per domain, and the 3 files that gave the most rows.
If you find nothing, say so clearly and name the files you inspected.
