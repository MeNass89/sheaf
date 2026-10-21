# F-001 — The Save button answers the click

Status: in progress (code done; the click itself can only be proved on a Mac)
Components: C-output-002, C-optimize-001 (the Save dialog that follows every tool run)
Grade before: C-output-002 function 4 · aesthetics 6 · performance 8 → 4
Why: Nassim (20/10): after Compress, clicking "Save" in the Save dialog does nothing; only Enter saves.

## Target
Every native file dialog Sheaf opens (Save, choose folder, open files, pick an image or a certificate) takes the
mouse exactly like the keyboard, as in PDF Expert, where Save As is a sheet attached to the document window.

## Cause
All dialogs were blocking `rfd::FileDialog` calls made inside the Slint click handler. On macOS that runs the
NSSavePanel's modal loop nested inside winit's mouse-event dispatch; the panel then gets keyboard events (Enter
confirms) but its buttons miss mouse-up. rfd's own sync path also forces a CGShieldingWindowLevel panel.

## Fix
`src/dialog.rs`: every dialog is an `rfd::AsyncFileDialog` with the window as parent, polled by
`slint::spawn_local`; the answer comes back on the UI thread through a closure. On macOS that is a sheet
(`beginSheetModalForWindow`), opened after the click handler has returned; on Linux the portal runs on a helper
thread; on Windows an owned dialog. The run continues only if the tab is still waiting (`Phase::Edit`).
The two dialogs whose answer Slint read synchronously (watermark image, signing certificate) now go through the
`Picked` global (path + counter, per tab).

## Closing criteria
- [x] Function: no blocking dialog left — proof: `rg 'rfd::' crates/sheaf-app/src` only in `dialog.rs`.
- [ ] Function: on macOS, Compress → Save sheet → click Save writes the file — proof: Mac session (pending: the last session ran on Linux).
- [x] Function: cancelling a dialog leaves the tab as it was; a closed tab ignores a late answer — proof: code (`run.rs` re-checks the phase).
- [ ] Aesthetics: Save As is a sheet on the window, like PDF Expert — proof: capture on a Mac.
- [x] Performance: the UI thread is never blocked by a dialog (async) — proof: design.

## Journal
- 2026-10-20: cause found in rfd 0.17 macOS backend (`panel_ffi.rs` run_modal); all 11 call sites moved to `dialog.rs`.
