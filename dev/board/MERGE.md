# Merging raw features into components — instructions for one agent (one domain)

Input: `dev/.tmp/merge/<domain>.json`, a list of rows. Rows with `"i"` come from the inspiration apps
(acrobat, pdfexpert, preview, ilovepdf, stirling); `"i"` is their global index. Rows with `"o"` are
Sheaf's own features (`app: "sheaf"`), with `"o"` their index and `limits` what Sheaf cannot do.

Output: `dev/.tmp/merge/<domain>.groups.json`, a JSON array of components:

```json
{ "id": "C-reader-001", "domain": "reader", "name": "Continuous vertical scroll",
  "definition": "Pages flow one after another in a single vertical scroll; the wheel, trackpad and scrollbar move through the whole document.",
  "weight": "strong", "rows": [12, 488, 903], "ours": [41] }
```

- A component is ONE user gesture or one tight gesture family that an app must do (e.g. "Highlight selected
  text", "Change an annotation's colour", "Compress a PDF (levels)", "Go to a page by number").
  The same gesture in several apps goes into the same component, even if worded differently.
- Size: about one component per 4–6 rows of the domain. Do not make catch-all components
  ("Annotation tools"), and do not split options of one tool into several components.
- `rows`: every `i` of the rows merged here. **Every `i` in the input must appear in exactly one component.**
- `ours`: every `o` of Sheaf's rows doing (part of) this gesture; `[]` when Sheaf lacks it. A Sheaf row may
  appear in several components only if it really covers them.
- Sheaf-only rows with no inspiration equivalent: make a component for them too (rows: [], ours: [o...]).
- `weight`: `strong` = felt every session by most users (open, scroll, zoom, search, highlight, save,
  compress, merge, sign…); `normal` = regular but not daily; `light` = rare or niche (developer, print
  production, accessibility tags, AI extras).
- `id`: `C-<domain>-NNN`, numbered from 001 in a logical order (most basic gesture first).
- `name` ≤ 8 words; `definition` one or two plain sentences describing the gesture, app-neutral.

Before finishing, run a check script: every input `i` used exactly once, ids unique, valid JSON.
Reply with: path, number of components, and any rows you were unsure about (index + why).
