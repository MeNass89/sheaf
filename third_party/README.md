# Third-party code, patched

Two crates of [PdfCraft](https://github.com/storytold/pdfcraft) (MIT OR Apache-2.0, licences in each
folder) at rev `d3a91c5c5f7b33dd39bc2ed8b5d3916960736d24`, copied here with one change each and used through the `[patch]` table of the
workspace `Cargo.toml`. Every other PdfCraft crate still comes from git at the same rev. Both changes
make large documents fast; measured on a 1,982-page, 70 MB book (`reader_doc::tests::edit_cost_on_a_big_file`).

| Crate | Change | Before → after |
|---|---|---|
| `pdfcraft-cos` | `EDITOR_CACHE_OBJECTS` (src/document.rs) 1,024 → 65,536. After each edit the engine walks every page twice (comment list, missing appearances); a 1,024-object cache was emptied on every walk and the file parsed and inflated again. The 32 MB byte budget is unchanged. | annotation 2.13 → 0.20 s, undo 2.0 → 0.19 s, open 8.6 → 4.5 s; peak memory 370 → 465 MB |
| `pdfcraft-engine` | `Session::mark_saved` (src/lib.rs): after an incremental save (old file + appended update, objects keep their numbers) the light refresh of a comment edit instead of the full re-inspection. A full rewrite (Save As) keeps the full refresh. `src/tests.rs` (upstream's tests, 250 KB) is left out. | ⌘S 4.6 s → see DECISIONS |

**To update PdfCraft:** bump the rev in the workspace `Cargo.toml` and in both `Cargo.toml` here, copy
`crates/cos/src` and `crates/engine/src` of the new rev over these, re-apply the two changes (search for
"Sheaf:"), run `bash scripts/test.sh`. Drop a folder once upstream carries its change.
