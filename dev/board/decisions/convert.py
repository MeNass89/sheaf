# Phase 4 decisions, domain convert (2026-10-20).
IL = "ilovepdf"
Z = (0, 0, 0)
D = {
 "001": ("acrobat", "Create PDF hub accepts any supported file in one place", {"stirling": "hide unavailable conversions"}, (6, 7, 8), "one 'Convert' entry that routes by file type"),
 "002": (IL, "Word/PPT/Excel to PDF with fidelity, batch; Acrobat needs Office installed on macOS", {"acrobat": "keeps links and bookmarks"}, (6, 6, 6), "needs LibreOffice installed; no sheet/print-area options"),
 "003": (IL, "images to PDF: orientation, margins, page size, one PDF or one per image", {"preview": "export as PDF"}, (6, 7, 8), "one PDF per image; Letter and other sizes"),
 "004": ("preview", "New from Clipboard ⌘N", {}, Z, "light"),
 "005": (IL, "HTML/URL to PDF with page size and screen mode", {"acrobat": "web capture"}, (5, 6, 7), "URL input"),
 "006": ("acrobat", "scan to PDF", {}, Z, "light"),
 "007": ("stirling", "other formats (ebook, markdown…)", {}, Z, "light"),
 "008": ("pdfexpert", "new blank document", {}, Z, "engine create_blank"),
 "009": (IL, "PDF to editable Word, OCR when scanned", {"acrobat": "flowing vs exact layout"}, (6, 6, 7), "layout fidelity"),
 "010": (IL, "PDF to Excel tables", {"acrobat": "one sheet per table option"}, (6, 6, 7), "table detection"),
 "011": (IL, "PDF to PowerPoint", {}, (6, 6, 7), "layout fidelity"),
 "012": (IL, "pages to JPG with quality; extract images mode", {"acrobat": "PNG/TIFF and DPI"}, (6, 7, 8), "PNG/TIFF, DPI choice"),
 "013": (IL, "extract embedded images", {}, Z, "after core"),
 "014": ("acrobat", "export as text/RTF", {}, Z, "export text (engine text layer)"),
 "015": ("acrobat", "HTML/XML export", {}, Z, "light"),
 "016": ("acrobat", "PostScript/ebook", {}, Z, "light"),
 "017": ("acrobat", "per-format export settings", {}, (4, 6, 8), "more options"),
 "018": (IL, "PDF/A conversion with level choice", {"acrobat": "preflight report"}, (7, 7, 7), "PDF/A-1b, accessible levels"),
 "019": ("acrobat", "PDF/X, PDF/E", {}, Z, "light"),
 "020": (IL, "offline vs online", {}, (9, 8, 9), "Sheaf is offline only by design"),
}
