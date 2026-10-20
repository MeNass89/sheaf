# Phase 4 decisions, domain sign (2026-10-20).
PE = "pdfexpert"
Z = (0, 0, 0)
D = {
 "001": (PE, "Fill & Sign strip in the reader, signature one click away", {"acrobat": "Fill & Sign tool"}, (5, 6, 8), "sign from the reader, not only a tool tab"),
 "002": (PE, "draw with smoothing, colour and thickness", {"preview": "trackpad signing"}, (6, 6, 8), "pen colour and thickness"),
 "003": ("stirling", "type with several fonts", {"pdfexpert": "type option"}, (6, 6, 8), "several script fonts"),
 "004": ("preview", "camera/image with background removal", {"pdfexpert": "image import"}, (5, 6, 8), "background removal"),
 "005": (PE, "drag to place, corner handles to resize, several signatures per document", {"ilovepdf": "place on several pages"}, (5, 6, 8), "free resize; several placements per run"),
 "006": ("acrobat", "initials", {}, Z, "initials"),
 "007": ("ilovepdf", "repeat on pages", {}, Z, "after core"),
 "008": ("stirling", "delete placed signature", {"preview": "select + Delete"}, (5, 6, 8), "Delete key"),
 "009": (PE, "saved signatures reused across documents", {"preview": "signatures in Keychain"}, Z, "remember signatures"),
 "010": ("acrobat", "check, cross, dot marks for forms", {}, Z, "with forms"),
 "011": ("acrobat", "request signatures (cloud)", {"stirling": "sign request"}, (2, 4, 8), "out of local-first scope (cloud); light"),
 "012": ("acrobat", "certificate signing with appearance, reason, location, timestamp", {"ilovepdf": "digital signature tool"}, (6, 6, 8), "timestamp server; validation"),
 "013": ("acrobat", "digital ID manager", {}, Z, "light"),
 "014": ("acrobat", "certify", {}, Z, "light"),
 "015": ("acrobat", "timestamp", {}, Z, "engine timestamp_document"),
 "016": ("acrobat", "signature panel with validity", {}, Z, "engine signatures panel"),
 "017": ("acrobat", "signature properties", {}, (3, 5, 8), "after validation"),
 "018": ("acrobat", "clear signature", {}, Z, "light"),
}
