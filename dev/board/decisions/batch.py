# Phase 4 decisions, domain batch (2026-10-20).
Z = (0, 0, 0)
D = {
 "001": ("ilovepdf", "each tool accepts many files and writes one output each", {"acrobat": "Action Wizard file lists"}, (7, 7, 8), "Sheaf tools take several files for compress, rotate, protect, OCR…"),
 "002": ("acrobat", "Action Wizard steps", {"stirling": "automate pipelines"}, Z, "after core"),
 "003": ("acrobat", "run saved actions", {}, Z, "after core"),
 "004": ("stirling", "import/export pipelines JSON", {}, Z, "light"),
 "005": ("acrobat", "suggested actions", {}, Z, "light"),
 "006": ("acrobat", "custom tools", {}, Z, "light"),
 "007": ("ilovepdf", "continue to another tool from the result (evidence 05c)", {}, (8, 7, 9), "—"),
 "008": ("stirling", "watched folders", {}, Z, "light; CLI covers scripting"),
 "009": ("ilovepdf", "minimise or cancel a running task", {}, (7, 7, 8), "run in background while reading"),
}
