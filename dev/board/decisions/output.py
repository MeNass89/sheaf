# Phase 4 decisions, domain output (2026-10-20).
PE = "pdfexpert"
Z = (0, 0, 0)
D = {
 "001": (PE, "⌘S saves annotations and edits in place, incremental", {"preview": "autosave and versions"}, Z, "save in place from the reader"),
 "002": (PE, "Save As sheet with name and folder", {"acrobat": "save as other formats"}, (4, 6, 8), "seed bug: the Save panel's Save button ignores clicks, only Enter works"),
 "003": ("ilovepdf", "always ask / next to original / fixed folder", {}, (8, 7, 9), "—"),
 "004": ("ours", "Sheaf-only progress overlay with cancel", {}, (8, 7, 8), "—"),
 "005": ("ilovepdf", "result screen: open, reveal, start over, continue with another tool (evidence 05c)", {"stirling": "chain tools"}, (8, 7, 8), "—"),
 "006": ("ilovepdf", "show in folder", {}, (8, 7, 9), "—"),
 "007": (PE, "print with scale options (evidence 12c)", {"acrobat": "page range, booklet"}, (0, 0, 0), "print via the OS"),
 "008": ("acrobat", "print layout options", {}, Z, "after print"),
 "009": (PE, "share sheet", {}, Z, "OS share (macOS only)"),
 "010": ("acrobat", "email as attachment", {}, Z, "mailto with attachment"),
 "011": (PE, "export flattened", {}, Z, "light"),
 "012": ("ilovepdf", "tasks history with reveal and rerun (evidence 06)", {}, (7, 7, 8), "rerun from history"),
 "013": ("ilovepdf", "notify when done", {}, Z, "light"),
 "014": ("preview", "revert to saved / versions", {"stirling": "undo history"}, Z, "revert to saved"),
 "015": ("preview", "rename / move to trash", {}, Z, "light"),
}
