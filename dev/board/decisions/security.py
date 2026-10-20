# Phase 4 decisions, domain security (2026-10-20).
Z = (0, 0, 0)
D = {
 "001": ("acrobat", "Protect tool hub", {}, (7, 7, 9), "—"),
 "002": ("ilovepdf", "set password with confirm, AES-256", {"acrobat": "compatibility level"}, (7, 7, 9), "strength meter, show-password toggle"),
 "003": ("pdfexpert", "change password", {}, (5, 6, 9), "change = unlock + protect today"),
 "004": ("acrobat", "permissions: printing, copying, editing", {"stirling": "permission checkboxes"}, (3, 5, 9), "permission toggles"),
 "005": ("pdfexpert", "password prompt on open", {}, (6, 6, 9), "prompt in the reader"),
 "006": ("ilovepdf", "unlock tool", {}, (8, 7, 9), "—"),
 "007": ("acrobat", "certificate encryption", {}, Z, "light"),
 "008": ("acrobat", "security policies", {}, Z, "light"),
 "009": ("acrobat", "Redact tool with marking, search and apply", {"stirling": "auto redact"}, (8, 7, 8), "—"),
 "010": ("acrobat", "mark text or area for redaction", {"pdfexpert": "redact by text selection"}, (6, 7, 8), "mark by text selection"),
 "011": ("acrobat", "redact pages by range", {}, (5, 6, 8), "page range"),
 "012": ("acrobat", "find text and patterns, mark all", {"stirling": "regex"}, (7, 7, 8), "regex, whole words, custom patterns"),
 "013": ("stirling", "review list of marks", {}, (7, 7, 8), "undo"),
 "014": ("acrobat", "overlay text and colour", {}, Z, "after core"),
 "015": ("acrobat", "apply with verification, removes hidden info", {}, (8, 7, 8), "preview of the redacted output"),
 "016": ("acrobat", "sanitize: metadata, JS, hidden layers", {"stirling": "options per item"}, Z, "engine hidden_info; add sanitize"),
 "017": ("stirling", "edit metadata fields", {"preview": "inspector"}, Z, "engine SetInfo"),
 "018": ("stirling", "PDF info report", {}, Z, "light"),
}
