# Phase 4 decisions, domain files (2026-10-20).
PE = "pdfexpert"
Z = (0, 0, 0)
D = {
 "001": (PE, "⌘O opens straight into reading; the open panel remembers the folder", {"ilovepdf": "open into a chosen tool"}, (6, 6, 8), "open into the reader, not only into tools"),
 "002": ("ilovepdf", "drop anywhere: home or tool; refuses non-PDFs with a message", {"stirling": "drop many files to the workbench"}, (7, 7, 8), "drop on the reader opens the document"),
 "003": (PE, "home with Open Files drop zone, tools grid and recents (evidence 01)", {"ilovepdf": "sidebar Recents/Tools/History/Preferences"}, (7, 7, 8), "recents with thumbnails; open-file drop zone"),
 "004": (PE, "recents with thumbnails, reopen at last page", {"preview": "File ▸ Open Recent menu"}, (6, 6, 8), "reopen into the reader at the last page; Open Recent menu"),
 "005": ("ilovepdf", "remove from recents, reveal, clear", {}, (5, 6, 8), "remove one / clear recents"),
 "006": (PE, "star favourites", {}, Z, "light"),
 "007": ("stirling", "search/sort the file list", {}, Z, "light"),
 "008": ("stirling", "local library", {}, Z, "light"),
 "009": ("acrobat", "cloud storages", {}, Z, "out of local-first scope (needs Nassim)"),
 "010": ("ilovepdf", "documents and tools open as tabs in one window", {"stirling": "workbench file switcher"}, (5, 6, 8), "document tabs"),
 "011": (PE, "⌘W closes the tab, asks to save if dirty", {}, (6, 6, 8), "⌘W, unsaved-changes prompt for documents"),
 "012": ("ilovepdf", "tools grid by category with colour tiles; same model", {"stirling": "tool search"}, (8, 8, 9), "—"),
 "013": ("stirling", "tool search field", {}, (6, 7, 9), "fuzzy matching, keyboard ↑↓ Enter"),
 "014": ("ilovepdf", "favourite tools", {}, Z, "light"),
 "015": ("ilovepdf", "add/remove/reorder input files in a tool", {}, (7, 7, 8), "drag reorder"),
 "016": ("ilovepdf", "password prompt per file", {}, (8, 7, 9), "—"),
 "017": ("ilovepdf", "default app prompt, OS context menu", {}, (3, 5, 8), "default-app registration (Linux .desktop done; macOS pending a Mac)"),
 "018": ("ilovepdf", "scan to PDF", {}, Z, "light"),
 "019": ("ours", "Sheaf-only: clear toasts for unreadable files", {}, (8, 7, 9), "—"),
}
