# Phase 4 decisions, domain optimize (2026-10-20).
IL = "ilovepdf"
Z = (0, 0, 0)
D = {
 "001": (IL, "three levels (extreme/recommended/less) on a one-screen tool, result shows savings (evidence 03–05c)", {"acrobat": "font subsetting, object streams", "stirling": "custom quality"}, (7, 7, 8), "seed bug: Save after Compress needs Enter; fonts/vector recompression"),
 "002": (IL, "savings shown on the result screen", {}, (8, 7, 9), "target size, custom DPI"),
 "003": ("stirling", "compress to a target size", {}, Z, "after core"),
 "004": ("ours", "Sheaf-only batch compress of many files", {}, (8, 7, 8), "—"),
 "005": ("acrobat", "PDF Optimizer audit space and settings", {}, Z, "light"),
 "006": (IL, "OCR to a searchable PDF", {"acrobat": "deskew, image-only output"}, (7, 6, 6), "speed on big scans; deskew"),
 "007": (IL, "OCR language list", {}, (3, 5, 8), "more languages than English"),
 "008": ("acrobat", "OCR many files", {}, (6, 6, 7), "Sheaf OCR accepts several files"),
 "009": ("acrobat", "correct recognised text", {}, Z, "light"),
 "010": ("acrobat", "enhance scans (deskew, background)", {}, Z, "after core"),
 "011": (IL, "repair a damaged PDF", {}, (8, 7, 8), "—"),
 "012": ("acrobat", "preflight", {}, Z, "light"),
 "013": ("stirling", "grayscale", {}, Z, "light"),
 "014": ("acrobat", "print production", {}, Z, "light"),
 "015": ("acrobat", "full-text index", {}, Z, "light"),
}
