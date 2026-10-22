//! The names and keys PDF (and PdfCraft) define, and the structure roles a document declares:
//! structure, never the user's text, for both the clean-up and the audit.

use super::pdf::sub_dict;
use std::collections::HashSet;

/// Names PDF itself defines (types, subtypes, filters, colour spaces, actions, annotation and
/// field kinds, standard structure roles, …): structure, never the user's text.
pub(super) const PDF_NAMES: &[&str] = &[
    "Catalog", "Pages", "Page", "Font", "FontDescriptor", "XObject", "Form", "Image", "Annot", "Annots", "Link", "Widget", "Text", "FreeText", "Line",
    "Square", "Circle", "Polygon", "PolyLine", "Highlight", "Underline", "Squiggly", "StrikeOut", "Stamp", "Caret", "Ink", "Popup", "FileAttachment",
    "Sound", "Movie", "Screen", "PrinterMark", "TrapNet", "Watermark", "3D", "Redact", "Projection", "RichMedia", "Outlines", "Metadata", "XML",
    "Filespec", "EmbeddedFile", "Encoding", "ExtGState", "Pattern", "Shading", "Group", "Transparency", "Mask", "Alpha", "Luminosity", "OCG", "OCMD",
    "OCProperties", "StructTreeRoot", "StructElem", "MCR", "OBJR", "ObjStm", "XRef", "Sig", "DocTimeStamp", "SigRef", "TransformParams", "Action",
    "Type0", "Type1", "Type1C", "Type3", "TrueType", "MMType1", "CIDFontType0", "CIDFontType0C", "CIDFontType2", "OpenType", "CMap", "Identity",
    "Identity-H", "Identity-V", "WinAnsiEncoding", "MacRomanEncoding", "MacExpertEncoding", "StandardEncoding", "PDFDocEncoding", "FlateDecode",
    "LZWDecode", "ASCIIHexDecode", "ASCII85Decode", "RunLengthDecode", "CCITTFaxDecode", "JBIG2Decode", "DCTDecode", "JPXDecode", "Crypt", "Fl",
    "AHx", "A85", "LZW", "RL", "CCF", "DCT", "DeviceGray", "DeviceRGB", "DeviceCMYK", "CalGray", "CalRGB", "Lab", "ICCBased", "Indexed",
    "Separation", "DeviceN", "NChannel", "All", "None", "G", "RGB", "CMYK", "I", "GoTo", "GoToR", "GoToE", "Launch", "Thread", "URI", "Named",
    "SubmitForm", "ResetForm", "ImportData", "JavaScript", "SetOCGState", "Rendition", "Trans", "GoTo3DView", "Hide", "NextPage", "PrevPage",
    "FirstPage", "LastPage", "Print", "View", "Design", "Export", "XYZ", "Fit", "FitH", "FitV", "FitR", "FitB", "FitBH", "FitBV", "Tx", "Btn", "Ch",
    "Off", "On", "Yes", "No", "True", "False", "Unknown", "UseNone", "UseOutlines", "UseThumbs", "FullScreen", "UseOC", "UseAttachments",
    "SinglePage", "OneColumn", "TwoColumnLeft", "TwoColumnRight", "TwoPageLeft", "TwoPageRight", "L2R", "R2L", "Simplex", "DuplexFlipShortEdge",
    "DuplexFlipLongEdge", "AppDefault", "Normal", "Multiply", "Overlay", "Darken", "Lighten", "ColorDodge", "ColorBurn", "HardLight", "SoftLight",
    "Difference", "Exclusion", "Hue", "Saturation", "Color", "Compatible", "Perceptual", "RelativeColorimetric", "Saturation", "AbsoluteColorimetric",
    "S", "D", "B", "U", "R", "P", "N", "C", "F", "H", "O", "T", "V", "W", "Q", "Comment", "Key", "Note", "Help", "NewParagraph", "Paragraph", "Insert",
    "Check", "Cross", "Star", "Diamond", "OpenArrow", "ClosedArrow", "Butt", "ROpenArrow", "RClosedArrow", "Slash", "Approved", "Experimental",
    "NotApproved", "AsIs", "Expired", "NotForPublicRelease", "Confidential", "Final", "Sold", "Departmental", "ForComment", "TopSecret", "Draft",
    "ForPublicRelease", "FreeTextCallout", "FreeTextTypeWriter", "LineArrow", "LineDimension", "PolygonCloud", "PolyLineDimension", "PolygonDimension",
    "Group", "Inline", "Document", "DocumentFragment", "Part", "Art", "Sect", "Div", "BlockQuote", "Caption", "TOC", "TOCI", "Index", "NonStruct",
    "Private", "H1", "H2", "H3", "H4", "H5", "H6", "Title", "FENote", "Sub", "Lbl", "LBody", "LI", "Table", "TR", "TH", "TD", "THead", "TBody",
    "TFoot", "Span", "Quote", "Reference", "BibEntry", "Code", "Em", "Strong", "Ruby", "RB", "RT", "RP", "Warichu", "WT", "WP", "Figure", "Formula",
    "Artifact", "Layout", "List", "PrintField", "Pagination", "Background", "Header", "Footer", "Watermark", "Disc", "Decimal", "UpperRoman",
    "LowerRoman", "UpperAlpha", "LowerAlpha", "Ordered", "Unordered", "Row", "Column", "Both", "Block", "Start", "Center", "End", "Justify",
    "Before", "Middle", "After", "Auto", "Solid", "Dashed", "Dotted", "Double", "Groove", "Ridge", "Inset", "Outset", "Hidden", "NoView",
    "Toggle", "ReadOnly", "Locked", "LockedContents", "AlternatePresentations", "SlideShow", "Media", "MediaClip", "MediaClipData", "MediaPlayers",
    "Rendition", "SR", "MR", "Pagelabels", "Lang", "Marked", "Supplement", "Registry", "Ordering", "Adobe", "UCS", "Event", "Ve", "Va", "Vf",
    "Distiller", "PageElement", "PageItem", "Unchecked", "Checked", "Matte", "Default", "DefaultGray", "DefaultRGB", "DefaultCMYK", "Halftone",
    "Function", "Sampled", "Interpolate", "BBox", "Matrix", "Resources", "Contents", "Kids", "Parent", "Count", "Type", "Subtype",
    // PdfCraft's own content tags (/PCMark values) and procedure sets.
    "Redaction", "Added", "Wrap", "HeaderFooter", "PDF", "ImageB", "ImageC", "ImageI",
];

/// The custom structure roles and classes a document declares (its `/RoleMap` and `/ClassMap`
/// keys): names its producer chose for its styles ("Text body"), structure like PDF's own.
pub(super) fn role_names(d: &lopdf::Document) -> HashSet<Vec<u8>> {
    let mut out = HashSet::new();
    let Some(root) = d.catalog().ok().and_then(|c| sub_dict(d, c, b"StructTreeRoot")) else { return out };
    for k in [&b"RoleMap"[..], b"ClassMap"] {
        if let Some(m) = sub_dict(d, root, k) {
            out.extend(m.iter().map(|(k, _)| k.clone()));
        }
    }
    out
}

pub(super) fn is_standard_name(n: &[u8], roles: &HashSet<Vec<u8>>) -> bool {
    let s = String::from_utf8_lossy(n);
    PDF_NAMES.contains(&s.as_ref()) || STANDARD_KEYS.binary_search(&s.as_ref()).is_ok() || roles.contains(n)
}

/// Keys (and a few names) PDF and PdfCraft define, sorted: a key named so is structure, not the
/// user's text. Drawn from the PDF reference and PdfCraft's own vocabulary.
pub(super) const STANDARD_KEYS: &[&str] = &[
    "A", "A85", "AA", "AC", "ADBE", "AESV2", "AESV3", "AF", "AFRelationship", "AHx", "AIMetaData", "AIS", "AP", "APRef", "AS", "ASCII85Decode",
    "ASCIIHexDecode", "AbsoluteColorimetric", "Access", "AcroForm", "Action", "ActualText", "Added", "Adobe", "After", "Align", "AllOff", "AllOn",
    "Alpha", "Alt", "Alternate", "Alternates", "Annot", "Annots", "AntiAlias", "AnyOff", "AnyOn", "App", "Approved", "Arabic", "Art", "ArtBox",
    "Artifact", "Ascent", "Attached", "Attribute", "AuthEvent", "Author", "AvgWidth", "B", "BBox", "BC", "BDC", "BE", "BEAD", "BG", "BG2", "BI", "BM",
    "BMC", "BPC", "BS", "BT", "BX", "Background", "BackgroundColor", "BaseEncoding", "BaseFont", "BaseState", "BaseVersion", "BaselineShift",
    "Before", "BibEntry", "BitsPerComponent", "BitsPerCoordinate", "BitsPerFlag", "BitsPerSample", "BlackIs1", "BlackPoint", "BleedBox", "BlockAlign",
    "BlockQuote", "Border", "BorderColor", "BorderStyle", "BorderThickness", "Bounds", "BoxColorInfo", "BrotliDecode", "Btn", "ByteRange", "C", "CA",
    "CCF", "CCITTFaxDecode", "CF", "CFM", "CI", "CIDFontType0", "CIDFontType0C", "CIDFontType2", "CIDSet", "CIDSystemInfo", "CIDToGIDMap", "CL",
    "CMYK", "CMap", "CMapName", "CMapType", "CNS1", "CO", "CP", "CRL", "CRLS", "CRLs", "CS", "CYX", "CalCMYK", "CalGray", "CalRGB", "CapHeight",
    "Caption", "Caret", "Catalog", "CenterWindow", "Cert", "Certs", "Ch", "CharProcs", "CharSet", "CheckSum", "Circle", "ClassMap", "ClosedArrow",
    "ClrF", "ClrFf", "CoBO", "CoBo", "CoOb", "Code", "ColSpan", "Collection", "CollectionField", "CollectionItem", "CollectionSchema",
    "CollectionSort", "CollectionSubitem", "Color", "ColorBurn", "ColorDodge", "ColorSpace", "ColorTransform", "Colorants", "Colors", "ColumnCount",
    "ColumnGap", "ColumnWidths", "Columns", "Compatible", "Components", "ContactInfo", "Contents", "Coords", "Count", "Cour", "Courier",
    "Courier-Bold", "Courier-BoldOblique", "Courier-Oblique", "CreationDate", "Creator", "Crop", "CropBox", "Crypt", "CryptFilter", "Custom",
    "CustomData", "D", "DA", "DCT", "DCTDecode", "DL", "DOS", "DP", "DR", "DS", "DSS", "DV", "DW", "DW2", "Darken", "Date", "Decode", "DecodeParms",
    "DefaultCMYK", "DefaultCryptFilter", "DefaultGray", "DefaultRGB", "Desc", "DescendantFonts", "Descent", "Dest", "DestOutputProfile", "Dests",
    "DeviceCMYK", "DeviceGray", "DeviceN", "DeviceRGB", "DeviceRGBA", "DeviceRgb", "Diamond", "Difference", "Differences", "DigestMethod",
    "Direction", "DisplayDocTitle", "Div", "Do", "Doc", "DocChecksum", "DocMDP", "DocTimeStamp", "Document", "DocumentFragment", "Duplex", "Dur", "E",
    "EF", "EFF", "EI", "EMC", "ET", "EX", "EarlyChange", "Em", "EmbeddedFDFs", "EmbeddedFile", "EmbeddedFiles", "Encode", "EncodedByteAlign",
    "Encoding", "Encrypt", "EncryptMetadata", "EncryptedPayload", "EndIndent", "EndOfBlock", "EndOfLine", "ExData", "Exclusion", "Exif", "Export",
    "ExportState", "ExtGState", "Extend", "Extends", "Extension", "ExtensionLevel", "Extensions", "Extra", "F", "FB", "FD", "FDF", "FDecodeParms",
    "FENote", "FFilter", "FL", "FRM", "FS", "FT", "False", "Ff", "Fields", "Figure", "FileAttachment", "Filespec", "Filter", "First", "FirstChar",
    "Fit", "FitB", "FitBH", "FitBV", "FitH", "FitR", "FitV", "FitWindow", "Fl", "Flags", "FlateDecode", "FlipH", "FlipV", "Folders", "Font",
    "FontBBox", "FontDescriptor", "FontFamily", "FontFile", "FontFile2", "FontFile3", "FontMatrix", "FontName", "FontStretch", "FontWeight", "Fonts",
    "Form", "FormType", "Formula", "FreeText", "FreeTextCallout", "FreeTextTypeWriter", "FullScreen", "Function", "FunctionType", "Functions", "G",
    "GB1", "GRAY", "GTS_PDFA1", "GTS_PDFX", "Gamma", "GlyphOrientationVertical", "GoTo", "GoTo3DView", "GoToE", "GoToR", "Gray", "Group", "H", "H1",
    "H2", "H3", "H4", "H5", "H6", "HT", "Halftone", "HardLight", "HeBO", "HeBo", "HeOb", "HeaderFooter", "Headers", "Height", "Helv", "HelvB",
    "Helvetica", "Helvetica-Bold", "Helvetica-BoldOblique", "Helvetica-Oblique", "Hide", "HideMenubar", "HideToolbar", "HideWindowUI", "Highlight",
    "Hue", "I", "IC", "ICCBased", "ID", "IDTree", "IF", "IM", "IRT", "IT", "Icon", "Identity", "Illustrator", "Image", "ImageB", "ImageC", "ImageI",
    "ImageMask", "Images", "ImportData", "Index", "Indexed", "Info", "Ink", "InkList", "InlineAlign", "IntegerItem", "Intent", "Interpolate",
    "Issuer", "ItalicAngle", "J", "JBIG2Decode", "JBIG2Globals", "JPXDecode", "JS", "Japan1", "JavaScript", "K", "KR", "KeyUsage", "Keywords", "Kids",
    "Kind", "Korea1", "L", "LBody", "LC", "LE", "LI", "LJ", "LL", "LLE", "LLO", "LW", "LZW", "LZWDecode", "Lab", "Lang", "Last", "LastChar",
    "LastModified", "Launch", "Lbl", "Leading", "Legal", "LegalAttestation", "Length", "Length1", "Length2", "Length3", "Lighten", "Limits", "Line",
    "LineDimension", "LineHeight", "Linearized", "Link", "ListNumbering", "Lock", "Locked", "Luminosity", "M", "MCID", "MCR", "MDP", "MK", "ML",
    "MMType1", "MP", "Mac", "MacExpertEncoding", "MacRomanEncoding", "MarkInfo", "Marked", "Marker", "Mask", "Matrix", "Matte", "MaxLen", "MaxWidth",
    "Measure", "MediaBox", "Metadata", "MissingWidth", "ModDate", "MoveOrigin", "Movie", "Multiply", "MyFont", "N", "NM", "Name", "Named", "Names",
    "NeedAppearances", "NeedsRendering", "NewWindow", "Next", "Nil", "NoSuchFilter", "NonEFontNoWarn", "NonFullScreenPageMode", "NonStruct", "None",
    "Normal", "NumCopies", "Nums", "O", "OBJR", "OC", "OCG", "OCGs", "OCMD", "OCProperties", "OCSP", "OCSPs", "OE", "OFF", "OID", "ON", "OP", "OPI",
    "OPM", "OS", "ObjStm", "Off", "OneColumn", "Opaque", "Open", "OpenAction", "OpenArrow", "OpenType", "Opt", "Order", "Ordering", "Outlines",
    "OutputCondition", "OutputConditionIdentifier", "OutputIntent", "OutputIntents", "Overlay", "OverlayText", "P", "PA", "PC", "PCAdded", "PCAr",
    "PCByLine", "PCCheck", "PCCour", "PCCourier", "PCCourierBold", "PCCourierBoldOblique", "PCCourierOblique", "PCCross", "PCCustomInitials",
    "PCCustomSignature", "PCDot", "PCFillSign", "PCFl0", "PCFl1", "PCFull", "PCFullFont", "PCHe", "PCHelv", "PCHelvetica", "PCHelveticaBold",
    "PCHelveticaBoldOblique", "PCHelveticaOblique", "PCItems", "PCJp", "PCLine", "PCMark", "PCMeasureLabel", "PCMeasureValue", "PCOutline",
    "PCPicture", "PCPictureImage", "PCPictureRotate", "PCPictureSize", "PCRedacted", "PCSom", "PCStamp", "PCStampRotate", "PCTimes", "PCTimesBold",
    "PCTimesBoldItalic", "PCTimesItalic", "PCTimesRoman", "PCTypedSignature", "PCUndrawn", "PCXfaClick", "PCXfaImage", "PCXfaImages", "PCXfaLayout",
    "PCXfaOverrides", "PCXfaSHA256", "PDF", "PDFDocEncoding", "PI", "PK", "PNG", "PO", "PS", "PV", "PZ", "Padding", "Page", "PageLabels",
    "PageLayout", "PageMode", "Pages", "PaintType", "Panose", "Paperclip", "Params", "Parent", "ParentTree", "ParentTreeNextKey", "Part", "Path",
    "Pattern", "PatternType", "Perceptual", "Perms", "Pg", "Pic", "PickTrayByPDFSize", "PieceInfo", "Placement", "PolyLine", "PolyLineDimension",
    "Polygon", "PolygonCloud", "PolygonDimension", "Popup", "PreRelease", "Predictor", "PresSteps", "PreserveRB", "Prev", "Print", "PrintArea",
    "PrintClip", "PrintPageRange", "PrintScaling", "PrintState", "PrinterMark", "Private", "PrivateChain", "PrivateData", "ProcSet", "Producer",
    "ProjectFont", "Properties", "PtData", "PubSec", "Q", "QuadPoints", "Quote", "R", "R2L", "RB", "RBGroups", "RC", "RClosedArrow", "RD", "RG",
    "RGB", "RGBA", "RI", "RIPEMD160", "RL", "ROpenArrow", "RP", "RT", "RV", "Range", "Reason", "Reasons", "Recipients", "Rect", "RectFont", "Redact",
    "Redaction", "Ref", "Reference", "Registry", "RegistryName", "RelativeColorimetric", "Rendition", "Requirements", "ResFork", "ResetForm",
    "Resources", "RichMedia", "Role", "RoleMap", "Root", "Rotate", "RowSpan", "Rows", "Ruby", "RubyAlign", "RubyPosition", "RunLengthDecode", "S",
    "SA", "SC", "SCN", "SE", "SHA1", "SHA256", "SHA384", "SHA512", "SM", "SMask", "SMaskInData", "SS", "SV", "SVCert", "SW", "Saturation", "Schema",
    "Scope", "Screen", "Sect", "Separation", "SeparationInfo", "SetF", "SetFf", "SetOCGState", "Shading", "ShadingType", "Short", "Sig", "SigFlags",
    "SigRef", "SinglePage", "Size", "Slash", "SoftLight", "Sort", "Sound", "SpaceAfter", "SpaceBefore", "Span", "SpiderInfo", "Square", "Squiggly",
    "Stamp", "Standard", "StandardEncoding", "StartIndent", "State", "StateModel", "Status", "StdCF", "StemH", "StemV", "Stm", "StmF", "StmOwn",
    "StrF", "StrikeOut", "StrikeOutTextEdit", "StringItem", "Strong", "StructElem", "StructParent", "StructParents", "StructTreeRoot", "Style", "Sub",
    "SubFilter", "Subdictionary", "Subj", "Subject", "SubjectDN", "SubmitForm", "Subtype", "Summary", "Supplement", "Suspects", "Symbol",
    "Synchronous", "T", "TBody", "TBorderStyle", "TD", "TFoot", "TH", "THead", "TI", "TJ", "TK", "TL", "TM", "TOC", "TOCI", "TP", "TPadding", "TR",
    "TR2", "TRef", "TS", "TU", "Table", "Tabs", "Target", "Tc", "Td", "TemplateInstantiated", "Templates", "Text", "TextAlign", "TextDecorationColor",
    "TextDecorationThickness", "TextDecorationType", "TextIndent", "Tf", "Thread", "Threads", "Thumb", "TiBI", "TiBo", "TiIt", "TiRo", "TilingType",
    "TimeStamp", "Times-Bold", "Times-BoldItalic", "Times-Italic", "Times-Roman", "Title", "Tj", "Tm", "ToUnicode", "Toggle", "Tr", "Trans",
    "TransformMethod", "TransformParams", "Transparency", "Trapped", "TrimBox", "True", "TrueType", "TrustedMode", "Ts", "Tw", "TwoColumnLeft",
    "TwoColumnRight", "TwoPageLeft", "TwoPageRight", "Tx", "Type", "Type0", "Type1", "Type1C", "Type3", "Tz", "U", "UCR", "UCR2", "UE", "UF", "URI",
    "URL", "URLType", "Underline", "Unix", "Usage", "UseAttachments", "UseCMap", "UseOC", "UseOutlines", "UseThumbs", "UserProperties", "UserUnit",
    "V", "V2", "VE", "VP", "VRI", "Value", "VendorKey", "VendorNote", "VendorState", "Version", "Vertices", "VerticesPerRow", "View", "ViewArea",
    "ViewClip", "ViewState", "ViewerPreferences", "Volume", "W", "W2", "WC", "WMode", "WP", "WS", "WT", "Warichu", "Watermark", "WhitePoint",
    "Widget", "Width", "Widths", "WinAnsiEncoding", "Wrap", "WritingMode", "X", "XFA", "XHeight", "XML", "XObject", "XRef", "XRefStm", "XStep", "XYZ",
    "Y", "YStep", "Yes", "ZaDb", "ZapfDingbats", "Zoom",
];
