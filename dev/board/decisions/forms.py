# Phase 4 decisions, domain forms (2026-10-20). Sheaf has no form support: 0/0/0.
PE = "pdfexpert"
Z = (0, 0, 0)
D = {
 "001": (PE, "click a field and type; tab order; checkboxes toggle", {"acrobat": "field highlight"}, Z, "fill AcroForm fields in the reader (engine SetFieldValue)"),
 "002": ("acrobat", "clear form", {}, Z, "engine ResetForm"),
 "003": ("acrobat", "required fields warning", {}, Z, "after fill"),
 "004": ("acrobat", "auto-complete", {}, Z, "light"),
 "005": ("acrobat", "highlight fields", {}, Z, "with fill"),
 "006": ("acrobat", "Prepare Form mode", {}, Z, "after core"),
 "007": ("pdfexpert", "auto-detect form fields", {"stirling": "auto form detection"}, Z, "after core"),
 "008": ("acrobat", "text field", {}, Z, "engine AddField"),
 "009": ("acrobat", "checkbox, radio, list, dropdown", {}, Z, "engine AddField"),
 "010": ("acrobat", "buttons", {}, Z, "light"),
 "011": ("acrobat", "barcode, signature fields", {}, Z, "light"),
 "012": ("acrobat", "properties dialog", {}, Z, "after core"),
 "013": ("acrobat", "format/validate/calculate", {}, Z, "light"),
 "014": ("acrobat", "move, duplicate, align", {}, Z, "after core"),
 "015": ("acrobat", "import/export FDF/XFDF", {}, Z, "light"),
 "016": ("acrobat", "distribute", {}, Z, "light"),
 "017": ("acrobat", "flatten", {"pdfexpert": "save as flattened"}, Z, "engine flatten"),
}
